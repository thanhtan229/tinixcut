"""G52 Step 1: locked luma SSIM (compare_parity.ssim, bicubic export resize, 3 px margin) at exact capture geometry.

Usage: python -I compare_exact.py <capture_dir> <export_dir> <frames,csv> <dpr,csv> [--out json]
Captures are <capture_dir>/f<N>_dpr<D>.png; exports are <export_dir>/export_f<N>.png (1920x1080).
No shifting or fitting. Verdict rule (fixed before capturing): a frame passes only if SSIM >= 0.92 at every DPR.
"""
import json, sys
from PIL import Image
sys.path.insert(0, __import__("os").path.dirname(__import__("os").path.abspath(__file__)))
from compare_parity import luma, ssim, SSIM_MIN

cap, exp, frames, dprs = sys.argv[1], sys.argv[2], sys.argv[3].split(","), sys.argv[4].split(",")
rows, ok = [], True
for f in frames:
    e = Image.open(f"{exp}/export_f{f}.png").convert("RGB")
    row = {"frame": int(f)}
    for d in dprs:
        p = Image.open(f"{cap}/f{f}_dpr{d}.png").convert("RGB")
        row[f"dpr{d}"] = round(float(ssim(luma(p), luma(e.resize(p.size, Image.BICUBIC)))), 4)
    row["pass"] = all(row[f"dpr{d}"] >= SSIM_MIN for d in dprs)
    ok &= row["pass"]
    rows.append(row)
    print(row)
print("VERDICT", "PASS" if ok else "FAIL")
if "--out" in sys.argv:
    json.dump({"rows": rows, "pass": ok}, open(sys.argv[sys.argv.index("--out") + 1], "w"), indent=1)
