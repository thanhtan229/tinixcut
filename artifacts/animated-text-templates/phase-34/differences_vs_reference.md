# Phase 34 source comparison

- Reference: `1011.mp4` (SHA-256 `95489CC32CBB56F6F9E7424FE9C9611FEB927D5C0E4EBD08841FA68A726A4DE1`).
- Timeline sample: frame 15 at 0.500s, 30 fps.
- ROI (x0, y0, x1, y1): `174, 474, 1288, 718` (1114 × 244).
- RGB PSNR: 9.623 dB; global RGB SSIM: 0.114207; RGB MAE: 40.789.

The comparison uses identical canvas coordinates and composites the Tinix alpha frame over the source's black canvas. No registration or color correction is applied. The metrics are a single-frame ROI diagnostic, not a claim of pixel parity; inspect `reference_roi_t015.png` and `tinix_roi_t015.png` for the compared crops. Native importer dialog/apply qualification remains separate.
