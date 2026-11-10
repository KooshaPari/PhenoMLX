# Minimal profile store contract v0 — PhenoMLX

Status: schema/architecture contract only. No database selected.

## Purpose
Persist exact runtime-profile and qualification history required by VS-01 without coupling the product to SQLite/Postgres/object store prematurely.

## Logical records

### ProfileRecord
Immutable:
- profile_id;
- schema_version;
- RequestedProfile digest;
- RealizedProfile payload/digest;
- RuntimeGeneration/effective-config fingerprint;
- created_at;
- provenance/authority.

### QualificationRecord
Append-only:
- qualification_id;
- profile_id;
- workload_digest;
- oracle_digest;
- environment/hardware refs;
- evidence refs;
- status qualified|failed|invalid|incomparable;
- observed_at;
- verifier/version;
- limitations.

### SupportRecord
Append/supersede:
- support_id;
- profile_id or profile predicate;
- lifecycle experimental|supported|deprecated|withdrawn;
- authority;
- effective_at;
- supersedes?;
- limitations/exclusions;
- evidence refs.

### CapacityObservation
Ephemeral/time-series:
- observation_id;
- profile/generation/environment;
- observed_at;
- free/required resources and collector scope;
- TTL/freshness.

## Store invariants
1. ProfileRecord immutable.
2. Qualification never rewritten; correction supersedes with provenance.
3. Support change never mutates Profile/Qualification.
4. duplicate IDs idempotent iff payload digest identical; conflict otherwise.
5. unknown schema version fails explicit.
6. deletion/retention leaves tombstone sufficient for referential integrity where policy permits.
7. current-view queries are projections over history.
8. transaction boundary prevents SupportRecord from referencing nonexistent qualification/profile when policy requires them.
9. storage backend does not determine semantic identity.
10. KernelRegistry TuningRecord may be referenced as evidence for extension/kernel claims; it is not copied/reinterpreted as generic profile state.

## Minimal VS-01 implementation
A directory/object-store/JSONL implementation is sufficient if it enforces invariants and atomic/idempotent append. Database choice is deferred until concurrency/query requirements justify one.

## Required tests
restart roundtrip; duplicate same payload; duplicate conflicting payload; support withdrawal; stale capacity; unknown schema; qualification supersession; tombstone traversal; concurrent/partial write recovery.
