"""Writes the G52 registration target: a 1920x1080 grid video (BT.709 tagged, H.264 yuv420p).

Lines every 60 px (3 px wide), thicker 120 px lines, a 2 px frame on the outermost pixels
(to expose any cropping of the canvas edge), corner marks and diagonals (to expose scale).
Usage: python -I make_registration_grid.py <out_dir>
"""
import os, subprocess, sys
import numpy as np
from PIL import Image

W, H = 1920, 1080
out = sys.argv[1]
os.makedirs(out, exist_ok=True)
a = np.full((H, W), 40, np.uint8)
for x in range(0, W, 60):
    a[:, x:x + (5 if x % 120 == 0 else 3)] = 235
for y in range(0, H, 60):
    a[y:y + (5 if y % 120 == 0 else 3), :] = 235
a[:2, :] = 255; a[-2:, :] = 255; a[:, :2] = 255; a[:, -2:] = 255
for i in range(W):  # diagonals
    for d in (-1, 0, 1):
        for y in (round(i * (H - 1) / (W - 1)), H - 1 - round(i * (H - 1) / (W - 1))):
            if 0 <= y + d < H:
                a[y + d, i] = 200
png = os.path.join(out, "grid.png")
Image.fromarray(a).convert("RGB").save(png)
subprocess.run(["ffmpeg", "-v", "error", "-y", "-loop", "1", "-framerate", "30", "-i", png, "-t", "6",
                "-vf", "scale=in_range=full:out_range=tv:out_color_matrix=bt709,format=yuv420p",
                "-c:v", "libx264", "-crf", "10", "-colorspace", "bt709", "-color_primaries", "bt709",
                "-color_trc", "bt709", os.path.join(out, "grid.mp4")], check=True)
print("ok")
