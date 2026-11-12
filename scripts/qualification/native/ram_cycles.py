"""G54 RAM measurement (locked definition 2026-11-12).

Samples summed private bytes of tinixcut.exe and all descendants every ~500 ms while
running 30 cycles of: Go to target, Play 2 s, Pause, superseded seek (cancel step).
PASS if peak <= 1536 MiB and mean end-of-cycle for cycles 21-30 <= 1.10 x cycles 1-10.
"""
import json, subprocess, sys, time, threading

out_path = sys.argv[1]
targets = [int(x) for x in sys.argv[2].split(",")]
samples = []
stop = False


def ps(*args):
    try:
        return subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", *args],
                              capture_output=True, text=True, timeout=25).stdout.strip()
    except subprocess.TimeoutExpired:
        return "uia-timeout"


def sampler():
    while not stop:
        raw = ps("proc_tree_mem.ps1")
        try:
            samples.append({"t": time.time(), **json.loads(raw)})
        except ValueError:
            samples.append({"t": time.time(), "error": raw[:200]})
        time.sleep(0.5)


thread = threading.Thread(target=sampler, daemon=True)
thread.start()
time.sleep(3)
cycles = []
for i in range(30):
    a = targets[i % len(targets)]
    b = targets[(i + 7) % len(targets)]
    start = time.time()
    steps = [ps("seek_uia.ps1", "-Frame", str(a))]
    time.sleep(0.5)
    steps.append(ps("ui_action.ps1", "-Action", "play"))
    time.sleep(2.0)
    steps.append(ps("ui_action.ps1", "-Action", "pause"))
    time.sleep(0.3)
    steps.append(ps("seek2_uia.ps1", "-First", str(a + 15), "-Second", str(b)))
    time.sleep(1.5)
    valid = [s for s in samples if "privateMiB" in s]
    end = valid[-1] if valid else None
    cycles.append({"cycle": i + 1, "start": start, "end": time.time(), "steps": steps,
                   "endPrivateMiB": end and end["privateMiB"], "endProcesses": end and end["processes"]})
    print(i + 1, end and end["privateMiB"], end and end["processes"], flush=True)
    json.dump({"cycles": cycles, "samples": samples}, open(out_path, "w"), indent=1)
stop = True
thread.join(timeout=10)
valid = [s for s in samples if "privateMiB" in s]
first = [c["endPrivateMiB"] for c in cycles[:10]]
last = [c["endPrivateMiB"] for c in cycles[20:]]
result = {"peakPrivateMiB": max(s["privateMiB"] for s in valid), "samples": len(valid),
          "meanEnd1to10": sum(first) / len(first), "meanEnd21to30": sum(last) / len(last)}
result["ratio"] = result["meanEnd21to30"] / result["meanEnd1to10"]
result["pass"] = result["peakPrivateMiB"] <= 1536 and result["ratio"] <= 1.10
print(json.dumps(result, indent=1))
json.dump({"result": result, "cycles": cycles, "samples": samples}, open(out_path, "w"), indent=1)
