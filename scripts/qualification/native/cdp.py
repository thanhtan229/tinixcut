import asyncio, json, sys, urllib.request, websockets
def ws_url():
    pages=json.load(urllib.request.urlopen("http://127.0.0.1:9333/json/list"))
    return [p for p in pages if p["type"]=="page" and ("1420" in p["url"] or p["url"].startswith(("tauri:","http://tauri.localhost","https://tauri.localhost")))][0]["webSocketDebuggerUrl"]
async def ev(expr, url=None):
    async with websockets.connect(url or ws_url(), max_size=2**25) as ws:
        await ws.send(json.dumps({"id":1,"method":"Runtime.evaluate","params":{"expression":expr,"returnByValue":True,"awaitPromise":True}}))
        while True:
            m=json.loads(await ws.recv())
            if m.get("id")==1:
                r=m["result"]
                if "exceptionDetails" in r: return {"__error__": r["exceptionDetails"].get("text"), "detail": r.get("result",{}).get("description")}
                return r["result"].get("value")
def evaluate(expr): return asyncio.run(ev(expr))
if __name__=="__main__":
    print(json.dumps(evaluate(sys.argv[1]), indent=1))
