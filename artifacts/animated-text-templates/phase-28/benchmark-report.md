# Phase 28 POC Benchmark Report — Real Typography & Rasterizer Engine

**Document Version:** 4.0.0  
**Phase:** Phase 28 (Shared Text Renderer POC & Architecture Gate)  
**Date:** 2026-10-25  
**Host Environment:** Windows 11 Pro 24H2, Intel Core i5-13400F (16 logical cores @ 2.50 GHz), 64 GB DDR4 RAM, NVIDIA GeForce RTX 3090 (24 GB VRAM)  
**Engine Implementation:** Pure Rust OpenType typography engine (`ab_glyph 0.2.32`, `unicode-normalization 0.1.25`, `unicode-segmentation 1.13.3`, `text_renderer_poc.rs`)  

---

## 1. Workload Definitions & Target Budgets (Phase 27 Contract)

- **W1 (Single Animated Rich Title):** 1920×1080 @ 30 fps, 120 grapheme clusters (evaluated at `progress = 1.0` with 100% of non-space characters visible), static-layout-anchored 118° linear gradient fill (`#F08519` $\rightarrow$ `#FFDC74` $\rightarrow$ `#FA6800`), 2 outer strokes (`4.0 px` `#AD5100` + `7.0 px` `#4A2000`, sorted widest-to-narrowest), 2 separable-blurred drop shadows (`#AD5100` + `#FFA758`), character-level in-stagger + Z-rotation.
  - *Target Budget:* p95 $\le 16.0\text{ ms}$ (to sustain 60 fps preview capability).
- **W2 (Multi-Title Concurrency):** 4 concurrent non-overlapping animated text titles (`y = 220, 430, 640, 850`) + rounded semi-transparent background boxes active simultaneously on separate tracks.
  - *Target Budget:* p95 $\le 33.3\text{ ms}$ (to sustain 30 fps interactive editing).
- **W3 (Maximum Layer Density):** 16-layer density stress (8 decorative rounded shape layers + 8 text slots × 32 graphemes each = 16 layers) evaluated at both `1920×1080` (16:9) and `1080×1920` (9:16).
  - *Target Budget:* p95 $\le 50.0\text{ ms}$.
- **W4 (Continuous Export Throughput):** Headless export streaming 60 frames (`2.0 s @ 30 fps`) of Workload W1 to FFmpeg via `rawvideo` (`pix_fmt=rgba`) pipe with background `stderr` draining.
  - *Target Budget:* Throughput $\ge 1.0\times$ real-time (target $\ge 1.5\times$ RTF).
- **Preview IPC Delivery & Interactive Seeks:**
  - *Target Budget:* End-to-end Preview IPC Delivery p95 $\le 16.0\text{ ms}$, Seek Latency p95 $\le 16.0\text{ ms}$, Edit Latency p95 $\le 25.0\text{ ms}$.

---

## 2. Measured Benchmark Results on Windows 11 Reference Host

Machine-readable CSV artifact: `artifacts/animated-text-templates/phase-28/benchmark-metrics.csv`

| Workload / Measurement | Resolution | Elements / Complexity | Cold Cache | Warm p50 | Warm p95 | Phase 27 Locked Budget | Status |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **W1 (Full Title Overlay)** | 1920×1080 | 120 graphemes (100% visible), 118° gradient, 2 strokes, 2 shadows | 11.31 ms | 5.60 ms | **7.10 ms** | $\le 16.0\text{ ms}$ | **PASS** |
| **W2 (4 Titles Concurrency)** | 1920×1080 | 4 non-overlapping concurrent titles + rounded bg boxes | 13.64 ms | 11.07 ms | **12.21 ms** | $\le 33.3\text{ ms}$ | **PASS** |
| **W3 (16:9 16 Layers)** | 1920×1080 | 16 layers (8 rounded shapes + 8×32-grapheme text slots) | 20.72 ms | 18.51 ms | **20.32 ms** | $\le 50.0\text{ ms}$ | **PASS** |
| **W3 (9:16 16 Layers)** | 1080×1920 | 16 layers (8 rounded shapes + 8×32-grapheme text slots, vertical) | 26.86 ms | 17.74 ms | **19.78 ms** | $\le 50.0\text{ ms}$ | **PASS** |
| **W1 Preview IPC Delivery** | 1920×1080 | Render + tight dirty ROI crop + PNG + JSON serialization | — | 12.81 ms | **13.83 ms** | $\le 16.0\text{ ms}$ | **PASS** |
| **Seek Latency** | 1920×1080 | Random non-monotonic timeline jumps across progress range | — | 2.55 ms | **8.25 ms** | $\le 16.0\text{ ms}$ | **PASS** |
| **Edit Latency** | 1920×1080 | Text mutation invalidation & re-render at $p=1.0$ | — | 2.89 ms | **2.96 ms** | $\le 25.0\text{ ms}$ | **PASS** |
| **W4 (FFmpeg Stream Export)** | 1920×1080 | Continuous FFmpeg `rawvideo` export stream (60 frames / 2.0 s) | — | — | **5.15× RTF** (`0.388 s`) | $\ge 1.0\times$ RTF | **PASS** |

- **Bounded Glyph & Effect Cache (`BoundedGlyphCache`):** `402,683 bytes` (`0.38 MiB` / `256.00 MiB` ceiling, `77` unique cluster mask entries, `4,437` cache hits, `77` cold misses).
- **Binary Footprint Impact (Measured vs Phase 27 Baseline):**
  - Phase 27 Baseline: Debug `24,498,688 bytes`, Release `7,381,504 bytes`.
  - Phase 28 Measured: Debug `25,775,616 bytes` ($+1.2\text{ MB}$), Release `8,561,664 bytes` ($+1.18\text{ MB}$). Pure Rust stack adds zero C++/LLVM overhead and well satisfies the packaging budget.

---

## 3. Visual & Functional Acceptance

1. **True Font, NFC Normalization & Unified Grapheme Cluster Shaping (`P28-AC-01`):**
   - Verified with Apache-2.0 open-source font `Roboto-Bold.ttf` (`src-tauri/fixtures/fonts/Roboto-Bold.ttf`, SHA-256 `61F89F8DB49261C2F6106E8DCCC35DF7B2F7ED909020DB40A3FC905E95F99334`, `0` missing `.notdef` glyphs).
   - Input strings are normalized to Unicode NFC (`unicode-normalization`) before UAX #29 segmentation (`unicode-segmentation`), and multi-codepoint grapheme clusters are rasterized onto a single unified cluster mask before 2D rotation (`rasterize_relative_cluster_mask`), ensuring Vietnamese diacritics (`ử`, `ệ`, `ế`) never split during cascade animation or Z-rotation.
   - Sample text `"Tinix — Thử nghiệm tiếng Việt"` yields exact `29` graphemes (`24` non-space characters: Group A = `1..21` vertical slide, Group B = `22..24` `'i'`, `'ệ'`, `'t'` bounce + Z-rotation).
2. **Deterministic Lossless Overlay Parity (`P28-AC-02`):**
   - Bit-for-bit identical RGBA buffers across repeated evaluations ($\Delta \text{PSNR} = \infty$, 0 mismatch bytes across 8,294,400 bytes).
3. **Consumer 1 — Tauri Preview IPC Seam & SSIM/MAE Parity (`P28-AC-03`):**
   - Registered Tauri command `render_text_template_poc_preview` (`src-tauri/src/lib.rs`) and TypeScript client `renderTextTemplatePocPreview` (`src/services/editorApi.ts`). Reuses static `OnceLock<SharedTextRendererPoc>` to preserve warm cache hits across frames.
   - Delivers `PreviewOverlayIpcResponse` with tight dirty ROI `[667, 471, 1087, 570]` (`420×99 px`, `166,497 bytes` tight ROI PNG vs `8,296,178 bytes` full-frame PNG).
   - Decoded Preview ROI PNG vs. Lossless Export RGBA buffer: **$\text{ROI SSIM} = 1.000000$** ($\ge 0.995$), **$\text{ROI MAE} = 0.000000$** ($\le 0.005$).
4. **Alpha Channel & Halo Elimination Across 4 Backgrounds & Non-Clipping Bounds (`P28-AC-04`):**
   - Straight RGBA storage with Porter-Duff Over blending verified mathematically and visually across 4 backgrounds: pure white (`poc_alpha_white.png`), pure black (`poc_alpha_black.png`), bright green (`poc_alpha_color.png`), and decoded video frame (`poc_alpha_video.png`).
   - Boundary perimeter test verifies 100% zero alpha around the outer edges of `dirty_roi` (`poc_alpha_*`), proving no shadow or stroke is clipped. Geometry bounds preserved across `0.59x`, `1.0x`, `1.208x` scaling and `-21°`, `+6°` rotation.
5. **Consumer 2 — Headless FFmpeg Export Streaming, MP4 Parity & Deadlock-Free Cancellation (`P28-AC-06`):**
   - Background thread drains FFmpeg `stderr` concurrently, completely eliminating OS pipe deadlock risk on long exports.
   - Verified 60-frame video export `artifacts/animated-text-templates/phase-28/poc_export_sample.mp4` (`5.15× RTF`).
   - Extracted MP4 frame vs. lossless composite: **$\text{Encoded MP4 ROI SSIM} = 0.9977$** ($\ge 0.985$).
   - Cancellation token aborts child FFmpeg process immediately and cleans up partial disk files.
