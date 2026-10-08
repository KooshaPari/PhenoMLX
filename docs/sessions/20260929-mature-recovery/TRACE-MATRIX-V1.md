# KooshaPari/PhenoMLX trace matrix v1

DRAFT structural trace; not completion evidence. States: VERIFIED_SOURCE, VERIFIED_OBSERVATION, DETERMINISTIC_SOURCE, PROPOSED, MISSING, CONTRADICTORY.

| Obligation | Source | Implementation | Oracle | Journeys | State |
|---|---|---|---|---|---|
| M-OB-001 | direct intent/SOTA | no unified profile identity | identity pending | J02/J03/J05/J09 | MISSING |
| M-OB-002 | engine SOTA | mapping incomplete | incompatibility pending | J02/J03 | PROPOSED |
| M-OB-003 | launcher | external app/env | clean install pending | J01 | VERIFIED_SOURCE |
| M-OB-004 | launcher | web branch | argv observation | J03/J04 | VERIFIED_OBSERVATION |
| M-OB-005 | comparison | artifacts exist | matched workload pending | J09/J10 | PROPOSED |
| M-OB-006 | cache report | byte accounting | 1.375x source record | J05/J09/J10 | VERIFIED_SOURCE |
| M-OB-007 | quality gate | historical checks | accepted oracle pending | J09/J10 | PROPOSED |
| M-OB-008 | lifecycle | unmapped | lifecycle faults pending | J06/J07/J08 | MISSING |
| M-OB-009 | existence gate | custom extensions exist | native baseline pending | J10 | VERIFIED_SOURCE |
| M-OB-010 | negative evidence | regression preserved | currentness pending | J09/J10 | DETERMINISTIC_SOURCE |
| M-OB-011 | capacity history | old VRAM-fit | OOM control pending | J02/J05 | VERIFIED_SOURCE |

No edge is upgraded by confidence. Implementation existence is not reachability; test existence is not verification; historical evidence stays historical; missing stays missing.
