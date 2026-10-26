# Phase Status Tracking — Tinix Animated Text Templates (Phases 27–36)

**Last Updated:** 2026-10-26
**Active Roadmap:** `Plans/plan_animated_text_templates.md`  
**Overall Program Status:** PHASES 27–33 FUNCTIONALLY COMPLETE (NATIVE QUALIFICATION ITEMS DEFERRED); PHASE 34 IMPLEMENTED, AC-09 NATIVE QUALIFICATION OPEN

---

## 1. Summary Matrix (Phases 27–36)

| Phase | Title | Status | Gate Outcome | Evidence / Deliverables |
| :--- | :--- | :--- | :--- | :--- |
| **Phase 27** | Template Contract, Fixtures & Baseline | **COMPLETE** | Gate 27 PASS | `artifacts/animated-text-templates/phase-27/` (`baseline-inventory.json`, `capcut-sample-index.json`, `compatibility-matrix.json`, `contract-semantics.md`, `fixture-corpus.json`, `workload-budgets.md`, `evidence-summary.json`) |
| **Phase 28** | Shared Text Renderer POC | **COMPLETE** | Gate 28 PASS | `artifacts/animated-text-templates/phase-28/` (`benchmark-report.md`, `benchmark-metrics.csv`, `candidate-evaluation.md`, `evidence-summary.json`, `poc_overlay_t05.png`, `poc_alpha_*.png`), `src-tauri/src/modules/media/text_renderer_poc.rs`, `src-tauri/tests/phase28_text_renderer_poc.rs`, ADR-007 locked |
| **Phase 29** | Rich-text & Animation Domain Model | **COMPLETE** | Gate 29 PASS | Schema v10 lossless migration, rich-text/animation domain + validation, 7 Command Service operations with grapheme-span rebinding, composition/preview carry-through, TS types, CLI/MCP capability parity; `artifacts/animated-text-templates/phase-29/` (`evidence-summary.json`, `verification-log.md`); QA PASS, Code Reviewer LGTM |
| **Phase 30** | Rich-Text Rendering & Parity | **COMPLETE WITH DEFERRED QUALIFICATION** | Gate 30 PASS (QA PASS, Code Reviewer LGTM; native qualification deferred) | Shared production renderer (`text_overlay.rs`), preview IPC + export PNG overlays, 7 acceptance tests, W1/W2 evidence; `artifacts/animated-text-templates/phase-30/` |
| **Phase 31** | Text Animation Engine & Native Sample | **IMPLEMENTED / T1 DEFERRED ACCEPTED** | Gate T1 deferred by PO | Evaluator, rigid unit motion, preview/export sequence, recreated sample + tests; QA PASS; Final Review requires native/reference evidence |
| **Phase 32** | Layered Templates & Packs | **COMPLETE** | Gate 32 PASS (QA PASS, Final Reviewer FINAL PASS) | Multilayer instances (<=16 layers), parameter bindings, `.tinixtpl` import/export |
| **Phase 33** | Template Authoring & Library UI | **COMPLETE** | Gate T2 PASS (native qualification deferred) | Inspector customizer, animated preview cards, responsive anchor controls (Milestone T2) |
| **Phase 34** | CapCut Template Import Adapter | **IMPLEMENTED / AC-09 OPEN** | Backend + frontend reviews LGTM; native qualification pending | Bounded/cancellable draft and ZIP inspection, resource-aware conversion, transactional save, guarded CommandService apply, CapCut importer UI; `artifacts/animated-text-templates/phase-34/` |
| **Phase 35** | Agent Template Generation (CLI/MCP) | **IN PROGRESS** | Pending | Agent template authoring/editing via Command Service transaction (Milestone T4) |
| **Phase 36** | Release Hardening & Qualification | **PLANNED** | Pending | Full performance benchmarks, installer integration, final user verification (Milestone T5) |

---

## 2. Phase 27 Acceptance Criteria Detail

- **P27-AC-01 [PASS]:** Source inventory & baseline established. Git revision `e6462544b6956054ee50af40727d44e92121487b`. All 44 frontend tests and 222 Rust tests pass (141 lib + 81 integration suites, skipping POC5 per policy). Baseline binary sizes recorded. Pre-existing POC5 matrix skip and 3 Clippy warnings documented.
- **P27-AC-02 [PASS]:** User CapCut sample (`TINIX_CAPCUT_IMPORT_TEST`) indexed with draft, video, style, script, and font SHA256 hashes (plus Apache-2.0 `Roboto-Bold.ttf` SHA-256 `61F89F8DB49261C2F6106E8DCCC35DF7B2F7ED909020DB40A3FC905E95F99334`). Exact 29 graphemes / 24 non-space letters, scale factors (0.31 / 1.903216), and Lua 21-slide (`1..21`) + 3-bounce (`22..24` `'i'`, `'ệ'`, `'t'`) sequence (`totalFrame = 156`) documented. Style (118° gradient, stroke `#AD5100`, 2 shadows) and timing (1.033s in-stagger) locked.
- **P27-AC-03 [PASS]:** 19-option matrix categorized into Core, Extended, and Unsupported tiers with owner phases and validation boundaries.
- **P27-AC-04 [PASS]:** Canonical `RationalTime`, Extended Grapheme Cluster character definition, grapheme-offset span indexing, duration collision rejection-by-default (with explicit proportional/clamp policies), and .tinixtpl packaging specified.
- **P27-AC-05 [PASS]:** 14 fixtures (F01–F14) defined covering exact CapCut recreation with variants ('Tinix', 'Hi'), Vietnamese NFC/NFD diacritics, ZWJ emoji, multiline, edge cases, Latin ligatures, CJK/RTL boundaries, multi-stroke/shadow stress, gradients, duration collision policies, responsive canvas anchors across 4 aspect ratios and 5 frame rates, rich spans + background box/glow, multilayer density stress (16 layers), and full in/loop/out lifecycle with 8 sampling checkpoints.
- **P27-AC-06 [PASS]:** Reference Windows host, workloads W1–W4, and proposed latency budgets (W1 p95 <= 16ms, W2 p95 <= 33.3ms, cache <= 256MiB) documented.
- **P27-AC-07 [PASS]:** Phase 26 deferred qualification items retained without falsification; zero application code modified in contract phase.

---

## 3. Phase 28 Acceptance Criteria Detail

- **P28-AC-01 [PASS]:** Sample typography and style primitives rendered via pure Rust typography engine (`src-tauri/src/modules/media/text_renderer_poc.rs` with `ab_glyph 0.2.32`, `unicode-normalization 0.1.25`, and `unicode-segmentation 1.13.3`). Character-level extended grapheme cluster segmentation with NFC precomposition, OpenType horizontal kerning, and unified cluster mask rasterization before 2D rotation correctly preserves Vietnamese diacritics (`ử`, `ệ`, `ế`) across 21-char vertical slide and 3-char bounce + Z-rotation (`24` non-space characters). Verified with Apache-2.0 open-source `Roboto-Bold.ttf` (`0` missing glyphs).
- **P28-AC-02 [PASS]:** Deterministic overlay generation proven. Bit-for-bit identical RGBA buffers generated on repeated runs across Preview and Export consumer pathways ($\Delta \text{PSNR} = \infty$, 0 mismatch bytes across 8,294,400 bytes).
- **P28-AC-03 [PASS]:** Consumer 1 Tauri IPC command `render_text_template_poc_preview` delivers `PreviewOverlayPacket` with tight dirty ROI bounding box `[667, 471, 1087, 570]` and cropped ROI PNG (`166,497 bytes`). Decoded Preview ROI vs Lossless Export achieves $\text{ROI SSIM} = 1.000000$ ($\ge 0.995$) and $\text{ROI MAE} = 0.000000$ ($\le 0.005$).
- **P28-AC-04 [PASS]:** Alpha channel integrity verified across 4 backgrounds (`poc_alpha_white.png`, `poc_alpha_black.png`, `poc_alpha_color.png`, `poc_alpha_video.png`) with zero dark halo or double-premultiplication artifacts.
- **P28-AC-05 [PASS]:** Windows 11 reference host benchmark budgets satisfied without relaxation (`benchmark-metrics.csv`): W1 Cold = 11.03 ms / Warm p95 = 5.21 ms (budget $\le$ 16.0 ms), W2 Cold = 12.92 ms / Warm p95 = 14.87 ms (budget $\le$ 33.3 ms), W3 16:9 Cold = 19.67 ms / Warm p95 = 17.23 ms (budget $\le$ 50.0 ms), W3 9:16 Cold = 19.60 ms / Warm p95 = 16.92 ms (budget $\le$ 50.0 ms), W4 throughput = 5.18× RTF (budget $\ge$ 1.0× RTF). `BoundedGlyphCache` uses `176,499 bytes` (`0.17 MiB` / `256 MiB` ceiling).
- **P28-AC-06 [PASS]:** Consumer 2 streams 60 frames directly into FFmpeg stdin pipe (`rawvideo rgba`, `overlay=format=auto`). Verified real MP4 export (`poc_export_sample.mp4`) and decoded MP4 frame parity ($\text{ROI SSIM} = 0.9978 \ge 0.985$). Cancellation token terminates child process immediately and removes incomplete disk files.
- **P28-AC-07 [PASS]:** Clean Windows delivery with zero external C++/LLVM/Python toolchain dependencies. Binary size overhead is < 850 KB (vs +42 MB for Skia). Phase 26 clean-VM qualification remains explicitly deferred.
- **P28-AC-08 [PASS]:** ADR-007 locked in `ARCHITECTURE.md`, root file `ADR_007_SHARED_TEXT_RENDERER_AND_OVERLAY_PIPELINE.md`, and `artifacts/animated-text-templates/phase-28/candidate-evaluation.md`.

---

## 4. Phase 29 Acceptance Criteria Detail

- **P29-AC-01 [PASS]:** Typed rich-text/animation validation with success and failure cases. Grapheme indexing via UAX #29 (Vietnamese NFC/NFD and ZWJ emoji); byte/UTF-16 indexing rejected by construction. Limits enforced: 512 graphemes, 4 strokes, 4 shadows, 8 gradient stops. Random stagger requires a seed; malformed colors/transforms rejected.
- **P29-AC-02 [PASS]:** Schema v9→v10 migration is a version bump only. A comprehensive fixture preserves `text_style`, `text_template`, item and track transitions, audio crossfade, keyframes and command ledger verbatim; new fields default to `None` and no animation is invented. Schema 11 is rejected.
- **P29-AC-03 [PASS]:** Atomic save/recovery behavior is unchanged; a newer-schema file fails safely without damaging the on-disk project.
- **P29-AC-04 [PASS]:** All 7 new operations apply atomically with undo/redo round trips and full transaction rollback when any operation fails.
- **P29-AC-05 [PASS]:** Stale revision rejected; replay after save/reopen is deduplicated with no duplicate mutation.
- **P29-AC-06 [PASS]:** `ClipTextSet` expands full-length spans, clamps partial spans, prunes empty spans on empty text, and never retains out-of-bounds indices; text templates and other tracks are untouched.
- **P29-AC-07 [PASS]:** Composition and sampled preview layers carry resolved rich-text/animation values for Text only; frontend types mirror Rust serde names/shape; CLI and MCP capability payloads and schemas expose the four new capability lists.
- **P29-AC-08 [PASS]:** Regression/build checks: 152 lib tests (POC5 matrix documented skip), 13 Phase 29 tests, 12 independent QA tests, Phase 14–18 and export suites, typecheck, 44 frontend tests, web build. Clippy retains only the 3 pre-existing findings.

---

## 5. Prior Baseline Phases Reference (Phases 0–26)

- **Phases 0–15:** Complete.
- **Phase 16:** Complete with deferred interactive Windows GUI qualification (`P16-AC-56`).
- **Phase 17:** Complete with deferred items (Performance benchmarks & release foundation).
- **Phase 18:** Complete with deferred items (Audible playback & audio crossfades verified by user; health check & cleanup observation deferred).
- **Phase 19:** Complete (Initial 6 text template presets, schema v5 text styles).
- **Phases 20–25:** Implemented (SRT/WebVTT captions, aspect ratio/canvas background, visual effects & dip to black, clip speed, text batch actions, auto-captions via local Whisper).
- **Phase 26:** Open for consolidated manual desktop qualification (`Plans/phase26_manual_qualification.md`).

---

## 6. Next Implementation Step

Proceed to **Phase 31 - Text Animation Engine & Native Sample** (`Plans/plan_phase31_text_animation.md`):
1. Evaluate the Phase 29 `text_animation` (in/loop/out, grapheme stagger, transforms) on the canonical timeline and feed the shared renderer per frame.
2. Preserve the Phase 30 preview/export parity seam; do not add a second renderer or mutation path.
3. Recreate the user's CapCut sample as an editable Tinix template (Milestone T1) with Vietnamese diacritics intact.
4. Keep the Phase 30 W1 packet transport limit and Phase 26 native qualification gaps explicit.

