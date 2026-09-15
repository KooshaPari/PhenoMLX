# Journey Manifests

Journey manifests live here. Each manifest binds a user-facing flow to its
spec, gates, eval verdict, and rich-media capture. See
`docs/operations/journey-traceability.md` for the canonical flow list.

## Index

| Flow | Spec | Manifest |
| --- | --- | --- |
| capacity-fit-estimate | `docs/specs/FR-HWL-CAPACITY-001.md` | `capacity-fit-estimate.md` |
| fleet-ledger-compare | `docs/specs/FR-HWL-FLEET-001.md` | `fleet-ledger-compare.md` |
| local-runtime-health | `docs/specs/FR-HWL-INFERENCE-001.md` | `local-runtime-health.md` |

## Conventions

- Filename matches the `journey="..."` attribute used in
  `docs/operations/journey-traceability.md` rich-media stubs.
- Each manifest declares: status, owning spec(s), entry point, expected
  capture, gates, eval verdict schema, and acceptance criteria.
- Status transitions: `TODO` -> `READY` only after the eval verdict passes and
  the rich-media capture is linked.
