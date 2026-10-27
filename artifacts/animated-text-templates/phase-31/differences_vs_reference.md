> **CORRECTION (2026-10-27, supersedes earlier reference claims in this file).**
> The frames `frame_t000.png`, `frame_t_mid.png`, `frame_t_end.png` in this folder are **Tinix renders** (Roboto geometry), not CapCut frames. The true ground truth is the CapCut video `C:\Users\Admin\AppData\Local\CapCut\Videos\1011.mp4`. Measured against CapCut frame 31:
> - Font: the sample's `MPLUS1p-ExtraBold.ttf` (SHA-256 `24993b...`, matches `capcut-sample-index.json`) matches glyph shape (scale-normalised core IoU 0.936 vs 0.528 for Roboto). The earlier "font is the main difference" conclusion is withdrawn.
> - Size: at `font_size` 138 (instead of 152) the core box matches CapCut (106 px high, 1545 vs 1536 px wide); core IoU rises 0.218 -> 0.524 (shape 0.954). A ~6 px vertical offset remains and is not yet analysed.
> - Glow: CapCut has ~92k halo pixels vs ~30k in Tinix. The Phase 29 domain has no glow field, so this is a feature gap.
> Evidence: `automated/capcut-truth/font-shape-analysis.json`, `automated/font-experiment/`.
> Sections below that cite these frames as "reference" are superseded by this correction.

# Phase 31 Native Recreated Sample — Differences vs CapCut Reference

## 1. Overview & Provenance
- **Provenance:** `recreated_reference` (constructed natively using Tinix typed domain and evaluator primitives; **no** Lua runtime, no JS expression engine, no CapCut proprietary assets).
- **Reference Sample:** `TINIX_CAPCUT_IMPORT_TEST` / `TINIX_CAPCUT_SAMPLE_01` ("Tinix — Thử nghiệm tiếng Việt").

## 2. Font Differences & Rationale
- **Reference:** `MPLUS1p-ExtraBold.ttf` (proprietary/local cache, non-distributable).
- **Tinix Recreation:** Apache-2.0 open-source `Roboto-Bold.ttf` (SHA-256: `61F89F8DB49261C2F6106E8DCCC35DF7B2F7ED909020DB40A3FC905E95F99334`).
- **Rationale:** Strict compliance with distribution policy. Roboto-Bold provides 100% full coverage of Vietnamese diacritics ('ử', 'ệ', 'ế') without missing glyphs (`0` notdef glyphs) under a permissive open redistributable license.

## 3. Style Differences & Rationale
- **Fill Gradient:** Matched 118° linear gradient from `#FFE35F` to `#FFFFFF` using `GradientSpace::All`.
- **Strokes:** Matched outer brown stroke `#AD5100` width 3.5px.
- **Shadows:** Dual shadows matched with colors `#AD5100` and `#FFA758` at -45° angle.
- **Observation:** Exact raster match to Tinix Phase 30 shared renderer.

## 4. Motion Differences & Rationale
- **Reference Motion:** CapCut Amaz SDFText Lua script (`AnimScript.lua`), which partitions characters into Group A (21 characters: vertical slide) and Group B (3 characters 'i', 'ệ', 't': bounce + 360° Z-rotation).
- **Tinix Recreation:** Evaluated on canonical frame-grid with typed `TextAnimation` in-segment (`AnimationUnit::Grapheme`, `StaggerOrder::Forward`, duration 31 frames / 1.033s, stagger 0.7, cubic bezier easing).
- **Rationale:** Tinix does not execute arbitrary Lua scripts in editor/preview/export backends for security, determinism, and portability. Future phase (P32+) will introduce compound/custom per-range animation rules; Phase 31 validates the complete canonical evaluator, stagger math, per-grapheme bilinear motion rasterizer, and image-sequence headless export.

## 5. Layout & Painter Order Differences
- **Stroke & Painter Passes:** CapCut executes dynamic SDF shaders inside its native engine. Tinix Phase 31 evaluates strokes in sorted dilated order (widest to narrowest) across global painter passes (all shadows -> all strokes -> all fills) to guarantee that overlapping animated glyphs never cast shadows on neighboring fills.
- **Layout Engine:** CapCut uses HarfBuzz/FreeType with complex script shaping; Tinix Phase 31 uses ab_glyph with Unicode UAX #29 NFC grapheme clusters and deterministic line wrapping.

## 6. Acceptance Status & Honest Declarations (Finding 10)
- **P31-AC-01 (Unicode & Unit Mapping):** PASS (pure headless engine).
- **P31-AC-02 (Stagger & Order Determinism):** PASS (pure headless engine).
- **P31-AC-03 (Phase Continuity & Boundaries):** PASS (pure headless engine).
- **P31-AC-04 (Replacement & Robustness):** PASS (pure headless engine).
- **P31-AC-05 (Duration Conflict Policies & Native Sample):** PARTIAL / DEFERRED NATIVE GUI QUALIFICATION (Headless evaluation and command-backed sample verified; native GUI smoke deferred because frontend is owned by Boss profile).
- **P31-AC-06 (Native Project Schema Parity):** PASS (pure headless engine).
- **P31-AC-07 (Preview & Export Parity):** PASS (pure headless engine).
- **P31-AC-08 (Headless Sequence Export & Cleanup):** PARTIAL / DEFERRED NATIVE GUI QUALIFICATION (Headless FFmpeg export and disposable temp dir cleanup verified; native GUI smoke deferred).
