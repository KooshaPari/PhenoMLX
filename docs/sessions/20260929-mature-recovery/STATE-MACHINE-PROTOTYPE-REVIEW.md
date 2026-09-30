# State-machine prototype review

The isolated model exercises requested→fallback realization, runtime generation stickiness, partial streams, support-vs-admission, cache-domain identity and support withdrawal.

## Result
No contradiction with v0.2.

## Important correction
Support lifecycle and runtime profile identity are orthogonal: withdrawing support must not mutate historical RealizedProfile identity. A SupportEnvelope revision references profiles; it does not rewrite them.

CapacitySnapshot is also time-scoped and should carry observation timestamp/environment/generation in the real schema; the minimal prototype omits those only for isolation.

## Contract delta
Make SupportEnvelope a separate versioned assessment/projection over immutable profile identity. Add timestamp/generation to CapacitySnapshot schema requirements.

Real two-engine execution remains mandatory.
