"""Compare a recurrent decode step without accidentally advancing one side.

This exists because of a specific, repeated, and expensive mistake. Comparing
a CUDA-graph replay against an eager step on the SAME cache gives a relative
logit difference of 9.31e-1, which reads as "graphs are broken" and nearly
ended the investigation. The graph was fine. The reference had run one step
further than the graph, because both a replay and an eager call MUTATE the
cache. The comparison was step N against step N+1, and on a hybrid recurrent
model that is not a small error.

The mistake has a second, sneakier form. Priming a capture needs side-stream
warmup steps, and those steps advance the cache they run on. The first fix
applied the warmup to the graph's cache only, which reproduced the identical
symptom. The tell was in the numbers rather than in the reasoning: divergence
shrank monotonically, 9.5e-1, 3.7e-1, 2.6e-1, down to 5.8e-2. A genuinely
incorrect kernel does not converge toward correct as it runs. A fixed offset
that the recurrent state gradually washes out does.

The rule this module enforces, in one line: a comparison is only meaningful if
both sides have seen the identical history, and any step that touches a cache
counts as history.

The subtlety that cost four consecutive wrong harnesses, and which no amount of
careful warmup bookkeeping fixes, is that CAPTURE PRIMING IS INHERENTLY
ONE-SIDED. Priming a graph runs a real forward pass to dirty the graph's cache,
so it advances that cache and only that cache. It cannot be applied to both
sides even in principle, so warmup can never be part of a symmetric history.
After priming, the reference must be advanced by exactly one step to match, and
only then may the capture be compared. On this model:

    priming, no match step      -> relative logit difference ~3e-1
    priming, WITH match step    -> exactly 0.0, over a full 8-step lockstep

`GraphHarness` makes that match step part of construction and refuses to
compare before it has happened, so the failure mode can no longer be reached by
forgetting. A second, related trap: a captured graph reads its token and
position from static buffers, so the reference must be fed the same token the
graph was captured with. `GraphHarness.step_pair` takes both from the capture
and cannot be handed a different token.

`LockstepPair` holds two eager caches, applies every step to both, and can
therefore only be compared at equal step counts.

`assert_cache_reads_are_live` closes the remaining hole. A graph could agree
with eager by reading a frozen copy of the state rather than the cache, and
that failure is invisible to any advance-once comparison. Mutating the cache in
place and confirming the next replay reflects it proves the graph reads live
memory. On this model, zeroing the full-attention KV moves the logits by 9.688.

Also provided: `tensor_census`, which walks a cache by object id rather than by
attribute name. The named-attribute version tracked 16 tensors, which is exactly
8 full-attention layers times keys and values, and reported "all stable" while
saying nothing at all about the 24 linear-attention layers whose state those
names missed. The id-based walk finds all 72. A stability check that silently
tracks nothing is worse than no check, so the census returns the count and the
caller is expected to print it.

Correctness acceptance for a decode-path change is 0.0, not merely "under
tolerance". Graph replay on Qwen3.5-9B is bit-identical to eager, and a
non-zero difference means the harness is wrong before it means the kernel is.
"""
import re
import sys

GIB = 2 ** 30

# One attribute name or one list index, in emission order.
_STEP = re.compile(r"\.([A-Za-z_][A-Za-z0-9_]*)|\[(\d+)\]")


def rel(a, b):
    """Max relative difference, the project's standard scalar for logits."""
    import torch
    return ((a - b).abs().max()
            / b.abs().max().clamp_min(1e-9)).item()


class LockstepPair:
    """Two identically initialized caches, advanced only together.

    Every mutation goes through `step`, which touches both sides, so a caller
    cannot compare the two at unequal step counts. Use it for warmup as well as
    for the compared steps: that is the whole point.
    """

    def __init__(self, model, tokens, positions, cache_factory, label="pair"):
        import torch
        self.model = model
        self.label = label
        self.n = 0
        self.caches = []
        for i in range(2):
            c = cache_factory()
            with torch.no_grad():
                model(tokens, past_key_values=c, use_cache=True,
                      cache_position=positions[0])
            self.caches.append(c)
        self.history = list(zip(tokens, positions))
        self.last = [None, None]

    def _device_pos(self, p):
        import torch
        return torch.tensor([p], device=self.model.device)

    def step(self, token, position):
        """Advance BOTH caches by exactly one step. Returns the reference logits."""
        return self.step_both([token], [position])

    def step_both(self, tokens, positions):
        """Apply a history to both sides, in order. Used for symmetric warmup."""
        import torch
        last = None
        for t, p in zip(tokens, positions):
            t = t.view(1, 1)
            p = self._device_pos(p)
            for i, c in enumerate(self.caches):
                with torch.no_grad():
                    out = self.model(t, past_key_values=c, use_cache=True,
                                     cache_position=p)
                self.last[i] = out.logits[:, -1].float().clone()
            self.n += 1
            last = self.last[1]
        return last

    def step_one(self, which, token, position):
        """Advance exactly one cache. Deliberately asymmetric.

        The only correct use is to BUILD an asymmetric state on purpose, so that
        `compare` can be shown to detect it. Using this by accident is how the
        original 9.31e-1 divergence happened, so it is named, separate, and the
        counter `n` is NOT advanced, which makes compare() raise rather than
        report a misleading zero.
        """
        import torch
        with torch.no_grad():
            out = self.model(token.view(1, 1), past_key_values=self.caches[which],
                             use_cache=True,
                             cache_position=self._device_pos(position))
        self.last[which] = out.logits[:, -1].float().clone()
        return self.last[which]

    def compare(self):
        """Relative logit difference between the two caches' last outputs.

        Both sides have had exactly self.n steps by construction, so a non-zero
        result is a real disagreement rather than a step-count artifact. Raises
        if nothing has been stepped yet, rather than comparing None to None.
        """
        if self.n == 0:
            raise ValueError("compare() called before any step(); there is "
                             "nothing to compare and a zero here would be a "
                             "false pass")
        return rel(self.last[0], self.last[1])

    def walk(self):
        """True if the two caches have identical memory in every tracked tensor.

        A cheap structural check that catches a cache which was mutated
        asymmetrically without stepping, for example by a manual in-place edit
        in a test. Compares shape and a full byte equality per tensor.
        """
        import torch
        a, b = tensor_census(self.caches[0]), tensor_census(self.caches[1])
        if set(a) != set(b):
            return False, f"different tensor sets: {len(a)} vs {len(b)}"
        bad = []
        for k in a:
            ta, tb = _get(self.caches[0], k), _get(self.caches[1], k)
            if ta.shape != tb.shape or not torch.equal(ta, tb):
                bad.append(k)
        return (not bad), (f"{len(bad)} tensors differ" if bad else
                           f"all {len(a)} tensors identical")


def compare_lockstep(replay_fn, eager_fn, tokens, positions):
    """Run identical histories through a graph-backed and an eager cache.

    replay_fn and eager_fn each own one cache and one step function. Both are
    given the full history, in order, and the final logits are compared. The
    caller cannot accidentally advance only one side, because the history is
    applied here to both.

    Returns the worst relative difference over the compared steps.
    """
    worst = 0.0
    n = 0
    for t, p in zip(tokens, positions):
        g = replay_fn(t, p)
        e = eager_fn(t, p)
        worst = max(worst, rel(g, e))
        n += 1
    return worst, n


def captured_graph(model, cache, inp, pos):
    """Prime on a side stream, then capture. Returns (graph, output).

    Priming is a real forward pass, so it ADVANCES the cache it runs on. It
    runs on the graph's cache only, by design: the point is to dirty that
    cache so the capture sees realistic state. This is the trap. See
    `GraphHarness.match_priming`, which is the only correct way to build a
    comparable graph, and read the module docstring before comparing anything.
    """
    import torch
    s = torch.cuda.Stream()
    s.wait_stream(torch.cuda.current_stream())
    with torch.cuda.stream(s):
        with torch.no_grad():
            model(inp, past_key_values=cache, use_cache=True,
                  cache_position=pos)
    torch.cuda.current_stream().wait_stream(s)
    torch.cuda.synchronize()
    g = torch.cuda.CUDAGraph()
    with torch.no_grad():
        with torch.cuda.graph(g):
            out = model(inp, past_key_values=cache, use_cache=True,
                        cache_position=pos)
    torch.cuda.synchronize()
    return g, out


class GraphHarness:
    """A captured decode step that can be compared against an eager reference.

    The whole reason this class exists: capture priming is one-sided, so a
    harness that treats warmup as a symmetric history is wrong every time, and
    the error looks exactly like a broken kernel. Concretely, on this model:

        priming, no match step      -> relative logit difference ~3e-1
        priming, WITH match step    -> exactly 0.0

    A 3e-1 error on a kernel that is in fact bit-exact is what nearly ended the
    CUDA-graph investigation, so the matching step is made part of construction
    here rather than left to the caller to remember.

    Two rules the class enforces:

    1. After priming, the reference is advanced by exactly one step at the
       capture position, so both caches hold the same history. This is
       `match_priming`, and it is not optional.
    2. A captured graph reads its token and position from STATIC buffers, so
       every later step must feed the reference the SAME token and position the
       graph was captured with. `step_pair` takes the pair from the capture and
       cannot be given a different token, which is the second way these
       comparisons go wrong.

    Note the positions in a lockstep. The captured step always computes the
    position it was captured at, so the position buffer is refilled in place
    before each replay, and the reference is stepped at the same value. Growing
    the position is the caller's job and must happen identically on both sides.
    """

    def __init__(self, model, cache, inp, pos, reference_cache):
        import torch
        self.model = model
        self.cache = cache
        self.inp = inp
        self.pos = pos
        self.reference = reference_cache
        self.matched = False
        self.steps = 0
        self.graph, self.out = captured_graph(model, cache, inp, pos)
        self.inp_snapshot = inp.clone()

    def match_priming(self, token):
        """Advance the reference by the one step that priming consumed.

        Call this BEFORE comparing anything. Without it the reference sits one
        step behind the graph and the comparison reports a large error that
        has nothing to do with the graph.
        """
        if self.matched:
            return
        self._eager(self.reference, self.inp_snapshot, self.pos)
        self.matched = True

    def _eager(self, cache, token, pos):
        import torch
        with torch.no_grad():
            return self.model(token.view(1, 1), past_key_values=cache,
                              use_cache=True,
                              cache_position=pos).logits[:, -1].float().clone()

    def step_pair(self):
        """Advance the graph by a replay and the reference by one eager step.

        Both sides receive the captured token and the captured position, so
        they cannot drift apart. Returns (graph_logits, eager_logits).
        """
        if not self.matched:
            raise ValueError(
                "step_pair() before match_priming(): the reference is one step "
                "behind because capture priming advanced the graph's cache. "
                "That mismatch reports a large divergence which looks like a "
                "broken graph but is a harness bug. Call match_priming first.")
        import torch
        self.graph.replay()
        torch.cuda.synchronize()
        g = self.out.logits[:, -1].float().clone()
        e = self._eager(self.reference, self.inp_snapshot, self.pos)
        self.steps += 1
        return g, e

    def compare(self):
        """Relative logit difference for the current pair."""
        return rel(*self.step_pair())

    def lockstep(self, n):
        """Replay and step together n times, returning the worst difference.

        This is the check that distinguishes a real correctness result from a
        one-step coincidence. A graph that matched once and then drifted would
        pass a single comparison and fail here.
        """
        worst = 0.0
        for _ in range(n):
            worst = max(worst, self.compare())
        return worst


def assert_cache_reads_are_live(replay_fn, cache, scale=1.0):
    """Prove a captured graph reads live cache memory, not a frozen copy.

    Runs a replay, zeroes the full-attention KV in place, runs it again, and
    returns the absolute change in the output. A graph that reads live memory
    must respond; a graph holding a stale copy returns ~0 and is wrong even
    though it matched eager once.

    On Qwen3.5-9B this returns about 9.688, which is the healthy value.
    """
    import torch
    before = replay_fn()
    n = 0
    for layer in getattr(cache, "layers", []):
        for name in ("keys", "values"):
            t = getattr(layer, name, None)
            if isinstance(t, torch.Tensor) and t.is_floating_point():
                t.mul_(0.0)
                n += 1
    torch.cuda.synchronize()
    after = replay_fn()
    return (after - before).abs().max().item(), n


def tensor_census(obj, depth=6):
    """Map every tensor reachable from obj to its data_ptr, by object id.

    The id-based walk is deliberate. Tracking named attributes missed the
    linear-attention state entirely on this model and reported a clean bill of
    health for a check that had not examined the tensors that mattered. The
    caller must print the count so a census that found nothing is visible
    rather than mistaken for a pass.
    """
    import torch
    out = {}
    seen = set()

    def walk(o, path, d):
        if d > depth or id(o) in seen:
            return
        seen.add(id(o))
        if isinstance(o, torch.Tensor):
            out[path] = o.data_ptr()
            return
        if isinstance(o, (list, tuple)):
            for i, v in enumerate(o):
                walk(v, f"{path}[{i}]", d + 1)
            return
        if isinstance(o, dict):
            for k, v in o.items():
                walk(v, f"{path}.{k}", d + 1)
            return
        if hasattr(o, "__dict__"):
            for k, v in vars(o).items():
                walk(v, f"{path}.{k}", d + 1)

    walk(obj, "cache", 0)
    return out


def moved_addresses(before, after):
    """Paths whose data_ptr changed between two tensor_census snapshots."""
    return sorted(k for k in before if k in after and before[k] != after[k])


def _get(obj, path):
    """Resolve a tensor_census path such as 'cache.layers[3].keys' to a tensor.

    The census emits dotted attributes and bracketed list indices, and a path
    must round-trip back to the identical object, because LockstepPair.walk
    compares tensors byte for byte through it. The first implementation used
    hand-rolled string partitioning and raised ValueError on paths like
    'cache.layers[0].conv_states[0]', so this parses both forms explicitly.
    """
    cur = obj
    for m in _STEP.finditer(path):
        name, idx = m.group(1), m.group(2)
        if name is not None:
            cur = cur[name] if isinstance(cur, dict) else getattr(cur, name)
        else:
            cur = cur[int(idx)]
    return cur


if __name__ == "__main__":
    print(__doc__)
    print("This module is imported by probes. It has no standalone mode.")
    sys.exit(0)
