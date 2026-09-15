# Journey Manifest — capacity-fit-estimate

- **Status:** TODO
- **Owns:** `docs/operations/journey-traceability.md` row 1
- **Requirements covered:** FR-HWL-CAPACITY-001, NFR-HWL-EXPLAINABILITY-001
- **Spec:** `docs/specs/FR-HWL-CAPACITY-001.md`

## Entry point

```bash
cargo install --path crates/hwledger-cli
hwledger fit llama-3-8b apple-m2-24gb
```

Streamlit equivalent: open the Capacity page and submit the same model/device
form.

## Expected capture

- Annotated screenshot: estimated model fit, memory headroom, warning states,
  and the CLI/UI command that reproduces it.
- Asset path: `apps/landing/dist/assets/rich-media/hwledger/capacity-fit-estimate.png`

## Gates that must pass

- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --all -- --check`
- CLI smoke: `hwledger fit <fixture-model> <fixture-device>` exits 0 and emits
  the JSON verdict schema.
- Doc link validation for the rich-media asset.

## Eval verdict

- Schema: `{ "fr": "FR-HWL-CAPACITY-001", "verdict": "pass|fail", "evidence":
  "path/to/verdict.json", "recorded_at": "<ISO8601>" }`
- Mapped back to the requirement spec.

## Acceptance

A journey is accepted only when the eval verdict passes, the capture is
linked from the manifest, and the rich-media stub status flips from `TODO` to
`READY` in `docs/operations/journey-traceability.md`.
