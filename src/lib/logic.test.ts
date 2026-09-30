import { describe, expect, it } from "vitest";
import { gradientBetween, hexToRgb, isHex, mix, normalizeHex, rgbToHex } from "./color";
import { editorReducer, initEditor } from "./editor";
import { actionLabel, codeToHid, HID_KEYS, isDefaultAction } from "./keys";
import { byKind, groupFamilies, variantLabel } from "./families";
import { colourFamily, filterProfiles, matches, previewColors } from "./library";
import { encodedSize, normalize, Recorder } from "./macro";
import { MODES } from "./modes";
import type { Layout, LayoutKey, Profile } from "./types";

const key = (id: string, x: number, y: number, w = 1): LayoutKey => ({ id, label: id, x, y, w, h: 1, slot: 0, hid: 4 });
const layout: Layout = {
  id: "t", name: "Test", width: 4, height: 2, userPictureLayers: 3, onboardProfiles: 4,
  keys: [key("a", 0, 0), key("b", 1, 0), key("c", 2, 0), key("d", 3, 0), key("space", 0, 1, 4)],
};
const profile = (id: string, over: Partial<Profile> = {}): Profile => ({
  id, name: id, category: "Blue", tags: [], description: "", source: "builtin",
  lighting: { kind: "effect", effect: { mode: "static", speed: 2, brightness: 4, direction: 0, rainbow: false, color: "#0040ff" } },
  ...over,
});

describe("colour", () => {
  it("round-trips hex", () => {
    expect(rgbToHex(...hexToRgb("#0a40ff"))).toBe("#0a40ff");
    expect(normalizeHex("0af")).toBe("#00aaff");
    expect(isHex("#12345")).toBe(false);
    expect(isHex("00ff00")).toBe(true);
  });
  it("mixes in linear light like the Rust generator", () => {
    expect(mix("#0000ff", "#ffffff", 0)).toBe("#0000ff");
    expect(mix("#0000ff", "#ffffff", 1)).toBe("#ffffff");
    expect(hexToRgb(mix("#0000ff", "#ffffff", 0.5))[0]).toBeGreaterThan(150);
  });
  it("gradients run from the start key to the end key", () => {
    const g = gradientBetween(layout.keys, layout.keys[0], layout.keys[3], "#000000", "#ffffff");
    expect(g.a).toBe("#000000");
    expect(g.d).toBe("#ffffff");
    expect(hexToRgb(g.b)[0]).toBeLessThan(hexToRgb(g.c)[0]);
  });
});

describe("editor reducer", () => {
  const blank = { a: "#000000", b: "#000000", c: "#000000", d: "#000000", space: "#000000" };
  it("paints, fills, undoes and redoes", () => {
    let s = initEditor(blank);
    s = editorReducer(s, { type: "paint", ids: ["a"], color: "#ff0000" });
    s = editorReducer(s, { type: "fill", color: "#00ff00", ids: ["b", "c"] });
    expect(s.keys).toMatchObject({ a: "#ff0000", b: "#00ff00", c: "#00ff00", d: "#000000" });
    s = editorReducer(s, { type: "undo" });
    expect(s.keys.b).toBe("#000000");
    s = editorReducer(s, { type: "redo" });
    expect(s.keys.b).toBe("#00ff00");
  });
  it("merges a brush stroke into one undo step", () => {
    let s = initEditor(blank);
    s = editorReducer(s, { type: "paint", ids: ["a"], color: "#ff0000" });
    s = editorReducer(s, { type: "paint", ids: ["b"], color: "#ff0000", merge: true });
    s = editorReducer(s, { type: "paint", ids: ["c"], color: "#ff0000", merge: true });
    s = editorReducer(s, { type: "undo" });
    expect(Object.values(s.keys).every((c) => c === "#000000")).toBe(true);
  });
  it("ignores no-op paints and new edits clear redo", () => {
    let s = initEditor(blank);
    const same = editorReducer(s, { type: "paint", ids: ["a"], color: "#000000" });
    expect(same).toBe(s);
    s = editorReducer(s, { type: "paint", ids: ["a"], color: "#111111" });
    s = editorReducer(s, { type: "undo" });
    s = editorReducer(s, { type: "paint", ids: ["b"], color: "#222222" });
    expect(s.future).toHaveLength(0);
  });
  it("gradient can be limited to a selection", () => {
    let s = initEditor(blank);
    s = editorReducer(s, { type: "gradient", all: layout.keys, from: layout.keys[0], to: layout.keys[3], a: "#0000ff", b: "#00ffff", only: ["a", "d"] });
    expect(s.keys.a).toBe("#0000ff");
    expect(s.keys.d).toBe("#00ffff");
    expect(s.keys.b).toBe("#000000");
  });
});

describe("library filtering", () => {
  const list = [
    profile("ocean", { name: "Deep Ocean", tags: ["blue", "ocean"] }),
    profile("ember", { name: "Ember", category: "Warm", tags: ["fire"] }),
    profile("mine", { name: "My Thing", source: "user", lighting: { kind: "perKey", keys: {}, brightness: 4 } }),
    profile("flow", { name: "Ocean Flow", category: "Live", lighting: { kind: "live", live: { kind: "paletteFlow", colors: ["#00ffff"], period: 8 } } }),
  ];
  it("matches every word of the query", () => {
    expect(matches(list[0], "deep blue")).toBe(true);
    expect(matches(list[0], "deep fire")).toBe(false);
  });
  it("filters by kind, category, favourites (in favourite order)", () => {
    const ids = (f: Parameters<typeof filterProfiles>[1], favs: string[] = []) => filterProfiles(list, f, favs).map((p) => p.id);
    const f = { scope: "all", kind: "any", section: "", category: "", query: "", hue: "" } as const;
    expect(ids({ ...f, query: "ocean" })).toEqual(["ocean", "flow"]);
    expect(ids({ ...f, scope: "mine" })).toEqual(["mine"]);
    expect(ids({ ...f, kind: "live" })).toEqual(["flow"]);
    expect(ids({ ...f, category: "Warm" })).toEqual(["ember"]);
    expect(ids({ ...f, scope: "favorites" }, ["flow", "ocean"])).toEqual(["flow", "ocean"]);
  });
  it("sorts designs into colour families", () => {
    const flat = (c: string) => profile(c, { lighting: { kind: "effect", effect: { mode: "static", speed: 2, brightness: 4, direction: 0, rainbow: false, color: c } } });
    expect(["#ff1020", "#ff8000", "#20ff40", "#0050ff", "#a040ff", "#ff40c0", "#ffffff"].map((c) => colourFamily(flat(c), layout)))
      .toEqual(["Red", "Orange", "Green", "Blue", "Purple", "Pink", "White"]);
  });
  it("groups the looks of a design into one family", () => {
    const looks = [
      profile("a1", { name: "Dawn · Cascade", lighting: { kind: "perKey", keys: {}, brightness: 4 } }), profile("a2", { name: "Dawn · Circle Wave Animation", lighting: { kind: "effect", effect: { mode: "circle-wave", speed: 2, brightness: 4, direction: 0, rainbow: false, color: "#ff8000" } } }),
      profile("b", { name: "Dusk" }), profile("a3", { name: "Dawn · Live Colours", category: "Blue · Animated", lighting: { kind: "live", live: { kind: "paletteFlow", colors: ["#ff8000"], period: 8 } } }),
      profile("m1", { name: "My design", source: "user" }), profile("m2", { name: "My design", source: "user" }),
    ];
    const { families, byProfile } = groupFamilies(looks);
    expect(families.map((f) => [f.name, f.variants.length])).toEqual([["Dawn", 3], ["Dusk", 1], ["My design", 1], ["My design", 1]]);
    expect(byProfile.get("a3")).toBe(families[0]); // "Blue · Animated" belongs with "Blue"
    expect(looks.slice(0, 3).map(variantLabel)).toEqual(["Cascade", "Circle Wave", "Original"]);
    expect(byKind(families[0].variants).map(([k, v]) => [k, v.length])).toEqual([["perKey", 1], ["effect", 1], ["live", 1]]);
  });
  it("previews every kind of profile for every key", () => {
    for (const p of list) expect(Object.keys(previewColors(p, layout)).length).toBeGreaterThanOrEqual(p.lighting.kind === "perKey" ? 0 : 5);
    expect(previewColors(list[3], layout).a).toBe("#00ffff");
  });
});

describe("keys", () => {
  it("has unique HID codes", () => {
    const codes = HID_KEYS.map((k) => k.code);
    expect(new Set(codes).size).toBe(codes.length);
  });
  it("maps browser key codes to HID usages", () => {
    expect(codeToHid("KeyA")).toBe(0x04);
    expect(codeToHid("KeyZ")).toBe(0x1d);
    expect(codeToHid("Digit1")).toBe(0x1e);
    expect(codeToHid("Digit0")).toBe(0x27);
    expect(codeToHid("F1")).toBe(0x3a);
    expect(codeToHid("F13")).toBe(0x68);
    expect(codeToHid("ControlLeft")).toBe(0xe0);
    expect(codeToHid("Unidentified")).toBeUndefined();
  });
  it("labels actions readably", () => {
    expect(actionLabel({ kind: "key", code: 0x06, modifier: 0xe0, code2: 0 })).toBe("Ctrl+C");
    expect(actionLabel({ kind: "consumer", usage: 0xcd })).toBe("Play / Pause");
    expect(actionLabel({ kind: "macro", index: 2, mode: 0 })).toBe("Macro 3");
    expect(actionLabel({ kind: "mouse", code: 0xf5, arg: 0xff })).toBe("Scroll down");
    expect(isDefaultAction({ kind: "key", code: 4, modifier: 0, code2: 0 }, 4, "a")).toBe(true);
    expect(isDefaultAction({ kind: "fn" }, 0, "fn")).toBe(true);
  });
});

describe("macros", () => {
  it("computes the same encoded size as the Rust encoder", () => {
    // Rust test `encodes_like_vendor_app`: 01 00 | 0b 9e | 0b 00 f4 01 | 0c 94 | 0c 00 00 00 (+ 4-byte terminator)
    const m = {
      repeat: 1,
      events: [
        { kind: "key" as const, code: 11, down: true }, { kind: "delay" as const, ms: 30 }, { kind: "key" as const, code: 11, down: false },
        { kind: "delay" as const, ms: 500 }, { kind: "key" as const, code: 12, down: true }, { kind: "delay" as const, ms: 20 },
        { kind: "key" as const, code: 12, down: false },
      ],
    };
    expect(encodedSize(m)).toBe(2 + 2 + 4 + 2 + 4 + 4);
  });
  it("normalises recordings: merges delays, trims ends", () => {
    expect(normalize([{ kind: "delay", ms: 5 }, { kind: "key", code: 4, down: true }, { kind: "delay", ms: 10 }, { kind: "delay", ms: 20 }, { kind: "key", code: 4, down: false }, { kind: "delay", ms: 3 }]))
      .toEqual([{ kind: "key", code: 4, down: true }, { kind: "delay", ms: 30 }, { kind: "key", code: 4, down: false }]);
  });
  it("recorder ignores auto-repeat and releases held keys", () => {
    const r = new Recorder();
    r.push(4, true, 0);
    r.push(4, true, 30); // auto-repeat
    r.push(5, true, 50);
    const ev = r.finish(100);
    expect(ev.filter((e) => e.kind === "key")).toEqual([
      { kind: "key", code: 4, down: true }, { kind: "key", code: 5, down: true },
      { kind: "key", code: 4, down: false }, { kind: "key", code: 5, down: false },
    ]);
  });
});

describe("modes", () => {
  it("lists every firmware mode once", () => {
    const modes = MODES.map((m) => m.mode);
    expect(new Set(modes).size).toBe(23);
  });
});

// ---- what Home shows -------------------------------------------------------------------

import { dimAll, effectFrame } from "./animate";
import { setLatestFrame, useLiveFrame } from "./frames";
import { cachedPreviews, previewFrames } from "./liveCache";
import { mirrorColors, mirrorMotion, mirrorNote } from "./mirror";
import { outcome } from "./store";
import { setApi, type KeylumeApi } from "./api";
import type { LightingState, LiveFrameEvent, Shown } from "./types";
import { renderHook, act } from "@testing-library/react";

const shownOf = (over: Partial<Shown>): Shown => ({
  request: 1, origin: "profile", profileId: "p", name: "P", brightness: 4, running: false, progress: null,
  lighting: { kind: "perKey", brightness: 4, keys: { a: "#ff0000", b: "#00ff00", c: "#0000ff", d: "#ffffff", space: "#808080" } },
  ...over,
});
const stateOf = (over: Partial<LightingState>, shown: Partial<Shown> | null = {}): LightingState => ({
  seq: 1, phase: "applied", request: { id: 1, profileId: "p", name: "P" }, error: null, shown: shown && shownOf(shown), ...over,
});

describe("the mirror on Home", () => {
  it("shows nothing it doesn't know: disconnected, paused, off, dark or unknown", () => {
    for (const s of [
      stateOf({ phase: "disconnected" }), stateOf({ phase: "paused" }), stateOf({ phase: "off" }, { origin: "off", lighting: null, brightness: 0 }),
      stateOf({}, { brightness: 0 }), stateOf({ phase: "unknown" }, null), null,
    ]) {
      expect(mirrorColors(s, layout, 0, null), JSON.stringify(s?.phase)).toEqual({});
    }
    // a picture layer that couldn't be read, or a stream nobody sends, isn't guessed at
    const unread = stateOf({}, { lighting: { kind: "effect", effect: { mode: "user-picture", speed: 0, brightness: 4, direction: 1, rainbow: false, color: "#00c8c8" } } });
    expect(mirrorColors(unread, layout, 0, null)).toEqual({});
    expect(mirrorNote(unread)).toMatch(/picture layer 2, which couldn't be read/);
  });

  it("dims per-key designs to the keyboard's brightness", () => {
    const full = mirrorColors(stateOf({}), layout, 0, null);
    expect(full.a).toBe("#ff0000");
    const half = mirrorColors(stateOf({}, { brightness: 2 }), layout, 0, null);
    expect(hexToRgb(half.a)[0]).toBeLessThan(255);
    expect(hexToRgb(half.a)[0]).toBeGreaterThan(128); // half the light is well above half the sRGB value
    expect(dimAll({ a: "#ffffff" }, 0).a).toBe("#000000");
  });

  it("holds a spell at the picture it has really reached", () => {
    const words = [{ text: "ab", color: "#ff0000" }];
    const spell = (done: number, running: boolean) =>
      stateOf({}, { lighting: { kind: "spell", words, background: "#000000", brightness: 4 }, running, progress: { done, total: 3 } });
    const first = mirrorColors(spell(1, true), layout, 0, null);
    const last = mirrorColors(spell(3, false), layout, 99, null);
    expect(first.b).toBe("#000000"); // only A so far
    expect(last).toMatchObject({ a: "#ff0000", b: "#ff0000" });
    expect(mirrorColors(spell(3, false), layout, 0, null)).toEqual(last); // no loop: time changes nothing
    expect(mirrorNote(spell(1, true))).toBe("Spelling: picture 1 of 3");
    expect(mirrorMotion(spell(1, true))).toBe("still");
  });

  it("plays live effects from the frames the keyboard got, and only its own", () => {
    const live = stateOf({}, { request: 7, lighting: { kind: "live", live: { kind: "heartbeat", color: "#ff0000", bpm: 60 } }, running: true });
    const frame = (request: number): LiveFrameEvent => ({ request, seq: 3, t: 0.12, frame: { color: "#123456" } });
    expect(mirrorColors(live, layout, 0, frame(7)).a).toBe("#123456");
    expect(mirrorColors(live, layout, 0, frame(6)).a).not.toBe("#123456"); // an older request's frame
    expect(mirrorMotion(live)).toBe("frames");
    expect(mirrorNote(live)).toBeNull();
    expect(mirrorNote({ ...live, shown: { ...live.shown!, running: false } })).toMatch(/keeps its last frame/);
  });

  it("approximates keyboard animations, labelled, without made-up key presses", () => {
    const effect = (mode: "wave" | "reactive" | "ripple") =>
      stateOf({}, { lighting: { kind: "effect", effect: { mode, speed: 2, brightness: 4, direction: 0, rainbow: false, color: "#ff0000" } } });
    expect(mirrorMotion(effect("wave"))).toBe("clock");
    expect(mirrorNote(effect("wave"))).toMatch(/approximation/);
    expect(mirrorNote(effect("reactive"))).toMatch(/as you type/);
    // with presses off, a reactive board stays dark whatever the time
    const e = { mode: "reactive" as const, speed: 2, brightness: 4, direction: 0, rainbow: false, color: "#ff0000" };
    const quiet = [0, 0.7, 2.3].map((t) => JSON.stringify(effectFrame(e, layout, t, { presses: false })));
    expect(new Set(quiet).size).toBe(1);
    const demo = [0, 0.7, 2.3].map((t) => JSON.stringify(effectFrame(e, layout, t)));
    expect(new Set(demo).size).toBeGreaterThan(1); // the library's demo still shows presses
  });
});

describe("following a request", () => {
  it("says applying, shown or failed, and nothing once a newer request took over", () => {
    expect(outcome(stateOf({ phase: "pending", request: { id: 5, profileId: null, name: "x" } }), 5)).toBe("applying");
    expect(outcome(stateOf({}, { request: 5 }), 5)).toBe("shown");
    expect(outcome(stateOf({ phase: "disconnected", request: { id: 5, profileId: null, name: "x" } }, { request: 5 }), 5)).toBe("failed");
    expect(outcome(stateOf({ phase: "failed", request: { id: 5, profileId: null, name: "x" }, error: "no" }, { request: 4 }), 5)).toBe("failed");
    expect(outcome(stateOf({ phase: "pending", request: { id: 6, profileId: null, name: "y" } }, { request: 4 }), 5)).toBe(null);
  });
});

describe("live frames for the mirror", () => {
  it("keeps the newest frame: never an older request's, never an earlier frame", () => {
    const ev = (request: number, seq: number): LiveFrameEvent => ({ request, seq, t: seq / 25, frame: { color: `#0000${String(seq).padStart(2, "0")}` } });
    setApi({ onLightingFrame: () => () => {}, watchLighting: async () => {} } as unknown as KeylumeApi);
    const { result } = renderHook(() => useLiveFrame(true));
    act(() => setLatestFrame(ev(10, 4)));
    expect(result.current?.seq).toBe(4);
    act(() => setLatestFrame(ev(10, 2)));
    act(() => setLatestFrame(ev(9, 50)));
    expect(result.current).toMatchObject({ request: 10, seq: 4 });
    act(() => setLatestFrame(ev(11, 0)));
    expect(result.current).toMatchObject({ request: 11, seq: 0 });
    const off = renderHook(() => useLiveFrame(false));
    expect(off.result.current).toBeNull();
  });
});

describe("live preview cache", () => {
  it("keeps a bounded number of effects and retries after a failure", async () => {
    let calls = 0;
    let fail = true;
    setApi({ previewLive: async () => { calls++; if (fail) throw new Error("busy"); return [{ color: "#ffffff" }]; } } as unknown as KeylumeApi);
    const fx = (n: number) => ({ kind: "heartbeat" as const, color: "#ff0000", bpm: 30 + n });
    expect(await previewFrames(fx(1))).toEqual([]);
    fail = false;
    expect(await previewFrames(fx(1))).toEqual([{ color: "#ffffff" }]); // tried again
    expect(calls).toBe(2);
    await previewFrames(fx(1));
    expect(calls).toBe(2); // then cached
    for (let n = 2; n < 100; n++) await previewFrames(fx(n));
    expect(cachedPreviews()).toBeLessThanOrEqual(64);
  });
});
