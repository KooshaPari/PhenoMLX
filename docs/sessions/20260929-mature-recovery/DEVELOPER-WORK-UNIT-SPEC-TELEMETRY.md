# Developer work unit — speculative telemetry v0

## Goal
Make one runtime path emit a truthful Speculative TrialReceipt. Prefer MLX/custom path if it exposes proposal/verify loop directly; otherwise choose the easiest pinned engine seam with real counters.

## Required implementation
- normalized collector interface;
- requested/applied method;
- activation proof;
- proposed/accepted tokens;
- draft and verify timing where separable;
- engine/build/effective config identity;
- target/draft/tokenizer identity;
- structured unsupported/fallback;
- raw telemetry artifact reference.

## Do not
- infer acceptance from throughput;
- treat startup flags as activation proof;
- fabricate unavailable fields;
- duplicate PhenoLab experiment orchestration.

## First validation
1. no-spec baseline receipt;
2. known unsupported/fallback receipt;
3. active speculative receipt with counters;
4. negative performance result remains valid evidence;
5. quality oracle attaches independently.

## Done
test_speculative_trial_receipt.py passes plus one live receipt whose active state is backed by method-specific runtime counters. This closes instrumentation sub-unit only, not EXP-M1.
