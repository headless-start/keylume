"""Hover a library card by name with a real mouse move, then capture two frames: hover.py "Card name" out_prefix"""
import base64, json, sys, time
from kcdp import ws, call

name, out = sys.argv[1], sys.argv[2]
c = ws(); c.settimeout(30)
js = f"""(() => {{
  const el = [...document.querySelectorAll('article.card')].find(a => a.querySelector('.name')?.textContent === {json.dumps(name)});
  if (!el) return null; el.scrollIntoView({{block: 'center'}});
  const r = el.getBoundingClientRect(); return JSON.stringify([r.x + r.width / 2, r.y + r.height / 3, r.x, r.y, r.width, r.height]);
}})()"""
time.sleep(0.3)
r = call(c, "Runtime.evaluate", {"expression": js, "returnByValue": True})
v = r["result"]["result"].get("value")
if not v:
    print("card not found"); sys.exit(1)
x, y, cx, cy, cw, ch = json.loads(v)
time.sleep(0.5)
r = call(c, "Runtime.evaluate", {"expression": js, "returnByValue": True})  # position after scroll
x, y, cx, cy, cw, ch = json.loads(r["result"]["result"]["value"])
call(c, "Input.dispatchMouseEvent", {"type": "mouseMoved", "x": 5, "y": 5})
call(c, "Input.dispatchMouseEvent", {"type": "mouseMoved", "x": x, "y": y})
clip = {"x": cx - 4, "y": cy - 4, "width": cw + 8, "height": ch + 8, "scale": 1}
for i, wait in enumerate([0.2, 1.6, 0.5]):
    time.sleep(wait)
    shot = call(c, "Page.captureScreenshot", {"format": "png", "clip": clip})
    open(f"{out}-{i}.png", "wb").write(base64.b64decode(shot["result"]["data"]))
call(c, "Input.dispatchMouseEvent", {"type": "mouseMoved", "x": 5, "y": 5})
print("saved", out)
