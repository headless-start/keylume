// In-browser backend: behaves like a connected TK68 so the whole UI can be developed and
// tested without Tauri or hardware. Data lives in memory (settings in localStorage).
// Its lighting follows the same rules as the Rust device worker: requests get ids, the
// newest wins, and a look counts as shown only once the (simulated) keyboard took it.
import type { KeylumeApi } from "./api";
import { mix as mixHex } from "./color";
import { ALL_FEATURES, LAMPARRAY_FEATURES } from "./features";
import { liveColor } from "./library";
import { spellFrames } from "./spell";
import { unpackLibrary } from "./wire";
import solarManifest from "../../packs/solar-terms/pack.json";
import solarThemes from "../../packs/solar-terms/themes.json";
import momentsManifest from "../../packs/lifes-moments/pack.json";
import momentsThemes from "../../packs/lifes-moments/themes.json";

/** The packs that come with Keylume (keylume-core `packs::BUNDLED`). */
const BUNDLED = [{ ...solarManifest, collections: [solarThemes] }, { ...momentsManifest, collections: [momentsThemes] }];
import type {
  AppSettings, DeviceInfo, DeviceSettings, Effect, Hex, KeyAction, Layout, Licence, Lighting, LightingState, LiveEffect, LiveFrame, LiveFrameEvent, Macro,
  MakeRequest, Maker, Profile, PackInfo, Shown, ShownOrigin, StagedBackup, Status, Upload,
} from "./types";

interface Fixture { layout: Layout; profiles: Profile[] }

/** `text` as a kebab-case id of at most `max` characters (keylume-core `packs::slug`). */
const packSlug = (text: string, max: number) =>
  text.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+/, "").slice(0, max).replace(/-+$/, "");

/** A maker key's fingerprint, "3F9A B2C1" (keylume-core `packs::fingerprint`). */
const fingerprint = (key: string) => `${key.slice(0, 4)} ${key.slice(4, 8)}`.toUpperCase();

const DEFAULT_SETTINGS: AppSettings = {
  favorites: ["deep-ocean-cascade", "deep-ocean-flame", "live-ocean-flow", "fx-ripple-cyan", "midnight-gamer"],
  lastProfile: null,
  liveLayer: 2,
  demoMode: true,
  demoBoard: "tk68",
  startMinimized: false,
  launchAtLogin: false,
  hotkeys: true,
  restoreOnConnect: true,
  sideFollow: true,
  sideCustom: { mode: "breathing", speed: 2, brightness: 4, direction: 0, rainbow: false, color: "#1f45ff" },
  shuffleMinutes: 0,
  trueColors: true,
};

/** What the simulated keyboard shows before anything is applied (as the Rust simulator). */
const FACTORY: Effect = { mode: "wave", speed: 2, brightness: 4, direction: 0, rainbow: true, color: "#ff0000" };

const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));

function slug(s: string) {
  return s.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "profile";
}

/** An approximate live frame (the browser has no animator): a pulse, or moving bars. */
function mockFrame(live: LiveEffect, t: number): LiveFrame {
  if (["spectrum", "bars", "sineBars", "rainBars", "scanner", "equalizer"].includes(live.kind)) {
    return { levels: Array.from({ length: 32 }, (_, b) => Math.round(3 + 3 * Math.sin(b * 0.4 - t * 4))) };
  }
  return { color: mixHex("#000000", liveColor(live), 0.3 + 0.7 * (0.5 + 0.5 * Math.sin(t * 3))) };
}

/** Switches for tests and screenshots: unplug the keyboard, pause, make a write fail. */
export interface MockSim {
  setConnected(on: boolean): void;
  /** Act as another kind of keyboard (a HID LampArray one draws everything itself). */
  useBoard(kind: "tk68" | "lamparray"): void;
  setPaused(reason: string | null): void;
  /** The next lighting write fails with this message. */
  failNext(message: string): void;
  /** The next file dialog "picks" this file. */
  pickFile(name: string, text: string): void;
  /** The last file saved through a dialog (a pack you made). */
  saved(): { name: string; text: string } | null;
}

export type MockApi = KeylumeApi & { sim: MockSim };

export async function loadFixture(): Promise<Fixture> {
  const res = await fetch("/mock/library.json");
  return unpackLibrary(await res.json());
}

export function createMockApiFrom(fx: Fixture, opts: { latency?: number } = {}): MockApi {
  const latency = opts.latency ?? 120;
  const { layout } = fx;
  let builtin: Profile[] = fx.profiles.map((p) => ({ ...p, source: "builtin" as const }));
  let user: Profile[] = [];
  let settings: AppSettings = { ...DEFAULT_SETTINGS };
  try {
    const s = localStorage.getItem("keylume.mock.settings");
    if (s) settings = { ...settings, ...JSON.parse(s) };
    const u = localStorage.getItem("keylume.mock.user");
    if (u) user = JSON.parse(u);
  } catch { /* storage unavailable: fine */ }
  const persist = () => {
    try {
      localStorage.setItem("keylume.mock.settings", JSON.stringify(settings));
      localStorage.setItem("keylume.mock.user", JSON.stringify(user));
    } catch { /* ignore */ }
  };

  let current: string | null = settings.lastProfile;
  let effect: Effect = { ...FACTORY };
  const layers: Record<string, Hex>[] = [0, 1, 2].map(() => Object.fromEntries(layout.keys.map((k) => [k.id, "#000000"])));
  const defaultMap = (): Record<string, KeyAction> =>
    Object.fromEntries(layout.keys.map((k) => [k.id, k.id === "fn" ? { kind: "fn" } : { kind: "key", code: k.hid, modifier: 0, code2: 0 }]));
  const keymaps = [0, 1, 2].map(defaultMap);
  // the Fn layer a TK68 comes with: media, brightness and shortcuts on the number row (not F-keys)
  const FACTORY_ROW: Record<string, KeyAction> = {
    "1": { kind: "consumer", usage: 0x70 }, "2": { kind: "consumer", usage: 0x6f }, "3": { kind: "key", code: 0xe3, modifier: 0, code2: 0x2b },
    "4": { kind: "key", code: 0xe3, modifier: 0, code2: 0x08 }, "5": { kind: "consumer", usage: 0x18a }, "6": { kind: "consumer", usage: 0x194 },
    "7": { kind: "consumer", usage: 0xb6 }, "8": { kind: "consumer", usage: 0xcd }, "9": { kind: "consumer", usage: 0xb5 },
    "0": { kind: "consumer", usage: 0xe2 }, minus: { kind: "consumer", usage: 0xea }, equal: { kind: "consumer", usage: 0xe9 },
  };
  const MOVED = ["z", "x", "c", "comma", "period", "slash", "lbracket", "rbracket"];
  const fnMap: Record<string, KeyAction> = Object.fromEntries(layout.keys.map((k) => [k.id,
    FACTORY_ROW[k.id] ?? (MOVED.includes(k.id) ? { kind: "disabled" } : layout.fnLayer?.[k.id] ?? { kind: "disabled" }) as KeyAction]));
  const macros: Macro[] = Array.from({ length: 16 }, () => ({ repeat: 1, events: [] }));
  let device: DeviceSettings = {
    reportRate: 1000, debounce: 2, profile: 0, firmware: 772,
    sleep: { bluetooth: 3600, wireless: 3600, deepBluetooth: 3600, deepWireless: 3600 },
    options: { winKeyLock: false, macMode: false, wasdArrowsSwap: false, ledOff: false, sideLedOff: false, keyboardMode: false, keyboardLock: false, fnMatrix: false, powerSave: false },
  };
  const TK68: DeviceInfo = {
    board: "epomaker-tk68", name: "Epomaker TK68", maker: "Epomaker", support: "verified", features: ALL_FEATURES,
    layoutId: "epomaker-tk68", vid: 0x05ac, pid: 0x024f, product: "TK68 (browser mock)", manufacturer: "Keylume", path: "mock://", simulated: true,
  };
  // the mock keeps the TK68's layout: it stands for any keyboard that draws its lights itself
  const LAMPARRAY: DeviceInfo = {
    ...TK68, board: "lamparray-0000-0001", name: "LampArray keyboard", maker: "Keylume", support: "experimental", features: LAMPARRAY_FEATURES,
    product: "LampArray keyboard (browser mock)", path: "mock://lamparray",
  };
  let DEVICE = TK68;
  const librarySubs = new Set<() => void>();
  let status: Status = { connected: true, firmware: 772, live: null, error: null, applied: 0, paused: null, busy: null, device: DEVICE };
  const statusSubs = new Set<(s: Status) => void>();
  const currentSubs = new Set<(id: string) => void>();
  const setStatus = (s: Partial<Status>) => { status = { ...status, ...s }; statusSubs.forEach((cb) => cb(status)); };
  const all = () => [...user, ...builtin];

  // ---- lighting, as the device worker has it -------------------------------------------
  type ReqStatus = "none" | "requested" | "pending" | "done" | { failed: string };
  interface Req { id: number; origin: ShownOrigin; profileId: string | null; name: string; lighting: Lighting | null }
  let seq = 0;
  let nextId = 0;
  let request: LightingState["request"] = null;
  let reqStatus: ReqStatus = "none";
  let queued: Req | null = null;
  let work: Promise<void> = Promise.resolve();
  let failNext: string | null = null;
  const readBack = (origin: ShownOrigin, name: string): Shown => ({
    request: 0, origin, profileId: null, name, lighting: { kind: "effect", effect }, brightness: effect.brightness, running: false, progress: null,
  });
  let shown: Shown | null = readBack("keyboard", "Wave");
  let lightsOff = false;
  const lightingSubs = new Set<(s: LightingState) => void>();
  const frameSubs = new Set<(f: LiveFrameEvent) => void>();
  let latestFrame: LiveFrameEvent | null = null;
  let watchedUntil = 0;
  let frameTimer: ReturnType<typeof setInterval> | undefined;
  let spellTimer: ReturnType<typeof setTimeout> | undefined;

  const newest = (id: number) => request?.id === id;
  const state = (): LightingState => {
    const off = !!shown && (shown.origin === "off" || shown.brightness === 0 || (shown.lighting?.kind === "effect" && shown.lighting.effect.mode === "off"));
    const phase: LightingState["phase"] = status.paused ? "paused" : !status.connected ? "disconnected"
      : reqStatus === "requested" ? "requested" : reqStatus === "pending" ? "pending" : typeof reqStatus === "object" ? "failed"
        : off ? "off" : shown ? "applied" : "unknown";
    return { seq, phase, request, error: typeof reqStatus === "object" ? reqStatus.failed : null, shown };
  };
  const publish = () => { seq += 1; const s = state(); lightingSubs.forEach((cb) => cb(s)); };

  const stopRunning = () => {
    clearInterval(frameTimer);
    clearTimeout(spellTimer);
    frameTimer = spellTimer = undefined;
    if (shown?.running) shown = { ...shown, running: false };
  };

  const startFrames = (id: number, live: LiveEffect) => {
    const t0 = Date.now();
    let n = 0;
    frameTimer = setInterval(() => {
      const t = (Date.now() - t0) / 1000;
      latestFrame = { request: id, seq: n++, t, frame: mockFrame(live, t) };
      if (Date.now() < watchedUntil) frameSubs.forEach((cb) => cb(latestFrame!));
    }, 40);
  };

  const startSpell = (id: number, frames: Record<string, Hex>[]) => {
    const total = frames.length;
    const step = () => {
      if (!shown || shown.request !== id || !shown.progress) return;
      const done = Math.min(shown.progress.done + 1, total);
      shown = { ...shown, progress: { done, total }, running: done < total };
      layers[settings.liveLayer] = frames[done - 1];
      publish();
      if (done < total) spellTimer = setTimeout(step, latency * 12);
      else setStatus({ live: null });
    };
    spellTimer = setTimeout(step, latency * 12);
  };

  /** The worker: applies the newest queued request (older ones were coalesced away). */
  const apply = async () => {
    await delay(latency);
    const r = queued;
    queued = null;
    if (!r) return;
    if (!status.connected || status.paused) {
      if (newest(r.id)) reqStatus = { failed: status.paused ? "Keylume is paused while another app uses the keyboard" : "no keyboard connected" };
      publish();
      return;
    }
    if (newest(r.id)) reqStatus = "pending";
    publish();
    await delay(latency);
    stopRunning();
    if (failNext) {
      const why = failNext;
      failNext = null;
      shown = null;
      if (newest(r.id)) reqStatus = { failed: why };
      setStatus({ live: null, error: why });
      publish();
      return;
    }
    const l = r.lighting;
    // what the board can't show is refused before anything is sent (as the worker does)
    const f = DEVICE.features;
    const refused = !l ? null
      : l.kind === "effect" && !f.effects.includes(l.effect.mode) ? `This keyboard can't show the ${l.effect.mode} animation.`
        : (l.kind === "perKey" || l.kind === "spell") && !f.perKey ? "This keyboard has no per-key lighting."
          : l.kind === "live" && !f.live ? "This keyboard can't show live effects." : null;
    if (refused) {
      if (newest(r.id)) reqStatus = { failed: refused };
      setStatus({ error: refused });
      publish();
      return;
    }
    const spell = l?.kind === "spell" ? spellFrames(layout, l.words, l.background) : null;
    const brightness = !l ? 0 : l.kind === "effect" ? l.effect.brightness : l.kind === "live" ? 4 : l.brightness;
    if (!l) effect = { ...effect, mode: "off", brightness: 0 };
    else if (l.kind === "effect") effect = l.effect;
    else if (l.kind === "perKey") { layers[settings.liveLayer] = { ...l.keys }; effect = { ...effect, mode: "user-picture", direction: settings.liveLayer }; }
    else if (spell) { layers[settings.liveLayer] = spell[0]; effect = { ...effect, mode: "user-picture", direction: settings.liveLayer }; }
    else effect = { ...effect, mode: "screen-sync" };
    const running = l?.kind === "live" || (!!spell && spell.length > 1);
    shown = { request: r.id, origin: r.origin, profileId: r.profileId, name: r.name, lighting: l, brightness, running, progress: spell ? { done: 1, total: spell.length } : null };
    if (newest(r.id)) reqStatus = "done";
    setStatus({ applied: status.applied + 1, error: null, live: running ? r.name : null });
    publish();
    if (l?.kind === "live") startFrames(r.id, l.live);
    if (spell && spell.length > 1) startSpell(r.id, spell);
  };

  const requestLighting = (origin: ShownOrigin, profileId: string | null, name: string, lighting: Lighting | null): number => {
    const id = ++nextId;
    request = { id, profileId, name };
    if (status.busy) {
      reqStatus = { failed: `the keyboard is busy (${status.busy})` };
      publish();
      return id;
    }
    reqStatus = "requested";
    queued = { id, origin, profileId, name, lighting };
    publish();
    work = work.then(apply);
    return id;
  };

  /** Device calls wait for the keyboard, and refuse while it's busy or gone. */
  const guard = async (ms: number) => {
    if (status.busy) throw new Error(`the keyboard is busy (${status.busy}); try again when it's done`);
    if (status.paused) throw new Error(`paused: ${status.paused}`);
    if (!status.connected) throw new Error("no keyboard connected");
    await delay(ms);
  };
  /** A backup, restore or reset: one at a time, nothing else meanwhile. */
  const maintain = async <T,>(label: string, f: () => T): Promise<T> => {
    await guard(0);
    setStatus({ busy: label });
    try {
      await delay(latency * 5);
      return f();
    } finally {
      setStatus({ busy: null });
    }
  };

  // Files the browser handed over (upload picker, drag-and-drop), under made-up paths.
  const files = new Map<string, string>();
  let nextPick: { name: string; text: string } | null = null;
  const stash = (f: File) => new Promise<string>((ok, fail) => {
    const r = new FileReader(); // (not f.text(): the test DOM lacks it)
    r.onload = () => {
      const path = `browser://${files.size}/${f.name}`;
      files.set(path, String(r.result));
      ok(path);
    };
    r.onerror = () => fail(r.error);
    r.readAsText(f);
  });
  /** The browser's stand-in for a file dialog (or the file a test set up). */
  const pick = (multiple: boolean): Promise<string[]> => {
    if (nextPick) {
      const path = `browser://${files.size}/${nextPick.name}`;
      files.set(path, nextPick.text);
      nextPick = null;
      return Promise.resolve([path]);
    }
    return new Promise((resolve) => {
      const input = Object.assign(document.createElement("input"), { type: "file", accept: ".json,application/json", multiple });
      input.onchange = () => Promise.all([...(input.files ?? [])].map(stash)).then(resolve);
      input.oncancel = () => resolve([]);
      input.click();
    });
  };
  const find = (id: string) => all().find((p) => p.id === id);
  const save = async (p: Profile) => {
    const exists = user.find((u) => u.id === p.id);
    let id = p.id;
    if (!exists) {
      const base = `user-${slug(p.name)}`;
      id = base;
      for (let n = 2; find(id); n++) id = `${base}-${n}`;
    }
    const saved: Profile = { ...p, id, source: "user" };
    user = exists ? user.map((u) => (u.id === id ? saved : u)) : [...user, saved];
    persist();
    return saved;
  };
  const importText = async (text: string): Promise<string[]> => {
    let list: Partial<Profile>[];
    try {
      const v = JSON.parse(text);
      list = Array.isArray(v?.profiles) ? v.profiles : [v];
    } catch {
      throw new Error("not a Keylume profile file");
    }
    if (!list.length || list.some((p) => !p?.name?.trim() || !p.lighting?.kind)) throw new Error("not a Keylume profile file");
    const ids: string[] = [];
    for (const p of list) ids.push((await save({ tags: [], description: "", ...p, id: "", category: p.category || "Mine" } as Profile)).id);
    return ids;
  };
  // Design packs: the browser has no design generator, so each theme becomes one simple
  // gradient across the keys (the app draws every look of it). Signatures aren't checked
  // here; who signed a pack is taken from the file (the app checks them: keylume-core packs).
  let packs: PackInfo[] = [];
  const signers = new Map<string, string | null>(); // pack id -> signing key
  type PackFile = {
    id: string; name: string; publisher: string; version: string; description: string; signature?: { key: string }; licence?: Licence; url?: string;
    collections?: { collection: string; themes: { id: string; name: string; description: string; tags: string[]; stops: Hex[] }[] }[];
    designs?: { id: string; name: string; tags?: string[]; description?: string; lighting: Lighting }[];
  };
  const installPack = (v: PackFile, bundled = false): PackInfo => {
    const collections = v.collections ?? [], made = v.designs ?? [];
    if (!v.id || !v.name || (!collections.length && !made.length)) throw new Error("it isn't a valid pack");
    const key = v.signature?.key ?? null;
    const official = bundled || key === "keylume-2026";
    const old = packs.find((p) => p.id === v.id);
    if (old?.bundled && !official) throw new Error(`only an official file can update ${old.name}, which comes with Keylume`);
    if (old && !old.bundled && signers.get(old.id) && signers.get(old.id) !== key) {
      throw new Error(`it isn't signed by whoever made the ${old.name} pack you have. If you trust this file, remove that pack first`);
    }
    const names = [...collections.map((c) => c.collection), ...(made.length && !collections.some((c) => c.collection === v.name) ? [v.name] : [])];
    const info: PackInfo = {
      id: v.id, name: v.name, publisher: v.publisher, version: v.version, description: v.description, official,
      collections: names, themes: collections.reduce((n, c) => n + c.themes.length, 0), designs: made.length, bundled,
      ...(key && !official ? { makerKey: fingerprint(key) } : {}), ...(v.licence ? { licence: v.licence } : {}), ...(v.url ? { url: v.url } : {}),
    };
    const width = Math.max(...layout.keys.map((k) => k.x + k.w));
    const designs: Profile[] = collections.flatMap((c) => c.themes.map((t): Profile => ({
      id: `${t.id}-cascade`, name: `${t.name} · Cascade`, category: c.collection, tags: t.tags, description: t.description, source: "builtin", section: (c as { section?: string }).section ?? "Themes",
      lighting: { kind: "perKey", brightness: 4, keys: Object.fromEntries(layout.keys.map((k) => {
        const at = ((k.x + k.w / 2) / width) * (t.stops.length - 1);
        const i = Math.min(t.stops.length - 2, Math.floor(at));
        return [k.id, mixHex(t.stops[i], t.stops[i + 1], at - i)];
      })) },
    })));
    for (const d of made) {
      designs.push({ id: `${v.id}-${d.id}`, name: d.name, category: v.name, tags: d.tags ?? [], description: d.description ?? "", source: "builtin", section: "Themes", lighting: d.lighting });
    }
    const gone = new Set(old?.collections ?? []);
    builtin = [...builtin.filter((p) => !gone.has(p.category)), ...designs];
    packs = [...packs.filter((p) => p.id !== info.id), info].sort((a, b) => a.id.localeCompare(b.id));
    signers.set(info.id, key);
    librarySubs.forEach((cb) => cb());
    return info;
  };
  // Making a pack: the maker's key is made up once and kept with the rest of the mock's data.
  let lastSaved: { name: string; text: string } | null = null;
  const MAKER = "keylume.mock.maker";
  const readMaker = (): Maker & { key: string } => {
    try {
      const m = JSON.parse(localStorage.getItem(MAKER) ?? "null");
      if (m) return m;
    } catch { /* storage unavailable */ }
    return { name: "", url: "", licence: "share", made: [], key: "" };
  };
  const writeMaker = (m: Maker & { key: string }) => { try { localStorage.setItem(MAKER, JSON.stringify(m)); } catch { /* ignore */ } };
  const makePack = async (req: MakeRequest): Promise<string | null> => {
    const mine = req.designs.map((id) => user.find((u) => u.id === id));
    if (!mine.length || mine.some((p) => !p)) throw new Error("pick at least one of your designs");
    const base = packSlug(req.name, 33);
    if (!base) throw new Error("give the pack a name with some letters in it");
    if (req.url.trim() && !/^https:\/\/[a-z0-9-]+(\.[a-z0-9-]+)+([/?#]\S*)?$/i.test(req.url.trim())) throw new Error("the web page should be an https:// address");
    const maker = readMaker();
    if (!maker.key) maker.key = Array.from({ length: 64 }, () => "0123456789abcdef"[Math.floor(Math.random() * 16)]).join("");
    const ids = new Set<string>(), names = new Set<string>();
    const designs = mine.map((p) => {
      const stem = packSlug(p!.name, 34) || "design";
      let id = stem, name = p!.name.trim();
      for (let n = 2; ids.has(id); n++) id = `${stem}-${n}`;
      for (let n = 2; names.has(name.toLowerCase()); n++) name = `${p!.name.trim()} ${n}`;
      ids.add(id); names.add(name.toLowerCase());
      return { id, name, tags: p!.tags, description: p!.description, lighting: p!.lighting };
    });
    const pack = {
      keylumePack: 1, id: `${base}-${maker.key.slice(0, 6)}`, name: req.name.trim(), publisher: req.maker.trim(), version: req.version.trim(),
      description: req.description.trim(), designs, licence: req.licence, ...(req.url.trim() ? { url: req.url.trim() } : {}),
      signature: { key: maker.key, sig: "mock" },
    };
    const text = JSON.stringify(pack, null, 2);
    const name = `${packSlug(req.name, 40)}-${packSlug(req.version, 20)}.keylumepack`;
    lastSaved = { name, text };
    // in a browser, offer the file as a download
    if (typeof URL.createObjectURL === "function" && !navigator.userAgent.includes("jsdom")) {
      const a = document.createElement("a");
      a.href = URL.createObjectURL(new Blob([text], { type: "application/json" }));
      a.download = name;
      a.click();
      setTimeout(() => URL.revokeObjectURL(a.href), 1000);
    }
    writeMaker({ ...maker, name: req.maker.trim(), url: req.url.trim(), licence: req.licence,
      made: [...maker.made.filter((m) => m.id !== pack.id), { id: pack.id, name: pack.name, version: pack.version, description: pack.description }] });
    return name;
  };
  for (const b of BUNDLED) installPack(b, true);
  const importPaths = async (paths: string[]): Promise<Upload> => {
    const out: Upload = { added: [], packs: [], failed: [] };
    for (const path of paths) {
      const text = files.get(path);
      const name = path.split("/").pop() ?? path;
      files.delete(path); // each file is accepted once, like a drop in the app
      if (text === undefined) { out.failed.push([name, "drop the file on the window, or use Upload"]); continue; }
      let v: unknown = null;
      try { v = JSON.parse(text); } catch { /* not JSON: importText says what it isn't */ }
      try {
        if (v && typeof v === "object" && "keylumePack" in v) out.packs.push(installPack(v as unknown as Parameters<typeof installPack>[0]));
        else out.added.push(...(await importText(text)));
      } catch (e) { out.failed.push([name, e instanceof Error ? e.message : String(e)]); }
    }
    return out;
  };

  let staged: { token: number; effect: Effect } | null = null;

  const sim: MockSim = {
    setConnected(on) {
      if (on === status.connected) return;
      if (!on) {
        stopRunning();
        shown = null;
        if (reqStatus === "requested" || reqStatus === "pending") reqStatus = { failed: "the keyboard was disconnected" };
        setStatus({ connected: false, device: null, firmware: null, live: null, error: "the keyboard was unplugged" });
      } else {
        shown = readBack("keyboard", effect.mode === "user-picture" ? `Picture layer ${effect.direction + 1}` : effect.mode);
        if (typeof reqStatus === "object") reqStatus = "none";
        setStatus({ connected: true, device: DEVICE, firmware: 772, error: null });
      }
      publish();
    },
    setPaused(reason) {
      if (reason) {
        stopRunning();
        shown = null;
        setStatus({ paused: reason, connected: false, device: null, firmware: null, live: null });
      } else {
        setStatus({ paused: null, connected: true, device: DEVICE, firmware: 772 });
        shown = readBack("keyboard", effect.mode);
      }
      publish();
    },
    useBoard(kind) {
      DEVICE = kind === "lamparray" ? LAMPARRAY : TK68;
      if (status.connected) setStatus({ device: DEVICE, firmware: kind === "lamparray" ? null : 772 });
      librarySubs.forEach((cb) => cb());
    },
    failNext(message) { failNext = message; },
    pickFile(name, text) { nextPick = { name, text }; },
    saved: () => lastSaved,
  };

  const api: MockApi = {
    kind: "mock",
    sim,
    appInfo: async () => ({ version: __APP_VERSION__, notices: [] }),
    getStatus: async () => status,
    onStatus: (cb) => { statusSubs.add(cb); return () => statusSubs.delete(cb); },
    onCurrentProfile: (cb) => { currentSubs.add(cb); return () => currentSubs.delete(cb); },
    getLayout: async () => layout,
    getLighting: async () => ({ state: state(), frame: latestFrame }),
    onLighting: (cb) => { lightingSubs.add(cb); return () => lightingSubs.delete(cb); },
    onLightingFrame: (cb) => { frameSubs.add(cb); return () => frameSubs.delete(cb); },
    watchLighting: async () => { watchedUntil = Date.now() + 3000; },
    listProfiles: async () => all(),
    saveProfile: save,
    deleteProfile: async (id) => {
      if (builtin.some((b) => b.id === id)) throw new Error("built-in profiles can't be modified; save a copy instead");
      user = user.filter((u) => u.id !== id);
      settings = { ...settings, favorites: settings.favorites.filter((f) => f !== id) };
      persist();
    },
    importProfiles: importPaths,
    listPacks: async () => packs,
    makerInfo: async () => { const { key: _key, ...m } = readMaker(); return m; },
    makePack,
    openPackPage: async (id) => {
      const url = packs.find((p) => p.id === id)?.url;
      if (!url) throw new Error("that pack has no page");
      window.open(url, "_blank", "noopener");
    },
    removePack: async (id) => {
      const gone = packs.find((p) => p.id === id);
      if (!gone) throw new Error(`no pack "${id}" is installed`);
      if (gone.bundled) throw new Error(`${gone.name} comes with Keylume`);
      packs = packs.filter((p) => p.id !== id);
      builtin = builtin.filter((p) => !gone.collections.includes(p.category));
      // a file that had replaced a pack that comes with Keylume: that one is back
      const original = BUNDLED.find((b) => b.id === id);
      if (original) installPack(original, true);
      librarySubs.forEach((cb) => cb());
    },
    uploadProfiles: async () => importPaths(await pick(true)),
    onFileDrop: (cb) => {
      const over = (e: DragEvent) => {
        if (!e.dataTransfer?.types.includes("Files")) return;
        e.preventDefault();
        cb({ kind: "over", paths: [] });
      };
      const leave = (e: DragEvent) => { if (!e.relatedTarget) cb({ kind: "leave", paths: [] }); };
      const drop = async (e: DragEvent) => {
        e.preventDefault();
        cb({ kind: "drop", paths: await Promise.all([...(e.dataTransfer?.files ?? [])].map(stash)) });
      };
      window.addEventListener("dragover", over);
      window.addEventListener("dragleave", leave);
      window.addEventListener("drop", drop);
      return () => {
        window.removeEventListener("dragover", over);
        window.removeEventListener("dragleave", leave);
        window.removeEventListener("drop", drop);
      };
    },
    applyProfile: async (id) => {
      const p = find(id);
      if (!p) throw new Error(`no profile ${id}`);
      lightsOff = false;
      current = id;
      settings = { ...settings, lastProfile: id };
      persist();
      currentSubs.forEach((cb) => cb(id));
      return requestLighting("profile", id, p.name, p.lighting);
    },
    getSideLight: async () => { await guard(latency); return settings.sideCustom; },
    // The browser preview has no animator: approximate (a pulse of the effect's colour, or moving bars).
    previewLive: async (live, seconds) => Array.from({ length: Math.round(seconds * 20) }, (_, i) => mockFrame(live, i / 20)),
    onProfilesChanged: () => () => {},
    onLibraryChanged: (cb) => { librarySubs.add(cb); return () => librarySubs.delete(cb); },
    onSettingsChanged: () => () => {},
    currentProfile: async () => current,
    applyEffect: async (e) => requestLighting("preview", null, "Unsaved animation", { kind: "effect", effect: e }),
    previewKeys: async (keys, brightness) => requestLighting("preview", null, "Unsaved design", { kind: "perKey", keys, brightness }),
    stopLive: async () => {
      stopRunning();
      setStatus({ live: null });
      publish();
    },
    toggleLights: async () => {
      if (lightsOff && current) {
        await api.applyProfile(current);
      } else {
        lightsOff = true;
        requestLighting("off", null, "Lights off", null);
      }
    },
    getSettings: async () => settings,
    setSettings: async (s) => { settings = s; persist(); },
    getEffect: async () => { await guard(latency); return effect; },
    readLayer: async (layer) => { await guard(latency * 3); return { ...layers[layer] }; },
    writeLayer: async (layer, keys) => { await guard(latency * 4); layers[layer] = { ...keys }; },
    getKeymap: async (layer, profile) => { await guard(latency * 2); return { ...(layer === "fn" ? fnMap : keymaps[profile]) }; },
    setKeymap: async (layer, profile, keys) => { await guard(latency * 4); Object.assign(layer === "fn" ? fnMap : keymaps[profile], keys); },
    getMacro: async (i) => { await guard(latency); return structuredClone(macros[i]); },
    setMacro: async (i, m) => { await guard(latency * 3); macros[i] = structuredClone(m); },
    getDeviceSettings: async () => { await guard(latency); return structuredClone(device); },
    setDeviceSettings: async (s) => { await guard(latency * 2); device = structuredClone(s); },
    factoryReset: async () => maintain("factory reset", () => {
      keymaps.forEach((_, i) => (keymaps[i] = defaultMap()));
      stopRunning();
      effect = { ...FACTORY };
      shown = readBack("reset", "Wave");
      current = null;
      settings = { ...settings, lastProfile: null };
      publish();
    }),
    closeConflictingApps: async () => { sim.setPaused(null); return 0; },
    backupToFile: async () => maintain("backing up", () => `${DEVICE.board}-backup-${new Date().toISOString().slice(0, 10)}.json`),
    chooseBackup: async (): Promise<StagedBackup | null> => {
      const [path] = await pick(false);
      if (!path) return null;
      const file = path.split("/").pop() ?? path;
      let b: { keylumeBackup?: number; device?: string; firmware?: number; effect?: Effect; macros?: Macro[] };
      try { b = JSON.parse(files.get(path) ?? ""); } catch { throw new Error(`${file} can't be restored: it isn't a Keylume backup.`); }
      if (b.keylumeBackup !== 1) throw new Error(`${file} can't be restored: it isn't a Keylume backup.`);
      if (b.device !== "epomaker-tk68") throw new Error(`${file} can't be restored: it was made on a "${b.device}".`);
      if (b.firmware !== 772) throw new Error(`${file} can't be restored: it was made on other firmware.`);
      staged = { token: Date.now(), effect: b.effect ?? { ...FACTORY } };
      return { token: staged.token, file, firmware: "3.04", macros: (b.macros ?? []).filter((m) => m.events.length).length, sideLight: false };
    },
    restoreBackup: async (token) => {
      if (!staged || staged.token !== token) throw new Error("pick the backup again");
      const s = staged;
      staged = null;
      return maintain("restoring a backup", () => {
        stopRunning();
        effect = s.effect;
        shown = readBack("restored", "Restored from a backup");
        current = null;
        settings = { ...settings, lastProfile: null };
        publish();
        return "browser preview: nothing saved";
      });
    },
    confirm: async (message) => window.confirm(message),
  };
  return api;
}

export async function createMockApi(): Promise<KeylumeApi> {
  const api = createMockApiFrom(await loadFixture());
  // the browser preview's switches, for trying states by hand and for tools/ui_shots.mjs
  (window as unknown as { __keylumeMock?: MockSim }).__keylumeMock = api.sim;
  return api;
}

