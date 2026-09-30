"""Drive the running Keylume window over the WebView2 DevTools protocol (debugging only).

Launch Keylume with WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9334, then:
  python tools/kcdp.py eval "document.title"      (or a .js file)
  python tools/kcdp.py shot out.png
Needs: pip install websocket-client. Relaunch Keylume normally afterwards to close the port.
"""
import json, sys, base64, urllib.request, websocket

def ws():
    pages = json.load(urllib.request.urlopen("http://127.0.0.1:9334/json"))
    p = [x for x in pages if x["type"] == "page"][0]
    return websocket.create_connection(p["webSocketDebuggerUrl"], timeout=120, suppress_origin=True)

_id = [0]
def call(c, method, params=None):
    _id[0] += 1
    c.send(json.dumps({"id": _id[0], "method": method, "params": params or {}}))
    while True:
        m = json.loads(c.recv())
        if m.get("id") == _id[0]:
            return m

if __name__ == "__main__":
    mode = sys.argv[1]
    c = ws()
    if mode == "eval":
        src = open(sys.argv[2], encoding="utf-8").read() if sys.argv[2].endswith(".js") else sys.argv[2]
        r = call(c, "Runtime.evaluate", {"expression": src, "awaitPromise": True, "returnByValue": True})
        res = r.get("result", {})
        if "exceptionDetails" in res:
            print("EXC:", json.dumps(res["exceptionDetails"])[:3000])
        else:
            v = res.get("result", {})
            print(v.get("value", v) if isinstance(v.get("value", None), str) else json.dumps(v.get("value", v), ensure_ascii=False)[:20000])
    elif mode == "shot":
        c.settimeout(25)
        r = call(c, "Page.captureScreenshot", {"format": "png"})
        open(sys.argv[2], "wb").write(base64.b64decode(r["result"]["data"]))
        print("saved", sys.argv[2])

