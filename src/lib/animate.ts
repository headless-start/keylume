// Moving previews of dynamic profiles (library hover). Keyboard animations are
// simulated here from the firmware effect's settings (the keyboard can't report where it
// is in one, so these are approximations); live effects are played from frames rendered
// by the real animator (`previewLive`), mapped onto the keys the way the keyboard shows
// them; spells replay their letter-by-letter pictures.
import { mix } from "./color";
import { hsl } from "./preview";
import { spellFrames } from "./spell";
import type { Effect, Hex, Layout, LayoutKey, LiveEffect, LiveFrame, Profile } from "./types";

type Colors = Record<string, Hex>;
const BLACK: Hex = "#000000";
const dim = (c: Hex, k: number) => mix(BLACK, c, Math.max(0, Math.min(1, k)));
const TAU = Math.PI * 2;

function hash01(n: number): number {
  const x = Math.sin(n * 127.1 + 311.7) * 43758.5453;
  return x - Math.floor(x);
}

interface K { id: string; x: number; y: number; row: number; order: number }

/** Key geometry in 0..1 units, cached per layout. */
const geoCache = new WeakMap<Layout, K[]>();
function geo(l: Layout): K[] {
  let g = geoCache.get(l);
  if (!g) {
    const keys = [...l.keys].sort((a, b) => a.y - b.y || a.x - b.x);
    g = keys.map((k: LayoutKey, i) => ({ id: k.id, x: (k.x + k.w / 2) / l.width, y: (k.y + k.h / 2) / l.height, row: Math.round(k.y), order: i }));
    geoCache.set(l, g);
  }
  return g;
}

/** Simulated key presses: which key was pressed at slot n (every `every` seconds). */
function press(g: K[], n: number): K {
  return g[Math.floor(hash01(n) * g.length)];
}

/** Keyboard animations that answer key presses (they sit dark until you type). */
export const REACTIVE: Effect["mode"][] = ["ripple", "reactive", "reactive-off", "laser"];

/**
 * One frame of a keyboard (firmware) animation at time t (seconds). `presses: false`
 * leaves out the made-up key presses that show off the reactive modes: what a mirror of
 * the real keyboard shows, since Keylume can't see your typing.
 */
export function effectFrame(e: Effect, l: Layout, t: number, { presses = true } = {}): Colors {
  const g = geo(l);
  const s = 0.35 + e.speed * 0.35; // speed 0..4 -> 0.35..1.75
  const out: Colors = {};
  const colorAt = (k: K, shift = 0) => (e.rainbow ? hsl(((k.x * 300 + t * 60 * s + shift) % 360 + 360) % 360) : e.color);
  // recent simulated presses for the reactive modes
  const every = 0.28 / s;
  const recent = (span: number) => {
    const out: { k: K; age: number }[] = [];
    if (!presses) return out;
    for (let n = Math.floor(t / every); n > Math.floor((t - span) / every); n--) out.push({ k: press(g, n), age: t - n * every });
    return out;
  };
  switch (e.mode) {
    case "off": for (const k of g) out[k.id] = BLACK; return out;
    case "static": case "user-picture": case "screen-sync": for (const k of g) out[k.id] = colorAt(k); return out;
    case "breathing": {
      const b = 0.08 + 0.92 * (0.5 - 0.5 * Math.cos((t * s * TAU) / 3));
      const breath = Math.floor((t * s) / 3);
      for (const k of g) out[k.id] = dim(e.rainbow ? hsl((breath * 60) % 360) : e.color, b);
      return out;
    }
    case "spectrum": { const c = hsl((t * 50 * s) % 360); for (const k of g) out[k.id] = c; return out; }
    case "wave": {
      const vertical = e.direction >= 2, sign = e.direction % 2 === 1 ? 1 : -1;
      for (const k of g) {
        const along = vertical ? k.y : k.x;
        out[k.id] = e.rainbow ? hsl(((along * 360 + sign * t * 120 * s) % 360 + 360) % 360) : dim(e.color, 0.2 + 0.8 * (0.5 + 0.5 * Math.cos(TAU * (along * 1.2 + sign * t * s * 0.6))));
      }
      return out;
    }
    case "line-wave": {
      const sign = e.direction === 1 ? -1 : 1;
      for (const k of g) {
        const p = ((k.x - sign * t * s * 0.5) % 1 + 1) % 1;
        out[k.id] = dim(colorAt(k), 0.08 + 0.92 * Math.exp(-(((p - 0.5) * 7) ** 2)));
      }
      return out;
    }
    case "sine-wave":
      for (const k of g) {
        const crest = 0.5 + 0.32 * Math.sin(TAU * (k.x * 1.3 - t * s * 0.5));
        out[k.id] = dim(colorAt(k), 0.08 + 0.92 * Math.exp(-(((k.y - crest) * 5) ** 2)));
      }
      return out;
    case "ripple": {
      const presses = recent(1.2);
      for (const k of g) {
        let v = 0.05;
        for (const p of presses) {
          const d = Math.hypot((k.x - p.k.x) * 3.2, k.y - p.k.y);
          v = Math.max(v, Math.exp(-(((d - p.age * 2.2 * s) * 5) ** 2)) * (1 - p.age / 1.2));
        }
        out[k.id] = dim(colorAt(k), v);
      }
      return out;
    }
    case "kaleidoscope": case "converge": {
      const dir = e.mode === "converge" || e.direction === 1 ? 1 : -1;
      for (const k of g) {
        const d = Math.hypot((k.x - 0.5) * 3.2, k.y - 0.5);
        out[k.id] = e.rainbow ? hsl(((d * 400 + dir * t * 150 * s) % 360 + 360) % 360) : dim(e.color, 0.1 + 0.9 * (0.5 + 0.5 * Math.cos(d * 12 + dir * t * s * 4)));
      }
      return out;
    }
    case "circle-wave": {
      const dir = e.direction === 1 ? 1 : -1;
      for (const k of g) {
        const a = Math.atan2((k.y - 0.5) * 0.4, k.x - 0.5);
        out[k.id] = e.rainbow ? hsl((((a / TAU) * 360 + dir * t * 120 * s) % 360 + 360) % 360) : dim(e.color, 0.1 + 0.9 * (0.5 + 0.5 * Math.cos(a - dir * t * s * 2.5)));
      }
      return out;
    }
    case "raindrop": // starlight: every key twinkles on its own clock
      for (const k of g) {
        const ph = ((t * s * 0.6 + hash01(k.order) * 7) % 3) / 3;
        out[k.id] = dim(e.rainbow ? hsl(hash01(k.order + Math.floor(t * s * 0.2)) * 360) : e.color, ph < 0.25 ? Math.sin((ph / 0.25) * Math.PI) : 0.04);
      }
      return out;
    case "rain-down": case "meteor": {
      const tail = e.mode === "meteor" ? 0.45 : 0.18;
      for (const k of g) {
        const col = Math.round(k.x * 16);
        const head = ((t * s * 0.7 + hash01(col) * 3) % 1.4) - 0.2;
        const behind = head - k.y;
        out[k.id] = dim(colorAt(k), behind >= 0 && behind < tail ? 1 - behind / tail : 0.04);
      }
      return out;
    }
    case "snake": {
      const n = g.length, head = (t * s * 14) % n;
      for (const k of g) {
        const behind = (head - k.order + n) % n; // row-by-row order approximates the path
        out[k.id] = dim(colorAt(k), behind < 10 ? 1 - behind / 10 : 0.04);
      }
      return out;
    }
    case "reactive": case "reactive-off": {
      const presses = recent(1.6 / s);
      for (const k of g) {
        let v = 0;
        for (const p of presses) if (p.k.id === k.id) v = Math.max(v, Math.exp(-p.age * 2.2 * s));
        out[k.id] = e.mode === "reactive" ? dim(colorAt(k), 0.04 + 0.96 * v) : dim(colorAt(k), 1 - 0.96 * v);
      }
      return out;
    }
    case "laser": {
      const presses = recent(0.9);
      for (const k of g) {
        let v = 0.04;
        for (const p of presses) {
          if (p.k.row !== k.row) continue;
          const reach = p.age * 1.8 * s;
          const d = Math.abs(k.x - p.k.x);
          if (d <= reach && d > reach - 0.25) v = Math.max(v, 1 - p.age / 0.9);
        }
        out[k.id] = dim(colorAt(k), v);
      }
      return out;
    }
    case "dazzle": {
      const tick = Math.floor(t * s * 3);
      for (const k of g) out[k.id] = e.rainbow ? hsl(hash01(k.order * 7 + tick) * 360) : dim(e.color, 0.2 + 0.8 * hash01(k.order * 7 + tick));
      return out;
    }
    case "music-bars": case "music-pulse":
      return barsFrame(l, Array.from({ length: 32 }, (_, i) => Math.round(6 * (0.5 + 0.5 * Math.sin(i * 0.45 - t * 4)))), e.color, e.rainbow);
  }
}

/** 32 band levels (0..6) as the keyboard draws them: bars rising from the bottom row. */
function barsFrame(l: Layout, levels: number[], color: Hex, rainbow: boolean): Colors {
  const out: Colors = {};
  for (const k of geo(l)) {
    const band = Math.min(31, Math.floor(k.x * 32));
    const lit = (1 - k.y) * 6 < levels[band] + 0.4;
    const c = rainbow ? hsl(k.x * 300) : color;
    out[k.id] = lit ? c : dim(c, 0.04);
  }
  return out;
}

/** A live frame on the keys: whole-board colour, or bars. */
export function liveFrameColors(f: LiveFrame, live: LiveEffect, l: Layout): Colors {
  if ("color" in f) {
    const out: Colors = {};
    for (const k of l.keys) out[k.id] = f.color;
    return out;
  }
  const color = "color" in live ? (live as { color: Hex }).color : "#0060ff";
  const rainbow = "rainbow" in live ? Boolean((live as { rainbow: boolean }).rainbow) : false;
  return barsFrame(l, f.levels, color, rainbow);
}

export const isDynamic = (p: Profile) => p.lighting.kind !== "perKey";

/** Colours as the keyboard shows them at brightness `b` (0..4): light scales linearly. */
export function dimAll(colors: Colors, b: number): Colors {
  if (b >= 4) return colors;
  const k = Math.max(0, b) / 4;
  const out: Colors = {};
  for (const [id, c] of Object.entries(colors)) out[id] = dim(c, k);
  return out;
}

/** Frame source for a profile: (t) => colours. `live` frames must be fetched first. */
export function player(p: Profile, l: Layout, liveFrames?: LiveFrame[]): ((t: number) => Colors) | null {
  const play = rawPlayer(p, l, liveFrames);
  // animation clocks can start a hair before "now": never ask for a negative time
  return play && ((t: number) => play(Math.max(0, t)));
}

function rawPlayer(p: Profile, l: Layout, liveFrames?: LiveFrame[]): ((t: number) => Colors) | null {
  const li = p.lighting;
  switch (li.kind) {
    case "perKey": return null;
    case "effect": return (t) => dimAll(effectFrame(li.effect, l, t), li.effect.brightness);
    case "spell": {
      // a preview of what a spell does, sped up and repeated (Home shows a real one's progress)
      const frames = spellFrames(l, li.words, li.background).map((f) => dimAll(f, li.brightness));
      const step = 0.45, hold = 1.6, total = frames.length * step + hold;
      return (t) => frames[Math.min(frames.length - 1, Math.floor((t % total) / step))];
    }
    case "live": {
      if (!liveFrames?.length) return null;
      const fps = 20;
      return (t) => liveFrameColors(liveFrames[Math.floor(t * fps) % liveFrames.length], li.live, l);
    }
  }
}
