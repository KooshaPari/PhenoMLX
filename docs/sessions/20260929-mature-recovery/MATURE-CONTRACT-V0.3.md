# PhenoMLX mature product contract v0.3 candidate

Status: CANDIDATE DRAFT; supersedes v0.2 review target.

Purpose/source-of-truth/delegation remain as v0.2.

## Refined identity/support model
RealizedProfile identity is immutable for the exact realized runtime configuration/generation.

SupportEnvelope is a **separate versioned projection/assessment** over profile(s), with lifecycle `experimental | supported | deprecated | withdrawn`, effective version/time and migration/rollback relations. Withdrawing support never rewrites historical profile identity or qualification evidence.

CapacitySnapshot is time/generation/environment scoped and distinct from SupportEnvelope. AdmissionDecision consumes current capacity plus request/profile constraints.

Requested→fallback always produces explicit RealizedProfile. In-flight request remains bound to its generation. Partial stream != complete. CacheDomain participates in identity/isolation. Observability constrains claim scope.

## Architecture gate
Isolated state-machine prototype supports fallback/generation/stream/admission/cache-domain/support separation. Real two-engine run remains mandatory before baseline promotion.

Other v0.2 blockers remain.
