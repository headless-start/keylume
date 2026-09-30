// Mirrors of the Rust types that cross the IPC boundary (serde camelCase).

export type Hex = string; // "#rrggbb"

export interface LayoutKey {
  id: string;
  label: string;
  x: number;
  y: number;
  w: number;
  h: number;
  slot: number;
  hid: number;
}

export interface Layout {
  id: string;
  name: string;
  /** The keyboard's own colour, for drawing it as it looks on the desk (Home). */
  finish?: "dark" | "white";
  /** What the Fn layer does by default, by key id (keylume-proto `Layout::fn_layer`). */
  fnLayer?: Record<string, KeyAction>;
  width: number;
  height: number;
  userPictureLayers: number;
  onboardProfiles: number;
  keys: LayoutKey[];
  /** LEDs with no key of their own (ISO positions), lit with the key they sit under. */
  hiddenLeds?: { slot: number; under: string }[];
}

export type Mode =
  | "off" | "static" | "breathing" | "spectrum" | "wave" | "ripple" | "raindrop" | "snake"
  | "reactive" | "converge" | "sine-wave" | "kaleidoscope" | "line-wave" | "user-picture"
  | "laser" | "circle-wave" | "dazzle" | "rain-down" | "meteor" | "reactive-off"
  | "music-bars" | "screen-sync" | "music-pulse";

export interface Effect {
  mode: Mode;
  speed: number; // 0..4
  brightness: number; // 0..4
  direction: number;
  rainbow: boolean;
  color: Hex;
}

export type LiveEffect =
  | { kind: "paletteFlow"; colors: Hex[]; period: number }
  | { kind: "heartbeat"; color: Hex; bpm: number }
  | { kind: "flicker"; color: Hex; intensity: number }
  | { kind: "tide"; a: Hex; b: Hex; period: number }
  | { kind: "storm"; sky: Hex; flash: Hex; rate: number }
  | { kind: "strobe"; a: Hex; b: Hex; hz: number }
  | { kind: "audioGlow"; quiet: Hex; loud: Hex }
  | { kind: "cpuHeat"; cool: Hex; hot: Hex }
  | { kind: "screenAmbient"; saturation: number }
  | { kind: "spectrum"; color: Hex; rainbow: boolean; mirror: boolean }
  | { kind: "sineBars"; color: Hex; speed: number }
  | { kind: "rainBars"; color: Hex; rate: number }
  | { kind: "scanner"; color: Hex; speed: number }
  | { kind: "equalizer"; color: Hex; speed: number }
  | { kind: "trip"; colors: Hex[]; speed: number }
  | { kind: "lava"; colors: Hex[]; speed: number }
  | { kind: "breathe"; colors: Hex[]; bpm: number }
  | { kind: "rave"; colors: Hex[]; bpm: number }
  | { kind: "morse"; message: string; color: Hex; background: Hex; wpm: number }
  | { kind: "bomb"; color: Hex; blast: Hex; fuse: number }
  | { kind: "flashbang"; color: Hex; every: number }
  | { kind: "focus"; work: Hex; rest: Hex; minutes: number; breakMinutes: number }
  | { kind: "beatFlash"; colors: Hex[] }
  | { kind: "aurora"; colors: Hex[]; speed: number }
  | { kind: "sunrise"; minutes: number }
  | { kind: "countdown"; color: Hex; warn: Hex; minutes: number }
  | { kind: "musicFlow"; colors: Hex[] }
  | { kind: "bars"; style: BarStyle; color: Hex; rainbow: boolean; speed: number }
  | { kind: "siren"; a: Hex; b: Hex; speed: number }
  | { kind: "glitter"; base: Hex; spark: Hex; density: number }
  | { kind: "pacedBreathing"; color: Hex; inhale: number; hold: number; exhale: number; rest: number }
  | { kind: "steps"; colors: Hex[]; hold: number };

export type BarStyle =
  | "plasma" | "bounce" | "fire" | "fountain" | "helix" | "ecg"
  | "stacker" | "glitch" | "pump" | "fireflies" | "terrain"
  | "fireworks" | "comet" | "matrix" | "snake" | "vu" | "ripple"
  | "pendulum" | "lightning" | "rally" | "hourglass" | "loading" | "collide";

export interface SpellWord { text: string; color: Hex }

/** One frame of a live effect: the whole board's colour, or 32 bar levels (0..6). */
export type LiveFrame = { color: Hex } | { levels: number[] };

export type Lighting =
  | { kind: "effect"; effect: Effect }
  | { kind: "perKey"; keys: Record<string, Hex>; brightness: number }
  | { kind: "live"; live: LiveEffect }
  | { kind: "spell"; words: SpellWord[]; background: Hex; brightness: number };

export type SideMode = "off" | "static" | "breathing" | "neon" | "wave" | "snake";

export interface SideLight {
  mode: SideMode;
  speed: number; // 0..4
  brightness: number; // 0..4
  direction: number;
  rainbow: boolean;
  color: Hex;
}

export interface Profile {
  id: string;
  name: string;
  category: string;
  tags: string[];
  description: string;
  source: "builtin" | "user";
  /** The Library section a built-in profile is listed in (Games, Themes, Colours, Flags, Effects). */
  section?: string;
  lighting: Lighting;
}

/** How far Keylume trusts its support for a board: checked on the real keyboard, or not yet. */
export type Support = "verified" | "experimental";

/** What a board can do (keylume-device `boards::Features`); the app offers only these. */
export interface Features {
  perKey: boolean;
  pictureLayers: number;
  /** Animations it has: run by the keyboard, or drawn by Keylume when `hostDriven`. */
  effects: Mode[];
  /** Keylume drives every light while it runs (a HID LampArray keyboard). */
  hostDriven: boolean;
  sideLight: boolean;
  live: boolean;
  keymap: boolean;
  macros: boolean;
  onboardProfiles: number;
  /** Report rate, debounce and sleep timers. */
  settings: boolean;
  backup: boolean;
  readBack: boolean;
}

export interface DeviceInfo {
  /** A board file's id, or `lamparray-<vid>-<pid>`. */
  board: string;
  name: string;
  maker: string;
  support: Support;
  features: Features;
  layoutId: string;
  vid: number;
  pid: number;
  product: string;
  manufacturer: string;
  path: string;
  simulated: boolean;
}

export interface Status {
  connected: boolean;
  device: DeviceInfo | null;
  firmware: number | null;
  live: string | null;
  error: string | null;
  applied: number;
  paused: string | null;
  /** A backup, restore or factory reset in progress: nothing else may use the keyboard. */
  busy: string | null;
}

/** Where the lighting stands (see keylume-core `lighting`). */
export type LightingPhase = "requested" | "pending" | "applied" | "failed" | "off" | "unknown" | "disconnected" | "paused";
export type ShownOrigin = "profile" | "preview" | "off" | "restored" | "reset" | "keyboard";

/** What the keys show: known because the keyboard accepted it (or it was read back). */
export interface Shown {
  /** The request that put it there (0 when it didn't come from one). */
  request: number;
  origin: ShownOrigin;
  profileId: string | null;
  name: string;
  /** In design colours (True colours are applied on the way to the keyboard, not here). */
  lighting: Lighting | null;
  /** 0..4 as the keyboard has it. */
  brightness: number;
  /** A live effect streaming or a spell spelling; false once stopped (the last frame stays). */
  running: boolean;
  /** Spells: pictures written so far, of how many. */
  progress: { done: number; total: number } | null;
}

export interface LightingState {
  /** Bumped on every change: keep the newest. */
  seq: number;
  phase: LightingPhase;
  /** The newest request. */
  request: { id: number; profileId: string | null; name: string } | null;
  /** Why the newest request failed. */
  error: string | null;
  shown: Shown | null;
}

/** One frame a live effect sent to the keyboard, in design colours. */
export interface LiveFrameEvent { request: number; seq: number; t: number; frame: LiveFrame }

export interface LightingSnapshot { state: LightingState; frame: LiveFrameEvent | null }

/** What an upload added, and the files that added nothing: [file name, why]. */
export interface Upload { added: string[]; packs: PackInfo[]; failed: [string, string][] }

/** What its maker lets people do with a pack (keylume-core `packs::Licence`). */
export type Licence = "share" | "personal";

/** A design pack the user added (keylume-core `packs::PackInfo`). */
export interface PackInfo {
  id: string;
  name: string;
  /** From the signing key when official, else what the file says. */
  publisher: string;
  version: string;
  description: string;
  /** Signed by a publisher Keylume knows (see docs/PACKS.md). */
  official: boolean;
  /** Its collections; its designs made in Keylume are listed under the pack's name. */
  collections: string[];
  themes: number;
  designs: number;
  /** It comes with Keylume: a newer file can update it, nothing removes it. */
  bundled: boolean;
  /** Signed by its maker's own key: the key's fingerprint ("3F9A B2C1"). */
  makerKey?: string;
  licence?: Licence;
  /** The maker's page (https). */
  url?: string;
}

/** A pack of your own designs, to make (keylume-core `packs::MakeRequest`). */
export interface MakeRequest {
  name: string;
  maker: string;
  version: string;
  description: string;
  licence: Licence;
  url: string;
  /** Ids of your own profiles. */
  designs: string[];
}

/** What you put in the packs you made before (keylume-core `store::Maker`, without the key). */
export interface Maker {
  name: string;
  url: string;
  licence: Licence;
  made: { id: string; name: string; version: string; description: string }[];
}

/** A backup file that passed every check, waiting for the user's go-ahead. */
export interface StagedBackup { token: number; file: string; firmware: string; macros: number; sideLight: boolean }

export interface AppSettings {
  favorites: string[];
  lastProfile: string | null;
  liveLayer: number;
  demoMode: boolean;
  /** Which keyboard the demo plays: the TK68, or a full-size keyboard Keylume drives light by light. */
  demoBoard: "tk68" | "lamparray";
  startMinimized: boolean;
  launchAtLogin: boolean;
  hotkeys: boolean;
  restoreOnConnect: boolean;
  sideFollow: boolean;
  sideCustom: SideLight;
  shuffleMinutes: number;
  /** Correct colours for the LEDs so the keys look like the screen. */
  trueColors: boolean;
}


export type KeyAction =
  | { kind: "disabled" }
  | { kind: "key"; code: number; modifier: number; code2: number }
  | { kind: "mouse"; code: number; arg: number }
  | { kind: "system"; code: number }
  | { kind: "consumer"; usage: number }
  | { kind: "profile"; op: number; arg: number }
  | { kind: "macro"; index: number; mode: number }
  | { kind: "fn" }
  | { kind: "raw"; bytes: [number, number, number, number] };

export type MacroEvent = { kind: "key"; code: number; down: boolean } | { kind: "delay"; ms: number };

export interface Macro {
  repeat: number;
  events: MacroEvent[];
}

export interface SleepTimers {
  bluetooth: number;
  wireless: number;
  deepBluetooth: number;
  deepWireless: number;
}

export interface KeyboardOptions {
  winKeyLock: boolean;
  macMode: boolean;
  wasdArrowsSwap: boolean;
  ledOff: boolean;
  sideLedOff: boolean;
  keyboardMode: boolean;
  keyboardLock: boolean;
  fnMatrix: boolean;
  powerSave: boolean;
}

export interface DeviceSettings {
  reportRate: number;
  debounce: number;
  sleep: SleepTimers;
  options: KeyboardOptions;
  profile: number;
  firmware: number;
}

export type KeyLayer = "base" | "fn";
