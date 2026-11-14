"""Fit the canvas->image map of an animated (scaled/moved) grid layer, without a prior guess.
Usage: python -I anim_fit.py <image.png> <expected_scale>  -> per-axis scale/offset in image px per canvas px.
Diagnostic for G52 Step 3; measurement only."""
import sys
import numpy as np
from PIL import Image
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
import registration_fit as r


def axis(profile, n, s_guess):
    exp = r.centres(n)
    prof = profile - np.median(profile)
    best = (-1, None)
    for s in np.arange(s_guess * 0.9, s_guess * 1.1, s_guess * 0.0005):
        for o in np.arange(-len(profile), len(profile), 1.0):
            pos = np.round(s * np.array(exp) + o).astype(int)
            ok = (pos >= 0) & (pos < len(profile))
            if ok.sum() < 8:
                continue
            v = prof[pos[ok]].mean()
            if v > best[0]:
                best = (v, (s, o))
    s, o = best[1]
    for _ in range(2):
        pairs = r.profile_peaks(profile, exp, s, o)
        s, o, res = r.fit(pairs)
    return s, o, res, len(pairs)


img = Image.open(sys.argv[1]); a = np.asarray(img.convert("L"), float); h, w = a.shape
g = float(sys.argv[2])
for name, prof, n, sg in (("x", np.median(a[int(h*.2):int(h*.3)], 0), 1920, g * w / 1920),
                          ("y", np.median(a[:, int(w*.2):int(w*.3)], 1), 1080, g * h / 1080)):
    s, o, res, k = axis(prof, n, sg)
    print(f"{name}: scale {s / (w/1920 if name=='x' else h/1080):.4f} of canvas, offset {o:+.2f} px (image {w}x{h}), residual {res:.3f}, lines {k}")
