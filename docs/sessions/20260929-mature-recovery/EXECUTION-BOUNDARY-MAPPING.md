# Execution-boundary mapping pass

Date 2026-09-30.

A spec-side backend source receipt now queries real BackendBase subclasses for `is_available()` and installed distribution version where discoverable.

Authority rule:
- class capability declaration = deterministic source fact;
- installed/available backend observation = verified environment observation;
- neither equals model/profile qualification until a real workload runs.

Unavailable engine must not fabricate version/qualification.

This receipt is the precursor to RealizedProfile capture for the two-engine experiment.

## Remaining execution boundary
Need an environment where two adapters are actually available with a common pinned model. Until then, profile semantics are validated but performance/support remains unverified.
