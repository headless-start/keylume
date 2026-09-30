// Screenshots of the UI in the browser preview (simulated keyboard), driven by steps:
//   node --experimental-websocket tools/ui_shots.mjs OUT_DIR [--size 1280x800] STEP...
// Steps: `nav:Label` clicks the nav button with that label, `click:Text` clicks the first
// button/link/tab whose text starts with Text, `type:selector=text` fills an input,
// `hover:Text` hovers a card by name, `key:Name` presses a key, `wait:MS`, `shot:name`
// saves OUT_DIR/name.png (`shot:name@selector` only that element, e.g. `shot:cards@.grid`),
// `eval:js` runs JavaScript in the page and prints the result, `media:feature=value` emulates
// a CSS media feature (`media:prefers-reduced-motion=reduce` is Windows with animation effects off).
// Same Chromium lookup as readme_shots.mjs (CHROME=/path, or Playwright's headless shell).
// Screenshots are for looking at, then deleting: keep them out of the repo.
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createServer } from "vite";

const args = process.argv.slice(2);
const OUT = resolve(args.shift() ?? ".");
let [W, H] = [1280, 800];
if (args[0] === "--size") [W, H] = args.splice(0, 2)[1].split("x").map(Number);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function findChrome() {
  if (process.env.CHROME) return process.env.CHROME;
  const cache = join(homedir(), ".cache/ms-playwright");
  for (const d of existsSync(cache) ? readdirSync(cache).sort().reverse() : []) {
    for (const exe of ["chrome-headless-shell-linux64/chrome-headless-shell", "chrome-linux64/chrome"]) {
      if (existsSync(join(cache, d, exe))) return join(cache, d, exe);
    }
  }
  throw new Error("no Chromium found: set CHROME=/path/to/chrome");
}

const server = await createServer({ server: { port: 1433, strictPort: false, host: "127.0.0.1" }, logLevel: "error" });
await server.listen();
const profileDir = mkdtempSync(join(tmpdir(), "keylume-ui-"));
const port = 9400 + Math.floor(Math.random() * 400);
const chrome = spawn(findChrome(), [
  "--headless", `--remote-debugging-port=${port}`, `--user-data-dir=${profileDir}`, "--no-first-run", "--hide-scrollbars",
  "--force-color-profile=srgb", `--window-size=${W},${H}`, "about:blank",
], { stdio: "ignore" });

let targets = [];
for (let i = 0; i < 50 && !targets.length; i++) {
  targets = (await fetch(`http://127.0.0.1:${port}/json`).then((r) => r.json()).catch(() => [])).filter((t) => t.type === "page");
  if (!targets.length) await sleep(100);
}
if (!targets.length) throw new Error("Chromium didn't start (missing libraries? set CHROME=/path/to/chrome)");
const ws = new WebSocket(targets[0].webSocketDebuggerUrl);
await new Promise((ok, fail) => { ws.onopen = ok; ws.onerror = fail; });
let id = 0;
const waiting = new Map();
ws.onmessage = (m) => {
  const msg = JSON.parse(m.data);
  const w = waiting.get(msg.id);
  if (w) { waiting.delete(msg.id); msg.error ? w.fail(new Error(msg.error.message)) : w.ok(msg.result); }
};
// every command gets 20 s: a page that stops answering says which step it was on, instead of hanging
const send = (method, params = {}) => new Promise((ok, fail) => {
  const n = ++id;
  const t = setTimeout(() => { waiting.delete(n); fail(new Error(`${method} got no answer in 20 s`)); }, 20000);
  waiting.set(n, { ok: (v) => { clearTimeout(t); ok(v); }, fail: (e) => { clearTimeout(t); fail(e); } });
  ws.send(JSON.stringify({ id: n, method, params }));
});
const js = async (expression) => {
  const r = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(`${expression}\n${JSON.stringify(r.exceptionDetails).slice(0, 400)}`);
  return r.result.value;
};

/** Click the first visible element matching `sel` whose text starts with `text`. */
const clickText = (sel, text) => js(`(() => {
  const t = ${JSON.stringify(text.toLowerCase())};
  const el = [...document.querySelectorAll(${JSON.stringify(sel)})].find((e) => e.offsetParent && e.textContent.trim().toLowerCase().startsWith(t)
    || e.getAttribute("aria-label")?.toLowerCase().startsWith(t));
  if (!el) return false;
  el.scrollIntoView({ block: "center" }); el.click(); return true;
})()`);

try {
  await send("Emulation.setDeviceMetricsOverride", { width: W, height: H, deviceScaleFactor: 1, mobile: false });
  await send("Page.enable");
  await send("Page.navigate", { url: server.resolvedUrls.local[0] });
  for (let i = 0; i < 100 && !(await js("!!document.querySelector('.app, main')").catch(() => false)); i++) await sleep(100);
  await sleep(800);
  mkdirSync(OUT, { recursive: true });
  for (const step of args) {
    console.error(`step: ${step}`);
    const [op, ...rest] = step.split(":");
    const arg = rest.join(":");
    let ok = true;
    if (op === "nav") ok = await clickText("nav button, nav a, [role=tab], .nav", arg);
    else if (op === "click") ok = await clickText("button, a, [role=tab], summary, label", arg);
    else if (op === "type") {
      const at = arg.indexOf("="); // the text may contain "=", the selector's first one splits them
      const [sel, text] = at < 0 ? [arg, ""] : [arg.slice(0, at), arg.slice(at + 1)];
      ok = await js(`(() => { const el = document.querySelector(${JSON.stringify(sel)}); if (!el) return false;
        const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set; set.call(el, ${JSON.stringify(text ?? "")});
        el.dispatchEvent(new Event("input", { bubbles: true })); return true; })()`);
    } else if (op === "hover") {
      const r = await js(`(() => { const c = [...document.querySelectorAll("article.card, .card")].find((e) => e.textContent.includes(${JSON.stringify(arg)}));
        if (!c) return null; c.scrollIntoView({ block: "center" }); const b = c.getBoundingClientRect(); return { x: b.x + b.width / 2, y: b.y + b.height / 3 }; })()`);
      ok = !!r;
      if (r) await send("Input.dispatchMouseEvent", { type: "mouseMoved", x: r.x, y: r.y });
    } else if (op === "key") {
      await send("Input.dispatchKeyEvent", { type: "keyDown", key: arg, code: arg });
      await send("Input.dispatchKeyEvent", { type: "keyUp", key: arg, code: arg });
    } else if (op === "media") {
      const [name, value] = arg.split("=");
      await send("Emulation.setEmulatedMedia", { features: [{ name, value }] });
    } else if (op === "wait") await sleep(Number(arg));
    else if (op === "eval") console.log(JSON.stringify(await js(arg)));
    else if (op === "shot") {
      await sleep(250);
      const [name, sel] = arg.split("@");
      const clip = sel && await js(`(() => { const e = document.querySelector(${JSON.stringify(sel)}); if (!e) return null;
        const b = e.getBoundingClientRect(); return { x: b.x, y: b.y, width: b.width, height: b.height, scale: 1 }; })()`);
      if (sel && !clip) ok = false;
      const r = await send("Page.captureScreenshot", { format: "png", ...(clip ? { clip } : {}) });
      writeFileSync(join(OUT, `${name}.png`), Buffer.from(r.data, "base64"));
      console.log("saved", join(OUT, `${name}.png`));
    } else throw new Error(`unknown step ${step}`);
    if (!ok) console.log(`step not found: ${step}`);
    if (op === "nav" || op === "click") await sleep(400);
  }
} finally {
  ws.close();
  chrome.kill();
  await server.close();
  rmSync(profileDir, { recursive: true, force: true });
}
