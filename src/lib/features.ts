// What the connected keyboard can do (keylume-device `boards::Features`), and which
// designs it can show. The app offers only these; the Library hides what doesn't fit.
import type { Features, Mode, Profile } from "./types";

/** Every firmware animation, in the keyboard's own order. */
const EVERY_MODE: Mode[] = [
  "off", "static", "breathing", "spectrum", "wave", "ripple", "raindrop", "snake", "reactive", "converge", "sine-wave",
  "kaleidoscope", "line-wave", "user-picture", "laser", "circle-wave", "dazzle", "rain-down", "meteor", "reactive-off",
  "music-bars", "screen-sync", "music-pulse",
];

/** A board that does everything (the TK68, `boards/epomaker-tk68.json`): what the app
 * assumes until a keyboard has connected. */
export const ALL_FEATURES: Features = {
  perKey: true, pictureLayers: 3, effects: EVERY_MODE, hostDriven: false, sideLight: true, live: true,
  keymap: true, macros: true, onboardProfiles: 3, settings: true, backup: true, readBack: true,
};

/** A HID LampArray keyboard (keylume-device `boards::lamparray_features`): Keylume draws
 * everything, and leaves out the animations that answer typing. */
export const LAMPARRAY_FEATURES: Features = {
  perKey: true, pictureLayers: 0, hostDriven: true, sideLight: false, live: true,
  effects: ["off", "static", "breathing", "spectrum", "wave", "raindrop", "snake", "converge", "sine-wave", "kaleidoscope", "line-wave", "circle-wave", "dazzle", "rain-down", "meteor"],
  keymap: false, macros: false, onboardProfiles: 0, settings: false, backup: false, readBack: false,
};

/** Can this keyboard show the design? */
export function canShow(p: Profile, f: Features): boolean {
  const l = p.lighting;
  switch (l.kind) {
    case "perKey": case "spell": return f.perKey;
    case "effect": return f.effects.includes(l.effect.mode);
    case "live": return f.live;
  }
}
