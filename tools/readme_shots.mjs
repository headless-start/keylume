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

/** The page's clock under the recorder's control: `__vt.step(ms)` moves time on by exactly
 * `ms` (timers, animation frames and the drawn pointer), so every frame of a clip is evenly
 * spaced however long a screenshot takes. `__vt.release()` hands time back to the browser. */
const VIRTUAL_TIME = `(() => {
  if (window.__vt) return;
  const real = { now: performance.now.bind(performance), date: Date.now, raf: requestAnimationFrame.bind(window),
    caf: cancelAnimationFrame.bind(window), st: setTimeout.bind(window), ct: clearTimeout.bind(window),
    si: setInterval.bind(window), ci: clearInterval.bind(window) };
  let now = real.now();
  const dateBase = real.date() - now;
  let rafs = new Map(), rafId = 0, id = 1e7;
  const timers = new Map();
  let ptr = null;
  performance.now = () => now;
  Date.now = () => Math.round(dateBase + now);
  window.requestAnimationFrame = (cb) => { rafs.set(++rafId, cb); return rafId; };
  window.cancelAnimationFrame = (i) => rafs.delete(i);
  window.setTimeout = (cb, ms = 0, ...a) => { timers.set(++id, { at: now + Math.max(0, +ms || 0), cb: () => cb(...a) }); return id; };
  window.setInterval = (cb, ms = 0, ...a) => { const every = Math.max(1, +ms || 0); timers.set(++id, { at: now + every, every, cb: () => cb(...a) }); return id; };
  window.clearTimeout = window.clearInterval = (i) => timers.delete(i);
  const ease = (x) => 1 - Math.pow(1 - x, 3);
  window.__vt = {
    pointTo(x, y, ms = 550) {
      let el = document.getElementById("shots-pointer");
      if (!el) {
        document.body.insertAdjacentHTML("beforeend", '<div id="shots-pointer" style="position:fixed;left:0;top:0;z-index:99999;pointer-events:none"><svg width="22" height="26" viewBox="0 0 22 26"><path d="M2 2l0 19 5-5 4 8 3-1.5-4-8h7z" fill="#fff" stroke="#111" stroke-width="1.6" stroke-linejoin="round"/></svg></div>');
        el = document.getElementById("shots-pointer");
        ptr = { x0: innerWidth / 2, y0: innerHeight * 0.6, x1: innerWidth / 2, y1: innerHeight * 0.6, t0: now, ms: 1 };
      }
      const at = ptr ? this.pointer() : { x: x, y: y };
      ptr = { x0: at.x, y0: at.y, x1: x, y1: y, t0: now, ms };
    },
    pointer() {
      if (!ptr) return null;
      const k = ease(Math.min(1, (now - ptr.t0) / ptr.ms));
      return { x: ptr.x0 + (ptr.x1 - ptr.x0) * k, y: ptr.y0 + (ptr.y1 - ptr.y0) * k };
    },
    async step(ms) {
      const end = now + ms;
      for (;;) {
        let next = null;
        for (const e of timers) if (e[1].at <= end && (!next || e[1].at < next[1].at)) next = e;
        if (!next) break;
        const [i, t] = next;
        now = Math.max(now, t.at);
        if (t.every) t.at += t.every; else timers.delete(i);
        try { t.cb(); } catch (err) { console.error(err); }
        await new Promise((r) => real.st(r, 0));
      }
      now = end;
      const cbs = [...rafs.values()];
      rafs = new Map();
      for (const cb of cbs) { try { cb(now); } catch (err) { console.error(err); } }
      const p = this.pointer();
      const el = document.getElementById("shots-pointer");
      if (p && el) el.style.transform = "translate(" + p.x + "px, " + p.y + "px)";
      await new Promise((r) => real.raf(() => real.raf(r)));
    },
    release() {
      const left = [...timers.values()];
      Object.assign(performance, { now: real.now });
      Date.now = real.date;
      Object.assign(window, { requestAnimationFrame: real.raf, cancelAnimationFrame: real.caf, setTimeout: real.st,
        clearTimeout: real.ct, setInterval: real.si, clearInterval: real.ci });
      for (const t of left) t.every ? real.si(t.cb, t.every) : real.st(t.cb, Math.max(0, t.at - now));
      for (const cb of rafs.values()) real.raf(cb);
      document.getElementById("shots-pointer")?.remove();
      delete window.__vt;
    },
  };
})()`;

/** Film a region as an animated WebP, in controlled time: `steps` are [ms, action] pairs run
 * when the clip reaches that moment (moving the pointer, clicking, putting a design on). */
async function film(name, region, seconds, steps = [], fps = 20, scale = 0.55) {
  const dir = mkdtempSync(join(tmpdir(), "keylume-frames-"));
  await js(VIRTUAL_TIME);
  const todo = [...steps].sort((a, b) => a[0] - b[0]);
  const frames = Math.round(seconds * fps);
  try {
    for (let i = 0; i < frames; i++) {
      while (todo.length && todo[0][0] <= (i * 1000) / fps) await todo.shift()[1]();
      await js(`__vt.step(${1000 / fps})`);
      const r = await send("Page.captureScreenshot", { format: "png", clip: { ...region, scale } });
      writeFileSync(join(dir, `${String(i).padStart(4, "0")}.png`), Buffer.from(r.data, "base64"));
    }
  } finally {
    await js(`window.__vt?.release()`);
  }
  const py = `
import sys, os
from PIL import Image
d, out, ms = sys.argv[1], sys.argv[2], int(sys.argv[3])
frames = [Image.open(os.path.join(d, f)).convert("RGB") for f in sorted(os.listdir(d))]
frames[0].save(out, save_all=True, append_images=frames[1:], duration=ms, loop=0, quality=78, method=6, minimize_size=True)
`;
  const r = spawnSync(process.env.PYTHON ?? "python3", ["-c", py, dir, join(OUT, name), String(Math.round(1000 / fps))], { stdio: "inherit" });
  rmSync(dir, { recursive: true, force: true });
  if (r.status !== 0) throw new Error("clip encoding failed (needs Python with Pillow)");
  console.log("saved", join(OUT, name), `(${frames} frames)`);
}
/** Clip steps: glide the drawn pointer to a point, then hover it (or click there). */
const glide = (x, y) => js(`__vt.pointTo(${x}, ${y})`);
const hoverAt = (x, y) => mouse(x, y);
const pressAt = async (x, y) => {
  await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
  await send("Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", clickCount: 1 });
  await send("Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", clickCount: 1 });
};

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

/** Look like the desktop app on a real keyboard: no browser-preview badge, a USB connection
 * (kept up while the page re-renders, for the clips). */
const DESKTOP_LOOK = `(() => {
  if (!document.getElementById("shots-look")) document.head.insertAdjacentHTML("beforeend", "<style id=shots-look>.pill.warn, .home-pending { display: none !important }</style>");
  const fix = () => {
    for (const el of document.querySelectorAll(".top-device small, .home-info .device-status")) {
      if (el.textContent.trim() === "Connected") continue;
      const dot = el.querySelector(".dot");
      el.textContent = "Connected";
      if (dot) el.prepend(dot);
    }
  };
  fix();
  window.__shotsLook ??= setInterval(fix, 30);
})()`;
const look = () => js(DESKTOP_LOOK);

/** Put a library design on the (simulated) keyboard, as a click in the Library does. */
const put = (id) => js(`import("/src/lib/store.ts").then((m) => m.useApp.getState().apply(${JSON.stringify(id)}))`);

const centre = (r, dy = 0.5) => [r.x + r.width / 2, r.y + r.height * dy];
/** Clip steps that glide the pointer at `at` ms to where `where()` says (found then), and hover
 * or click there once it arrives. */
function point(at, where, act = "hover") {
  let xy;
  return [
    [at, async () => { xy = centre(...[].concat(await where())); await glide(...xy); }],
    [at + 600, async () => (act === "click" ? pressAt(...xy) : hoverAt(...xy))],
  ];
}
/** The box of the first element matching `selector` whose text starts with `text`. */
const boxOf = (selector, text) => js(`(() => {
  const el = [...document.querySelectorAll(${JSON.stringify(selector)})].find((e) => e.textContent.trim().startsWith(${JSON.stringify(text)}));
  if (!el) throw new Error("not found: ${selector} ${text}");
  el.scrollIntoView({ block: "nearest" });
  const r = el.getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; })()`);
const scenes = {
  async home() {
    // the keyboard on Home going through a few designs, each shown once the keyboard took it
    await home();
    await sleep(1200);
    await look();
    const board = await rect(".home-board");
    const info = await rect(".home-info");
    const region = { x: board.x - 36, y: board.y - 36, width: board.width + 72, height: info.y + info.height - board.y + 64 };
    // bright ones: on white keycaps a dim key reads as white
    const designs = ["fx-wave-rainbow", "acid-bloom", "fx-vortex-rainbow", "flag-seychelles", "fest-diwali-lamps-cascade", "fx-dazzle-rainbow"];
    const hold = 2400;
    await film("home.webp", region, ((designs.length + 1) * hold) / 1000, designs.map((id, i) => [(i + 1) * hold, () => put(id)]));
  },
  async hover() {
    await tab("Games");
    await tile("CS2 Skins");
    await kind("effect");
    await sleep(700);
    const row = (await cards()).slice(0, 3);
    const region = { x: row[0].x - 12, y: row[0].y - 12, width: row[2].x + row[2].width - row[0].x + 24, height: row[0].height + 24 };
    await film("library-hover.webp", region, 7.5, [...point(0, () => [row[1], 0.35]), ...point(3600, () => [row[2], 0.35])], 20, 0.75);
    await mouse(5, 5);
  },
  async create() {
    // Create's animations: one click each, on the keyboard at once
    await rail("Create");
    await sleep(300);
    await click("[aria-label='What to create'] [role=tab]", "Animated");
    await sleep(300);
    await click(".mode", "Wave");
    await sleep(200);
    await js(`[...document.querySelectorAll("label")].find((l) => l.textContent.includes("Rainbow"))?.querySelector("input")?.click()`);
    await sleep(900);
    await look();
    const stage = await rect(".builder");
    const region = { x: stage.x - 16, y: stage.y - 16, width: stage.width + 32, height: stage.height + 32 };
    const modes = ["Kaleidoscope", "Circle Wave", "Dazzle", "Sine Wave", "Line Wave"];
    await film("create.webp", region, 2 + modes.length * 2.6, modes.flatMap((m, i) => point(1400 + i * 2600, () => boxOf(".mode", m), "click")), 20, 0.6);
    await mouse(5, 5);
  },
  async browse() {
    // the Library in use: a collection, animated designs playing under the pointer, one put on
    // the keyboard (the side panel then plays it big)
    await look();
    await sleep(600);
    const card = async (i) => [(await cards())[i], 0.35];
    await film("browse.webp", { x: 0, y: 0, width: W, height: H }, 15, [
      ...point(700, () => boxOf(".lib-tabs [role=tab]", "Themes"), "click"),
      ...point(2300, () => boxOf(".ctile", "Space"), "click"),
      ...point(4000, () => rect(".kind-select")),
      [4700, () => kind("effect")],
      ...point(5400, () => card(0)),
      ...point(7400, () => card(1)),
      ...point(9400, () => card(1), "click"),
      ...point(12200, () => card(2)),
    ], 16, 0.5);
    await mouse(5, 5);
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
