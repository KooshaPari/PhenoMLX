# Architecture experiment results v0

Date 2026-09-30.

| Fixture | State | Evidence |
|---|---|---|
| requested engine vs fallback realized identity | PASS_STATIC | profile prototype |
| runtime generation changes identity | PASS_STATIC | profile prototype |
| unknown remote mechanism remains unknown | PASS_STATIC | profile prototype |
| two real engines same workload | NOT_RUN_NATIVE | requires pinned runnable engines/model |
| unsupported capability explicit | OPEN | adapter wrapper not implemented |
| partial stream not complete | OPEN | lifecycle adapter experiment |
| support vs admission | OPEN | capacity/admission integration |
| cache-domain isolation | OPEN | engine-specific integration |
| extension quality hard gate | OPEN | real benchmark |
| historical negative evidence binding | PASS_STATIC/PARTIAL_SOURCE | current negative source evidence already profile-bounded manually |

Architecture hypothesis: **SUPPORTED IN ISOLATION; REAL CROSS-ENGINE VALUE UNPROVEN**.

No commit-associated workflow receipt returned for prototype commit. Not green.
