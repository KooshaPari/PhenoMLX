# Journey Manifest — fleet-ledger-compare

- **Status:** TODO
- **Owns:** `docs/operations/journey-traceability.md` row 2
- **Requirements covered:** FR-HWL-FLEET-001, NFR-HWL-REPRODUCIBILITY-001
- **Spec:** `docs/specs/FR-HWL-FLEET-001.md`

## Entry point

```bash
hwledger fleet add fixtures/devices/mac-mini-m2.json
hwledger fleet add fixtures/devices/rtx-4090.json
hwledger fleet compare mac-mini-m2 rtx-4090
```

## Expected capture

- Animated GIF / video: load two fixture devices, compare their usable
  inference capacity, and surface provenance for each measured field.
- Asset path: `apps/landing/dist/assets/rich-media/hwledger/fleet-ledger-compare.gif`

## Gates that must pass

- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- Probe fixture determinism: `cargo test -p hwledger-probe fixture`
- Ledger snapshot byte-stability: `cargo test -p hwledger-ledger snapshot`
- mTLS handshake + compare round-trip:
  `cargo test -p hwledger-server fleet_proto`

## Eval verdict

- Schema: `{ "fr": "FR-HWL-FLEET-001", "verdict": "pass|fail", "evidence":
  "path/to/verdict.json", "recorded_at": "<ISO8601>" }`
- Includes a provenance pointer per cell.

## Acceptance

A journey is accepted only when the eval verdict passes, the capture is
linked from the manifest, and the rich-media stub status flips from `TODO` to
`READY` in `docs/operations/journey-traceability.md`.
