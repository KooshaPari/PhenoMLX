> **Work state:** SCAFFOLD · **Progress:** `███░░░░░░░ 25%`
> LLM capacity planner + fleet ledger + desktop inference runtime (Rust core + per-OS native GUIs). Pre-alpha Phase 0; ambitious 11-crate plan, 42 files committed so far (mostly docs/scaffold). · updated 2026-06-02

# hwLedger

[![AI Slop Inside](https://sladge.net/badge.svg)](https://sladge.net) [![GitHub Downloads (all assets, all releases)](https://img.shields.io/github/downloads/KooshaPari/hwLedger/total)](https://github.com/KooshaPari/hwLedger/releases)

_LLM capacity planner + fleet ledger + desktop inference runtime._

**Not a financial ledger.** hwLedger tracks hardware fleet audit and provenance for machine learning workloads. It provides per-layer VRAM estimation for LLMs, reconciles predictions against live telemetry from inference engines (MLX, mistral.rs, llama.cpp, vLLM, TGI), and maintains an event-sourced audit log for heterogeneous compute fleets (Apple Silicon, NVIDIA/AMD, cloud rentals).

**Status:** pre-alpha, Phase 0 bootstrap. See [PLAN.md](./PLAN.md) for the implementation roadmap.

hwLedger is an Apache-2.0 desktop app + agent/server pair that:

1. **Plans** VRAM and throughput for any HF / GGUF / MLX / Ollama model, correctly handling dense, MoE, MLA, GQA, sliding-window, SSM/Mamba, and hybrid-attention architectures — with a slider UX over a live per-layer breakdown.
2. **Reconciles** predictions against live telemetry from MLX, mistral.rs, llama.cpp, vLLM, or TGI.
3. **Runs** inference locally on Apple Silicon via a forked oMlx sidecar with SSD-paged KV cache.
4. **Ledgers** a heterogeneous fleet — local NVIDIA/AMD boxes, Apple Silicon laptops, cheap cloud rentals (Vast.ai, RunPod, Lambda) — with a shared event-sourced audit log, dispatch planner, and spot-price-aware cost model.
5. Ships as **per-OS native GUIs** (SwiftUI / WinUI 3 / Qt 6 + Slint) over a shared Rust FFI core.

> _A hobbyist-sized fleet with enterprise bones._

## Quickstart

**Install the CLI from source** — this is the fastest path to a working hwLedger today:

```bash
cargo install --path crates/hwledger-cli
hwledger --help
```

**Web fallback (no compilation needed):** A Streamlit web interface is available at [`apps/streamlit/`](./apps/streamlit/) — run `cargo run -p hwledger-devtools -- up` to launch it locally on `localhost:8501` along with the API server.

> **macOS DMG note:** Native macOS DMG distribution is currently blocked on Apple Developer certificate renewal — see [WP21 Apple Developer secrets](./docs/reports/WP21-APPLE-DEV-SECRETS.md) for setup instructions. Use the Streamlit fallback or CLI above for now.

## Why

Every existing public VRAM calculator (HF Accelerate, can-it-run-llm, LM Studio's gauge) gets MoE and MLA wrong — they under-count KV cache and over-count MoE throughput. hwLedger's math core is architecture-keyed: it dispatches per `AttentionKind` (MHA / GQA / MQA / MLA / Sliding / SSM / Hybrid / Sink) and treats resident-vs-active parameters separately for MoE. See [PLAN.md §5](./PLAN.md#5-math-core-51-is-the-products-soul).

## Architecture

- **Core**: Rust workspace (`hwledger-core`, `-arch`, `-ingest`, `-probe`, `-inference`, `-ledger`, `-fleet-proto`, `-agent`, `-server`, `-cli`, `-ffi`)
- **Sidecar**: `sidecars/omlx-fork/` — fat fork of [jundot/omlx](https://github.com/jundot/omlx), Apache-2.0
- **Native apps**: `apps/macos/` (SwiftUI + UniFFI + XCFramework), `apps/windows/` (WinUI 3 + .NET 9 + csbindgen), `apps/linux-qt/` (Qt 6 + cxx-qt + QML), `apps/linux-slint/` (Rust-native)
- **Fleet wire**: Axum + rustls mTLS for agents; russh + deadpool for SSH agentless; reqwest for Vast/RunPod/Lambda/Modal; `tailscale status --json` for tailnet discovery

See the component diagram in [PLAN.md §4.1](./PLAN.md#41-component-map).

## Substrate (per ADR-023 + ADR-035A)

- **[`KooshaPari/pheno-capacity`](https://github.com/KooshaPari/pheno-capacity)** — VRAM estimation, model-fit scoring, Chinchilla tokens, optimizer state. no_std-compatible Rust crate. **Active since 2026-06-18 (L5-105).** Streamlit Planner/WhatIf pages will consume this in Phase 2 (see [docs/integrations/cost-model-migration.md](./docs/integrations/cost-model-migration.md)).

## Dev setup

One-liner to build FFI + launch server, docs-site, and Streamlit:

```bash
cargo run -p hwledger-devtools -- up
```

See [docs-site/getting-started/dev-setup.md](./docs-site/getting-started/dev-setup.md) for ports, log locations, and troubleshooting (FFI auto-build, Swift "engine missing" sheet, streamlit hot-reload).

## Documentation

- [PLAN.md](./PLAN.md) — phased WBS + DAG + risks + reuse opportunities
- [PRD.md](./PRD.md) — product requirements (forthcoming)
- [ADR.md](./ADR.md) — index of architecture decisions (see `docs/adr/`)
- [CHARTER.md](./CHARTER.md) — scope + principles (forthcoming)
- [AGENTS.md](./AGENTS.md) — AI-agent operating notes (forthcoming)
- [docs/research/](./docs/research/) — archived Haiku research briefs (oMlx, MLX IPC, inference engines, KV cache formulas, config ingestion, GPU telemetry, Swift/WinUI/Qt FFI, fleet wire, competitor survey)

## Development status

| Phase | Status |
|-------|--------|
| P0 Foundation | in progress |
| P1 Math core | planned |
| P2 Ingestion + probe | planned |
| P3 macOS GUI MVP | planned |
| P4 Inference | planned (macOS only in MVP) |
| P5 Fleet | planned |
| P6 Windows GUI | deferred |
| P7 Linux GUI | deferred |
| **WP21 macOS Release** | **code complete** (waiting notarization creds) |

Tracked in AgilePlus: feature `hwledger-v1-macos-mvp` (see `agileplus status`).

WP21 deliverables (macOS distribution):
- Codesigning infrastructure: READY (Developer ID cert installed, entitlements defined, scripts complete)
- GitHub Actions release workflow: READY (release.yml deployed)
- DMG + notarization flow: READY (scripts deployed, awaiting App Store Connect credentials)
- Sparkle integration: READY (Package.swift updated, updater wired, key generation documented)
- Documentation: READY (docs/reports/WP21-APPLE-DEV-SECRETS.md with step-by-step setup)

## License

Apache-2.0. See [LICENSE](./LICENSE).

---

## Rich Media

<!-- RICH-MEDIA-STUB type="annotated-screenshot" subject="hwLedger CLI quickstart — first hwledger --help output" journey="install-cargo" status="PUBLISHED" -->
### CLI Quickstart — `cargo install` + `hwledger --help`

> **Journey:** `install-cargo` — Install hwledger from source with cargo, then verify version and help

![CLI install — terminal prompt, about to run cargo install](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/keyframes/install-cargo/frame-001.annotated.png)

_**Intent:** Terminal prompt, about to run `cargo install`. **Verified:** pass._

Full recorded journey: [apps/cli-journeys/manifests/install-cargo/manifest.verified.json](./apps/cli-journeys/manifests/install-cargo/manifest.verified.json)
<!-- END-RICH-MEDIA-STUB -->

<!-- RICH-MEDIA-STUB type="recording-gif" subject="VRAM plan slider UI — live per-layer breakdown" journey="first-plan" status="PUBLISHED" -->
### VRAM Plan — First Run (Llama 70B GQA)

> **Journey:** `first-plan` — Run your first plan with colored output showing token distribution for 4 users

![first-plan recording](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/recordings/first-plan.gif)

_**Intent:** Full VRAM breakdown table — weights · KV cache · activations · overhead. **Verified:** overall score 0.92._

Annotated keyframe (VRAM fits indicator):

![first-plan frame-005 annotated](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/keyframes/first-plan/frame-005.annotated.png)

Full manifest: [apps/cli-journeys/manifests/first-plan/manifest.verified.json](./apps/cli-journeys/manifests/first-plan/manifest.verified.json)
<!-- END-RICH-MEDIA-STUB -->

<!-- RICH-MEDIA-STUB type="recording-gif" subject="Fleet register — adding a new device to the ledger" journey="fleet-register" status="PUBLISHED" -->
### Fleet Register — Add a Device

> **Journey:** `fleet-register` — Register a new agent with the fleet, then verify it appears in fleet status

![fleet-register recording](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/recordings/fleet-register.gif)

_**Intent:** Device announces GPU inventory, receives mTLS cert, joins gossip network._

Annotated keyframe (registration confirmed):

![fleet-register frame-003 annotated](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/keyframes/fleet-register/frame-003.annotated.png)

Full manifest: [apps/cli-journeys/manifests/fleet-register/manifest.verified.json](./apps/cli-journeys/manifests/fleet-register/manifest.verified.json)
<!-- END-RICH-MEDIA-STUB -->

<!-- RICH-MEDIA-STUB type="recording-mp4" subject="Traceability report — audit log + provenance chain view" journey="traceability-report" status="PUBLISHED" -->
### Traceability Report — Audit Log + Provenance Chain

> **Journey:** `traceability-report` — Generate a markdown traceability report with coverage data and inspect the output

![traceability-report recording](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/recordings/traceability-report.gif)

> Full MP4 (richer quality): [apps/cli-journeys/recordings/traceability-report/traceability-report.rich.mp4](https://github.com/KooshaPari/hwLedger/blob/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/recordings/traceability-report/traceability-report.rich.mp4)

Annotated keyframe (traceability runner start):

![traceability-report frame-001 annotated](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/keyframes/traceability-report/frame-001.annotated.png)

Full manifest: [apps/cli-journeys/manifests/traceability-report/manifest.verified.json](./apps/cli-journeys/manifests/traceability-report/manifest.verified.json)
<!-- END-RICH-MEDIA-STUB -->

<!-- RICH-MEDIA-STUB type="recording-mp4" subject="VRAM reconcile — prediction vs live telemetry diff" journey="vram-reconcile" status="PUBLISHED" -->
### VRAM Reconcile — Prediction vs Live Telemetry

> **Nearest recorded journey:** `plan-mla-deepseek` — Show MLA classification and KV sequence invariance across 2K, 32K, 128K sequences (dedicated vram-reconcile journey not yet recorded; this shows the prediction side)

![plan-mla-deepseek recording](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/recordings/plan-mla-deepseek.gif)

> Full MP4: [apps/cli-journeys/recordings/plan-mla-deepseek/plan-mla-deepseek.rich.mp4](https://github.com/KooshaPari/hwLedger/blob/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/recordings/plan-mla-deepseek/plan-mla-deepseek.rich.mp4)

Annotated keyframe (MLA classification + KV invariance):

![plan-mla-deepseek frame-002 annotated](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/keyframes/plan-mla-deepseek/frame-002.annotated.png)

Full manifest: [apps/cli-journeys/manifests/plan-mla-deepseek/manifest.verified.json](./apps/cli-journeys/manifests/plan-mla-deepseek/manifest.verified.json)
<!-- END-RICH-MEDIA-STUB -->

<!-- RICH-MEDIA-STUB type="recording-gif" subject="Inference run — local Apple Silicon sidecar in action" journey="inference-run" status="PUBLISHED" -->
### Inference Run — Local GGUF Model Load

> **Nearest recorded journey:** `ingest-local-gguf` — Ingest a local GGUF model file and output JSON metadata (dedicated inference-run journey not yet recorded; this shows the ingest/load side)

![ingest-local-gguf recording](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/recordings/ingest-local-gguf.gif)

Full manifest: [apps/cli-journeys/manifests/ingest-local-gguf/manifest.verified.json](./apps/cli-journeys/manifests/ingest-local-gguf/manifest.verified.json)
<!-- END-RICH-MEDIA-STUB -->

<!-- RICH-MEDIA-STUB type="recording-mp4" subject="Fleet probe — SSH agentless hardware scan" journey="fleet-probe" status="PUBLISHED" -->
### Fleet Probe — SSH Agentless Hardware Scan

> **Journey:** `probe-list` — List all available probes in both table and JSON formats

![probe-list recording](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/recordings/probe-list.gif)

> Full MP4: [apps/cli-journeys/recordings/probe-list/probe-list.rich.mp4](https://github.com/KooshaPari/hwLedger/blob/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/recordings/probe-list/probe-list.rich.mp4)

Annotated keyframe (probe table output):

![probe-list frame-002 annotated](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/keyframes/probe-list/frame-002.annotated.png)

Full manifest: [apps/cli-journeys/manifests/probe-list/manifest.verified.json](./apps/cli-journeys/manifests/probe-list/manifest.verified.json)
<!-- END-RICH-MEDIA-STUB -->

<!-- RICH-MEDIA-STUB type="annotated-screenshot" subject="Cost model — spot-price-aware fleet dispatch view" journey="cost-model" status="PUBLISHED" -->
### Cost Model — Spot-Price Fleet Dispatch

> **Nearest recorded journey:** `probe-watch` — Watch probe metrics update in real time (dedicated cost-model journey not yet recorded; this shows live fleet telemetry)

![probe-watch recording](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/recordings/probe-watch.gif)

Annotated keyframe (probe watch start):

![probe-watch frame-001 annotated](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/keyframes/probe-watch/frame-001.annotated.png)

Full manifest: [apps/cli-journeys/manifests/probe-watch/manifest.verified.json](./apps/cli-journeys/manifests/probe-watch/manifest.verified.json)
<!-- END-RICH-MEDIA-STUB -->

<!-- RICH-MEDIA-STUB type="recording-mp4" subject="Audit log — event-sourced ledger timeline" journey="audit-log" status="PUBLISHED" -->
### Audit Log — Event-Sourced Fleet Timeline

> **Journey:** `fleet-audit` — Audit the fleet with a 3-agent limit to see agent metadata and status

![fleet-audit recording](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/recordings/fleet-audit.gif)

> Full MP4: [apps/cli-journeys/recordings/fleet-audit/fleet-audit.rich.mp4](https://github.com/KooshaPari/hwLedger/blob/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/recordings/fleet-audit/fleet-audit.rich.mp4)

Annotated keyframe (fleet audit agent metadata):

![fleet-audit frame-002 annotated](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/keyframes/fleet-audit/frame-002.annotated.png)

Full manifest: [apps/cli-journeys/manifests/fleet-audit/manifest.verified.json](./apps/cli-journeys/manifests/fleet-audit/manifest.verified.json)
<!-- END-RICH-MEDIA-STUB -->

<!-- RICH-MEDIA-STUB type="recording-gif" subject="Fleet dispatch — assign model run to cheapest fleet node" journey="fleet-dispatch" status="PUBLISHED" -->
### Fleet Dispatch — Assign Run to Cheapest Node

> **Nearest recorded journey:** `fleet-register` — Register a new agent with the fleet (dedicated fleet-dispatch journey not yet recorded; registration shows the fleet membership side of dispatch)

![fleet-register recording](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/recordings/fleet-register.gif)

Full manifest: [apps/cli-journeys/manifests/fleet-register/manifest.verified.json](./apps/cli-journeys/manifests/fleet-register/manifest.verified.json)
<!-- END-RICH-MEDIA-STUB -->

<!-- RICH-MEDIA-STUB type="recording-gif" subject="Model ingest — HuggingFace / GGUF config auto-parse" journey="model-ingest" status="PUBLISHED" -->
### Model Ingest — HuggingFace / GGUF Config Auto-Parse

> **Journey:** `ingest-local-gguf` — Ingest a local GGUF model file and output JSON metadata

![ingest-local-gguf recording](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/recordings/ingest-local-gguf.gif)

See also: `ingest-error` journey (error path) —

![ingest-error recording](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/recordings/ingest-error.gif)

Annotated keyframe (ingest error path):

![ingest-error frame-001 annotated](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/keyframes/ingest-error/frame-001.annotated.png)

Full manifests: [ingest-local-gguf](./apps/cli-journeys/manifests/ingest-local-gguf/manifest.verified.json) · [ingest-error](./apps/cli-journeys/manifests/ingest-error/manifest.verified.json)
<!-- END-RICH-MEDIA-STUB -->

<!-- RICH-MEDIA-STUB type="recording-gif" subject="Telemetry sync — live GPU stats from vLLM/TGI" journey="telemetry-sync" status="PUBLISHED" -->
### Telemetry Sync — Live GPU Stats

> **Nearest recorded journey:** `probe-watch` — Watch probe metrics update in real time with 1-second refresh intervals (dedicated telemetry-sync journey not yet recorded)

![probe-watch recording](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/recordings/probe-watch.gif)

Full manifest: [apps/cli-journeys/manifests/probe-watch/manifest.verified.json](./apps/cli-journeys/manifests/probe-watch/manifest.verified.json)
<!-- END-RICH-MEDIA-STUB -->

<!-- RICH-MEDIA-STUB type="recording-mp4" subject="KV-cache plan — sliding window + GQA breakdown" journey="kv-cache-plan" status="PUBLISHED" -->
### KV-Cache Plan — MLA vs GQA Breakdown

> **Journey:** `plan-mla-deepseek` — Show MLA classification and KV sequence invariance across 2K, 32K, 128K sequences

![plan-mla-deepseek recording](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/recordings/plan-mla-deepseek.gif)

> Full MP4: [apps/cli-journeys/recordings/plan-mla-deepseek/plan-mla-deepseek.rich.mp4](https://github.com/KooshaPari/hwLedger/blob/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/recordings/plan-mla-deepseek/plan-mla-deepseek.rich.mp4)

_**Intent:** MLA latent projection compresses KV by 16x vs full-rank GQA — sequence length invariant._

Annotated keyframe:

![plan-mla-deepseek frame-002 annotated](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/keyframes/plan-mla-deepseek/frame-002.annotated.png)

Full manifest: [apps/cli-journeys/manifests/plan-mla-deepseek/manifest.verified.json](./apps/cli-journeys/manifests/plan-mla-deepseek/manifest.verified.json)
<!-- END-RICH-MEDIA-STUB -->

<!-- RICH-MEDIA-STUB type="annotated-screenshot" subject="Spot price scan — cloud rental comparison table" journey="spot-price-scan" status="PUBLISHED" -->
### Spot Price Scan — Cloud Rental Comparison

> **Nearest recorded journey:** `plan-hf-resolve` — Plan via HF resolver: bare repo id, full HF URL, and gold fixture shortcut (dedicated spot-price-scan journey not yet recorded; HF resolve shows the model-to-hardware cost estimation entry point)

![plan-hf-resolve recording](https://raw.githubusercontent.com/KooshaPari/hwLedger/feat/user-story-batch3-playwright-plugin/apps/cli-journeys/recordings/plan-hf-resolve.gif)

Full manifest: [apps/cli-journeys/manifests/plan-hf-resolve/manifest.verified.json](./apps/cli-journeys/manifests/plan-hf-resolve/manifest.verified.json)
<!-- END-RICH-MEDIA-STUB -->
