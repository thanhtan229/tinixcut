"""G54 UI-response measurement (locked definition 2026-11-12).

For each action, the page is armed to catch the native input event (click; `change`
for Go-to-frame), then polls the result condition every animation frame. Latency is
the event timeStamp to the second rAF after the result is first visible.
"""
import json, subprocess, sys, time, os, math
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cdp import evaluate

ARM = r"""((kind, predSrc) => {
  const pred = new Function(predSrc);
  window.__ui = {kind, result: null};
  // Go-to-frame: timing starts at the commit (field blur after a change, or form submit).
  // Clip select: the Enter keydown on the focused clip. Others: the button click.
  const types = kind === 'go' ? ['change', 'focusout', 'submit'] : kind === 'select' ? ['keydown'] : ['click'];
  let changed = false;
  const onEvent = (e) => {
    if (kind === 'go') {
      if (e.type === 'change') { changed = true; return; }
      if (!changed || (e.type === 'focusout' && e.target.id !== 'preview-frame-seek')) return;
    }
    if (kind === 'select' && e.key !== 'Enter') return;
    types.forEach(t => document.removeEventListener(t, onEvent, true));
    const t0 = e.timeStamp;
    const poll = (t1) => {
      if (pred()) { requestAnimationFrame((t2) => { window.__ui.result = {t0, t1, t2, latencyMs: t2 - t0, event: e.type}; }); }
      else if (t1 - t0 > 5000) { window.__ui.result = {t0, timeout: true}; }
      else requestAnimationFrame(poll);
    };
    requestAnimationFrame(poll);
  };
  types.forEach(t => document.addEventListener(t, onEvent, true));
  return true;
})"""
LABEL = "(+(document.body.innerText.match(/\\((\\d+)f\\)/)||[0,-1])[1])"
BUTTON_SHOWS = "return [...document.querySelectorAll('button')].some(b=>/^\\S+ %s$/.test(b.innerText.trim()))"


def ps(*args):
    try:
        return subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", *args],
                              capture_output=True, text=True, timeout=25).stdout.strip()
    except subprocess.TimeoutExpired:
        return "uia-timeout"


def measure(kind, pred, trigger):
    evaluate(f"{ARM}({json.dumps(kind)}, {json.dumps(pred)})")
    out = trigger()
    for _ in range(60):
        r = evaluate("window.__ui && window.__ui.result")
        if r:
            return {"kind": kind, **r, "trigger": out}
        time.sleep(0.1)
    return {"kind": kind, "timeout": True, "trigger": out, "note": "no input event observed"}


def p95(v):
    v = sorted(v)
    return v[math.ceil(0.95 * len(v)) - 1] if v else None


def main():
    out_path = sys.argv[1]
    clips = sys.argv[2].split(",")
    targets = [int(x) for x in sys.argv[3].split(",")]
    n = int(os.environ.get("UI_SAMPLES", "20"))
    results = []
    for i in range(n):
        results.append(measure("play", BUTTON_SHOWS % "Pause", lambda: ps("ui_action.ps1", "-Action", "play")))
        time.sleep(0.6)
        results.append(measure("pause", BUTTON_SHOWS % "Play", lambda: ps("ui_action.ps1", "-Action", "pause")))
        time.sleep(0.3)
        before = evaluate(LABEL)
        results.append(measure("step", f"return {LABEL} === {before + 1}", lambda: ps("ui_action.ps1", "-Action", "step")))
        target = targets[i % len(targets)]
        if target == evaluate(LABEL):
            target += 30
        results.append(measure("go", f"return {LABEL} === {target}", lambda t=target: ps("seek_uia.ps1", "-Frame", str(t))))
        clip = clips[i % len(clips)]
        pred = ("return [...document.querySelectorAll('[data-timeline-clip]')].some(e=>{const l=e.getAttribute('aria-label');"
                f"return l.startsWith({json.dumps(clip + ',')}) && l.includes(', selected,');}})")
        results.append(measure("select", pred, lambda c=clip: ps("ui_action.ps1", "-Action", "select", "-ClipName", c)))
        json.dump(results, open(out_path, "w"), indent=1)
        print(i, [(r["kind"], round(r["latencyMs"], 1) if "latencyMs" in r else "FAIL") for r in results[-5:]], flush=True)
    summary = {}
    for k in ["play", "pause", "step", "go", "select"]:
        v = [r["latencyMs"] for r in results if r["kind"] == k and "latencyMs" in r]
        summary[k] = {"n": len(v), "failures": sum(1 for r in results if r["kind"] == k and "latencyMs" not in r),
                      "p50": sorted(v)[len(v) // 2] if v else None, "p95": p95(v), "max": max(v) if v else None,
                      "pass": bool(v) and len(v) == n and p95(v) <= 100}
    print(json.dumps(summary, indent=1))
    json.dump({"summary": summary, "samples": results}, open(out_path, "w"), indent=1)


main()
