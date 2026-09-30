// Representative still frames for animated and live profiles, so library cards show
// what an effect looks like, within what the keyboard can do: a wave looks like a wave,
// bars like bars, and a live effect that colours the whole board shows one colour, as
// the keyboard does at any moment.
import { hexToRgb, mix, rgbToHex } from "./color";
import { liveColor } from "./live";
import type { Effect, Hex, Layout, LayoutKey, LiveEffect } from "./types";

const BLACK: Hex = "#000000";

export function hsl(h: number, s = 1, l = 0.5): Hex {
  const k = (n: number) => (n + h / 30) % 12;
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
  return rgbToHex(f(0) * 255, f(8) * 255, f(4) * 255);
}

const dim = (c: Hex, k: number) => mix(BLACK, c, Math.max(0, Math.min(1, k)));

/** Stable 0..1 per key id (for "random" sparkles). */
function hash01(s: string, salt = 0): number {
  let h = 2166136261 ^ salt;
  for (const ch of s) h = Math.imul(h ^ ch.charCodeAt(0), 16777619);
  return ((h >>> 0) % 10000) / 10000;
}

interface Pos { x: number; y: number; d: number } // 0..1 across, 0..1 down, distance from centre 0..~1
function pos(l: Layout, k: LayoutKey): Pos {
  const x = (k.x + k.w / 2) / l.width;
  const y = (k.y + k.h / 2) / l.height;
  return { x, y, d: Math.hypot((x - 0.5) * 2, (y - 0.5) * 2 * (l.height / l.width) * 2.2) };
}

/** Bar-graph look (music modes and bar patterns): column heights from a gentle wave. */
function bars(l: Layout, color: (x: number) => Hex, seed = 0): Record<string, Hex> {
  return Object.fromEntries(l.keys.map((k) => {
    const p = pos(l, k);
    const h = 0.35 + 0.5 * (0.5 + 0.5 * Math.sin(p.x * 11 + seed)) + 0.15 * hash01(String(Math.round(p.x * 16)), seed);
    const lit = 1 - p.y < h;
    return [k.id, lit ? color(p.x) : dim(color(p.x), 0.06)]; // lit bars at full colour, as the keyboard draws them
  }));
}

export function effectPreview(e: Effect, l: Layout): Record<string, Hex> {
  const at = (p: Pos) => (e.rainbow ? hsl(p.x * 300) : e.color);
  const out: Record<string, Hex> = {};
  for (const k of l.keys) {
    const p = pos(l, k);
    const r = hash01(k.id);
    let c: Hex;
    switch (e.mode) {
      case "off": c = BLACK; break;
      case "spectrum": c = hsl(200); break; // the whole board cycles as one
      case "breathing": c = dim(e.rainbow ? hsl(200) : e.color, 0.7); break; // every key breathes together
      case "wave": case "line-wave": case "sine-wave": {
        const along = e.mode === "wave" && e.direction >= 2 ? p.y : p.x;
        const rev = e.direction % 2 === 1 ? -1 : 1;
        c = dim(at(p), 0.2 + 0.8 * (0.5 + 0.5 * Math.cos((along * 2.2 * rev) * Math.PI * 2)));
        break;
      }
      case "ripple": case "circle-wave": case "kaleidoscope": case "converge":
        c = dim(at(p), 0.15 + 0.85 * (0.5 + 0.5 * Math.cos(p.d * 14))); break;
      case "raindrop": case "rain-down": case "meteor":
        // scattered stars / drops at different brightnesses
        c = r < 0.22 ? dim(at(p), 0.4 + r * 2.7) : dim(at(p), e.mode === "meteor" && r < 0.35 ? 0.35 : 0.07); break;
      case "dazzle": c = e.rainbow ? hsl(r * 360) : dim(e.color, 0.25 + 0.75 * r); break;
      case "snake": c = Math.abs(p.y * 5 - 2) < 0.6 && p.x < 0.7 ? at(p) : dim(at(p), 0.07); break;
      case "reactive": c = r < 0.12 ? at(p) : r < 0.2 ? dim(at(p), 0.35) : dim(at(p), 0.04); break; // a few ghosts
      case "laser": c = Math.abs(p.y - 0.5) < 0.12 && p.x > 0.3 ? at(p) : r < 0.08 ? at(p) : dim(at(p), 0.05); break;
      case "reactive-off": c = r < 0.12 ? dim(at(p), 0.05) : at(p); break;
      case "music-bars": case "music-pulse": c = BLACK; break; // drawn below
      default: c = at(p);
    }
    out[k.id] = c;
  }
  if (e.mode === "music-bars" || e.mode === "music-pulse") return bars(l, (x) => (e.rainbow ? hsl(x * 300) : e.color));
  return out;
}

/** The most eye-catching of some colours (colourful first, then bright). */
function vivid(colors: Hex[]): Hex {
  const score = (c: Hex) => {
    const [r, g, b] = hexToRgb(c);
    return (Math.max(r, g, b) - Math.min(r, g, b)) * 2 + Math.max(r, g, b);
  };
  return colors.reduce((best, c) => (score(c) > score(best) ? c : best), colors[0] ?? "#0040ff");
}

/** The colour a whole-board live effect shows at a moment that represents it: palettes
 * pass through each of their colours, so their most vivid one is a real moment too. */
function liveMoment(v: LiveEffect): Hex {
  switch (v.kind) {
    case "paletteFlow": case "trip": case "lava": case "breathe": case "rave": case "beatFlash": case "aurora": case "musicFlow": case "steps":
      return vivid(v.colors);
    case "storm": return mix(v.sky, v.flash, 0.5); // a strike fading
    case "screenAmbient": return hsl(215, 0.55, 0.45);
    default: return liveColor(v);
  }
}

export function livePreview(v: LiveEffect, l: Layout): Record<string, Hex> {
  switch (v.kind) {
    // the 32 bars of the music modes
    case "spectrum": return bars(l, (x) => (v.rainbow ? hsl(x * 300) : v.color));
    case "bars": return bars(l, (x) => (v.rainbow ? hsl(x * 300) : v.color), v.style.length);
    case "sineBars": case "scanner": case "equalizer": return bars(l, () => v.color, v.kind.length);
    case "rainBars": return bars(l, () => v.color, 3);
    // everything else sets the whole board to one colour at a time
    default: {
      const c = liveMoment(v);
      return Object.fromEntries(l.keys.map((k) => [k.id, c]));
    }
  }
}

/** Average brightness 0..1 of a preview (tests: nothing should be blank). */
export function brightness(colors: Record<string, Hex>): number {
  const v = Object.values(colors).map((c) => (c.startsWith("#") ? hexToRgb(c).reduce((a, b) => a + b, 0) / 765 : 0.5));
  return v.reduce((a, b) => a + b, 0) / Math.max(1, v.length);
}
