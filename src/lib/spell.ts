// Mirror of keylume-profiles::spell, for on-screen previews of a spell.
import { mix } from "./color";
import type { Hex, Layout, SpellWord } from "./types";

const WHITE: Hex = "#ffffff";

/** Key ids for the letters and digits of `text` that exist on the layout, in order. */
export function keysOf(layout: Layout, text: string): string[] {
  const ids = new Set(layout.keys.map((k) => k.id));
  return [...text].filter((c) => /[a-z0-9]/i.test(c)).map((c) => c.toLowerCase()).filter((id) => ids.has(id));
}

const blank = (layout: Layout, bg: Hex) => Object.fromEntries(layout.keys.map((k) => [k.id, bg])) as Record<string, Hex>;

/** Every word lit in its colour; keys shared by several words glow white. */
export function finalFrame(layout: Layout, words: SpellWord[], bg: Hex): Record<string, Hex> {
  const f = blank(layout, bg);
  const seen = new Map<string, number>();
  for (const w of words) {
    for (const id of new Set(keysOf(layout, w.text))) {
      seen.set(id, (seen.get(id) ?? 0) + 1);
      f[id] = w.color;
    }
  }
  for (const [id, n] of seen) if (n > 1) f[id] = WHITE;
  return f;
}

/** The spell as the keyboard will show it, one picture per letter, then the final picture. */
export function spellFrames(layout: Layout, words: SpellWord[], bg: Hex): Record<string, Hex>[] {
  const out: Record<string, Hex>[] = [];
  const done: [string[], Hex][] = [];
  for (const w of words) {
    const lit: string[] = [];
    for (const id of keysOf(layout, w.text)) {
      const f = blank(layout, bg);
      for (const [ids, c] of done) for (const k of ids) f[k] = mix("#000000", c, 0.12);
      for (const k of lit) f[k] = w.color;
      if (lit.includes(id)) f[id] = WHITE;
      else { f[id] = mix(w.color, WHITE, 0.55); lit.push(id); }
      out.push(f);
    }
    if (lit.length) done.push([lit, w.color]);
  }
  out.push(finalFrame(layout, words, bg));
  return out;
}
