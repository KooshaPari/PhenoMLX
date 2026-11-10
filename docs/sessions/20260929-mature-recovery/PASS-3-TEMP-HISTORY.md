# PhenoMLX — pass 3 temp-repository history proof

Observation date: 2026-09-29. Current product source remains `1ab4ed250e6b3a1ed9025a674d4814339d32fcd3`.

## Stable identity and shared commit proof

Accessible archived repository `KooshaPari/PhenoMLX-temp` has stable repository ID `1372886076` and main head:

`70529a879715a8078e26eb564f8fc312e1923f57`.

That **exact commit exists in the current KooshaPari/PhenoMLX repository**, with the same tree `c38e5b8906dd8af1df277eebddd77ea14d1c01c8` and parent `f845f764...`. Its one-file patch changes the handoff remote from the original phenotype-omlx URL to `PhenoMLX-temp` and says the temp remote exists because the original was accidentally deleted while awaiting restore.

This closes the identity question: accessible `PhenoMLX-temp` is not merely a similarly named project. Its main history is directly contained in current PhenoMLX history.

## Current repo is strictly ahead of accessible temp main

GitHub compare inside the current repository:

- base: `70529a879715a8078e26eb564f8fc312e1923f57`
- head: `1ab4ed250e6b3a1ed9025a674d4814339d32fcd3`
- status: **ahead**
- ahead_by: **89**
- behind_by: **0**
- merge base: exactly `70529a879715a8078e26eb564f8fc312e1923f57`

Therefore the accessible temp **main branch contributes no commit after its head that is missing from current PhenoMLX main history**. It remains useful as historical evidence, but its main branch is not a missing implementation candidate.

This statement is intentionally limited to accessible temp **main**. Registry preservation records mention older zz-archive tmp/temp supersets with many branches and an all-ref/import-one-missing-ref plan. Those non-main refs have not been proven fully represented by this comparison.

## File-level corroboration and contamination warning

At temp head and current history, major files such as `ARCHITECTURE.md`, `docs/dossiers/HANDOFF.md`, and some product documents share exact blobs. Current PhenoMLX later changes README/AGENTS/license/dossiers.

However, temp `PRD.md` and `FUNCTIONAL_REQUIREMENTS.md` are exact blobs still present in current PhenoMLX while their content describes **phenotype-shared** infrastructure (event sourcing, cache, policy engine, state machine), not the PhenoMLX runtime product. Their presence is repository contamination/history, not accepted PhenoMLX product obligations. They must be classified non-normative/superseded unless a later authority source explicitly adopts them.

This is a concrete warning against deriving the mature contract from filename or apparent formality.

## Gate movement

Accessible `PhenoMLX-temp` main identity: **RESOLVED**.
Missing commits after temp main: **NONE relative to current main history**.
Archived tmp/temp all-ref completeness: **OPEN**.
Contaminated formal-looking requirements: **IDENTIFIED; non-normative pending authority reconciliation**.
