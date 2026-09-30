// The Library shows designs, not every variant: "Deep Ocean · Cascade", "Deep Ocean ·
// Flame" and "Deep Ocean · Wave Animation" are one design (a family) in many looks.
import { collectionOf } from "./library";
import type { Profile } from "./types";

export type VariantKind = Profile["lighting"]["kind"];

export interface Family {
  /** Collection + design name; your own profiles stand alone. */
  key: string;
  name: string;
  collection: string;
  section?: string;
  variants: Profile[];
}

const SEP = " · ";

/** The design a profile belongs to: its name before " · ". */
export function familyName(p: Profile): string {
  const i = p.name.indexOf(SEP);
  return i < 0 ? p.name : p.name.slice(0, i);
}

export const familyKey = (p: Profile): string => (p.source === "user" ? `user|${p.id}` : `${collectionOf(p)}|${familyName(p)}`);

/** How a variant is named inside its family ("Circle Wave Animation" -> "Circle Wave"). */
export function variantLabel(p: Profile): string {
  const i = p.name.indexOf(SEP);
  if (i < 0) return "Original";
  const rest = p.name.slice(i + SEP.length);
  return p.lighting.kind === "effect" ? rest.replace(/ Animation$/, "") : rest;
}

/** Every profile grouped into its family, in library order (a family sits where its first profile does). */
export function groupFamilies(profiles: Profile[]): { families: Family[]; byProfile: Map<string, Family> } {
  const byKey = new Map<string, Family>();
  const byProfile = new Map<string, Family>();
  for (const p of profiles) {
    const key = familyKey(p);
    let f = byKey.get(key);
    if (!f) {
      f = { key, name: familyName(p), collection: p.source === "user" ? "Mine" : collectionOf(p), section: p.section, variants: [] };
      byKey.set(key, f);
    }
    f.variants.push(p);
    byProfile.set(p.id, f);
  }
  return { families: [...byKey.values()], byProfile };
}

export const KIND_ORDER: VariantKind[] = ["perKey", "effect", "live", "spell"];
export const KIND_TITLE: Record<VariantKind, string> = { perKey: "Per-key", effect: "Animated", live: "Live", spell: "Spell" };

/** A family's variants by kind, in a fixed order, empty kinds left out. */
export function byKind(variants: Profile[]): [VariantKind, Profile[]][] {
  return KIND_ORDER.map((k) => [k, variants.filter((v) => v.lighting.kind === k)] as [VariantKind, Profile[]]).filter(([, v]) => v.length > 0);
}
