# PROJECT_STATUS.md

> **Current Project Execution Snapshot**
>
> This document records the current durable state of the project so another coding agent or a future session can resume quickly.
>
> It is a snapshot, not a chronological development log.

---

## 0. Status Metadata

**Project:** Automation-First Desktop Video Editor
**Overall Status:** PHASES 0–25 IMPLEMENTED; PHASE 26 QUALIFICATION OPEN; PHASES 27–33 FUNCTIONALLY COMPLETE (NATIVE QUALIFICATION ITEMS DEFERRED); PHASE 34 IMPLEMENTED WITH AC-09 QUALIFICATION OPEN; PHASES 35–36 PLANNED
**Current Selected Milestone:** Rich Animated Text Templates — Phase 35 Agent Template Generation IN PROGRESS
**Current Active Implementation Phase:** Phase 34 — CapCut Template Import (implementation complete; native acceptance qualification open)
**Phase 16 Status:** COMPLETE WITH DEFERRED ITEMS
**Deferred Acceptance Qualification:** P16-AC-56 (Real interactive Windows GUI qualification deferred), True Clean-VM qualification deferred
**Implementation:** Phases 0–25 implemented with automated validation; Phases 27–33 functionally complete; Phase 34 importer backend and UI implemented, with native acceptance qualification pending
**ADR:** ADR-006 accepted for Phase 18; ADR-007 accepted for Phase 28 (Shared Text Renderer & Deterministic Overlay Seam — see `ADR_007_SHARED_TEXT_RENDERER_AND_OVERLAY_PIPELINE.md`)
**Plan Status:** Phases 27–33 functionally complete; Phase 34 implemented with AC-09 native UI/source-comparison qualification pending; status tracked in `phase_status.md`
**Project Schema:** Schema v11 active; lossless v1–v3 migration, v4→v5 text-style migration, v5→v6 media-dimension/placement migration, v6→v7 canvas-background migration, v7→v8 visual-effect defaults, v8→v9 default clip-speed migration, v9→v10 rich-text/animation defaults, and v10→v11 template defaults verified
**Phase 17:** COMPLETE WITH DEFERRED ITEMS — qualification commit 28f94c6; durable reconciliation recorded
**Baseline Commit:** 94a084e076609f5e441c6376e3c8e45443653b80 (`chore(baseline): consolidate verified phases 19-29`); Phase 30 is an uncommitted working-tree candidate
**Phase 18:** COMPLETE WITH DEFERRED ITEMS. User passed the documented video/audio transport and crossfade scenarios; direct native `app_health_check` observation and independent preview-media cleanup observation remain deferred.
**Phase 27 Status:** COMPLETE (Gate 27 PASS). Baseline inventory, CapCut user sample index (exact 29 graphemes / 24 non-space letters = 21-char vertical slide + 3-char bounce/rotation, plus Apache-2.0 `Roboto-Bold.ttf` SHA-256), option compatibility matrix, canonical contract semantics, 14-fixture corpus, and reference workload budgets locked in `artifacts/animated-text-templates/phase-27/`.
**Phase 28 Status:** COMPLETE (Gate 28 PASS). Pure Rust typography and rasterizer engine implemented (`src-tauri/src/modules/media/text_renderer_poc.rs` via `ab_glyph 0.2.32` + `unicode-normalization 0.1.25` + `unicode-segmentation 1.13.3`), true OpenType glyph rasterization with horizontal kerning and unified cluster masks, static-bounds-anchored 118° linear gradient, 2 outer strokes, 2 blurred drop shadows, `BoundedGlyphCache`, Tauri IPC command `render_text_template_poc_preview`, 6 integration tests passing, W1–W4 benchmarks satisfying Phase 27 budgets without relaxation (W1 Warm p95 = 5.21 ms, W2 = 14.87 ms, W3 16:9 = 17.23 ms, W3 9:16 = 16.92 ms, W4 = 5.18x RTF), lossless parity (ROI SSIM = 1.000000, MP4 ROI SSIM = 0.9978), 4-background alpha integrity, real FFmpeg stream export and cancellation verified, ADR-007 locked. Benchmarks must be run with the documented optimized test profile (`CARGO_PROFILE_TEST_OPT_LEVEL=3`); see phase-29/verification-log.md.
**Phase 29 Status:** COMPLETE (Gate 29 PASS; QA PASS, Code Reviewer LGTM). Rich-text/animation domain model, schema v10 lossless migration, 7 Command Service operations with grapheme-span rebinding, composition/preview carry-through, TS types and CLI/MCP capability parity implemented. Evidence in `artifacts/animated-text-templates/phase-29/evidence-summary.json`; 13 Phase 29 + 12 QA tests pass.
**Phase 30 Status:** IMPLEMENTED WITH DEFERRED NATIVE QUALIFICATION (Gate 30 functional criteria PASS; QA PASS; Code Reviewer LGTM after remediation). Shared production renderer `src-tauri/src/modules/media/text_overlay.rs` consumes Phase 29 `text_rich` in preview (`preview_rich_text_overlay` IPC) and headless export (materialized ROI PNGs, never drawtext for rich items). 10 Phase 30 acceptance tests pass (1 ignored evidence writer); 162 lib + 13 Phase 29 tests, typecheck, 44 frontend tests, web build, native build and fmt pass. Evidence and documented limits in `artifacts/animated-text-templates/phase-30/` (W1 buffer p95 12.61 ms ≤ 16 ms; W2 19.83 ms ≤ 33.3 ms; oversized multi-line PNG packet 23.00 ms documented; native GUI/installed-app qualification deferred).
**Next Action:** Complete Phase 34 AC-09 native qualification: exercise the importer dialog against the documented sample, verify inspection/resource reporting, then apply and save only in a clean test project and capture source-comparison ROI/frame evidence. Keep Phase 26 native-desktop qualification, Phase 30 native GUI/installed-resource observation, automatic-caption smoke, clean-VM and hardware-encoder evidence gaps visible.
**Last Updated:** 2026-10-26

---

## 1. Current Objective

The user approved implementation of the Animated Text Templates program per Plans/plan_animated_text_templates.md. Phases 27 (Template Contract, Fixtures and Baseline), 28 (Shared Text Renderer POC & Architecture Gate), 29 (Rich-text & Animation Domain Model) and 30 (Rich-Text Rendering & Parity) are COMPLETE. Phase 31 (Text Animation Engine & Native Sample) is ready to begin.

Phase tracking across the entire program is maintained in both PROJECT_STATUS.md and phase_status.md.

### Phase 30 implementation checkpoint — 2026-10-25

- **Shared production renderer (`src-tauri/src/modules/media/text_overlay.rs`)**: font resolution without silent substitution (bundled Roboto-Bold or explicit project font asset), NFC + UAX #29 grapheme layout with kerning, line height, tracking, alignment, Wrap/AutoFit, per-span fill/font-size/tracking, solid/linear/radial fills, ordered multi-stroke, multi-shadow blur, background boxes, bounded versioned mask cache, straight-alpha compositing, and placement scale/rotation/opacity baked into the overlay pixels.
- **Preview seam**: `preview_rich_text_overlay` Tauri command + `editorApi.previewRichTextOverlay`; `PreviewCanvas` renders the ROI PNG for rich items (legacy text keeps the DOM path) and caches one overlay per item per project revision. Missing fonts/glyphs surface as an explicit banner, never a silent fallback.
- **Export seam**: `ExportEngine::render_snapshot` materializes rich-text ROI PNGs through the same renderer and passes them to `build_video_filtergraph_with_overlays`; rich items never emit `drawtext`, legacy items keep it. Derived overlay and automation directories are removed on every exit path; output promotion retries briefly against Windows scan locks.
- **Verification**: 7 Phase 30 tests pass (1 ignored evidence writer), including a real FFmpeg export with decoded non-background text pixels; 162 lib tests (POC5 skipped), 13 Phase 29 tests, typecheck, 44 frontend tests, web build, native build, clippy (no new lib findings) and `cargo fmt --check` pass.
- **Performance**: W1 buffer warm p95 14.87 ms ≤ 16 ms; W2 19.83 ms ≤ 33.3 ms; canonical one-line IPC packet 7.49 ms. Oversized multi-line PNG packet p95 23.00 ms is documented as a transport limit (preview caches per revision).
- **Deferred**: native GUI/installed-app observation remains under Phase 26; glow has no Phase 29 domain field; text animation and animated transforms are Phase 31.
- **Evidence**: `artifacts/animated-text-templates/phase-30/` (`evidence-summary.json`, `verification-log.md`, `benchmark-production.json`, `export-metrics.json`, parity/alpha PNGs, `export-smoke.mp4`).

### Phase 29 implementation checkpoint — 2026-10-25

- **Domain (`src-tauri/src/modules/domain/text.rs`)**: `FontReference`, `TextLayout`, `TextFill` (internally tagged gradient/solid), ordered strokes/shadows, background box, grapheme-indexed `RichTextSpan`, `TextRichStyle`, and animation storage (`AnimationUnit`, `StaggerOrder`, `LoopMode`, `DurationConflictPolicy`, `AnimationTransform`, in/loop/out segments, pivot, seed). Hard limits 512 graphemes / 4 strokes / 4 shadows / 8 gradient stops.
- **Schema v10**: additive `TimelineItem.text_rich` and `text_animation` (`#[serde(default)]`, default None); `migrate_v9_to_v10` bumps the version only and preserves legacy text/style/template/keyframes/transitions/ledger.
- **Commands**: `ClipTextRichSet`, `ClipTextSpansSet`, `ClipTextLayoutSet`, `ClipTextAppearanceSet`, `ClipTextFontSet`, `ClipTextAnimationSet`, `ClipTextRichReset`; `ClipTextSet` expands full-length spans, clamps partial spans, prunes empty spans and validates before commit. Revision/idempotency/undo-redo behavior is unchanged.
- **Consumers**: normalized composition and sampled preview layers carry `text_rich`/`text_animation`; TypeScript types mirror Rust; CLI `CapabilitiesReport` and MCP payload/schema expose the four new capability lists.
- **Verification**: fmt/build clean; 152 lib tests pass (POC5 matrix filtered); 13 Phase 29 tests; 12 independent QA tests; Phase 14–18 and export suites pass; typecheck + 44 frontend tests + web build pass; clippy shows only the 3 pre-existing findings. Phase 28 optimized-profile invocation passes 6/6 with AC03 IPC 6.70 ms and no canonical-evidence overwrite.
- **Review**: QA PASS (including BF-01/BF-02 supplement); Code Reviewer LGTM after two closed findings.
- **Deferred**: rich-text rasterization integration is Phase 30; animation evaluation and deterministic split partitioning are Phase 31.

### Phase 28 implementation checkpoint — 2026-10-25

- **Shared Text Renderer POC Engine (`src-tauri/src/modules/media/text_renderer_poc.rs`)**:
  - Implemented high-performance, deterministic software rasterizer in pure Rust using `ab_glyph 0.2.32`, `unicode-normalization 0.1.25`, and `unicode-segmentation 1.13.3`.
  - Built-in static-bounds-anchored multi-stop linear gradient color sampling (118° angle), multi-layer circular outline stroke dilation (sorted widest-to-narrowest: 7.0 px `#4A2000` + 4.0 px `#AD5100`), separable box blur shadows (`#AD5100` + `#FFA758`), rounded background boxes, and decorative shape layers.
  - Implemented character-level Extended Grapheme Cluster segmentation (Unicode UAX #29) with NFC precomposition, OpenType horizontal kerning, and unified cluster mask rasterization before 2D rotation, preserving Vietnamese diacritics (`ử`, `ệ`, `ế`) as single atomic units.
  - Implemented Lua virtual timeline mapping (`totalFrame = 156`), cubic Bézier evaluation (`[0.166667, 0.166667, 0.14, 1.0]`, etc.), character stagger cascade, 21-char vertical slide (`1..21`), and 3-char bounce + Z-axis rotation (`22..24` `'i'`, `'ệ'`, `'t'`).
  - Implemented straight-alpha frame rasterizer with Porter-Duff $(A \text{ over } B)$ blending, `BoundedGlyphCache` ($\le 256\text{ MiB}$ ceiling), store-mode PNG encoder/decoder, tight dirty ROI cropping (`roi_png_bytes`), Tauri IPC command `render_text_template_poc_preview`, and bounded FFmpeg streaming export.
- **Integration Test Suite (`src-tauri/tests/phase28_text_renderer_poc.rs`)**:
  - All 6 acceptance tests pass (`test_p28_ac01` through `test_p28_ac06`).
  - Proves 100% bit-for-bit deterministic overlay output between Preview and Headless Export pathways ($\Delta \text{PSNR} = \infty$, $\text{ROI SSIM} = 1.000000$, $\text{ROI MAE} = 0.000000$, encoded MP4 $\text{ROI SSIM} = 0.9978$).
  - Proves alpha channel integrity across 4 backgrounds (`poc_alpha_white.png`, `poc_alpha_black.png`, `poc_alpha_color.png`, `poc_alpha_video.png`) with zero dark halo or double-premultiplication.
  - Proves child FFmpeg process cancellation and cleanup.
- **Benchmark & Delivery Results (`artifacts/animated-text-templates/phase-28/benchmark-report.md` & `benchmark-metrics.csv`)**:
  - Measured on Windows 11 reference host (i5-13400F, 64GB, RTX 3090): W1 Cold = 11.03 ms / Warm p95 = 5.21 ms (budget $\le 16.0\text{ ms}$), W2 Cold = 12.92 ms / Warm p95 = 14.87 ms (budget $\le 33.3\text{ ms}$), W3 16:9 Cold = 19.67 ms / Warm p95 = 17.23 ms (budget $\le 50.0\text{ ms}$), W3 9:16 Cold = 19.60 ms / Warm p95 = 16.92 ms (budget $\le 50.0\text{ ms}$), W4 continuous FFmpeg export stream = 5.18× RTF (`0.386 s` for 60 frames, budget $\ge 1.0\times$ RTF).
  - Bounded Glyph Cache footprint: `176,499 bytes` (`0.17 MiB` / `256.00 MiB` max ceiling; `35` entries, `4,165` hits, `35` misses).
- **Architecture Gate 28 & ADR-007 Locked**:
  - Candidate B (Pure Rust Native Typography & Rasterization Stack) selected over Candidate A (`skia-safe`) due to toolchain portability (zero C++/LLVM requirement on Windows) and binary efficiency (< 850 KB overhead vs +42 MB for Skia).
  - Locked in `ARCHITECTURE.md` (ADR-007) and documented in `RESEARCH.md` and `artifacts/animated-text-templates/phase-28/candidate-evaluation.md`.

### Phase 27 implementation checkpoint — 2026-10-25

- **Specification & Baseline Artifacts**: All deliverables completed and locked under `artifacts/animated-text-templates/phase-27/`:
  - `baseline-inventory.json`: Current Git HEAD e6462544b6956054ee50af40727d44e92121487b, schema v9 baseline, 44 frontend tests passing, 222 total Rust tests passing (141 library + 81 integration suites, skipping non-terminating POC5 test per policy), clean format check, production build pass, baseline binary sizes, and 3 pre-existing uncommitted Clippy warnings tracked.
  - `capcut-sample-index.json`: User sample TINIX_CAPCUT_IMPORT_TEST fully indexed with draft/video/style/script/font SHA256 hashes (plus Apache-2.0 `Roboto-Bold.ttf` SHA256 `61F89F8DB49261C2F6106E8DCCC35DF7B2F7ED909020DB40A3FC905E95F99334`), exact 29 graphemes / 24 non-space letters, scale factors (0.31 / 1.903216), 118° gradient style, brown stroke #AD5100 width 0.03808219, double shadow (#AD5100 & #FFA758), exact Lua animation sequence (21-char slide + 3-char bounce/rotation), font licensing constraints, and Lua engine exclusion.
  - `compatibility-matrix.json`: 19 features partitioned into Core, Extended, and Unsupported tiers with owner phases (P29–P35), explicit test scenarios, .tinixtpl template pack format, and hard validation bounds (max 16 layers, 8 text slots, 512 graphemes/slot, 4 strokes, 4 shadows, 8 gradient stops).
  - `contract-semantics.md`: Unambiguous definitions for canonical RationalTime { value: i64, rate: FrameRate }, graphics pipeline 2D transform multiplication order, character as Extended Grapheme Cluster (Unicode UAX #29), grapheme-offset rich text span indexing across Rust and TypeScript, rejection-by-default duration collision resolution with explicit proportional/clamp policies, and .tinixtpl template packaging.
  - `fixture-corpus.json`: 14 standard fixtures (F01–F14) covering exact CapCut sample recreation, Vietnamese NFC/NFD diacritics, ZWJ emoji, multiline wrapping, edge cases, Latin ligatures, CJK/RTL diagnostic boundaries, appearance stress, duration collision policies, and responsive canvas anchors across 4 aspect ratios and 5 frame rates. Strict text ROI + alpha tolerance evaluation policy established.
  - `workload-budgets.md`: Windows reference host specified (Windows 11 Pro 24H2, i5-13400F, 64GB, RTX 3090, 990 PRO NVMe); baseline binary sizes documented; workloads W1–W4 standardized; performance budgets proposed for Phase 28 validation (W1 p95 <= 16ms, W2 p95 <= 33.3ms, overlay cache <= 256MiB).
  - `milestone-verification-checklist.md`: Comprehensive checklists for Milestones T1 (Motion Prototype), T2 (Native Authoring), T3 (CapCut Parity Gate), T4 (Headless Automation), and T5 (Hardening & Release).
  - `evidence-summary.json`: Detailed machine-readable verification evidence confirming P27-AC-01 through P27-AC-07 all PASS.
- **P26 Qualification Continuity**: All open Phase 26 items (interactive GUI qualification, clean Windows VM, hardware encoder) remain documented and open.
- **Application Code Invariant**: Phase 27 modified specification and documentation only; application source and existing uncommitted working tree were strictly preserved.

### Previous implementation checkpoints — retained evidence

Phases 18–24 are implemented while preserving their deferred native qualification for the consolidated Phase 26 pass:
- **Phase 18 Implementation Status**: COMPLETE WITH DEFERRED ITEMS; interactive transport and crossfade qualification pass per user; typed health IPC and preview cleanup remain deferred. See `Plans/plan_phase18.md` for criterion-by-criterion evidence.
- **Schema**: v6 is active; v5→v6 preserves existing media items while adding optional source dimensions and defaulting all existing clips to `Fit` placement.
- **Automated Verification**: `pnpm run test:all` passes with 44 frontend tests and 214 Rust tests passed (5 benchmark tests intentionally ignored); frontend typecheck/build, `cargo fmt --check`, native build, and library Clippy pass. All-target Clippy remains blocked by existing warnings across acceptance tests and Rust unit tests.
- **Reported Failures**: Crossfade create/duration/remove/undo-redo; play/pause/seek/start/end without audio; imported video freezes after a few seconds. Follow-up testing isolated video silence to the preview's unconditional `muted` attribute and likely freeze behavior to seeking the video decoder on each sampled playhead update.
- **Remediation**: Import persists probed media duration; adding an audio crossfade chooses a duration supported by the source handles; duration edits commit on blur/Enter; audio context unlock is initiated by the play gesture and media playback errors are shown. Active video audio now uses the video element's sampled gain and clock; ordinary playback drift no longer triggers repeated seeks, while paused scrubbing remains precise.
- **Inspector Selection Smoke Retest**: User reports PASS: selecting a clip leaves the preview visible and the Inspector present. The user subsequently passed the crossfade editing scenarios as well.
- **User interactive acceptance**: PASS for P18 transport (play past 10 seconds, pause, seek, project end, mute/unmute) and crossfade (create, audible transition, duration, remove, undo/redo, clear missing-handle failure).
- **Deferred Gates**: Direct typed `app_health_check` observation and independent preview media-resource cleanup observation.
- **Runtime**: The project-owned `pnpm tauri dev` runtime was restarted and verified current in the latest implementation checkpoint. Its Vite endpoint responds HTTP 200 and the native TinixCut process/window is responsive. The computer-use surface in this session exposes only a normal browser, not the native window, so typed IPC and selection-dependent UI behavior could not be observed here.
- **Phase 19 Checkpoint**: Schema v5 domain values, migration, six-template registry, template/style/reset commands, normalized composition propagation, preview/export consumption, Inspector controls, command-backed `+ Text` creation, and automation capability exposure are implemented. The user reports PASS for template insertion, picker/style/reset, undo/redo, save/reopen, preview, and export. The deterministic FFmpeg font fallback is accepted for personal-use v1; font weight and line-height are not fully represented by the current drawtext fallback.
- **Architecture Reconciliation**: Reviewed against the approved architecture; Phase 19 keeps text style and template identity in the canonical Project Domain Model and routes mutations through the Command Service, so `ARCHITECTURE.md` requires no change.
- **Checkpoint**: Automated remediation checks and runtime restart passed. The user passed the native audible playback, crossfade, and Phase 19 text-template scenarios. Native Computer Use still exposes no app window in this session, so typed IPC and independent cleanup observation remain unqualified.

### Phase 24 implementation checkpoint — 2026-10-09

- Captions remain ordinary Phase 20 Text items; no new caption schema or migration was introduced.
- Added atomic `TextTrackStyleSet` and `TextTrackTemplateApply` Command Service operations. Each validates its input, changes every Text item on exactly one track, returns that track as the affected scope, and rejects an empty target track without advancing the revision.
- Inspector text-template and style controls expose track-wide batch actions only for tracks with more than one text clip. Batch template selection is one-shot, so a later template choice returns to the selected clip unless the user explicitly opts in again.
- Focused regression evidence confirms both operations are track-scoped and atomic, preserve other text tracks, record the versioned template reference, and reject an empty track.
- Verification passed: TypeScript typecheck, 44 frontend tests, production frontend build, Rust formatting, the focused Phase 24 test, 139 other library tests with the known non-terminating POC5 matrix excluded, native Rust build, and diff check.

### Phase 25 implementation checkpoint — 2026-10-09

- Added an opt-in Auto captions dialog for one selected audio or video clip. The user supplies a local `whisper-cli.exe` and compatible model `.bin`; their paths and language preference stay in local UI storage and are not written to the project.
- The backend submits one cancellable, bounded transcription job. It creates a disposable 16 kHz mono WAV using the existing FFmpeg boundary, invokes Whisper JSON output, then removes temporary WAV and JSON artifacts after every terminal outcome.
- Provider offsets are parsed as integer milliseconds, limited to the clip's source trim, mapped through its fixed speed into canonical project time, and committed as normal Text clips on an `Auto Captions` track through one Command Service transaction. The result is therefore editable and uses the normal undo/redo flow.
- Explicit errors cover unavailable local tools, unsupported selection, cancellation, provider failure, and stale selected-media results. A completed job may be requested again; only active work is deduplicated.
- Verification passed: TypeScript typecheck and production build; focused transcription and job tests; 141 Rust library tests plus all integration/acceptance suites with the known non-terminating POC5 matrix test excluded; native Rust build; and `git diff --check`.
- Remaining evidence is intentionally batched in Phase 26: native end-to-end generation using a compatible local Whisper CLI/model, inspection of generated text/timing, and undo/redo. No transcription executable or model is bundled with the project.

### Phase 26 preparation — 2026-10-09

- Consolidated all remaining native-desktop acceptance into `Plans/phase26_manual_qualification.md`, including canvas/template, manual and automatic captions, transport/mute, transitions/crossfades/speed, export, save/reopen, and missing-media relink checks.
- The current restarted development runtime is ready for this one-pass qualification. The automatic-caption case is `BLOCKED` only when a compatible user-provided local Whisper CLI/model is unavailable; an invalid supplied path must instead produce the documented clear failure.

### Phase 23 implementation checkpoint — 2026-10-09

- Added canonical `Half`, `Normal`, and `Double` `ClipSpeed` values with schema v9 migration that defaults existing clips to `Normal`.
- Timeline/source mapping, handle validation, normalized composition, preview, and FFmpeg export share the same bounded speed semantics. Preview/export preserve pitch through `preservesPitch` and deterministic `atempo` mappings.
- Targeted regression, frontend typecheck/tests/build, native build, and runtime restart completed. The full library test command remains excluded because the existing POC5 matrix harness has no timeout and can hang after reporting its completed tests.

### Phase 22 implementation checkpoint — 2026-10-09

- Added canonical `VisualEffectPreset` values (`None`, `Mono`, `Vivid`, and `Muted`) for video and image clips, with schema v8 migration that safely defaults existing clips to `None`.
- Added command-backed visual effect selection and cut-owned `DipToBlack` transitions. Both route through the existing Command Service, preserve revision/history behavior, and remain distinct from item-owned fade transitions.
- Normalized composition carries the effect values into preview and FFmpeg export. Preview uses matching CSS filters; export maps the bounded presets to deterministic `eq` filters and maps `DipToBlack` to FFmpeg's `xfade=fadeblack`.
- Inspector controls expose clip effects and distinct Cross Dissolve / Dip to Black seam actions without accepting cut transitions as item transitions.
- Targeted effects, command-history, migration, composition, and filtergraph tests pass. Full verification passes: 44 frontend tests, 137 Rust unit tests, all existing integration/acceptance tests, production frontend build, native Rust build, format, and diff checks.
- The project-owned runtime restart and health verification follow this checkpoint's final code change. Native manual qualification remains batched in Phase 26.

### Phase 21 implementation checkpoint — 2026-10-09

- Added the canonical `ProjectSettingsSet` command for revision-checked canvas changes. The command validates H.264-compatible dimensions and round-trips through undo/redo.
- Added 16:9, 9:16, 1:1, and 4:5 toolbar presets. The preview viewport derives from the project canvas instead of a fixed 16:9 display.
- The default export profile now inherits the project’s canvas, frame rate, and audio sample rate, so changing a project ratio is carried into default export semantics.
- Media import now persists probed source dimensions in the canonical project model. `Fit` and `Fill` placement are Command Service mutations and are represented in the shared Composition Plan, so preview and FFmpeg export calculate the same content geometry.
- Schema v6 migrates existing assets without dimensions and existing clips without a placement mode safely to `Fit`; schema v7 adds a black canonical canvas background to prior projects.
- `CanvasBackgroundSet` persists normalized solid background color through the Command Service, normalized composition, preview, and FFmpeg export. Preview-only title/action safe-zone overlays are non-persistent and do not alter export timing or media semantics.
- The Output menu supplies balanced, high-quality, and smaller-file export quality presets. Every preset inherits the canonical canvas and frame rate; the backend rejects contradictory direct output geometry or requests that specify both profile and preset.
- Targeted command, migration, export, and composition regressions pass. Full project verification passes: 44 frontend tests, 134 Rust unit tests, the existing acceptance/integration suites, production frontend build, native Rust build, `cargo fmt --check`, and `git diff --check`.
- Native manual verification remains batched in Phase 26. The project-owned runtime restart and health verification follow this checkpoint's final code change.

### Current Phase 20 implementation checkpoint — 2026-10-09

- Added bounded SRT and WebVTT parsing with UTF-8/BOM/CRLF normalization, integer frame-rate conversion, overlap rejection, and input limits.
- Added `captions_import` and `captions_export` Tauri commands. Import creates a dedicated Text caption track through one Command Service transaction; export uses an atomic sibling-file replacement.
- Added desktop Import captions and Export captions actions.
- Parser regression tests pass: SRT multiline/CRLF, VTT identifiers/settings, malformed/empty/zero/overlapping cues, 29.97fps rounding, and case-insensitive extensions.
- Full validation passes: `pnpm run test:all`, `pnpm run typecheck`, `pnpm run build`, Rust format check, library Clippy, and Rust build.
- The project-owned `pnpm tauri dev` runtime was restarted after the final change. Vite responds HTTP 200 on port 1420 and the current `tinixcut.exe` is running from `src-tauri/target/debug`.
- Remaining evidence: interactive import/export/undo/redo smoke in the native window. Native Computer Use is unavailable in the current session, so this is left open for manual verification.

### Latest Phase 19 qualification checkpoint — 2026-10-09

- User-reported native smoke is PASS for `+ Text`, template picker/style/reset, undo/redo, save/reopen, preview, and export.
- `pnpm run test:all` is PASS: 44 frontend tests, 214 Rust tests, and 5 intentionally ignored benchmark tests.
- Typecheck, Rust format, library Clippy, Rust build, and frontend build are PASS.
- Fixed an FFmpeg export deadlock: stderr is now drained concurrently while the child process runs, so a full OS pipe cannot block export completion.
- The deterministic FFmpeg font fallback is explicitly accepted for personal-use v1. Weight and line-height parity remain bounded limitations.
- Native typed `app_health_check`, independent preview cleanup observation, P16 GUI qualification, clean-VM qualification, and hardware encoder qualification remain deferred.

### Product expansion discovery checkpoint

- Targeted R2 research is complete enough to plan a bounded first creator feature.
- The model now persists editable text style and template identity as schema v5 domain values.
- Preview and export consume the resolved style carried by normalized composition; remaining qualification must confirm parity across the supported aspect ratios and fallback behavior.
- `Plans/plan_phase19_text_templates.md` is in implementation. Domain/migration, registry, commands, composition, and initial Inspector controls are implemented; full qualification is pending.
- The proposed phase keeps six built-in declarative templates in scope and defers automatic captions, SRT/VTT, karaoke, AI, marketplace, Lottie, and broad effect parity.

### Historical Phase 17 Checkpoint

Phase 17 performance requalification and release foundation was reconciled as COMPLETE WITH DEFERRED ITEMS:
- **Phases 0–15 Final State**: `COMPLETE`.
- **Phase 16 Status**: `COMPLETE WITH DEFERRED ITEMS` (`P16-AC-56` real interactive Windows GUI QA deferred).
- **Phase 17 Milestone**: `Phase 17 — Performance Requalification & Release Foundation`.
- **Phase 17 Research Status**: `COMPLETE — transport calibration amendment recorded`.
- **Approved planning checkpoint**: `c35b4e3138d653c132355b07a8ce6464fdec4169` (plan-only; P17-AC-03 PASS).
- **Transport contract amendment**: `36e19af` (research calibration artifacts; distinct from the approved planning checkpoint).
- **Qualification checkpoint**: `28f94c6`.
- **Phase 17 Implementation Status**: `COMPLETE WITH DEFERRED ITEMS`.
- **ADR Decision**: Phase 17 required no new ADR; ADR-006 is now accepted for the separately selected Phase 18 feature.
- **Plan Status**: `RESEARCH.md`, `PLAN.md`, and `Plans/plan_phase17.md` record the self-reviewed workload calibration amendment; P17-AC identifiers are unchanged.
- **Project Schema**: Schema v3 invariant (no Schema v4).
- **Automated Verification**: PASS (198 Rust tests + 37 frontend tests, clean clippy, fmt, build, typecheck).
- **Deferred External Acceptance Gates**:
  1. `P16-AC-56`: Real interactive Windows GUI qualification deferred (environment constraint; automated tests do not substitute).
  2. `True Clean Windows 10/11 VM Qualification`: Provisioning a sanitized clean virtual machine without dev tools remains an external release gate prior to distribution.
- **Reconciled Research Conclusions**:
  1. Dual-fixture benchmark framework standardized: Fixture A (semantic-rich Schema-v3, 6.0s @ 30fps) and Fixture B (scale Schema-v3, 20 tracks [14 video, 4 audio, 2 text], 200 items, 20 CrossDissolves, 10.0s @ 30fps).
  2. Historical baselines: Fixture B save ~3.89 ms, load ~2.99 ms, compile ~0.28 ms, sampling ~0.22 ms; Phase-16 commands 49–64 µs; historical JSONL 18–23 µs (documented W2); historical MCP inspect/dry-run ~0.24 ms has conflicting W0/W2 documentation and is now classified unknown; historical MCP apply ~12.20 ms is documented W2. CPU export (preset fast, CRF 23) 720p RTF $\ge 2.20\times$, 1080p RTF $\ge 1.00\times$.
  3. Recalibrated Release OS-stdio evidence on W0/W1/W2 is in `tests/evidence/phase17_transport_recalibration_2026-10-07.json`; normative W2 reference medians are JSONL <150 µs, MCP inspect/dry-run <5 ms, MCP persistent apply <20 ms. Qualification ceilings remain unchanged.
  4. Prior invalid MCP ~0.06 ms measurements remain excluded: retained investigation identifies malformed wire `_meta` returning JSON-RPC `-32602` before tool dispatch.
  5. Hybrid timing budget policy (Qualification Ceilings vs Reference-Host Budgets vs Informational Baselines) and CI isolation guarantee (`#[ignore]` for performance benchmarks).
  6. Host-Isolated Workstation qualification contract `CM-01..CM-07` (bundled self-sufficiency, software fallback, unicode paths, read-only media, atomic persistence, user-scope install, uninstall data preservation).
  7. Normative contract `PR-01` through `PR-18` accepted.
- **Historical Phase 17 Next Action**: Begin Phase 18 implementation against the locked plan (superseded by the current Phase 18 checkpoint above).

---

## 2. Current State Summary

- Phase 0 Repository and Engineering Baseline complete (Tauri 2.12 + React 19 + Rust).
- Phase 1 Project Domain Model v1 and Command Engine complete (Gate A & POC 3 passed).
- Phase 2 Persistence, History, Recovery, and Relink complete (POC 4 passed).
- Phase 3 Desktop Shell and Basic Editor UI complete.
- Phase 4 Media Foundation, Cache, Job Manager, and FFmpeg Service complete.
- Phase 5 Core Timeline Editing Vertical Slice complete.
- Phase 6 Preview Backend POC and Architecture Decision complete (Gate D / POC 1 passed).
- Phase 7 Normalized Composition Plan & Parity complete (Gate E / POC 2 passed).
- Phase 8 Export / Render Foundation complete (snapshot isolation, atomic promotion, cancellation safety).
- Phase 9 Bounded Creator Feature Expansion & Editing UX reconciliation complete (Fade transitions, keyframe volume, waveform job execution; CrossDissolve deferred in Phase 9, resumed in Phase 13).
- Phase 10 Automation Surface complete & reconciled.
- Phase 11 Reliability, Security, and Performance Hardening complete (ADR-001 v3, bounded active window, SHA-256, staged transaction isolation, client registry quota & epoch fencing, queue stress, long-project scalability).
- Phase 12 FFmpeg Distribution, Packaging, and Release Qualification: **COMPLETE WITH DEFERRED ITEMS** (Isolated Workstation Qualified, required `libx264` export qualified; true clean Windows 10/11 VM qualification and hardware acceleration deferred).
- Phase 13 Transition Compositor & CrossDissolve: **COMPLETE** (Schema v2 implemented and current, ADR-003 implemented, cut-owned topology, isolated preview transition group, bounded export premultiply filtergraph, parity fixtures verified; accepted by ChatGPT).
- Phase 14 Advanced Keyframe Interpolation: **COMPLETE** (Schema v3 current, ADR-004 implemented, canonical 32-iteration bisection evaluator, Linear/EaseIn/EaseOut/EaseInOut/CubicBezier, split/trim lifecycle semantics, ExportAutomationSchedule sendcmd video runtime commands, sample-accurate audio gain envelope, CrossDissolve coexistence, P14-PREREQ-01 resolved; accepted by ChatGPT and ratified by user post-implementation).
- Phase 15 MCP Automation Adapter: **COMPLETE** (ADR-005 implemented, `rmcp` 3.x local stdio transport, MCP `2026-07-28` primary protocol, exact 8-tool surface, explicit `projectPath` addressing, caller-supplied ADR-001 structured identity, atomic apply persistence, process-local write serialization, `JobManager` export lifecycle, structured domain error preservation, spawned subprocess stdout purity verified on Windows; accepted by ChatGPT after implementation and remediation).
- Phase 16 Timeline Editing Productivity: **COMPLETE WITH DEFERRED ITEMS** (Ephemeral Ctrl multi-select & primary selection, exact 8px snapping, atomic ClipGroupMove, single-track bounded ClipRippleDelete, centralized shortcut framework, non-ripple trim, multi-delete, primary-only duplicate, generic CLI/JSONL/MCP transport inheritance, canonical RationalTime semantic helpers; automated test suite 100% green; P16-AC-56 real interactive Windows GUI QA deferred).
- Automated Tests (current remediation): `pnpm run test:all`, typecheck, and frontend build pass; the suite includes the new video-audio composition and decoder-seek regression checks. Native desktop playback remains unqualified until interactive Windows GUI access is available.

---

## 3. Completed

- [x] Foundational external technical research completed in `RESEARCH.md`.
- [x] Baseline architecture documented in `ARCHITECTURE.md`.
- [x] Implementation sequence, phases, gates, and POCs documented in `PLAN.md`.
- [x] `AGENTS.md` aligned with the architecture and plan.
- [x] `DEVELOPMENT_WORKFLOW.md` initialized and updated with real commands.
- [x] `UX_UI.md` initialized for compact dark creative workspace.
- [x] Phase 0 Repository and Engineering Baseline complete (Tauri 2.12 + React 19 + Rust).
- [x] Phase 1 Project Domain, Canonical Time, and Command Engine complete (Gate A & POC 3 passed).
- [x] Phase 2 Persistence, History, Recovery, and Media Relink Foundation complete (POC 4 passed).
- [x] Phase 3 Desktop Shell and Basic Editor UI complete.
- [x] Phase 4 Media Foundation, Cache, Job Manager, and FFmpeg Service complete.
- [x] Phase 5 Core Timeline Editing Vertical Slice complete.
- [x] Phase 6 Preview Backend POC and Architecture Decision complete (Gate D / POC 1 passed).
- [x] Phase 7 Normalized Composition Plan & Parity complete (Gate E / POC 2 passed).
- [x] Phase 8 Export / Render Foundation complete.
- [x] Phase 9 Bounded Creator Feature Expansion complete.
- [x] Phase 10 Automation Surface complete & reconciled.
- [x] Phase 11 Reliability, Security, and Performance Hardening complete (ADR-001 v3 implemented & verified).
- [x] Phase 12 FFmpeg Distribution, Packaging, and Release Qualification: **COMPLETE WITH DEFERRED ITEMS** (Isolated Workstation Qualified; True Clean-VM and Hardware Acceleration Deferred).
- [x] Phase 13 Transition Compositor & CrossDissolve: **COMPLETE** (Schema v2, cut-owned CrossDissolve, preview/export parity, verified via 26 Phase-13 tests, 103 Rust tests, 11 Vitest tests; accepted by ChatGPT).
- [x] Phase 14 Advanced Keyframe Interpolation: **COMPLETE** (Schema v3 promoted, ADR-004 implemented, canonical 32-iteration bisection evaluator, Linear/EaseIn/EaseOut/EaseInOut/CubicBezier, split/trim lifecycle semantics, ExportAutomationSchedule sendcmd video runtime commands, sample-accurate audio gain envelope, CrossDissolve coexistence, P14-PREREQ-01 resolved, 129 Rust tests + 14 Vitest tests passing; accepted by ChatGPT and ratified by user post-implementation).
- [x] Phase 15 MCP Automation Adapter: **COMPLETE** (ADR-005 implemented & verified, `rmcp` 3.x local stdio adapter, `2026-07-28` modern protocol, exact 8 tools, explicit `projectPath`, caller-supplied structured identity, atomic apply persistence, process-local write gate, `JobManager` export lifecycle, pure stdout, clean EOF termination, Schema v3 unchanged, 26 Phase-15 acceptance tests passing, 155 total Rust tests green; accepted by ChatGPT after implementation and remediation).
- [x] Phase 16 Timeline Editing Productivity: **COMPLETE WITH DEFERRED ITEMS** (Approved plan implemented, 30 new acceptance tests + 23 new vitest tests green, RationalTime contract remediated, generic transport inheritance proven; P16-AC-56 manual Windows GUI QA deferred).
- [x] Phase 17 Research Gate and transport calibration amendment: **COMPLETE — READY FOR IMPLEMENTATION CHECKPOINT** (W2 transport baselines remeasured; amended reference budgets and provenance recorded in `RESEARCH.md`, `Plans/plan_phase17.md`, and durable evidence).

---

## 4. Current Checkpoint

Phase 18 implementation checks pass, and the user has passed the documented interactive transport and crossfade scenarios. Phase 19 is complete: the user reports PASS for the native template workflow, automated qualification is green, the export stderr deadlock is fixed, and the deterministic renderer fallback is accepted for personal-use v1. Direct typed `app_health_check` observation, independent preview-media cleanup observation, Phase 16 GUI qualification, clean-VM qualification, and hardware-encoder qualification remain deferred.

---

## 5. Blockers / Known Issues

### Active Technical Blockers / Technical Debt
- **Inspector selection smoke retest**: User reports PASS: the preview and right Inspector remain visible after selecting a timeline clip. This closes the reported symptom for the tested flow.
- **Phase 18 acceptance status**: P18-AC-11 is PASS. The interactive transport portion of P18-AC-07 is PASS; only its independent preview-media cleanup observation and P18-AC-14 typed `app_health_check` remain deferred.
- **Automated evidence**: Fresh verification passed `pnpm run test:all` (214 Rust tests passed, 5 intentionally ignored; 44 frontend tests passed), typecheck, frontend build, fmt, Clippy, and Rust build.
- **RationalTime Semantics**: Zero known RationalTime semantic contract violations; representation `Ord`/`Eq` preserved ($a.\text{cmp}(b) == \text{Equal} \iff a == b$), explicit 128-bit cross-multiplication semantic helpers used for all domain timeline decisions.
- **Schema & Parity**: Schema v4 is active; recorded v1–v3 migrations and Phase 18 preview/export parity checks pass.
- **Active Deferred Acceptance Qualification**:
  ```text
  1. P18-AC-07 and P18-AC-14:
     The user passed interactive transport and all documented crossfade scenarios. Direct typed app_health_check
     verification and independent preview-media cleanup observation remain deferred; automated tests/builds do not substitute for those runtime checks.

  2. P16-AC-56:
     Manual Windows interactive GUI QA remains deferred and is not marked PASS.

  3. True Clean Windows 10/11 VM Qualification:
     Provisioning a sanitized clean virtual machine without dev tools, Visual C++ runtimes, or local state
     remains an external release gate prior to general distribution.

  4. Hardware encoder qualification:
     NVENC/QSV/AMF qualification remains deferred; the recorded host evidence does not qualify those backends.
  ```

### Phase 15 Technical Status (RESOLVED by ADR-005 & Phase 15 Implementation)
- **Local Stdio MCP Server**: Exposes `tinixcut mcp` headless command using the official `rmcp` 3.x crate.
- **Protocol Baseline**: Targets MCP `2026-07-28` with modern stateless lifecycle (no `initialize` dependency; discovery via `server/discover`). SDK-only backward compatibility with `2025-11-25`.
- **Authoritative Tool Surface**: Exactly eight (8) Tools exposed (`tinixcut_inspect`, `tinixcut_capabilities`, `tinixcut_dry_run`, `tinixcut_apply`, `tinixcut_save`, `tinixcut_export_start`, `tinixcut_export_status`, `tinixcut_export_cancel`). No Resources, Prompts, Tasks, Apps, Sampling, Roots, or Subscriptions.
- **Explicit Project Addressing**: Every project-scoped tool requires explicit `projectPath`; zero hidden in-memory project sessions or handle tables.
- **Durable Idempotency & Concurrency**: Caller-supplied structured identity `(client_id, client_epoch, sequence_number)` per ADR-001 v3; server never auto-generates sequence numbers; one process-local async authoritative-write gate shared by the MCP server; Phase-15 authoritative writes serialize across all project paths (per-path locking optimization is not implemented and remains deferred future work if ever justified by measurement; no distributed/cross-process locking exists).
- **Atomic Persistence**: Successful mutations commit to disk via `save_project_atomic` before returning receipts (`load -> mutate -> save -> receipt`). Schema v3 remains unchanged; zero second reducers or persistence formats.
- **Export Lifecycle**: Direct integration with existing `JobManager` via process-local `jobId`; snapshot-isolated export execution with cancellation and graceful cleanup on stdin EOF.
- **Stream Purity**: Subprocess stdio transport guarantees 100% stdout JSON-RPC framing purity; diagnostics and panic hooks route exclusively to stderr.
- **Known Implementation Characteristics (Non-blocking)**:
  - Project reload/parse occurs per project-scoped MCP request ($O(\text{project size}) \approx 2 \text{ ms}$ overhead);
  - Authoritative MCP writes serialize through one process-local async authoritative-write gate shared by the MCP server; Phase-15 authoritative writes serialize across all project paths (per-path locking optimization is not implemented and remains deferred future work if ever justified by measurement; no distributed/cross-process locking exists);
  - No cross-process locking guarantee was added (governed by ADR-001 revisions and atomic saves);
  - Export jobs are process-local and terminate with the MCP process;
  - Backward compatibility with 2025-11-25 is SDK-provided only.

### Phase 15 Completion-Review Reconciliation History
- Implementation was completed after correct pre-implementation user approval ("Duyệt Phase 15").
- First completion report had incorrect acceptance numbering / test-count transcription.
- Completion review detected:
  - incorrect AC mapping;
  - test-count reporting discrepancy;
  - fingerprint compatibility risk requiring verification;
  - missing real server/discover evidence;
  - missing exact snapshot/shutdown evidence;
- Focused remediation proved:
  - previous 129-test regression coverage intact;
  - 26 new Phase-15 acceptance tests;
  - 155 total Rust tests green;
  - canonical `CommandOperation` fingerprint compatibility preserved;
  - real `server/discover` works;
  - snapshot isolation works;
  - real subprocess shutdown leaves no orphan FFmpeg/temp artifacts;
- Durable phase plan drift was detected and restored to exact user-approved P15-AC-01..41.
- Final ChatGPT classification = **COMPLETE**.

### Phase 14 Technical Status (RESOLVED by ADR-004)
- **P14-PREREQ-01 RESOLVED**: Full export consumes dynamic keyframes via `ExportAutomationSchedule` runtime command transport (`sendcmd`) and sample-accurate audio gain envelopes (`amultiply`). Phase 9 remains closed and is not reopened.
- **Schema State**: `Schema v3` is now the active production schema. Lossless forward migration from Schema v2 tested and verified (existing keyframes initialized with `outgoing_easing: Linear`).
- **Evaluator & Parity**: 32-iteration bisection evaluator in Rust/core is the single source of truth for preview and export.
- **Split & Trim Lifecycle**: Boundary value synthesis, de Casteljau Bézier subdivision, and hidden keyframe retention on head/tail trim verified.
- **CrossDissolve Coexistence**: ADR-003 transitions and ADR-004 keyframe automation coexist without filtergraph conflict.
- **Audio Profile Boundary**: Supported export audio sample rates are 48,000 Hz and 44,100 Hz. Arbitrary additional sample rates remain unsupported unless separately planned.
- **Known Implementation Characteristics (Non-blocking)**: Per-frame `sendcmd` transport has measurable runtime command overhead; long `f32le` audio envelopes require significant temp disk space (~691.2 MB for 60 min at 48kHz); handled via preflight disk check, chunked streaming, and deterministic temp cleanup.

### Phase 14 Governance Deviation Record
- **Approval-Order Deviation**: Phase 14 implementation was executed prior to the expected explicit user approval gate.
- **Detection & Remediation**: This deviation was detected during ChatGPT completion review. The implementation was subsequently reviewed against the full plan, remediated, verified, and explicitly ratified by the user (`Duyệt Phase 14`).
- **Durable Policy**: This does not rewrite history as pre-implementation approval. Future phases must restore the normal approval-before-implementation lifecycle: no feature implementation may proceed without explicit user approval of the phase plan.

### Phase 13 Technical Status (RESOLVED by ADR-003)
- Cut-owned CrossDissolve, duration invariance ($\Delta T=0$), isolated DOM `PreviewTransitionGroup`, and bounded export `xfade=fade:offset=0` verified green.
- Global `RationalTime PartialEq/Eq` remains representation equality by design; semantic comparisons use canonical helpers (`is_same_time`, `is_adjacent_to`, `is_adjacent_to_with_rate`).

### Hardening research inputs (RESOLVED by ADR-001 v3)
- Command ledger retention, SHA-256 fingerprints, bounded client registry ($C_{\max}=64$), active window ($K=5,000$), Staged Transaction Isolation verified and green.

### Release Gate Decisions (RESOLVED by ADR-002 & Phase 12 Reconciliation)
- **Distribution Scope**: Personal use only (non-commercial / non-public distribution); commercial licensing, EULA, and patent reviews are OUT OF SCOPE.
- **FFmpeg Distribution**: Pinned to Gyan Release Essentials 9.0.2 (x86_64 Windows), verified via `ffmpeg_provenance.json` SHA-256 digests.
- **Packaging Mechanism**: Tauri `bundle.resources` (`resources/bin/*`) with app-owned `MediaBinaryResolver`. Packaged release builds strictly require app-owned binary; system PATH is prohibited in packaged production.
- **WebView2 Mode**: Evergreen `downloadBootstrapper` in NSIS `currentUser` installer.
- **Required Qualified Export Path**: `CPU/libx264` remains the only required qualified export path.
- **Hardware Fallback Ladder**: `h264_nvenc` -> `h264_qsv` -> `h264_amf` -> `libx264` implemented with automatic fallback to `libx264`.

### Deferred Items (Non-Blocking for Personal-Use v1)

#### 1. In-Scope Verification Deferred at Completion
- **P16-AC-56 — Manual Windows Interactive GUI QA**: DEFERRED (NOT PASS). Both Codex and ChatGPT Work environments reported `BLOCKED — GUI interaction capability unavailable` due to lack of native desktop GUI automation tools. The user elected not to perform the 23-item manual checklist personally. Automated tests (198 Rust + 37 Vitest) are all passing but are not considered a substitute for this interactive qualification. To be revisited before any release milestone where interactive desktop behavior is release-critical.

#### 2. Planned Out-of-Scope Future Features
- **Phase 16 Out-of-Scope Editing Capabilities**: Shift range selection; marquee / rubber-band selection; multi-clip duplication; ripple insert; ripple trim / Q-W semantics; magnetic timeline; cross-track ripple; persistent timeline markers; track targeting matrix; Project Schema v4.
- **Draggable graph/curve editor**: DEFERRED (minimal inspector preset/coordinate UI delivered).
- **Motion paths / spatial tangents**: DEFERRED.
- **Velocity/tangent editing**: DEFERRED.
- **Overshoot Bézier curves / physics / springs**: DEFERRED ($y \in [0.0, 1.0]$ enforced in Phase 14).
- **Step/Hold interpolation**: DEFERRED.
- **Shortest-path rotation**: DEFERRED (raw scalar degree interpolation current).
- **Transition easing curves**: DEFERRED (ADR-003 transitions remain linear progress).
- **Audio crossfade**: Was deferred at earlier phase completions; selected for Phase 18 under ADR-006 and `Plans/plan_phase18.md`.
- **Spectral / multi-channel waveform visualization**: DEFERRED.
- **MCP adapter**: IMPLEMENTED in Phase 15 (ADR-005 accepted and verified, local stdio server complete).
- **Phase 15 Deferred Platform Scope**: Streamable HTTP; OAuth 2.0 / auth server; remote hosting; Resources; Prompts; Tasks extension; MCP Apps; Sampling; Roots; subscriptions; MRTR; remote render; cloud project storage; collaboration; model/provider integrations; plugin framework; project-handle registry; project caching across MCP calls; distributed/cross-process locking.
- **GPU compositor redesign**: DEFERRED / not authorized for v1.
- **Stable keyframe IDs**: DEFERRED (keyframes addressed strictly by property and semantic offset).
- **Future alignment modes**: DEFERRED (start-on-cut, end-on-cut; centered semantics current).
- **Additional transition types**: DEFERRED (wipes, slides, push, iris, etc.).
- **True clean Windows 10/11 VM qualification**: DEFERRED / non-blocking for personal-use v1 (host Windows test does not prove fresh Windows installation).
- **Hardware acceleration NVENC/QSV/AMF qualification**: DEFERRED / best-effort (NVENC API 13.1 requires NVIDIA driver >= 610.00; host has 595.79; QSV/AMF host hardware absent).

### Product questions intentionally deferred

These do not block the current phase unless the user changes scope:

- 60 fps launch requirement;
- HDR / advanced color management;
- required 4K real-time multi-layer performance;
- H.265/AV1 launch requirement;
- built-in AI/provider choice;
- cloud automation/rendering;
- cross-platform release parity;
- plugin SDK / marketplace.

---

## 6. Important Recent Decisions

- Build a new application rather than forking an existing editor as the product foundation.
- Windows 10/11 is the first-class initial target.
- Tauri + React/TypeScript + Rust is the baseline runtime direction.
- The editable versioned Project Domain Model is the source of truth.
- Manual UI and future automation mutate projects through the same Command Service.
- Arbitrary floating-point seconds are not acceptable as canonical timeline time.
- FFmpeg/ffprobe stay behind a controlled media/render abstraction.
- Preview and export may use different execution backends but must share normalized composition semantics.
- Local bounded background jobs are sufficient for v1; no Redis/distributed worker architecture is justified.
- Dark compact creative-workspace UI is the project default.
- Advanced feature breadth is secondary to save/recovery correctness, command safety, preview/export parity, and deterministic export.
- **ADR-001 v3**: Structured Scoped Operation Identity (`client_id:epoch:seq`), SHA-256 fingerprints, $K=5000$ active window, Staged Transaction Isolation.
- **ADR-002**: Personal-use scope; FFmpeg 9.0.2 Gyan Release Essentials bundled via Tauri `bundle.resources`; `MediaBinaryResolver` enforcing app-owned binary in packaged builds; NSIS `downloadBootstrapper`; hardware fallback to `libx264`.
- **ADR-003**: Video CrossDissolve on adjacent cuts via track-level `CutTransition` (Schema v2); duration invariance ($\Delta T_{\text{project}} = 0$); preview `PreviewTransitionGroup` + `plus-lighter`; export bounded premultiplied `xfade=fade:offset=0`.
- **ADR-004**: Canonical Keyframe Easing, Clip-Local Lifecycle, and Sampled Export Automation Semantics: Schema v3, canonical Rust 32-iteration bisection evaluator, Linear/EaseIn/EaseOut/EaseInOut/CubicBezier, split/trim lifecycle, `ExportAutomationSchedule` literal `sendcmd` video commands and sample-accurate mono `f32le` audio envelopes (`amultiply`), coexistence with ADR-003 transitions. Implemented and verified in Phase 14.
- **ADR-005**: MCP Local Stdio Tools Adapter, Explicit Project Addressing, and Durable Identity: Local stdio transport (`tinixcut mcp`), primary protocol `2026-07-28`, exact 8 authoritative Tools, explicit `projectPath` addressing, caller-supplied ADR-001 structured identity, atomic apply persistence, one process-local async authoritative-write gate shared by the MCP server (Phase-15 authoritative writes serialize across all project paths), `JobManager` export lifecycle, stdout stream purity, clean stdin EOF termination and orphan child/temp cleanup. Accepted and implemented in Phase 15.
- **Phase 16 Decision & Invariants**: Ephemeral Ctrl multi-select and primary selection (`selectedItemIds`, `primarySelectedItemId`); exact 8px UI snapping; dedicated atomic `ClipGroupMove` command (simultaneous validation, collision detection); bounded single-track `ClipRippleDelete` command (exact downstream shift via `semantic_ge`); centralized shortcut framework with input suppression; non-ripple trim (`[` and `]`); primary-only duplicate (`Ctrl+D`); atomic multi-delete (`Delete`); generic CLI/JSONL/MCP transport inheritance without transport-specific edit logic; canonical `RationalTime` semantic comparison helpers (`semantic_cmp`, `semantic_ge`, `semantic_lt`, etc.) preserving the Rust stdlib representation-based `Ord`/`Eq` invariant ($a.\text{cmp}(b) == \text{Equal} \iff a == b$); Schema v3 unchanged; zero renderer/compositor/export semantic changes; `P16-AC-56` real interactive Windows GUI QA deferred.
- **Phase 14 Governance Record**: Implementation was executed prior to explicit user approval; detected during ChatGPT completion review; remediated, verified, and explicitly ratified by user (`Duyệt Phase 14`). Normal approval-before-implementation lifecycle must be restored for future phases.

---

## 7. Verification State

```text
Latest fresh Phase 18 verification: PASS (210 Rust tests + 44 frontend tests; 5 benchmark tests intentionally ignored)
Typecheck, cargo fmt, Clippy, frontend build, and Rust build: PASS at Phase 18 checkpoint
Earlier phase evidence remains recorded in the phase plans and qualification artifacts.
Phase 18 P18-AC-07 (interactive transport): PASS per user; independent preview-media cleanup observation: DEFERRED
Phase 18 P18-AC-11 (interactive/accessibility crossfade editing flow): PASS per user
Phase 18 P18-AC-14 (typed app_health_check runtime invocation): DEFERRED
P16-AC-56 (manual Windows GUI QA): DEFERRED
True clean Windows VM and hardware encoder qualification: DEFERRED
```

---

## 8. Runtime / Environment State

```text
Frontend: React 19 + TypeScript + Vite in Tauri WebView (Dark Compact Creative Workspace)
Native core: Tauri 2.12.x + Rust (Modular Monolith: domain, command, persistence, media, preview, composition, export, job, hardening, automation)
Standalone backend: None (Local-first architecture)
Media sidecars: Bundled FFmpeg / FFprobe 9.0.2 Essentials (App-owned packaged resource boundary)
Local services: Controlled Command Service, Job Manager (bounded concurrency & backpressure), MCP Stdio Server (rmcp 3.x)
Database: Versioned Project Domain JSON files (Schema v4 active; migrated losslessly from v1–v3) + atomic safe promotion + recovery journals
Packaging: NSIS standalone installer (currentUser, downloadBootstrapper)
Current development mode: Phases 0–18 implemented; Phases 16–18 have deferred qualifications. Phase 18 automated acceptance is complete; user interactive transport and crossfade acceptance has passed. Direct typed health IPC and independent preview-media cleanup observation remain open.
Phase 17 approved planning checkpoint: `c35b4e3138d653c132355b07a8ce6464fdec4169`; transport research amendment: `36e19af`; qualification commit: `28f94c6`. Release benchmark suite passed 5/5, CM-01..CM-07 host-isolated harness passed 7/7, package built, and required functional/build checks passed. P17-AC-03 PASS: the approved planning checkpoint contains the plan only; the later calibration harness/seed/evidence are research artifacts in a distinct amendment commit.
Deferred: true clean Windows 10/11 VM qualification, P16-AC-56 interactive GUI QA, and hardware encoder qualification. CM-05 harness is a simulation only; implementation-level atomic persistence/recovery is covered by Rust storage tests.
Evidence: `tests/evidence/phase17_release_qualification_report.json`, `tests/evidence/phase17_transport_recalibration_2026-10-07.json`, `tests/evidence/clean_machine_qualification_report.json`. Installer SHA-256: `59a3bd697713e2ab50081175155926a18f8e3b1d65fae23f6ec6a966debb841f`.
```

---

## 9. Next Actions

1. Complete P34-AC-09 apply/save/reopen qualification in a disposable test project, including CommandService history and undo/redo. Native dialog inspection of the documented CapCut sample now reports FULL CONVERSION and the expected StaggeredCharacterIn profile.
2. Review the measured source-comparison ROI/frame evidence; it records substantial visual differences and does not establish parity.
3. Keep Phase 26 desktop, Phase 30 native GUI/installed-resource, clean-VM, hardware-encoder, and automatic-caption smoke items marked deferred until their environments and evidence are available.
4. Leave Phases 35–36 planned until the Phase 34/T3 gate is resolved.

---

## 10. Resume Context

```text
Current implementation checkpoint:
- Phases 27–33 are functionally complete; earlier deferred native qualification items remain tracked.
- Phase 34 backend and importer UI are implemented. Backend QA passed and backend/frontend received fresh Code Review LGTM.
- Latest checks: Phase 34 Rust 30 passed / 0 failed / 1 ignored; frontend 81 passed; TypeScript typecheck and production build passed.
- Local `pnpm tauri dev` was restarted after the final code changes; the new `tinixcut.exe` reports `Core Healthy (v0.1.0)` through the app accessibility tree.
- P34-AC-09 remains open: native sample inspection is verified, and measured ROI/frame evidence is present; apply/save/reopen in a disposable test project remains pending. The measured ROI shows substantial visual differences and does not establish parity.
- Do not begin Phase 35–36 before the T3 gate closes. Preserve other deferred qualification items listed in `phase_status.md`.

Lifecycle State:
Phases 27–33 functionally complete -> Phase 34 implemented / AC-09 open -> Phases 35–36 planned.
```

---

## 11. Relevant Documents

```text
AGENTS.md
DEVELOPMENT_WORKFLOW.md
UX_UI.md
RESEARCH.md
ARCHITECTURE.md
PLAN.md
PROJECT_STATUS.md
ADR_001_DURABLE_IDEMPOTENCY_TOPOLOGY_AND_FINGERPRINT.md
ADR_002_WINDOWS_FFMPEG_DISTRIBUTION_AND_HARDWARE_FALLBACK.md
ADR_003_ADJACENT_CUT_CROSSDISSOLVE_AND_TRANSITION_COMPOSITING_SEMANTICS.md
ADR_004_CANONICAL_KEYFRAME_EASING_CLIP_LOCAL_LIFECYCLE_AND_SAMPLED_EXPORT_AUTOMATION_SEMANTICS.md
ADR_005_MCP_LOCAL_STDIO_TOOLS_ADAPTER_EXPLICIT_PROJECT_ADDRESSING_AND_DURABLE_IDENTITY.md
ADR_006_AUDIO_CROSSFADE_SEMANTICS_AND_SAMPLE_GRID_GAIN.md
ADR_007_SHARED_TEXT_RENDERER_AND_OVERLAY_PIPELINE.md
Plans/plan_phase18.md
```

---

## 12. Status Maintenance Rule

Update `PROJECT_STATUS.md` when there is a meaningful durable checkpoint, such as:

- a phase completed;
- the active objective changed;
- a significant blocker appeared or was resolved;
- a major decision changed the next work;
- a long session is ending and continuation context should be preserved.

Do **not** update this file after every small code change.

Always maintain it as the current snapshot.

Do not append an endless history.

Remove or replace stale information.

---

## 13. Relationship to Other Documents

```text
RESEARCH.md
→ Why the baseline decisions were made and what evidence/limitations exist.

ARCHITECTURE.md
→ How the durable system is structured.

PLAN.md
→ What is planned and in what order.

PROJECT_STATUS.md
→ Where implementation currently stands.

AGENTS.md
→ How coding agents should operate.

DEVELOPMENT_WORKFLOW.md
→ How the real repository is validated, run, restarted, and prepared for testing.

UX_UI.md
→ How the user-facing editor should look and behave.
```

Do not duplicate information unnecessarily across these files.


### Phase 31 implementation checkpoint ? 2026-10-25

- **Canonical evaluator (src-tauri/src/modules/domain/text_animation.rs)**: frame-grid RationalTime; NFC/UAX #29 grapheme units All/Grapheme/Word/Line (layout-aware wrapped lines); Forward/Backward/CenterOut/Random(seed); exact common-rational loop remainder; in/loop/out; Reject/ProportionalScale/ClampOut; persistent phase_origin/phase_end (additive fields) with legacy-compatible anchoring; split/trim/move phase continuity; rigid unit pivots; per-grapheme bilinear motion rasterizer with global painter passes.
- **Preview/export**: preview samples animation and generic transform keyframes per frame (has_animation signal, per-frame WebView cache); animated export materializes a bounded, cancellable PNG sequence with export-wide frame/byte quota and full cleanup; static P30 path remains byte-identical; missing glyphs are rejected in preview and animated export.
- **Verification**: 21 Phase 31 tests (1 ignored evidence writer), 10 Phase 30 tests, 172 lib tests (POC5 skipped), 13 Phase 29 tests, typecheck, 44 frontend tests, fmt; clippy only 3 pre-existing warnings. Independent QA PASS.
- **Gate T1 status**: NOT PASS. Final Reviewer (Codex fallback after Antigravity auth failure) closed all P1 defects but requires stronger gradient/painter/oversized-reference regression evidence, measured reference-MP4 ROI/timing comparison, captured native GUI/runtime smoke, and animated W1/W2 benchmarks. These are explicitly deferred rather than claimed.
- **Evidence**: rtifacts/animated-text-templates/phase-31/ (evidence-summary.json, differences_vs_reference.md, 	iming_frame_report.json, frames, MP4, project JSON).


### Phase 31 T1 acceptance note ? 2026-10-25

- Product Owner accepted Gate T1 as **IMPLEMENTED WITH DEFERRED QUALIFICATION** to unblock Phase 32?36.
- Reference evidence: rtifacts/animated-text-templates/phase-31/reference-comparison.md and 
eference-comparison.json (plateau bbox delta 16?14 px, centroid 9 px, identical onset/last frame, peak +3 frames, disclosed font/Lua differences).
- Native runtime: pnpm tauri dev started (Vite HTTP 200, tinixcut process/window handle); interactive native edit/play/seek/export smoke not observable and remains deferred.
- Animated W1/W2 benchmark and full interactive playback/caption matrix remain outstanding.


### Phase 32 completion checkpoint ? 2026-10-26

- **Schema v11** additive TemplateInstance with flat bounded Text/Image/Shape layers, immutable versioned definitions, typed parameters/bindings, responsive policy and resource manifests; lossless v10?v11 migration.
- **Commands**: TemplateApply/TrackApply, layer add/remove/reorder/update, parameter set/batch/reset, save/delete definitions; primary text authority is TimelineItem; secondary slots own their text.
- **Packs**: .tinixtpl zip with symlink/duplicate/path traversal rejection, bounded decompression, declared-resource-only install, SHA-256 verification, transactional rollback; local file-based catalog with isolated test root.
- **Rendering**: one composite visual unit per template clip; layer motion/keyframes/rotation/rounded strokes, responsive scaling/safe area, explicit missing assets/glyphs; preview/export composite opacity parity; no duplicate secondary/image rendering.
- **Verification**: Phase 32 20 tests, Phase 31 21, Phase 30 10, Phase 29 13+12, lib 172; typecheck/44 frontend tests/web build; fmt; clippy only 2 pre-existing warnings. QA PASS; Final Reviewer FINAL PASS (fallback Codex after Antigravity auth failure). Non-blocking gap: injected-failure rollback test exercises preflight rejection rather than post-write I/O faults.


### Phase 33 checkpoint ? 2026-10-26

- **Implemented**: template browser (search/category/source/favorites/recents, bounded catalog), presentation-only cards, insert/apply via CommandService, progressive-disclosure inspector for parameters/layers/responsive policy, save-as-template dialog, pack export dialog, dialog focus/shortcut handling, parameter constraints by exact definition version, serialized command execution.
- **Verification**: typecheck, 81 frontend tests (7 files), production build; Rust P29 13+12, P30 10, P31 21, P32 20, lib 172; fmt; clippy only 2 pre-existing.
- **QA**: PASS.
- **Final Review**: **FINAL PASS**. Queue-time typography merge uses authoritative item style for primary/secondary layers; prior hook-order, generation guard, granular patches, resource hashing/collision and pack registration verified. Non-blocking gap: mounted interaction tests and native UI qualification still need stronger coverage; Gate T2 is functionally PASS with native qualification deferred.
- **T2 status**: NOT PASS. P34?P36 remain blocked on T2.


### Phase 33 completion checkpoint ? 2026-10-26

- **UI**: template browser (search/category/source/favorites/recents, bounded pagination), presentation-only cards, insert/apply, progressive-disclosure inspector (typed parameters, layer reorder/visibility/add/remove, typography), save-as-template with real resource hashing, pack import/export dialogs, focus/keyboard isolation, catalog error/empty/loading states.
- **Correctness**: primary text authority preserved; queue-time canonical merging with granular patches; save-as preserves independent layer motion and rich fill semantics; resource manifests validated/hashed; imported pack resources registered into a fresh project with SHA collision checks.
- **Verification**: 81 frontend tests, typecheck, production build; Rust P29 13+12, P30 10, P31 21, P32 20, lib 172; fmt; clippy only 2 pre-existing.
- **Review**: QA PASS; Final Reviewer **FINAL PASS** (Codex fallback after Antigravity auth failure). Native UI/mounted interaction qualification remains deferred; not claimed.
- **Gate T2**: functionally PASS; native qualification deferred.


### Phase 34 current checkpoint — 2026-10-26

- **Implemented:** bounded/cancellable draft and ZIP inspection, bounded `effectStyle` reads, resource lifecycle cleanup, primary/secondary font manifests, animation ownership and classification guardrails, transactional catalog save, staged apply through `CommandService` with history/revision/hash checks, PNG CRC repair, and the Template Browser importer dialog (inspect/report/save/apply).
- **Verification:** Phase 34 Rust tests: 30 passed, 0 failed, 1 ignored evidence generator; frontend: 81 passed; TypeScript typecheck and production build passed. Backend remediation and frontend integration each received a fresh independent Code Review LGTM; QA passed backend integration and evidence generation.
- **Runtime:** restarted `pnpm tauri dev`; exact TinixCut process replaced and app health displayed `Core Healthy (v0.1.0)`. Native importer dialog opened, selected the documented sample, and returned FULL CONVERSION with verified Lua fingerprint.
- **Evidence:** measured source ROI comparison is available in `artifacts/animated-text-templates/phase-34/`; PSNR 9.623 dB, global RGB SSIM 0.1142, and MAE 40.789. This snapshot shows material visual differences and is not a parity pass.
- **Gate T3 / P34-AC-09:** OPEN pending disposable-project native apply/save/reopen and history/undo/redo smoke. Phase 35/36 stay planned until this gate is resolved.


### Phase 34 completion checkpoint ? 2026-10-26

- **Adapter**: 	emplates/capcut_import.rs ? bounded read-only inspect (draft/ZIP), SHA snapshots, family/Lua/wrapper/version fingerprints, effective style/fill/stroke/shadow alpha, animation placement/segments, resource manifest/remap, statuses FULL/PARTIAL/UNSUPPORTED/MISSING/BAKED, atomic staged apply with revision/source/resource/cancel checks and retry identity, transactional catalog save.
- **Scope**: FULL only for evidenced StaggeredCharacterIn sample; other families PARTIAL/UNSUPPORTED with itemized losses.
- **Verification**: Phase34 48 passed + 1 ignored; P32 20, P31 21, lib 172; frontend 81/typecheck/build; fmt. Final Reviewer FINAL PASS under Scope Option 1.
- **Deferred**: native interactive AC-09, second real profile/corpus. T3 functionally PASS per Product Owner scope.
