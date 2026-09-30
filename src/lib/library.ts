import { dimAll } from "./animate";
import { liveColor } from "./live";
import { effectPreview, livePreview } from "./preview";
import { finalFrame } from "./spell";
import type { Hex, Layout, Profile } from "./types";

export { liveColor };

/** Whose profiles to show. */
export type Scope = "all" | "favorites" | "mine";
/** Which kind of lighting ("any", or one kind). */
export type KindFilter = "any" | "perKey" | "effect" | "live";

export interface Filter {
  scope: Scope;
  kind: KindFilter;
  section: string; // "" = any
  category: string; // "" = any
  query: string;
  /** A colour family from `HUES` ("" = any). */
  hue: string;
}

/** Colour families for "show me designs in this colour", with the hue ranges they cover. */
export const HUES: { name: string; swatch: Hex; from: number; to: number }[] = [
  { name: "Red", swatch: "#ff2436", from: 345, to: 15 },
  { name: "Orange", swatch: "#ff7a1a", from: 15, to: 42 },
  { name: "Yellow", swatch: "#ffd21a", from: 42, to: 70 },
  { name: "Green", swatch: "#2ee85a", from: 70, to: 160 },
  { name: "Cyan", swatch: "#1ad8f0", from: 160, to: 200 },
  { name: "Blue", swatch: "#2f6bff", from: 200, to: 250 },
  { name: "Purple", swatch: "#9a4dff", from: 250, to: 290 },
  { name: "Pink", swatch: "#ff45c8", from: 290, to: 345 },
  { name: "White", swatch: "#eef1f7", from: -1, to: -1 },
];

/** How bright a profile runs, 0..4 (live effects always run at full brightness). */
export function brightnessOf(p: Profile): number {
  const l = p.lighting;
  return l.kind === "effect" ? l.effect.brightness : l.kind === "live" ? 4 : l.brightness;
}

/** A profile's picture as the keys would show it: its colours at its brightness (cards,
 * thumbnails). Editors load `previewColors`, which leaves brightness to the profile. */
export function stillColors(p: Profile, layout: Layout): Record<string, Hex> {
  return dimAll(previewColors(p, layout), brightnessOf(p));
}

/** Per-key colours to draw for a profile, at full brightness. */
export function previewColors(p: Profile, layout: Layout): Record<string, Hex> {
  const l = p.lighting;
  if (l.kind === "perKey") return l.keys;
  if (l.kind === "spell") return finalFrame(layout, l.words, l.background);
  // animated and live: a representative frame of the effect (see preview.ts)
  return l.kind === "effect" ? effectPreview(l.effect, layout) : livePreview(l.live, layout);
}

export function kindLabel(p: Profile): string {
  return { perKey: "Per-key", effect: "Animated", live: "Live", spell: "Spell" }[p.lighting.kind];
}

/** The colours a profile shows (still frame), without decoding packed per-key colours. */
function colours(p: Profile, layout: Layout): string[] {
  const packed = (p.lighting as { packed?: string }).packed;
  if (packed) return Array.from({ length: packed.length / 6 }, (_, i) => packed.slice(i * 6, i * 6 + 6));
  return Object.values(previewColors(p, layout)).map((c) => c.slice(1));
}

const familyCache = new WeakMap<Profile, string>();
/** The colour family (a `HUES` name) a profile mostly shows, weighted by how colourful each key is. */
export function colourFamily(p: Profile, layout: Layout): string {
  let f = familyCache.get(p);
  if (f !== undefined) return f;
  const w = new Map<string, number>();
  for (const c of colours(p, layout)) {
    const n = parseInt(c, 16);
    const r = (n >> 16) & 255, g = (n >> 8) & 255, b = n & 255;
    const max = Math.max(r, g, b), chroma = max - Math.min(r, g, b);
    if (max < 60) continue; // dark keys say little about the design's colour
    if (chroma < max * 0.3) { w.set("White", (w.get("White") ?? 0) + max / 255); continue; }
    const h = max === r ? ((g - b) / chroma + 6) % 6 : max === g ? (b - r) / chroma + 2 : (r - g) / chroma + 4;
    const deg = h * 60;
    const hue = HUES.find((x) => x.from >= 0 && (x.from < x.to ? deg >= x.from && deg < x.to : deg >= x.from || deg < x.to));
    if (hue) w.set(hue.name, (w.get(hue.name) ?? 0) + chroma / 255);
  }
  f = [...w].sort((a, b) => b[1] - a[1])[0]?.[0] ?? "";
  familyCache.set(p, f);
  return f;
}

/** The Library's sections, in order. */
export const SECTIONS = ["Games", "Comics", "Themes", "Colours", "Flags", "Effects"];
/** Collections that open their section; the rest keep the library's own order. */
const FEATURED = ["CS2", "CS2 Skins"];

/** A profile's collection ("Red · Animated" belongs with "Red"). */
export const collectionOf = (p: Profile) => p.category.replace(" · Animated", "");

/** Each section's collections (built-in profiles only), featured first, then in library order. */
export function sections(profiles: Profile[]): Map<string, string[]> {
  const out = new Map<string, string[]>(SECTIONS.map((s) => [s, []]));
  for (const p of profiles) {
    const list = p.section ? out.get(p.section) : undefined;
    const c = collectionOf(p);
    if (list && !list.includes(c)) list.push(c);
  }
  const rank = (c: string) => (FEATURED.includes(c) ? FEATURED.indexOf(c) : FEATURED.length);
  for (const [s, list] of out) {
    if (!list.length) out.delete(s);
    else list.sort((a, b) => rank(a) - rank(b)); // stable: library order otherwise
  }
  return out;
}

/** Every word of the query must match the name, category, tags or description. */
export function matches(p: Profile, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  const hay = `${p.name} ${p.category} ${p.tags.join(" ")} ${p.description}`.toLowerCase();
  return q.split(/\s+/).every((w) => hay.includes(w));
}

/** Does a profile pass the filter? (Favourites are checked against `favorites`.) */
export function passes(p: Profile, f: Filter, favorites: Set<string>): boolean {
  if (f.scope === "favorites" && !favorites.has(p.id)) return false;
  if (f.scope === "mine" && p.source !== "user") return false;
  if (f.kind !== "any" && p.lighting.kind !== f.kind) return false;
  if (f.section && p.section !== f.section) return false;
  if (f.category && collectionOf(p) !== f.category) return false;
  return matches(p, f.query);
}

/** The profiles that pass the filter, favourites in favourite order. */
export function filterProfiles(profiles: Profile[], f: Filter, favorites: string[]): Profile[] {
  const fav = new Set(favorites);
  const list = profiles.filter((p) => passes(p, f, fav));
  if (f.scope === "favorites") list.sort((a, b) => favorites.indexOf(a.id) - favorites.indexOf(b.id));
  return list;
}
