"""G52 registration self-check: fit the canvas -> capture mapping from the grid target.

Fits capture = scale * canvas + offset separately for x and y from the grid line centres
(1920x1080 canvas, lines at known positions, see make_registration_grid.py), and reports
the residual, the fit per half of the frame (to expose stretch) and the outermost
white-frame pixels (to expose cropping). Measurement only; never applied to a verdict.

Usage: python -I registration_fit.py <capture.png> [--controls]
"""
import sys
import numpy as np
from PIL import Image

W, H = 1920, 1080


def centres(n, step=60):
    # thick (5 px) at multiples of 120, thin (3 px) at the other multiples of 60
    return [(p + (2.5 if p % 120 == 0 else 1.5)) for p in range(step, n - step, step)]


def profile_peaks(profile, expected, scale, off):
    got = []
    for c in expected:
        pc = scale * c + off
        half = max(3, int(round(scale * 8)))
        lo, hi = int(pc) - half, int(pc) + half + 1
        if lo < 0 or hi > len(profile):
            continue
        seg = profile[lo:hi] - np.median(profile)
        seg = np.clip(seg, 0, None)
        if seg.sum() <= 0:
            continue
        got.append((c, (np.arange(lo, hi) * seg).sum() / seg.sum() + 0.5))  # +0.5: pixel centre -> edge coords
    return got


def fit(pairs):
    c = np.array([p[0] for p in pairs]); g = np.array([p[1] for p in pairs])
    s, o = np.polyfit(c, g, 1)
    return s, o, float(np.abs(g - (s * c + o)).max())


def analyse(img):
    a = np.asarray(img.convert("L"), float)
    h, w = a.shape
    sx0, sy0 = w / W, h / H
    # background rows/cols avoid the diagonals: use the central band for the column profile
    colp = np.median(a[int(h * 0.2):int(h * 0.3)], 0)
    rowp = np.median(a[:, int(w * 0.2):int(w * 0.3)], 1)
    out = {"size": (w, h)}
    for name, prof, n, s0 in (("x", colp, W, sx0), ("y", rowp, H, sy0)):
        ex = centres(n)
        pairs = profile_peaks(prof, ex, s0, 0)
        s, o, r = fit(pairs)
        pairs = profile_peaks(prof, ex, s, o)
        s, o, r = fit(pairs)
        half = len(pairs) // 2
        s1, o1, _ = fit(pairs[:half]); s2, o2, _ = fit(pairs[half:])
        out[name] = {"scale": s, "offset": o, "max_residual": r, "scale_first_half": s1,
                     "scale_second_half": s2, "n": len(pairs),
                     "canvas_end_in_capture": s * n + o}
    return out


if __name__ == "__main__":
    img = Image.open(sys.argv[1])
    res = analyse(img)
    for k in ("x", "y"):
        v = res[k]
        print(f"{k}: scale {v['scale']:.5f} offset {v['offset']:+.3f} px, max residual {v['max_residual']:.3f}, "
              f"halves {v['scale_first_half']:.5f}/{v['scale_second_half']:.5f}, canvas end at {v['canvas_end_in_capture']:.2f}, n={v['n']}")
    print("capture size", res["size"])
