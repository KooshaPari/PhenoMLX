# Engine qualification matrix — external evidence pass 2

Date: 2026-09-30. This records current external project facts relevant to bootstrap/health; it does not qualify any engine on our hardware.

| Engine | Current external evidence | License | Health interpretation | PhenoMLX consequence |
|---|---|---|---|---|
| vLLM | GitHub releases currently list v0.30.0 latest; pyproject supports Python 3.10–3.14 and pins substantial build/runtime stack | Apache-2.0 | highly active/mature serving baseline | must be first-class baseline on NVIDIA where model support exists; custom equivalent requires evidence |
| SGLang | releases list v0.5.20 latest; current project/docs show active 2026 releases and broad model/hardware work | Apache-2.0 | highly active; rapid release cadence | first-class NVIDIA/accelerator baseline; pin immutable release/build |
| TensorRT-LLM | current source/setup active in 2026, Python package classified Beta, NVIDIA GPU focused | Apache-2.0 with bundled third-party notices | active vendor engine; hardware/vendor-specific integration cost | baseline where NVIDIA stack/hardware warrants it; exact CUDA/TensorRT compatibility must be recorded |
| llama.cpp | continuous build/release stream plus tagged release mechanism; broad platform binaries | MIT | very active, unusually broad portability | important CPU/Metal/CUDA/Vulkan/ROCm portability baseline; build commit/options matter as much as tag |
| MLX-LM | current releases include v0.31.3; setup requires MLX >=0.32.2 on Darwin | MIT | active Apple reference implementation | baseline for Apple Silicon; compare oMLX/extensions against idiomatic MLX-LM where capability overlaps |
| oMLX | current pyproject pins MLX 0.32.2/nanobind ABI; project identifies itself alpha and Apache-2.0; active Sept 2026 commits | Apache-2.0 | active but alpha; strong Mac serving specialization | candidate serving baseline on Apple, but version/ABI pinning and alpha lifecycle must be explicit |

## Evidence limitations
- “Health” here is descriptive project activity/release evidence, not a reliability score.
- GitHub release cadence does not establish support for a specific model/hardware/profile.
- License is project-level; bundled/optional components can carry additional notices/terms.
- Exact selected versions for VS-01 must be captured at execution time; this matrix is not a lockfile.

## Architecture consequence
There is no defensible reason for PhenoMLX to build a generic serving engine before testing these mature baselines. Its value hypothesis remains typed profile/qualification/control plus only experimentally justified extensions.
