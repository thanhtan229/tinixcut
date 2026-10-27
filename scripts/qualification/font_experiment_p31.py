"""Measure Phase 31 end-frame ROI with a different font (local experiment only).

Builds a scratch copy of the recreated sample whose text uses a font asset
(Be Vietnam Pro ExtraBold, SIL OFL) and compares the rendered end frame with the
reference using analyze_font_roi. Nothing in the repo's sample or fonts is changed.

The font is registered as an Image-kind asset because MediaKind has no Font
variant; font resolution only reads the asset path, so this is a measurement
shortcut, not the product font-asset path (that is a Phase 32 concern).
"""
from __future__ import annotations
import json, subprocess, sys, tempfile
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
import headless_p31 as h
import analyze_font_roi as afr

import os
FONT = Path(os.environ.get("P31_FONT", r"C:\Windows\Fonts\BeVietnamPro-ExtraBold.ttf"))
FONT_LABEL = os.environ.get("P31_FONT_LABEL", "bevietnam_xb")
FONT_FAMILY = os.environ.get("P31_FONT_FAMILY", "Be Vietnam Pro")
_fs = os.environ.get("P31_FONT_SIZE")
FONT_SIZE = float(_fs) if _fs else None
OUT_DIR = h.OUT_DIR / "font-experiment"


def main() -> int:
    if not FONT.exists():
        print(f"font missing: {FONT}", file=sys.stderr); return 2
    base = json.loads(h.SAMPLE.read_text(encoding="utf-8"))["project"]
    item_id, _ = h.find_text_item(base)
    asset_id = "asset_font_bevietnam_xb"
    base["assets"] = [{
        "asset_id": asset_id, "name": FONT_LABEL, "path": str(FONT),
        "media_kind": "Image", "duration": None, "file_size": FONT.stat().st_size, "dimensions": None,
    }]
    for track in base["tracks"]:
        for it in track["items"]:
            if it.get("kind") == "Text" and it.get("text_rich"):
                it["text_rich"]["font"] = {"family": FONT_FAMILY, "weight": 800, "style": "Normal", "asset_id": asset_id}
                if FONT_SIZE is not None:
                    it["text_rich"]["font_size"] = FONT_SIZE

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix="p31_font_"))
    try:
        proj = work / "font_exp.tcproj"
        proj.write_text(json.dumps(base, ensure_ascii=False), encoding="utf-8")
        mp4 = work / "font_exp.mp4"
        r = h.render(proj, mp4)
        frame = OUT_DIR / f"t_end_{FONT_LABEL}.png"
        h.extract_frame(mp4, 31, frame)
        report_path = OUT_DIR / f"font-roi-analysis-{FONT_LABEL}.json"
        afr.main(candidate=frame, out=report_path)
        (OUT_DIR / f"render-{FONT_LABEL}.json").write_text(json.dumps(r, indent=2), encoding="utf-8")
    finally:
        import shutil; shutil.rmtree(work, ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
