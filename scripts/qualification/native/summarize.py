import json, sys, math
for fn in sys.argv[1:]:
    R=json.load(open(fn)); ok=[r for r in R if r["status"]=="synchronized"]
    def p95(v):
        v=sorted(v); return v[math.ceil(0.95*len(v))-1] if v else None
    def med(v):
        v=sorted(v); return v[len(v)//2] if v else None
    go=[r["latencyMs"] for r in ok]; ip=[r.get("seekRequestToSeekedMs") for r in ok if r.get("seekRequestToSeekedMs") is not None]
    print(fn, f"{len(ok)}/{len(R)} sync", "fail:",[ (r['frame'],r['status']) for r in R if r['status']!='synchronized'],
          "stall:",sum(r.get('stallRecoveries',0) or 0 for r in R),
          "go p50/p95/max:",med(go),p95(go),max(go) if go else None, "inpage p50/p95/max:",med(ip),p95(ip),max(ip) if ip else None,
          "maxMediaErr:", max((abs(v['t']-list(r['seekTargetsSec'].values())[0]) for r in ok for v in r['videoState']), default=None))
