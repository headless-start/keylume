"""Screenshot the Keylume window at an emulated size: kshot_size.py W H out.png [js]"""
import base64, sys
from kcdp import ws, call

w, h, out = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
c = ws()
c.settimeout(30)
call(c, "Emulation.setDeviceMetricsOverride", {"width": w, "height": h, "deviceScaleFactor": 1, "mobile": False})
if len(sys.argv) > 4:
    call(c, "Runtime.evaluate", {"expression": sys.argv[4], "awaitPromise": True})
import time; time.sleep(1.5)
r = call(c, "Page.captureScreenshot", {"format": "png"})
open(out, "wb").write(base64.b64decode(r["result"]["data"]))
call(c, "Emulation.clearDeviceMetricsOverride")
print("saved", out)
