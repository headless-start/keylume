// Per-key colours arrive packed: one "rrggbb…" string in layout order instead of a
// 68-entry map (a third of the size; the library has tens of thousands of designs).
// They're unpacked the first time something reads `lighting.keys`.
import type { Hex, Layout, Profile } from "./types";

type Packed = { kind: "perKey"; brightness?: number; packed: string };

function decode(packed: string, layout: Layout): Record<string, Hex> {
  const keys: Record<string, Hex> = {};
  layout.keys.forEach((k, i) => (keys[k.id] = `#${packed.slice(i * 6, i * 6 + 6)}`));
  return keys;
}

/** A profile as the UI uses it (`lighting.keys` decoded on first read). */
export function unpack(p: Profile, layout: Layout): Profile {
  const l = p.lighting as unknown as Packed;
  if (l.kind !== "perKey" || typeof l.packed !== "string") return p;
  let keys: Record<string, Hex> | undefined;
  const lighting = { kind: "perKey", brightness: l.brightness ?? 4 } as Profile["lighting"];
  Object.defineProperty(lighting, "keys", {
    enumerable: true,
    get: () => (keys ??= decode(l.packed, layout)),
    set: (v: Record<string, Hex>) => (keys = v),
  });
  Object.defineProperty(lighting, "packed", { value: l.packed }); // for quick colour stats (`colourFamily`)
  return { ...p, lighting };
}

/** The mock's library file (and the tests' fixture): layout plus packed profiles. */
export function unpackLibrary<T extends { layout: Layout; profiles: Profile[] }>(fx: T): T {
  return { ...fx, profiles: fx.profiles.map((p) => unpack(p, fx.layout)) };
}
