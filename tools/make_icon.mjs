// The app icon from the logo: renders src/assets/logo.svg with headless Chromium to a
// transparent PNG (1024 px by default), then `npx tauri icon` makes every size from it.
//   LD_LIBRARY_PATH=… node --experimental-websocket tools/make_icon.mjs [out.png] [size]
//   npx tauri icon src-tauri/app-icon.png
// Same Chromium lookup as ui_shots.mjs (CHROME=/path, or Playwright's headless shell).
import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";

const OUT = resolve(process.argv[2] ?? "src-tauri/app-icon.png");
const SIZE = Number(process.argv[3] ?? 1024);
const svg = readFileSync("src/assets/logo.svg", "utf8");
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

const profile = mkdtempSync(join(tmpdir(), "keylume-icon-"));
const port = 9800 + Math.floor(Math.random() * 100);
const chrome = spawn(findChrome(), [
  "--headless", `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`, "--no-first-run", "--hide-scrollbars", "--force-color-profile=srgb", "about:blank",
], { stdio: "ignore" });
try {
  let targets = [];
  for (let i = 0; i < 50 && !targets.length; i++) {
    targets = (await fetch(`http://127.0.0.1:${port}/json`).then((r) => r.json()).catch(() => [])).filter((t) => t.type === "page");
    if (!targets.length) await sleep(100);
  }
  if (!targets.length) throw new Error("Chromium didn't start");
  const ws = new WebSocket(targets[0].webSocketDebuggerUrl);
  await new Promise((ok, fail) => { ws.onopen = ok; ws.onerror = fail; });
  let id = 0;
  const waiting = new Map();
  ws.onmessage = (m) => { const msg = JSON.parse(m.data); waiting.get(msg.id)?.(msg); waiting.delete(msg.id); };
  const send = (method, params = {}) => new Promise((ok) => { waiting.set(++id, ok); ws.send(JSON.stringify({ id, method, params })); });
  await send("Emulation.setDeviceMetricsOverride", { width: SIZE, height: SIZE, deviceScaleFactor: 1, mobile: false });
  await send("Emulation.setDefaultBackgroundColorOverride", { color: { r: 0, g: 0, b: 0, a: 0 } });
  const html = `<!doctype html><html><body style="margin:0;overflow:hidden;background:transparent">${svg.replace("<svg ", `<svg style="display:block" width="${SIZE}" height="${SIZE}" `)}</body></html>`;
  await send("Page.navigate", { url: `data:text/html;base64,${Buffer.from(html).toString("base64")}` });
  await sleep(600);
  const shot = await send("Page.captureScreenshot", { format: "png", clip: { x: 0, y: 0, width: SIZE, height: SIZE, scale: 1 } });
  writeFileSync(OUT, Buffer.from(shot.result.data, "base64"));
  console.log("saved", OUT);
  ws.close();
} finally {
  const exited = new Promise((r) => chrome.once("exit", r));
  chrome.kill();
  await exited;
  rmSync(profile, { recursive: true, force: true, maxRetries: 5 });
}
