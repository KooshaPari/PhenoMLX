# PhenoMLX — pass 2 canonical-lineage resolution

Observation date: 2026-09-29. Production source remains `1ab4ed250e6b3a1ed9025a674d4814339d32fcd3`.

## Canonical cutover is explicit

`docs/guides/CANONICAL_REPO_CUTOVER.md` establishes that the active Phenotype oMLX stack has one canonical development home and that `phenotype-omlx-tmp` / `phenotype-omlx-temp` (and their `zz-archive-*` forms) are **historical supersets**, read-only for lineage. It explicitly says not to author new work or PRs against them and to preserve stale worktrees/remotes until reviewed.

This closes one pass-1 ambiguity: tmp/temp are predecessor/history sources, not viable alternative active product identities. It does **not** prove that every useful predecessor change is present in the current source. A proper predecessor-delta audit remains required before declaring archaeology exhausted.

Registry separately lists `KooshaPari/PhenoMLX-temp` as repository ID `1372886076`. Its relationship to the older `phenotype-omlx-temp` naming needs stable-ID/history confirmation before treating it as the same predecessor. No archive was restored and no work was started there.

## Current architecture implication

The cutover guide strengthens the rule already present in AGENTS: current work belongs to the canonical source/worktree hub, while predecessor trees are evidence. Mature-contract recovery should therefore:
- mine archived supersets for accepted/rejected capability history;
- never run them as the current candidate merely because they contain more code;
- map any missing predecessor capability to an explicit retain/reject/reimplement/upstream decision;
- preserve consumer compatibility and provenance through any reintroduction.

The pass-1 launcher and source-recorded cache findings remain unchanged. They are candidate-specific observations and do not authorize work in predecessor repositories.

## CI state

No GitHub Actions workflow runs were returned for specification commit `6ce6eff8b5274054c047f9bf3bab3c9cd060d063` in the commit-associated PR-run query. That is **no CI receipt**, not a green. Native launcher/model/cache/lifecycle experiments remain unperformed.

Next: enumerate the exact predecessor revisions and current fork ancestor, then compare their meaningful runtime/packaging surfaces before freezing bootstrap decisions.
