import asyncio, json, subprocess, sys, os, collections
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import websockets
from cdp import ws_url
async def main(frame, out):
    async with websockets.connect(ws_url(), max_size=2**28) as ws:
        n=[0]
        async def call(method, params=None):
            n[0]+=1; i=n[0]; await ws.send(json.dumps({"id":i,"method":method,"params":params or {}}))
            while True:
                m=json.loads(await ws.recv())
                if m.get("id")==i: return m.get("result")
        await call("Profiler.enable"); await call("Profiler.setSamplingInterval",{"interval":500}); await call("Profiler.start")
        await asyncio.get_event_loop().run_in_executor(None, lambda: subprocess.run(["powershell","-NoProfile","-ExecutionPolicy","Bypass","-File","seek_uia.ps1","-Frame",str(frame)],capture_output=True,timeout=30))
        await asyncio.sleep(6)
        prof=(await call("Profiler.stop"))["profile"]
    json.dump(prof,open(out,"w"))
    nodes={x["id"]:x for x in prof["nodes"]}
    dt=collections.Counter()
    for sid,d in zip(prof["samples"],prof["timeDeltas"]): dt[sid]+=d
    selft=collections.Counter(); total=collections.Counter()
    parent={c:x["id"] for x in prof["nodes"] for c in x.get("children",[])}
    for nid,t in dt.items():
        f=nodes[nid]["callFrame"]; key=f"{f['functionName'] or '(anon)'} {f['url'].split('/')[-1].split('?')[0]}:{f['lineNumber']+1}"
        selft[key]+=t
        seen=set(); cur=nid
        while cur in nodes:
            f=nodes[cur]["callFrame"]; k=f"{f['functionName'] or '(anon)'} {f['url'].split('/')[-1].split('?')[0]}:{f['lineNumber']+1}"
            if k not in seen: total[k]+=t; seen.add(k)
            cur=parent.get(cur)
    print("TOP SELF (ms)"); [print(round(t/1000), k) for k,t in selft.most_common(15)]
    print("TOP TOTAL (ms)"); [print(round(t/1000), k) for k,t in total.most_common(40) if not k.startswith(('(root)','(program)','(idle)'))]
asyncio.run(main(int(sys.argv[1]), sys.argv[2]))
