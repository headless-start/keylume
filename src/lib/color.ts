import type { Hex, LayoutKey } from "./types";

export function hexToRgb(h: Hex): [number, number, number] {
  const s = h.replace("#", "");
  const full = s.length === 3 ? s.split("").map((c) => c + c).join("") : s.padEnd(6, "0");
  const n = parseInt(full.slice(0, 6), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

export function rgbToHex(r: number, g: number, b: number): Hex {
  const c = (v: number) => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, "0");
  return `#${c(r)}${c(g)}${c(b)}`;
}

export function isHex(s: string): boolean {
  return /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.test(s.trim());
}

export function normalizeHex(s: string): Hex {
  const [r, g, b] = hexToRgb(s.trim().startsWith("#") ? s.trim() : `#${s.trim()}`);
  return rgbToHex(r, g, b);
}

/** Blend in linear light (same maths as the Rust profile generator). */
export function mix(a: Hex, b: Hex, t: number): Hex {
  const lin = (c: number) => { const v = c / 255; return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; };
  const srgb = (v: number) => 255 * (v <= 0.0031308 ? v * 12.92 : 1.055 * v ** (1 / 2.4) - 0.055);
  const k = Math.max(0, Math.min(1, t));
  const A = hexToRgb(a), B = hexToRgb(b);
  const out = A.map((x, i) => srgb(lin(x) + (lin(B[i]) - lin(x)) * k));
  return rgbToHex(out[0], out[1], out[2]);
}

/** Perceived brightness 0..1 (for choosing label colour on a key). */
export function luminance(h: Hex): number {
  const [r, g, b] = hexToRgb(h);
  return (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
}

const centre = (k: LayoutKey) => [k.x + k.w / 2, k.y + k.h / 2] as const;

/**
 * Linear gradient between two keys: every key is projected onto the line from `from`
 * to `to` and coloured by its position along it.
 */
export function gradientBetween(keys: LayoutKey[], from: LayoutKey, to: LayoutKey, a: Hex, b: Hex, only?: Set<string>): Record<string, Hex> {
  const [x0, y0] = centre(from);
  const [x1, y1] = centre(to);
  const dx = x1 - x0, dy = y1 - y0;
  const len2 = dx * dx + dy * dy || 1;
  const out: Record<string, Hex> = {};
  for (const k of keys) {
    if (only && !only.has(k.id)) continue;
    const [x, y] = centre(k);
    const t = ((x - x0) * dx + (y - y0) * dy) / len2;
    out[k.id] = mix(a, b, t);
  }
  return out;
}

export const BLUE_SWATCHES: Hex[] = [
  "#00fff0", "#00e5ff", "#7df9ff", "#00c8ff", "#00a8ff", "#1e90ff", "#0080ff",
  "#0060ff", "#0050ff", "#0040ff", "#2962ff", "#3d5afe", "#1a28a8", "#001455",
];

export const OTHER_SWATCHES: Hex[] = [
  "#ffffff", "#e0b0ff", "#8a2be2", "#ff00c8", "#ff2a6d", "#ff4000", "#ff8c00",
  "#ffd000", "#b8ff60", "#00ff60", "#00ffc8", "#000000",
];
