# Runtime extension lifecycle and experiment isolation v1

Fresh falsification found the vNext research contract too loose around custom extension compatibility.

Every custom runtime extension must declare:
- extension ID/version/source digest;
- hook layer/API/ABI;
- engine/version/model/backend/platform compatibility envelope;
- build/compiler/dependency identity;
- activation proof;
- fallback/disable behavior;
- isolation from stock baseline;
- factor(s) intentionally changed;
- incidental changes/confounders;
- rollback/unload/restart requirement;
- maintenance/retirement state.

Experiment matrix must identify controlled vs varied factors. A custom patch that changes scheduler + quantization + kernel cannot attribute gain to one factor without ablation; it may still be compared as a compound replacement.

Engine upgrade invalidates extension qualification when ABI/API/runtime semantics can change until compatibility is re-established.

Research UI/history must preserve extension version and negative/incompatible outcomes.
