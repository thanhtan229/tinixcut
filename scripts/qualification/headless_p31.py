"""Headless qualification for Phase 31 recreated sample (B3-B6 of the native checklist).

Drives the TinixCut binary through its JSONL stdio session and one-shot `render`
command. Does NOT exercise the GUI: playback, dialog, and Undo/Redo UI observations
remain manual. Results are measurements, not PASS claims about parity.

Usage (from repo root, after `cargo build` in src-tauri):
    python scripts/qualification/headless_p31.py
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import numpy as np
from PIL import Image

REPO = Path(__file__).resolve().parents[2]
BIN = REPO / "src-tauri" / "target" / "debug" / "tinixcut.exe"
FFMPEG = REPO / "src-tauri" / "resources" / "bin" / "ffmpeg.exe"
SAMPLE = REPO / "artifacts" / "animated-text-templates" / "phase-31" / "recreated_sample_project.json"
REF_DIR = REPO / "artifacts" / "animated-text-templates" / "phase-31"
OUT_DIR = REPO / "artifacts" / "animated-text-templates" / "phase-31" / "automated"

# Frame indices for the reference milestones (150-frame sample at 30 fps).
# Loaded project is migrated to the current schema (v10 sample -> v11); commands must match.
CURRENT_SCHEMA_VERSION = 11

MILESTONES = {"t000": ("frame_t000.png", 0), "t_mid": ("frame_t_mid.png", 75), "t_end": ("frame_t_end.png", 149)}

VARIANTS = {
    "original": None,  # unchanged sample text; compared against reference frames
    "short_ascii": "Tinix",
    "long_multiline": "Chào mừng bạn đến với TinixCut, phần mềm dựng video tự động",
    "vietnamese_marks": "Ửng dụng ệ ế ử",
}


class Session:
    """Minimal JSONL client for `tinixcut session`."""

    def __init__(self, workdir: Path):
        self.proc = subprocess.Popen(
            [str(BIN), "session"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=workdir,
            text=True,
            encoding="utf-8",
        )
        self._n = 0

    def call(self, method: str, params: dict | None = None) -> dict:
        self._n += 1
        req = {"protocolVersion": 1, "requestId": f"r{self._n}", "method": method, "params": params or {}}
        self.proc.stdin.write(json.dumps(req) + "\n")
        self.proc.stdin.flush()
        line = self.proc.stdout.readline()
        if not line:
            raise RuntimeError(f"session closed; stderr={self.proc.stderr.read()}")
        resp = json.loads(line)
        if not resp.get("ok"):
            raise RuntimeError(f"{method} failed: {resp.get('error')}")
        return resp["result"]

    def close(self) -> None:
        try:
            self.call("session.shutdown")
        except Exception:
            pass
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        self.proc.wait(timeout=30)


def load_project_file(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def find_text_item(project: dict) -> tuple[str, str]:
    for track in project["tracks"]:
        for item in track["items"]:
            if item.get("kind") == "Text":
                return item["item_id"], item.get("text_content") or ""
    raise RuntimeError("no Text item in sample project")


def text_of(project_path: Path, item_id: str) -> str:
    proj = load_project_file(project_path)
    for track in proj["tracks"]:
        for item in track["items"]:
            if item["item_id"] == item_id:
                return item.get("text_content") or ""
    raise RuntimeError(f"item {item_id} not found")


def render(project_path: Path, out_mp4: Path) -> dict:
    t0 = time.perf_counter()
    proc = subprocess.run(
        [str(BIN), "render", str(project_path), str(out_mp4)],
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=900,
    )
    elapsed = time.perf_counter() - t0
    if proc.returncode != 0 or not out_mp4.exists():
        raise RuntimeError(f"render failed rc={proc.returncode}: {proc.stderr[-800:]}")
    return {"seconds": round(elapsed, 2), "bytes": out_mp4.stat().st_size, "stdout_tail": proc.stdout[-300:]}


def extract_frame(mp4: Path, frame_index: int, out_png: Path) -> None:
    subprocess.run(
        [
            str(FFMPEG), "-y", "-v", "error",
            "-i", str(mp4),
            "-vf", f"select=eq(n\\,{frame_index})",
            "-vframes", "1",
            str(out_png),
        ],
        check=True,
        timeout=120,
    )


def load_rgb(path: Path) -> np.ndarray:
    """Decode with ffmpeg so reference and candidate share one decoder.

    PIL rejects the Phase 31 reference PNGs even though they are valid PNG files
    (ffmpeg decodes them without error), so both sides use ffmpeg for fairness.
    """
    probe = subprocess.run(
        [str(FFMPEG), "-v", "error", "-i", str(path), "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
        capture_output=True,
        timeout=120,
    )
    if probe.returncode != 0 or not probe.stdout:
        raise RuntimeError(f"ffmpeg could not decode {path}: {probe.stderr[-300:]!r}")
    # Dimensions come from ffprobe-free reading of the PNG IHDR chunk (bytes 16-24).
    width = int.from_bytes(path.read_bytes()[16:20], "big")
    height = int.from_bytes(path.read_bytes()[20:24], "big")
    return np.frombuffer(probe.stdout, dtype=np.uint8).reshape(height, width, 3).astype(np.float32)


def compare(candidate: Path, reference: Path) -> dict:
    a = load_rgb(candidate)
    b = load_rgb(reference)
    if a.shape != b.shape:
        return {"status": "SIZE_MISMATCH", "candidate": a.shape, "reference": b.shape}
    diff = np.abs(a - b)
    mask = (b.sum(axis=2) > 30)  # reference non-background pixels (text ROI proxy)
    ys, xs = np.where(mask)
    roi_mae = float(diff[mask].mean()) if mask.any() else None
    bbox = [int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max())] if mask.any() else None
    return {
        "full_frame_mae": round(float(diff.mean()), 3),
        "roi_mae": round(roi_mae, 3) if roi_mae is not None else None,
        "roi_bbox_ref": bbox,
        "roi_pixels": int(mask.sum()),
    }


def run_variant(name: str, text: str | None, base_project: dict, item_id: str, work: Path) -> dict:
    result: dict = {"variant": name, "text": text if text is not None else base_project and find_text_item(base_project)[1]}
    proj_path = work / f"{name}.tcproj"
    proj_path.write_text(json.dumps(base_project, ensure_ascii=False), encoding="utf-8")

    session = Session(work)
    try:
        opened = session.call("session.open", {"path": str(proj_path)})
        project_id = opened.get("projectId") or base_project["project_id"]
        if text is not None:
            base_rev = session.call("project.inspect").get("revision", 0)
            envelope = {
                "schemaVersion": CURRENT_SCHEMA_VERSION,
                "projectId": project_id,
                "baseRevision": base_rev,
                "operations": [
                    {"operationId": f"op-{name}", "operation": {"type": "ClipTextSet", "payload": {"item_id": item_id, "text": text}}}
                ],
            }
            receipt = session.call("command.apply", {"envelope": envelope, "saveAfter": True})
            result["apply_status"] = receipt.get("status", "applied")
        else:
            session.call("project.save")
        session.call("session.close")
    finally:
        session.close()

    result["saved_text"] = text_of(proj_path, item_id)
    result["text_persisted"] = (text is None) or (result["saved_text"] == text)

    # Reopen and confirm the edit survives a fresh process.
    session = Session(work)
    try:
        session.call("session.open", {"path": str(proj_path)})
        inspect = session.call("project.inspect")
        result["reopen_revision"] = inspect.get("revision")
        session.call("session.close")
    finally:
        session.close()

    mp4 = work / f"{name}.mp4"
    result["render"] = render(proj_path, mp4)

    frames_dir = work / f"{name}_frames"
    frames_dir.mkdir(exist_ok=True)
    result["frames"] = {}
    for key, (ref_name, idx) in MILESTONES.items():
        out_png = frames_dir / f"{key}.png"
        extract_frame(mp4, idx, out_png)
        if name == "original":
            result["frames"][key] = {"frame_index": idx, **compare(out_png, REF_DIR / ref_name)}
        else:
            result["frames"][key] = {"frame_index": idx, "png": str(out_png.name)}
    return result


def main() -> int:
    if not BIN.exists():
        print(f"missing binary: {BIN}. Run `cargo build` in src-tauri first.", file=sys.stderr)
        return 2
    base = json.loads(SAMPLE.read_text(encoding="utf-8"))["project"]
    item_id, original_text = find_text_item(base)

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix="p31_headless_"))
    report = {"sample": str(SAMPLE.name), "item_id": item_id, "original_text": original_text, "variants": []}
    try:
        for name, text in VARIANTS.items():
            print(f"[variant] {name}", flush=True)
            report["variants"].append(run_variant(name, text, base, item_id, work))
        # Keep MP4s and frames as evidence.
        evidence = OUT_DIR / "work"
        if evidence.exists():
            shutil.rmtree(evidence)
        shutil.copytree(work, evidence, ignore=shutil.ignore_patterns("*.tcproj"))
    finally:
        shutil.rmtree(work, ignore_errors=True)

    (OUT_DIR / "headless-report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"wrote {OUT_DIR / 'headless-report.json'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
