# ADR-007: Shared Text Renderer & Deterministic Overlay Seam

**Status:** APPROVED & LOCKED  
**Date:** 2026-10-25  
**Deciders:** Core Engineering Team  
**Consulted:** `ARCHITECTURE.md`, `RESEARCH.md`, `Plans/plan_phase28_text_renderer_poc.md`, `artifacts/animated-text-templates/phase-27/`  
**Supercedes:** Initial reconnaissance evaluating `skia-safe` / C++ Skia bindings.

---

## 1. Context and Problem Statement

TinixCut's Animated Text Template system (Phases 27–36) requires rich typographic styling (multi-stop 118° linear gradients, multi-layer outer strokes, multi-shadow with separable blur) and character-level cascaded motion (staggered in-animation, 21-char slide, 3-char bounce with Z-axis rotation) for both interactive preview and headless video export.

Prior to Phase 28, Preview evaluated text via the WebView DOM / HTML5 Canvas, while Export relied on FFmpeg's `drawtext` filter with Arial font fallback. This caused unavoidable cross-backend visual drift, baseline mismatch, and font metric disparities. To eliminate this drift, a shared text overlay engine was required.

The key architectural question: What rendering and shaping stack should be selected to guarantee bit-for-bit preview/export parity on Windows without inflating the application installer or introducing fragile native C++ toolchain dependencies?

---

## 2. Evaluated Options

### Option A: `skia-safe` (Rust bindings to Google Skia C++ library)
- **Strengths:** Mature 2D vector graphics library, rich feature set, hardware acceleration support.
- **Critical Drawbacks:**
  1. *Toolchain Fragility:* Compiling `skia-bindings` on Windows requires Clang, LLVM, Python, depot_tools, and Ninja. This frequently fails on clean Windows machines and CI environments without full Visual Studio C++ desktop SDKs.
  2. *Binary Bloat:* Increases the uncompressed binary weight by +42 MB to +65 MB and the NSIS installer by ~25–35 MB, breaching the packaging budget.
  3. *Cross-Platform Drift:* Different Skia dynamic library builds and GPU/CPU backend paths introduce micro-discrepancies in subpixel antialiasing.

### Option B: Pure Rust Native Typography & Rasterization Stack (`ab_glyph` + `unicode-segmentation` + Native Compositor)
- **Strengths:**
  1. *Zero External Toolchain Burden:* 100% pure Rust compiled via standard `cargo build`. Zero C++, Clang, Python, or LLVM prerequisites.
  2. *Lightweight Footprint:* Total binary overhead is < 850 KB, having zero measurable impact on installer packaging size.
  3. *True Font Parsing & Glyph Outline Extraction:* Employs OpenType font parsing (`ttf-parser` / `ab_glyph`) to extract exact glyph contours, advance widths, baselines, and bearings from TrueType/OpenType font files (`.ttf`/`.otf`) identified by SHA-256 hash.
  4. *Unicode Cluster Preservation:* Implements Unicode UAX #29 extended grapheme clusters via `unicode-segmentation`, guaranteeing that complex Vietnamese diacritics (`ử`, `ệ`, `ế`) remain indivisible units during cascade animations.
  5. *100% Bit-for-Bit Determinism:* Software rasterization guarantees bit-for-bit identical RGBA buffers across Preview and Headless Export ($\Delta \text{PSNR} = \infty$).
  6. *Straight Alpha Integrity:* Native Porter-Duff $(A \text{ over } B)$ compositing completely eliminates black halo artifacts on light and video backgrounds.

---

## 3. Decision Outcome

**Chosen Option:** **Option B — Pure Rust Native Typography & Rasterization Stack.**

Option A (`skia-safe`) is formally rejected due to installer bloat and toolchain build fragility on Windows.

---

## 4. Architectural Invariants and Seams

1. **Single Source of Typographic Truth:**
   - Both Preview (Tauri IPC) and Export (FFmpeg stdin pipe) consume the shared native Rust text renderer (`modules::media::text_renderer_poc`).
   - The WebView DOM is strictly a presentation display surface; it does not measure or layout text.

2. **Font Resolution Policy:**
   - Font files are resolved as immutable asset blobs verified by SHA-256 hash.
   - Missing fonts trigger explicit relinking or user-confirmed fallback commands; silent font substitution is prohibited.

3. **Painter's Order & Compositing Rules:**
   - Primitives are rendered strictly from back to front:
     1. Drop Shadows (multi-layer with offset and separable blur)
     2. Outer Strokes (multi-layer dilation around glyph alpha masks)
     3. Glyph Fills (multi-stop directional linear gradient)
     4. Inner Strokes / Glows (if specified)
   - Alpha compositing uses straight RGBA with Porter-Duff $(A \text{ over } B)$ blending.

4. **Consumer Seams:**
   - **Preview Consumer:** Generates a lightweight `PreviewOverlayPacket` containing the dirty ROI bounding box, dimensions, and lossless PNG bytes, delivered over Tauri IPC to the overlay `<canvas>`.
   - **Export Consumer:** Streams raw RGBA buffers (`-f rawvideo -pixel_format rgba`) directly into the stdin pipe of FFmpeg's `filter_complex` (`overlay=format=auto`), avoiding disk I/O and temporary frame files.

5. **Lifecycle, Bounded Memory, and Cancellation:**
   - Memory is strictly bounded: canvas frame buffer (8.29 MB for 1080p, 33.18 MB for 4K) plus a `BoundedGlyphCache` with a configurable ceiling ($\le 256\text{ MiB}$, measured at $< 0.5\text{ MiB}$ under full W1–W3 load).
   - Export pipelines accept an `Arc<AtomicBool>` cancellation token. Immediate cancellation closes the FFmpeg pipe, terminates the child process, drains `stderr`, and removes partial output files.
