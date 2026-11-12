"""Preview/export parity metrics for G52 (locked 2026-11-12).

For each frame N, compares <dir>/preview_fN.png (native canvas capture) with
<dir>/export_fN.png (frame N of the export, scaled bicubic to the preview size):

- SSIM on luma over the canvas (Gaussian window 11, sigma 1.5; a 3 px margin is
  trimmed to drop the canvas viewport's 1 px UI border). Threshold >= 0.92.
- Centroid of each layer's rendered region, as an offset from the canvas center in
  canvas pixels (canvas width = --canvas-width). Threshold <= 1 % of canvas width.
  The video layer is located from the main frames; other layers from an optional
  isolation directory (--isolation <dir>, frames where only that layer is visible).

Usage: python -I compare_parity.py <dir> <frame,...> [--isolation <dir>] [--out <json>]
"""
import argparse, json, math
import numpy as np
from PIL import Image

SSIM_MIN = 0.92
CENTROID_MAX_FRACTION = 0.01
MARGIN = 3


def load_pair(directory, frame):
    preview = Image.open(f"{directory}/preview_f{frame}.png").convert("RGB")
    export = Image.open(f"{directory}/export_f{frame}.png").convert("RGB").resize(preview.size, Image.BICUBIC)
    return preview, export


def luma(img):
    a = np.asarray(img).astype(np.float64)
    y = 0.2126 * a[..., 0] + 0.7152 * a[..., 1] + 0.0722 * a[..., 2]
    return y[MARGIN:-MARGIN, MARGIN:-MARGIN]


def _gauss_filter(img, sigma=1.5, size=11):
    ax = np.arange(size) - size // 2
    k = np.exp(-(ax ** 2) / (2 * sigma ** 2))
    k /= k.sum()
    # Separable valid-mode convolution.
    tmp = np.apply_along_axis(lambda r: np.convolve(r, k, mode="valid"), 1, img)
    return np.apply_along_axis(lambda c: np.convolve(c, k, mode="valid"), 0, tmp)


def ssim(a, b):
    c1, c2 = (0.01 * 255) ** 2, (0.03 * 255) ** 2
    mu_a, mu_b = _gauss_filter(a), _gauss_filter(b)
    saa = _gauss_filter(a * a) - mu_a ** 2
    sbb = _gauss_filter(b * b) - mu_b ** 2
    sab = _gauss_filter(a * b) - mu_a * mu_b
    m = ((2 * mu_a * mu_b + c1) * (2 * sab + c2)) / ((mu_a ** 2 + mu_b ** 2 + c1) * (saa + sbb + c2))
    return float(m.mean())


def region_centroid(y, canvas_width, threshold=40.0):
    """Centroid of the bright region, as (dx, dy) from canvas center in canvas pixels."""
    mask = y > threshold
    if mask.sum() < 50:
        return None
    rows, cols = np.nonzero(mask)
    scale = canvas_width / (y.shape[1] + 2 * MARGIN)
    cx = (cols.mean() + MARGIN) * scale - canvas_width / 2
    cy = (rows.mean() + MARGIN) * scale - (y.shape[0] + 2 * MARGIN) * scale / 2
    return round(float(cx), 1), round(float(cy), 1)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("directory")
    ap.add_argument("frames")
    ap.add_argument("--isolation")
    ap.add_argument("--canvas-width", type=float, default=1920.0)
    ap.add_argument("--out")
    args = ap.parse_args()
    limit = CENTROID_MAX_FRACTION * args.canvas_width
    results = []
    for frame in [int(f) for f in args.frames.split(",")]:
        p, e = load_pair(args.directory, frame)
        yp, ye = luma(p), luma(e)
        row = {"frame": frame, "ssim": round(ssim(yp, ye), 4)}
        layers = {"all": (region_centroid(yp, args.canvas_width), region_centroid(ye, args.canvas_width))}
        if args.isolation:
            try:
                ip, ie = load_pair(args.isolation, frame)
                layers["isolated"] = (region_centroid(luma(ip), args.canvas_width), region_centroid(luma(ie), args.canvas_width))
            except FileNotFoundError:
                pass
        row["centroids"] = {}
        ok = row["ssim"] >= SSIM_MIN
        for name, (cp, ce) in layers.items():
            if cp is None and ce is None:
                row["centroids"][name] = {"preview": None, "export": None, "errorPx": 0.0, "note": "layer not visible in either"}
                continue
            if cp is None or ce is None:
                row["centroids"][name] = {"preview": cp, "export": ce, "errorPx": None, "note": "visible in only one"}
                ok = False
                continue
            err = math.hypot(cp[0] - ce[0], cp[1] - ce[1])
            row["centroids"][name] = {"preview": cp, "export": ce, "errorPx": round(err, 1)}
            ok = ok and err <= limit
        row["pass"] = ok
        results.append(row)
        print(json.dumps(row, ensure_ascii=False))
    summary = {"ssimMin": SSIM_MIN, "centroidMaxPx": limit, "allPass": all(r["pass"] for r in results), "frames": results}
    if args.out:
        json.dump(summary, open(args.out, "w"), indent=1)
    print("ALL PASS" if summary["allPass"] else "FAIL")


main()
