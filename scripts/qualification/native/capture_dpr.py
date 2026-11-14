"""Capture the preview canvas at an exact device pixel ratio (G52 Step 0/1).

Overrides the device metrics for the session (window size kept, deviceScaleFactor = DPR), then
clips the preview viewport and records the geometry the capture used: border-box rect, border
widths, device size of the PNG. The override is removed before exit.

Usage: python -I capture_dpr.py <out_dir> <dpr> <name> [--padding]
  --padding clips to the padding box (inside the 1 px border) instead of the border box.
Writes <out_dir>/<name>_dpr<dpr>.png and .json
"""
import asyncio, base64, json, os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import websockets
from cdp import ws_url
from capture_preview import HIDE, UNHIDE

GEOM = r"""(() => {
  const c = document.querySelector('.canvas-viewport');
  const r = c.getBoundingClientRect(); const cs = getComputedStyle(c);
  return {rect: [r.x, r.y, r.width, r.height], dpr: window.devicePixelRatio,
    border: [parseFloat(cs.borderLeftWidth), parseFloat(cs.borderTopWidth)], radius: cs.borderRadius,
    inner: [window.innerWidth, window.innerHeight]};
})()"""


async def main():
    out_dir, dpr, name = sys.argv[1], float(sys.argv[2]), sys.argv[3]
    padding = "--padding" in sys.argv
    os.makedirs(out_dir, exist_ok=True)
    async with websockets.connect(ws_url(), max_size=2**27) as ws:
        n = 0

        async def call(method, params=None):
            nonlocal n
            n += 1
            my = n
            await ws.send(json.dumps({"id": my, "method": method, "params": params or {}}))
            while True:
                m = json.loads(await ws.recv())
                if m.get("id") == my:
                    return m.get("result", m)

        async def js(expr):
            r = await call("Runtime.evaluate", {"expression": expr, "returnByValue": True})
            return r["result"].get("value")

        base = await js("({w: window.innerWidth, h: window.innerHeight})")
        await call("Emulation.setDeviceMetricsOverride", {"width": base["w"], "height": base["h"],
                   "deviceScaleFactor": dpr, "mobile": False})
        time.sleep(1.5)
        geom = await js(GEOM)
        await js(HIDE)
        # radius would round the corners; remove it for the measurement only
        await js("(() => { const s=document.createElement('style'); s.id='__g52_r'; "
                 "s.textContent='.canvas-viewport{border-radius:0 !important;box-shadow:none !important}'; "
                 "document.head.appendChild(s); })()")
        time.sleep(0.4)
        x, y, w, h = geom["rect"]
        bx, by = geom["border"]
        clip = ([x + bx, y + by, w - 2 * bx, h - 2 * by] if padding else [x, y, w, h])
        shot = await call("Page.captureScreenshot", {"format": "png", "clip": {
            "x": clip[0], "y": clip[1], "width": clip[2], "height": clip[3], "scale": 1}})
        await js("document.getElementById('__g52_r')?.remove()")
        await js(UNHIDE)
        await call("Emulation.clearDeviceMetricsOverride")
        stem = os.path.join(out_dir, f"{name}_dpr{dpr:g}{'_pad' if padding else ''}")
        open(stem + ".png", "wb").write(base64.b64decode(shot["data"]))
        geom["clip"] = clip
        json.dump(geom, open(stem + ".json", "w"), indent=1)
        print(json.dumps(geom))

asyncio.run(main())
