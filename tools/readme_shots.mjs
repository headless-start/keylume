// README screenshots and clips, taken from the browser preview (simulated keyboard, no
// personal data): node --experimental-websocket tools/readme_shots.mjs [out_dir]
// Needs a Chromium (CHROME=/path, or Playwright's headless shell) and Python with Pillow
// (PYTHON=..., for the animated WebP clips).
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createServer } from "vite";

const OUT = resolve(process.argv[2] ?? "docs/images");
const W = 1280, H = 800, DPR = 2;
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

/** A minimal DevTools protocol client for one page. */
async function devtools(port) {
  let targets = [];
  for (let i = 0; i < 50 && !targets.length; i++) {
    targets = await fetch(`http://127.0.0.1:${port}/json`).then((r) => r.json()).catch(() => []);
    targets = targets.filter((t) => t.type === "page");
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
  const send = (method, params = {}) => new Promise((ok, fail) => {
    waiting.set(++id, { ok, fail });
    ws.send(JSON.stringify({ id, method, params }));
  });
  const js = async (expression) => {
    const r = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error(`${expression}\n${JSON.stringify(r.exceptionDetails).slice(0, 500)}`);
    return r.result.value;
  };
  return { send, js, close: () => ws.close() };
}

const server = await createServer({ server: { port: 1431, strictPort: false, host: "127.0.0.1" }, logLevel: "error" });
await server.listen();
const url = server.resolvedUrls.local[0];
const profileDir = mkdtempSync(join(tmpdir(), "keylume-shots-"));
const port = 9400 + Math.floor(Math.random() * 400);
const chrome = spawn(findChrome(), [
  "--headless", `--remote-debugging-port=${port}`, `--user-data-dir=${profileDir}`, "--no-first-run", "--hide-scrollbars",
  "--force-color-profile=srgb", `--window-size=${W},${H}`, "about:blank",
], { stdio: "ignore" });

const cdp = await devtools(port);
const { send, js } = cdp;
mkdirSync(OUT, { recursive: true });

/** Screenshot the page, or a region of it (cut to its first `rows` rows of cards). */
async function shot(name, clip, rows) {
  if (clip && rows) {
    const card = await rect("article.card");
    clip = { ...clip, x: clip.x - 12, y: clip.y - 12, width: clip.width + 24, height: Math.min(clip.height, rows * (card.height + 14)) + 10 };
  }
  const r = await send("Page.captureScreenshot", { format: "webp", quality: 90, ...(clip ? { clip: { ...clip, scale: 1 } } : {}) });
  writeFileSync(join(OUT, name), Buffer.from(r.data, "base64"));
  console.log("saved", join(OUT, name));
}

/** Record `seconds` of a region as an animated WebP (frames timed by the real clock). */
async function clip(name, region, seconds, fps = 20, scale = 0.75) {
  const dir = mkdtempSync(join(tmpdir(), "keylume-frames-"));
  const times = [];
  const start = performance.now();
  for (let i = 0; performance.now() - start < seconds * 1000; i++) {
    const due = start + (i * 1000) / fps;
    if (performance.now() < due) await sleep(due - performance.now());
    const r = await send("Page.captureScreenshot", { format: "png", clip: { ...region, scale } });
    times.push(performance.now());
    writeFileSync(join(dir, `${String(i).padStart(4, "0")}.png`), Buffer.from(r.data, "base64"));
  }
  const durations = times.map((t, i) => Math.round((times[i + 1] ?? t + 1000 / fps) - t));
  const py = `
import sys, json, os
from PIL import Image
d, out, durs = sys.argv[1], sys.argv[2], json.loads(sys.argv[3])
frames = [Image.open(os.path.join(d, f)).convert("RGB") for f in sorted(os.listdir(d))]
frames[0].save(out, save_all=True, append_images=frames[1:], duration=durs, loop=0, quality=80, method=6, minimize_size=True)
`;
  const r = spawnSync(process.env.PYTHON ?? "python3", ["-c", py, dir, join(OUT, name), JSON.stringify(durations)], { stdio: "inherit" });
  rmSync(dir, { recursive: true, force: true });
  if (r.status !== 0) throw new Error("clip encoding failed (needs Python with Pillow)");
  console.log("saved", join(OUT, name), `(${times.length} frames)`);
}

const rect = (selector) => js(`(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; })()`);
const click = (selector, text) => js(`(() => {
  const el = [...document.querySelectorAll(${JSON.stringify(selector)})].find((e) => ${text ? `e.textContent.trim() === ${JSON.stringify(text)}` : "true"});
  if (!el) throw new Error("not found: ${selector} ${text ?? ""}"); el.click(); })()`);
/** Click the first element matching `selector` whose text is `text`. */
const clickText = (selector, text) => js(`(() => {
  const el = [...document.querySelectorAll(${JSON.stringify(selector)})].find((e) => e.textContent.trim() === ${JSON.stringify(text)});
  if (!el) throw new Error("not found: ${selector} ${text}"); el.click(); })()`);
const rail = (label) => clickText(".rail-item", label);
const tab = (label) => clickText(".lib-tabs [role=tab]", label);
const home = () => js(`document.querySelector(".back-home")?.click()`);
/** From anywhere to the Library. */
async function library() {
  await js(`document.querySelector(".home-actions .btn.primary")?.click()`);
  await sleep(300);
  await rail("Library");
  await sleep(200);
}
const tile = (name) => js(`(() => {
  const el = [...document.querySelectorAll(".ctile")].find((e) => e.querySelector("b")?.textContent === ${JSON.stringify(name)});
  if (!el) throw new Error("no tile ${name}"); el.click(); })()`);
const kind = (value) => js(`(() => {
  const el = document.querySelector(".kind-select");
  Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value").set.call(el, ${JSON.stringify(value)});
  el.dispatchEvent(new Event("change", { bubbles: true })); })()`);

/** Type into a React-controlled input. */
const type = (selector, value) => js(`(() => {
  const el = document.querySelector(${JSON.stringify(selector)});
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(el, ${JSON.stringify(value)});
  el.dispatchEvent(new Event("input", { bubbles: true })); })()`);
const cards = () => js(`[...document.querySelectorAll("article.card")].map((c) => { const r = c.getBoundingClientRect(); return { name: c.querySelector(".name").textContent, x: r.x, y: r.y, width: r.width, height: r.height }; })`);
const mouse = (x, y) => send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
const openFirstDesign = () => js(`document.querySelector("article.card .card-main").click()`);

/** Look like the desktop app on a real keyboard: no browser-preview badge, a USB connection. */
const DESKTOP_LOOK = `(() => {
  if (!document.getElementById("shots-look")) document.head.insertAdjacentHTML("beforeend", "<style id=shots-look>.pill.warn { display: none !important }</style>");
  for (const el of document.querySelectorAll(".top-device small, .home-info .device-status")) {
    const dot = el.querySelector(".dot");
    el.textContent = "Connected";
    if (dot) el.prepend(dot);
  }
})()`;
const look = () => js(DESKTOP_LOOK);

const scenes = {
  async home() {
    await home();
    await sleep(1200);
    await look();
    await shot("home.webp");
  },
  async discover() {
    await tab("Discover");
    await sleep(900);
    await look();
    await shot("discover.webp");
  },
  async library() {
    await tab("Comics");
    await tile("Heroes");
    await sleep(500);
    await openFirstDesign();
    await sleep(1200);
    await look();
    await shot("library.webp");
  },
  async hover() {
    await tab("Games");
    await tile("CS2 Skins");
    await kind("effect");
    await sleep(700);
    const row = (await cards()).slice(0, 3);
    const region = { x: row[0].x - 12, y: row[0].y - 12, width: row[2].x + row[2].width - row[0].x + 24, height: row[0].height + 24 };
    const at = (c) => mouse(c.x + c.width / 2, c.y + c.height / 3);
    await at(row[1]);
    const recording = clip("library-hover.webp", region, 7);
    await sleep(3500);
    await at(row[2]);
    await recording;
    await mouse(5, 5);
  },
  async create() {
    await rail("Create");
    await sleep(300);
    await click("[aria-label='What to create'] [role=tab]", "Animated");
    await sleep(300);
    await click(".mode", "Wave");
    await sleep(200);
    await js(`[...document.querySelectorAll("label")].find((l) => l.textContent.includes("Rainbow"))?.querySelector("input")?.click()`);
    await sleep(1200);
    await look();
    await shot("create.webp");
  },
  async games() {
    await tab("Games");
    await tile("Dota 2");
    await sleep(500);
    await openFirstDesign();
    await sleep(1200);
    await look();
    await shot("games.webp");
  },
  async flags() {
    await tab("Flags");
    await tile("Europe");
    await sleep(700);
    await shot("flags.webp", await rect(".grid"), 3);
  },
};

try {
  await send("Emulation.setDeviceMetricsOverride", { width: W, height: H, deviceScaleFactor: DPR, mobile: false });
  await send("Page.navigate", { url });
  for (let i = 0; i < 100 && !(await js("!!document.querySelector('.home')")); i++) await sleep(100);
  // something on the keyboard, so Home and the side panel show it
  await library();
  await type("input[type=search]", "northern lights");
  await sleep(500);
  await click("article.card .name", "Northern Lights");
  await type("input[type=search]", "");
  await sleep(800);
  const only = process.env.ONLY?.split(",");
  for (const [name, run] of Object.entries(scenes)) {
    if (!only || only.includes(name)) {
      if (name !== "home") {
        // the Library remembers its view: start each scene from Discover
        await library();
        await tab("Discover");
        await kind("any");
        await type("input[type=search]", "");
        await sleep(300);
      }
      await run();
    }
  }
} finally {
  cdp.close();
  chrome.kill();
  await server.close();
  rmSync(profileDir, { recursive: true, force: true });
}
