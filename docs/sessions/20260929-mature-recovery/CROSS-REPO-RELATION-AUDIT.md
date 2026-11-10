# Cross-repository relation audit — PhenoMLX

Date: 2026-09-30. Applies CROSS-REPO-RELATION-AUTHORITY-RULE.

## hwLedger

Authoritative registry boundary states hwLedger is canonical home for hardware/fleet ledger and capacity-planner UX, including live telemetry reconciliation. It explicitly excludes LLM route/proxy plane.

**Typed relation to PhenoMLX candidate:**
- relation: data producer/consumer + optional shared substrate/integration;
- hwLedger may provide hardware/fleet observations or capacity data;
- this does NOT make hwLedger owner of PhenoMLX RuntimeProfile/QualificationAssessment;
- PhenoMLX must not duplicate hwLedger's canonical fleet ledger if it consumes it.

Status: **SUPPORTED boundary fact**, but exact current code/API integration with PhenoMLX remains OPEN.

## OmniRoute

Current registry boundary file is scaffold/unknown. A separate routing ADR describes OmniRoute as deployed transport/blend data plane, but that ADR is **PROPOSED**, not accepted.

Therefore prior recovery wording that routers “consume PhenoMLX qualification truth” is too strong as an ownership/interface claim.

**Typed relation:**
- possible consumer/integration;
- no normative dependency;
- no source-of-truth relationship established;
- no claim that OmniRoute must consume PhenoMLX until direct intent/accepted interface evidence exists.

Status: **UNRESOLVED OPTIONAL INTEGRATION**.

## Corrected product boundary

PhenoMLX stands on its own as LLM runtime-profile qualification/control R&D/product candidate. It may consume hardware observations from hwLedger and may expose profiles to routing systems, but neither relation defines its product identity.

This supersedes stronger routing-consumer language in prior provisional artifacts where unsupported.
