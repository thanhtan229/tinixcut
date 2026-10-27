"""Read-only ROI analysis: separate font/glyph-shape effects from fill/motion effects.

Compares the Phase 31 reference frame with the Tinix render at the same milestone.
Reports text-mask agreement, interior (fill) vs edge (outline/shape) error, and word
start positions (advance/kerning drift). Measurements only; no pass/fail claim.
"""
from __future__ import annotations
import json, sys
from pathlib import Path
import numpy as np

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).resolve().parent))
import headless_p31 as h  # reuses ffmpeg-based decoding

REF = h.REF_DIR / "frame_t_end.png"
CAND = REPO / "artifacts/animated-text-templates/phase-31/automated/work/original_frames/t_end.png"
OUT = REPO / "artifacts/animated-text-templates/phase-31/automated/investigation/font-roi-analysis.json"


def shift_or(mask: np.ndarray, dy: int, dx: int) -> np.ndarray:
    out = np.zeros_like(mask)
    ys = slice(max(dy, 0), mask.shape[0] + min(dy, 0))
    yd = slice(max(-dy, 0), mask.shape[0] + min(-dy, 0))
    xs = slice(max(dx, 0), mask.shape[1] + min(dx, 0))
    xd = slice(max(-dx, 0), mask.shape[1] + min(-dx, 0))
    out[ys, xs] = mask[yd, xd]
    return out


def erode(mask: np.ndarray) -> np.ndarray:
    m = mask.copy()
    for dy, dx in [(1,0),(-1,0),(0,1),(0,-1)]:
        m &= shift_or(mask, dy, dx)
    return m


def word_starts(mask: np.ndarray) -> list[int]:
    cols = mask.any(axis=0)
    starts, gap, inrun = [], 0, False
    for x, on in enumerate(cols):
        if on and not inrun:
            if not starts or x - last_end > 18:  # word gap heuristic (px)
                starts.append(x)
            inrun = True
        if not on and inrun:
            last_end = x
            inrun = False
    return starts


def main(candidate: Path = CAND, out: Path = OUT) -> int:
    ref = h.load_rgb(REF)
    cand = h.load_rgb(candidate)
    mr = ref.sum(axis=2) > 30
    mc = cand.sum(axis=2) > 30
    inter = (mr & mc).sum(); union = (mr | mc).sum()
    iou = float(inter / union) if union else None

    er = erode(mr)  # glyph interior (fill-dominated)
    edge = mr & ~er
    diff = np.abs(ref - cand).mean(axis=2)
    interior_mae = float(diff[er].mean()) if er.any() else None
    edge_mae = float(diff[edge].mean()) if edge.any() else None

    # Best global shift that maximises mask overlap (checks for pure offset).
    best = (iou, 0, 0)
    for dy in range(-4, 5):
        for dx in range(-6, 7):
            s = shift_or(mc, dy, dx)
            v = (mr & s).sum() / max((mr | s).sum(), 1)
            if v > best[0]:
                best = (float(v), dy, dx)

    wr = word_starts(mr); wc = word_starts(mc)
    result = {
        "reference": REF.name, "candidate": str(candidate),
        "mask_pixels_ref": int(mr.sum()), "mask_pixels_candidate": int(mc.sum()),
        "mask_iou": round(iou, 4) if iou is not None else None,
        "best_shift_iou": {"iou": round(best[0], 4), "dy": best[1], "dx": best[2]},
        "glyph_interior_px": int(er.sum()), "glyph_interior_mae": round(interior_mae, 3),
        "glyph_edge_px": int(edge.sum()), "glyph_edge_mae": round(edge_mae, 3),
        "word_starts_ref": wr, "word_starts_candidate": wc,
        "interpretation_note": "High interior agreement with lower edge agreement and shifted word starts indicates glyph-shape/advance differences (font), not fill/colour errors.",
    }
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
