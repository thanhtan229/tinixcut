> **CORRECTION (2026-10-27).** The `frame_*.png` files in this folder are Tinix renders, not CapCut output, so the comparison numbers below that use them as the reference are invalid. Ground truth is `C:\Users\Admin\AppData\Local\CapCut\Videos\1011.mp4`. See `differences_vs_reference.md` (correction block) and `automated/capcut-truth/font-shape-analysis.json`.

# Phase 31 — Reference MP4 Comparison

**Reference:** `C:\\Users\\Admin\\AppData\\Local\\CapCut\\Videos\\1011.mp4`  
**Recreation:** `artifacts/animated-text-templates/phase-31/recreated_sample_export.mp4`  
**Canvas/fps/duration:** 1920×1080, 30 fps, 5.0 s (150 frames)  
**Machine data:** `reference-comparison.json`  
**Contact sheet:** `reference-comparison-contact-sheet.png`

> The reference uses the proprietary **M PLUS 1p ExtraBold** face and a CapCut Lua/Amaz SDFText
> script. The Tinix recreation uses Apache-2.0 **Roboto-Bold** and typed Tinix primitives. This is a
> measured approximation report, not a claim of pixel-identical conversion.

## 1. Declared tolerances

| Dimension | Tolerance | Rationale |
| :--- | :--- | :--- |
| Text bounding box at plateau (t ≥ in-duration) | width/height delta ≤ 24 px | Different font metrics |
| Text centroid at plateau | ≤ 16 px | Centering/layout approximation |
| Visible onset / last visible frame | same frame ± 1 | Canonical timeline alignment |
| Peak ink frame | ± 5 frames | Stagger approximation |
| ROI SSIM | informational only | Font/shader differences preclude pixel parity |
| Full-frame metric | not used | Black background dominates and would mask text differences |

## 2. Measured results

Plateau samples (t = 1.0333, 1.5, 2.5, 3.5, 4.5, 4.9 s):

| Metric | Reference | Recreation | Delta |
| :--- | ---: | ---: | ---: |
| ROI (x0,y0,x1,y1) | 176,472,1746,612 | 184,488,1738,614 | width −16 px, height −14 px |
| ROI centroid | 961,542 | 961,551 | (0, +9) px |
| Peak ink frame (480×270 analysis) | 23 | 26 | +3 frames |
| Onset / last visible frame | 1 / 149 | 1 / 149 | 0 |
| ROI SSIM (union ROI) | — | ≈ 0.6205 | informational |
| ROI MAE (0–1) | — | ≈ 0.1997 | informational |

Early entrance samples (t = 0.3–0.8 s) show larger transient differences (SSIM 0.50–0.65,
vertical offset up to 78 px) because the CapCut script partitions characters into a 21-character
slide group plus a 3-character bounce/rotation group, while the Tinix recreation applies one typed
stagger in-segment. This is disclosed as a motion-detail difference.

## 3. Conclusion

- Text placement, plateau bounds, visibility window and onset align within the declared tolerances.
- The recreation is editable, uses `recreated_reference` provenance, and renders without CapCut or the
  reference MP4.
- It is **not** pixel-identical: font, shader-style strokes/shadows and the two-group Lua motion
  differ. Those differences are quantified here rather than hidden.

## 4. Native runtime observation

A development runtime was started with `pnpm tauri dev`: Vite returned HTTP 200 and the
`tinixcut` process ran. A native window handle existed, but this session could not capture an
interactive window surface (screen-capture returned a 15×15 hidden helper window). Interactive
edit/play/seek/export smoke therefore remains deferred and is not claimed as PASS.
