"""Capture the native preview canvas at given timeline frames (G52 parity).

For each frame: seek through UI Automation (seek_uia.ps1), wait until the timecode label
shows the frame, every <video> layer has settled (not seeking, readyState >= 2) and the
rich-text/overlay count is stable, then screenshot exactly the `.canvas-viewport` region
over CDP at device pixels. HUD text and the timecode badge are hidden only for the
screenshot (measurement-only style injection, removed afterwards).

Usage: python -I capture_preview.py <out_dir> <frame> [<frame> ...]
"""
import asyncio, base64, json, os, subprocess, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import websockets
from cdp import ws_url, evaluate

STATE = r"""(() => {
  const c = document.querySelector('.canvas-viewport');
  const t = document.body.innerText;
  const vids = [...c.querySelectorAll('video')];
  return {
    label: +(t.match(/\((\d+)f\)/) || [0, -1])[1],
    videosSettled: vids.every(v => !v.seeking && v.readyState >= 2),
    videos: vids.length,
    overlays: (c.innerText.match(/Text overlays: (\d+)/) || [0, '?'])[1],
    rect: (() => { const r = c.getBoundingClientRect(); return [r.x, r.y, r.width, r.height]; })(),
    // Anything drawn over the canvas (e.g. a recovery prompt) invalidates the capture.
    obstructed: [...document.querySelectorAll('[role=dialog],[role=alertdialog]')].length > 0 || (() => {
      const r = c.getBoundingClientRect();
      return [[0.5, 0.5], [0.1, 0.1], [0.9, 0.9], [0.1, 0.9], [0.9, 0.1]].some(([fx, fy]) => {
        const e = document.elementFromPoint(r.x + r.width * fx, r.y + r.height * fy);
        return !e || !c.contains(e);
      });
    })(),
  };
})()"""

HIDE = r"""(() => {
  const c = document.querySelector('.canvas-viewport');
  const s = document.createElement('style'); s.id = '__g52_hide';
  document.head.appendChild(s);
  const marked = [];
  for (const e of c.querySelectorAll('*')) {
    const own = [...e.childNodes].filter(n => n.nodeType === 3).map(n => n.textContent).join('');
    if (/^\s*(Seek:|Layers:|Exact frame|Superseded:|Text overlays:|Blank:|Overlay errors:|ROI:|Proxies:|\d\d:\d\d:\d\d:\d\d \(\d+f\))/.test(own)) {
      e.setAttribute('data-g52-hide', '1'); marked.push(e);
      // Also hide the HUD row's own box (it has a translucent background).
      const p = e.parentElement;
      if (p && p !== c && getComputedStyle(p).backgroundColor !== 'rgba(0, 0, 0, 0)') { p.setAttribute('data-g52-hide', '1'); marked.push(p); }
    }
  }
  // Selection handles drawn over the selected layer are editor chrome, not composition.
  for (const e of c.querySelectorAll('[aria-label]')) {
    if (/ on canvas$|^Scale |^Rotate |^Crop .* edge of /.test(e.getAttribute('aria-label'))) { e.setAttribute('data-g52-hide', '1'); marked.push(e); }
  }
  s.textContent = '[data-g52-hide]{visibility:hidden !important}';
  return marked.length;
})()"""

UNHIDE = r"""(() => { document.getElementById('__g52_hide')?.remove();
  document.querySelectorAll('[data-g52-hide]').forEach(e => e.removeAttribute('data-g52-hide')); return true; })()"""


async def screenshot(rect, path):
    async with websockets.connect(ws_url(), max_size=2**27) as ws:
        x, y, w, h = rect
        await ws.send(json.dumps({"id": 1, "method": "Page.captureScreenshot", "params": {
            "format": "png", "clip": {"x": x, "y": y, "width": w, "height": h,
            # CAPTURE_SCALE > 1 only yields real pixels when the page is rendered at a
            # matching device pixel ratio (see set_dpr.py); check sharpness to confirm.
            "scale": float(os.environ.get("CAPTURE_SCALE", "1"))}}}))
        while True:
            m = json.loads(await ws.recv())
            if m.get("id") == 1:
                open(path, "wb").write(base64.b64decode(m["result"]["data"]))
                return


def main():
    out_dir = sys.argv[1]
    os.makedirs(out_dir, exist_ok=True)
    results = []
    for frame in [int(f) for f in sys.argv[2:]]:
        subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "seek_uia.ps1",
                        "-Frame", str(frame)], capture_output=True, text=True, timeout=25)
        deadline, stable, last, state = time.time() + 10, 0, None, None
        while time.time() < deadline:
            state = evaluate(STATE)
            key = (state["label"], state["videosSettled"], state["overlays"])
            stable = stable + 1 if key == last and state["label"] == frame and state["videosSettled"] else 0
            last = key
            if stable >= 6:  # ~0.6 s unchanged after settling
                break
            time.sleep(0.1)
        if state["obstructed"]:
            raise SystemExit(f"frame {frame}: preview canvas is obstructed (dialog or overlay); capture aborted")
        hidden = evaluate(HIDE)
        time.sleep(0.25)
        path = os.path.join(out_dir, f"preview_f{frame}.png")
        asyncio.run(screenshot(state["rect"], path))
        evaluate(UNHIDE)
        results.append({"frame": frame, "settled": stable >= 6, "state": state, "hiddenHudElements": hidden, "file": path})
        print(frame, "settled" if stable >= 6 else "NOT SETTLED", state["videos"], "videos", "overlays", state["overlays"], flush=True)
    json.dump(results, open(os.path.join(out_dir, "preview_capture.json"), "w"), indent=1)


if __name__ == "__main__":
    main()
