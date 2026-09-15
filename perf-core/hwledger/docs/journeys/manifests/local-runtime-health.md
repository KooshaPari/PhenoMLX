# Journey Manifest — local-runtime-health

- **Status:** TODO
- **Owns:** `docs/operations/journey-traceability.md` row 3
- **Requirements covered:** FR-HWL-INFERENCE-001, NFR-HWL-OBSERVABILITY-001
- **Spec:** `docs/specs/FR-HWL-INFERENCE-001.md`

## Entry point

```bash
cargo run -p hwledger-devtools -- up
curl -s http://127.0.0.1:8080/healthz | jq .
phenotype-journey verify local-runtime-health
```

## Expected capture

- Screenshot: runtime health document, constraint flags, and the eval verdict
  JSON mapping to FR-HWL-INFERENCE-001 and NFR-HWL-OBSERVABILITY-001.
- Asset path: `apps/landing/dist/assets/rich-media/hwledger/local-runtime-health.png`

## Gates that must pass

- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo test -p hwledger-inference health`
- `cargo test -p hwledger-server healthz`
- `cargo test -p hwledger-inference sidecar_health`
- `phenotype-journey verify` exits 0 with a `verdict_schema_version` that
  matches the manifest.

## Eval verdict

- Schema: `{ "fr": "FR-HWL-INFERENCE-001", "nfr": "NFR-HWL-OBSERVABILITY-001",
  "verdict": "pass|fail", "evidence": "path/to/verdict.json", "recorded_at":
  "<ISO8601>" }`
- Health-document fields are validated for non-null constraints.

## Acceptance

A journey is accepted only when the eval verdict passes, the capture is
linked from the manifest, and the rich-media stub status flips from `TODO` to
`READY` in `docs/operations/journey-traceability.md`.
