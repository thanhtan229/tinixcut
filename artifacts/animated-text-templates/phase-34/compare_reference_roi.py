"""Reproducible single-frame ROI comparison for Phase 34 sample evidence."""

from __future__ import annotations

import hashlib
import json
import subprocess
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image


ROOT = Path(__file__).resolve().parent
REFERENCE = Path(r"C:\Users\Admin\AppData\Local\CapCut\Videos\1011.mp4")
TINIX_FRAME = ROOT / "frame_t_mid.png"
FRAME_INDEX = 15
FPS = 30


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest().upper()


def main() -> None:
    with tempfile.TemporaryDirectory(prefix="phase34-roi-") as temp:
        reference_frame = Path(temp) / "reference.png"
        subprocess.run(
            [
                "ffmpeg",
                "-v",
                "error",
                "-y",
                "-i",
                str(REFERENCE),
                "-vf",
                f"select=eq(n\\,{FRAME_INDEX})",
                "-frames:v",
                "1",
                str(reference_frame),
            ],
            check=True,
        )

        ref = np.asarray(Image.open(reference_frame).convert("RGB"), dtype=np.float64)
        tinix_rgba = np.asarray(Image.open(TINIX_FRAME).convert("RGBA"), dtype=np.float64)
        if ref.shape[:2] != tinix_rgba.shape[:2]:
            raise ValueError(f"Frame dimensions differ: ref={ref.shape}, Tinix={tinix_rgba.shape}")

        alpha = tinix_rgba[..., 3:4] / 255.0
        tinix = tinix_rgba[..., :3] * alpha  # Composite over the reference's black canvas.
        ref_mask = np.max(ref, axis=2) > 8
        tinix_mask = tinix_rgba[..., 3] > 8
        mask = ref_mask | tinix_mask
        ys, xs = np.where(mask)
        if not len(xs):
            raise ValueError("No foreground pixels found in either frame")
        x0, y0 = int(xs.min()), int(ys.min())
        x1, y1 = int(xs.max()) + 1, int(ys.max()) + 1
        a = ref[y0:y1, x0:x1]
        b = tinix[y0:y1, x0:x1]
        diff = a - b
        mse = float(np.mean(diff * diff))
        psnr = float("inf") if mse == 0 else float(10 * np.log10((255.0**2) / mse))

        # Global SSIM over the declared ROI, computed per channel and averaged.
        mu_a, mu_b = a.mean(axis=(0, 1)), b.mean(axis=(0, 1))
        var_a, var_b = a.var(axis=(0, 1)), b.var(axis=(0, 1))
        cov = ((a - mu_a) * (b - mu_b)).mean(axis=(0, 1))
        c1, c2 = (0.01 * 255) ** 2, (0.03 * 255) ** 2
        ssim = float(np.mean(((2 * mu_a * mu_b + c1) * (2 * cov + c2)) / ((mu_a**2 + mu_b**2 + c1) * (var_a + var_b + c2))))

        report = {
            "method": "direct same-coordinate RGB comparison; Tinix alpha composited over source black canvas; no registration or color correction",
            "reference_video": str(REFERENCE),
            "reference_sha256": sha256(REFERENCE),
            "tinix_frame": TINIX_FRAME.name,
            "frame_index": FRAME_INDEX,
            "timestamp_seconds": FRAME_INDEX / FPS,
            "fps": FPS,
            "dimensions": {"width": int(ref.shape[1]), "height": int(ref.shape[0])},
            "roi_xyxy": [x0, y0, x1, y1],
            "roi_dimensions": {"width": x1 - x0, "height": y1 - y0},
            "foreground_pixels_reference": int(ref_mask.sum()),
            "foreground_pixels_tinix": int(tinix_mask.sum()),
            "metrics": {"rgb_psnr_db": psnr, "global_rgb_ssim": ssim, "rgb_mae": float(np.mean(np.abs(diff)))},
            "interpretation": "Quantitative snapshot only; animation/font/effect differences remain visible in the ROI and require human review.",
        }
        (ROOT / "reference-comparison.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        Image.fromarray(np.clip(a, 0, 255).astype(np.uint8)).save(ROOT / "reference_roi_t015.png")
        Image.fromarray(np.clip(b, 0, 255).astype(np.uint8)).save(ROOT / "tinix_roi_t015.png")
        (ROOT / "differences_vs_reference.md").write_text(
            "# Phase 34 source comparison\n\n"
            f"- Reference: `{REFERENCE.name}` (SHA-256 `{report['reference_sha256']}`).\n"
            f"- Timeline sample: frame {FRAME_INDEX} at {FRAME_INDEX / FPS:.3f}s, {FPS} fps.\n"
            f"- ROI (x0, y0, x1, y1): `{x0}, {y0}, {x1}, {y1}` ({x1-x0} × {y1-y0}).\n"
            f"- RGB PSNR: {psnr:.3f} dB; global RGB SSIM: {ssim:.6f}; RGB MAE: {report['metrics']['rgb_mae']:.3f}.\n\n"
            "The comparison uses identical canvas coordinates and composites the Tinix alpha frame over the source's black canvas. "
            "No registration or color correction is applied. The metrics are a single-frame ROI diagnostic, not a claim of pixel parity; "
            "inspect `reference_roi_t015.png` and `tinix_roi_t015.png` for the compared crops. Native importer dialog/apply qualification remains separate.\n",
            encoding="utf-8",
        )


if __name__ == "__main__":
    main()
