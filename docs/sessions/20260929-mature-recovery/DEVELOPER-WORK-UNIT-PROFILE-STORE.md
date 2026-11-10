# Developer work unit — PhenoMLX VS-01 profile store

Status: bounded implementation package. Depends on VS-01 typed profile schema; does not require real two-engine performance run to implement storage semantics.

## Goal
Implement the smallest durable store satisfying MINIMAL-PROFILE-STORE-CONTRACT-V0 and profile-store-event-v0.schema.json.

## Preferred first implementation
Append-only JSONL or directory/object records with atomic write/rename and an in-memory/index projection. Do not add a database unless tests prove the simple implementation inadequate.

## Required operations
- append_profile(ProfileRecord)
- append_qualification(QualificationRecord)
- append_support(SupportRecord)
- append_capacity(CapacityObservation)
- get_profile(id)
- qualification_history(profile_id)
- support_history(profile_id)
- current_support(profile_id)
- current_capacity(profile_id, generation, now)
- validate_references()
- recover/reindex()

## Invariants
- immutable profile;
- identical duplicate id idempotent;
- conflicting duplicate id error;
- qualification correction new ID/supersession;
- support withdrawal append/supersede;
- stale capacity excluded from current view;
- unknown schema explicit error;
- partial/corrupt tail recoverable without manufacturing event;
- referential integrity/tombstone policy;
- storage backend never changes semantic IDs.

## Reuse
Reference KernelRegistry TuningRecord evidence by ID/path/digest where relevant. Do not copy it into a generic profile row.

## Required tests
Existing recovery test_profile_store_contract.py plus:
- disk restart;
- truncated last record;
- two writers/conflicting ID;
- support references missing profile;
- qualification references wrong profile;
- current-view projection;
- schema-version rejection;
- tombstone traversal.

## Done
A clean checkout can write, restart, reindex and query the event history with all tests passing. This closes the storage sub-work-unit only, not VS-01 native qualification or PhenoMLX CVP.
