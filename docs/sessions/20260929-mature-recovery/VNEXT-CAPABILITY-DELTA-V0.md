# vNext capability delta v0

Distinct obligations restored by the experimental-runtime/research-studio thesis.

| ID | Obligation | Role | Gate |
|---|---|---|---|
| M-RD-001 | Represent a runtime research hypothesis with mechanism, expected effect and falsification condition. | core | EXP-M1 |
| M-RD-002 | Experiment matrix varies declared runtime factors while holding comparison controls explicit. | core | EXP-M1 |
| M-RD-003 | Runtime extension can hook decoding/speculation/quantization/cache/kernel/scheduler layers without copying an entire engine where an extension seam exists. | core | EXP-M1 |
| M-RD-004 | Engine-native best practice is the default baseline for a mechanism. | existence | EXP-M1 |
| M-RD-005 | Custom mechanism records exact patch/build/source lineage and effective runtime activation. | core | EXP-M1 |
| M-RD-006 | Experiment proves the custom path actually executed rather than silently falling back. | core | EXP-M1 |
| M-RD-007 | Research UI exposes experiment factors, realized runtime state, evidence and negative results—not only model chat controls. | core | EXP-M1 |
| M-RD-008 | Mechanism comparison preserves quality/non-inferiority gates before performance claims. | core | EXP-M1 |
| M-RD-009 | Backend/platform portability distinguishes portable research logic from backend-specific implementation. | core | EXP-M1 |
| M-RD-010 | Cross-platform packaging declares supported OS/ISA/accelerator/backend combinations and installation evidence. | mature | platform experiment |
| M-RD-011 | Unsupported mechanism/backend pair is explicit and queryable. | core | EXP-M1 |
| M-RD-012 | Negative mechanism results remain discoverable to prevent repeated hand-rolling. | core | research store |
| M-RD-013 | Existing Unsloth/engine/library primitive is preferred unless custom work has a declared research reason. | existence | SOTA gate |
| M-RD-014 | Experiment can compare stock, integrated third-party and custom extension under one evidence model. | core | EXP-M1 |
| M-RD-015 | Dashboard/state can distinguish requested, applied, measured and inferred runtime facts. | core | INFRA-01 + EXP-M1 |

Typed profiles support these obligations but do not satisfy M-RD-001..014 by themselves.
