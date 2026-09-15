# FR-HWL-FLEET-001 — Fleet Ledger Compare

> Functional requirement for the fleet-ledger user-facing flow.
> Linked from `docs/operations/journey-traceability.md`.

## Statement

A user MUST be able to record local devices in a fleet ledger and compare their
measured inference capacity side-by-side with full provenance.

## User Story

> As a **fleet operator**, I want to record and compare devices I've measured so
> that I can pick the best fit for a workload and audit how I know what I know.

## Acceptance Criteria

1. `hwledger fleet add <probe.json>` ingests a probe result into the local
   ledger with timestamp, host, and probe-version metadata.
2. `hwledger fleet compare <device-a> <device-b>` returns a side-by-side table
   covering parameters, KV cache, memory headroom, and observed throughput.
3. Each measurement cell carries a provenance pointer (probe id, fixture id,
   user override) per `NFR-HWL-REPRODUCIBILITY-001`.
4. The ledger is event-sourced: re-running a compare is idempotent and the
   snapshot is byte-stable given identical inputs.

## Non-Regression Constraints

- Append-only ledger; no silent mutation of historical events.
- Snapshots are deterministic — same input order, same output bytes.
- Provenance pointer is mandatory; rows missing provenance MUST fail validation.

## Linked Surfaces

- Code: `crates/hwledger-ledger`, `crates/hwledger-probe`, desktop/runtime UI
- Wire: `crates/hwledger-fleet-proto` (Axum + rustls mTLS)
- Docs: `docs/operations/journey-traceability.md#user-facing-flows`
- Journey: `docs/journeys/manifests/fleet-ledger-compare.md`
- Evidence: `../assets/rich-media/hwledger/fleet-ledger-compare.gif` (TODO)

## Test / Gate Coverage

- `cargo test -p hwledger-probe fixture` — fixture determinism
- `cargo test -p hwledger-ledger snapshot` — ledger snapshot byte-stability
- Integration: `cargo test -p hwledger-server fleet_proto` — mTLS handshake +
  compare round-trip
- Journey manifest validation: `docs/journeys/manifests/fleet-ledger-compare.md`

## Status

- [x] Requirement authored
- [ ] Manifest written
- [ ] Capture recorded
- [ ] Eval verdict linked
