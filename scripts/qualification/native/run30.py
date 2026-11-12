import json, subprocess, sys, time
import os; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cdp import evaluate
FPS=30
targets=[int(a) for a in sys.argv[2:]] if len(sys.argv)>2 else list(range(300,17701,600))
out=sys.argv[1] if len(sys.argv)>1 else "run30.json"
READ="""(()=>{const d=window.__tinixPreviewVideoDiagnostics; const ev=d?d.read():[];
 const vids=[...document.querySelectorAll('video')].map(v=>({t:v.currentTime,seeking:v.seeking,rs:v.readyState,src:v.currentSrc.slice(-40)}));
 const m=document.body.innerText.match(/\((\d+)f\)/); const err=[...document.querySelectorAll('[role=alert]')].map(e=>e.innerText);
 return {origin:performance.timeOrigin, ev, vids, label:m?+m[1]:null, err}})()"""
results=[]
PROBE="""(()=>{const el=document.getElementById('preview-frame-seek'); if(el.__probed) return 1; el.__probed=1; const form=el.form; const go=form.querySelector('button[type=submit]');
window.__p=[]; const rec=(t)=>e=>window.__p.push([t,el.value,(performance.now()|0)]);
['input','change','focus','blur'].forEach(t=>el.addEventListener(t,rec(t),{capture:true}));
go.addEventListener('click',rec('go-click'),{capture:true}); form.addEventListener('submit',rec('submit'),{capture:true}); return 1})()"""
evaluate(PROBE)
if "--from-zero" in sys.argv or True:
    subprocess.run(["powershell","-NoProfile","-ExecutionPolicy","Bypass","-File","seek_uia.ps1","-Frame","0"],capture_output=True,timeout=20); time.sleep(1.5)
    print("reset label", evaluate(READ)["label"], flush=True)
for f in targets:
    evaluate("window.__p=[]")
    ok_clear=evaluate("(()=>{const d=window.__tinixPreviewVideoDiagnostics; if(d){d.clear(); return d.read().length===0} return true})()")
    try:
        u=json.loads(subprocess.run(["powershell","-NoProfile","-ExecutionPolicy","Bypass","-File","seek_uia.ps1","-Frame",str(f)],capture_output=True,text=True,timeout=20).stdout.strip().splitlines()[-1])
    except Exception as exc:
        r={"frame":f,"status":"uia-error","error":repr(exc)[:300]}; results.append(r); print(f,"uia-error",flush=True); json.dump(results,open(out,"w"),indent=1); continue
    t0=u["invokeEpochMs"]; deadline=time.time()+6.0; status="timeout"; snap=None
    while time.time()<deadline:
        snap=evaluate(READ)
        req={}; done={}; stall=0
        for e in snap["ev"]:
            ep=snap["origin"]+e["atMs"]
            if e["event"]=="seek-request": req[e["videoKey"]]=(ep,e["targetTimeSec"]); done.pop(e["videoKey"],None)
            elif e["event"]=="seeked" and e["videoKey"] in req: done[e["videoKey"]]=ep
            elif e["event"]=="seek-stall-recovery": stall+=1
        vids_ok = snap["vids"] and all(not v["seeking"] for v in snap["vids"])
        if snap["label"]==f and req and set(done)==set(req) and vids_ok:
            status="synchronized"; break
        time.sleep(0.05)
    lat=max(done.values())-t0 if status=="synchronized" else None
    inpage=max(done[k]-req[k][0] for k in done) if status=="synchronized" else None
    deltas={k:None for k in req}
    for k,(ep,tgt) in req.items(): deltas[k]=tgt
    r={"frame":f,"status":status,"readback":u["readback"],"label":snap["label"] if snap else None,"bufferCleared":ok_clear,
       "windowState":u.get("windowState"),"latencyMs":round(lat,1) if lat is not None else None,"uiaInvokeMs":u["invokeReturnedEpochMs"]-t0,"seekRequestToSeekedMs":round(inpage,1) if inpage is not None else None,
       "videos":len(req),"stallRecoveries":stall,"videoState":snap["vids"] if snap else None,"seekTargetsSec":deltas,"alerts":snap["err"] if snap else None,
       "events":snap["ev"] if snap else None,"inputEvents":evaluate("window.__p")}
    results.append(r); print(f, status, u.get("windowState"), r["latencyMs"], r["seekRequestToSeekedMs"], "videos",len(req),"stall",stall, flush=True)
    json.dump(results,open(out,"w"),indent=1)
    time.sleep(0.3)
