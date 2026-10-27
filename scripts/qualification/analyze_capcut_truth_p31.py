"""Ground-truth ROI analysis for Phase 31 against the real CapCut frame (read-only).

Compares bright text cores (luminance > 200) so the CapCut glow does not dominate
the metric. Reports raw core geometry, then a scale-normalised shape IoU per font
render, to separate glyph shape (font) from size/glow effects. Measurements only.
"""
from __future__ import annotations
import json, sys
from pathlib import Path
import numpy as np
from PIL import Image

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).resolve().parent))
import headless_p31 as h

O = REPO / "artifacts/animated-text-templates/phase-31/automated"
CAPCUT = O / "capcut-truth/capcut_f31.png"
CANDIDATES = {
    "tinix_mplus1p": O / "font-experiment/t_end_mplus1p_xb.png",
    "tinix_roboto": O / "work/original_frames/t_end.png",
    "tinix_mplus1p_fontsize138": O / "font-experiment/t_end_mplus1p_xb_fs138.png",
}


def core(a: np.ndarray, thr: float = 200) -> np.ndarray:
    return (0.299 * a[..., 0] + 0.587 * a[..., 1] + 0.114 * a[..., 2]) > thr


def box(m: np.ndarray) -> tuple[int, int, int, int]:
    ys = np.where(m.any(1))[0]; xs = np.where(m.any(0))[0]
    return int(ys.min()), int(ys.max()) + 1, int(xs.min()), int(xs.max()) + 1


def baseline_row(m: np.ndarray, x0: int = 190, x1: int = 330) -> int:
    """Bottom ink row of the word 'Tinix' (no descenders), i.e. the baseline."""
    rows = np.where(m[:, x0:x1].any(1))[0]
    return int(rows.max()) + 1


def main() -> int:
    cap = core(h.load_rgb(CAPCUT))
    cy0, cy1, cx0, cx1 = box(cap)
    capc = cap[cy0:cy1, cx0:cx1]
    out = {"capcut_baseline_y": baseline_row(cap), "capcut_core_box": {"y0": cy0, "y1": cy1, "x0": cx0, "x1": cx1}, "capcut_core_px": int(cap.sum()), "candidates": {}}
    for name, path in CANDIDATES.items():
        m = core(h.load_rgb(path))
        y0, y1, x0, x1 = box(m)
        sub = m[y0:y1, x0:x1]
        raw_iou = None
        if m.shape == cap.shape:
            raw_iou = float((m & cap).sum() / max((m | cap).sum(), 1))
        norm = np.asarray(Image.fromarray((sub * 255).astype(np.uint8)).resize((capc.shape[1], capc.shape[0]), Image.NEAREST)) > 127
        out["candidates"][name] = {
            "baseline_y": baseline_row(m),
            "baseline_offset_vs_capcut_px": baseline_row(m) - baseline_row(cap),
            "core_px": int(m.sum()),
            "core_box": {"y0": y0, "y1": y1, "x0": x0, "x1": x1},
            "raw_core_iou_same_frame": round(raw_iou, 4) if raw_iou is not None else None,
            "scale_x_to_capcut": round(capc.shape[1] / sub.shape[1], 4),
            "scale_y_to_capcut": round(capc.shape[0] / sub.shape[0], 4),
            "scale_normalised_shape_iou": round(float((norm & capc).sum() / max((norm | capc).sum(), 1)), 4),
        }
    p = O / "capcut-truth/font-shape-analysis.json"
    p.write_text(json.dumps(out, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(out, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
