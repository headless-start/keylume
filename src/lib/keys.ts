import type { KeyAction, Layout } from "./types";

// HID keyboard usages (USB HID Usage Tables, page 0x07) available for remapping.
export interface HidKey { code: number; name: string; group: string }

const k = (code: number, name: string, group: string): HidKey => ({ code, name, group });

export const HID_KEYS: HidKey[] = [
  ...Array.from({ length: 26 }, (_, i) => k(0x04 + i, String.fromCharCode(65 + i), "Letters")),
  ...["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"].map((n, i) => k(0x1e + i, n, "Numbers")),
  k(0x28, "Enter", "Editing"), k(0x29, "Esc", "Editing"), k(0x2a, "Backspace", "Editing"), k(0x2b, "Tab", "Editing"),
  k(0x2c, "Space", "Editing"), k(0x4c, "Delete", "Editing"), k(0x49, "Insert", "Editing"),
  k(0x2d, "-", "Symbols"), k(0x2e, "=", "Symbols"), k(0x2f, "[", "Symbols"), k(0x30, "]", "Symbols"), k(0x31, "\\", "Symbols"),
  k(0x33, ";", "Symbols"), k(0x34, "'", "Symbols"), k(0x35, "`", "Symbols"), k(0x36, ",", "Symbols"), k(0x37, ".", "Symbols"), k(0x38, "/", "Symbols"),
  k(0x39, "Caps Lock", "Locks"), k(0x47, "Scroll Lock", "Locks"), k(0x53, "Num Lock", "Locks"),
  ...Array.from({ length: 12 }, (_, i) => k(0x3a + i, `F${i + 1}`, "Function")),
  ...Array.from({ length: 12 }, (_, i) => k(0x68 + i, `F${i + 13}`, "Function")),
  k(0x46, "Print Screen", "System"), k(0x48, "Pause", "System"), k(0x65, "Menu", "System"),
  k(0x4a, "Home", "Navigation"), k(0x4b, "Page Up", "Navigation"), k(0x4d, "End", "Navigation"), k(0x4e, "Page Down", "Navigation"),
  k(0x4f, "→", "Navigation"), k(0x50, "←", "Navigation"), k(0x51, "↓", "Navigation"), k(0x52, "↑", "Navigation"),
  ...["/", "*", "-", "+", "Enter", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "."].map((n, i) => k(0x54 + i, `Num ${n}`, "Numpad")),
  k(0xe0, "Left Ctrl", "Modifiers"), k(0xe1, "Left Shift", "Modifiers"), k(0xe2, "Left Alt", "Modifiers"), k(0xe3, "Left Win", "Modifiers"),
  k(0xe4, "Right Ctrl", "Modifiers"), k(0xe5, "Right Shift", "Modifiers"), k(0xe6, "Right Alt", "Modifiers"), k(0xe7, "Right Win", "Modifiers"),
];

export const MODIFIERS = [
  { code: 0, name: "None" },
  { code: 0xe0, name: "Ctrl" },
  { code: 0xe1, name: "Shift" },
  { code: 0xe2, name: "Alt" },
  { code: 0xe3, name: "Win" },
];

export const CONSUMER = [
  { usage: 0x00cd, name: "Play / Pause" }, { usage: 0x00b7, name: "Stop" },
  { usage: 0x00b5, name: "Next track" }, { usage: 0x00b6, name: "Previous track" },
  { usage: 0x00e2, name: "Mute" }, { usage: 0x00e9, name: "Volume up" }, { usage: 0x00ea, name: "Volume down" },
  { usage: 0x0183, name: "Media player" }, { usage: 0x018a, name: "Mail" }, { usage: 0x0192, name: "Calculator" },
  { usage: 0x0194, name: "My Computer" }, { usage: 0x0221, name: "Search" }, { usage: 0x0223, name: "Browser home" },
  { usage: 0x0224, name: "Browser back" }, { usage: 0x0227, name: "Refresh" },
  { usage: 0x006f, name: "Screen brightness up" }, { usage: 0x0070, name: "Screen brightness down" },
];

export const MOUSE = [
  { code: 0xf0, arg: 0, name: "Left click" }, { code: 0xf1, arg: 0, name: "Right click" },
  { code: 0xf2, arg: 0, name: "Middle click" }, { code: 0xf3, arg: 0, name: "Back" }, { code: 0xf4, arg: 0, name: "Forward" },
  { code: 0xf5, arg: 1, name: "Scroll up" }, { code: 0xf5, arg: 0xff, name: "Scroll down" },
];

export const SYSTEM = [
  { code: 0x81, name: "Power" }, { code: 0x82, name: "Sleep" }, { code: 0x83, name: "Wake" },
];

export const MACRO_MODES = ["Repeat", "Toggle", "While held"];

export const hidName = (code: number): string => HID_KEYS.find((x) => x.code === code)?.name ?? `0x${code.toString(16)}`;

/** Short label for a key action, e.g. "Ctrl+C", "Play / Pause", "Macro 3". */
export function actionLabel(a: KeyAction): string {
  switch (a.kind) {
    case "disabled": return "Disabled";
    case "key": {
      const mod = MODIFIERS.find((m) => m.code === a.modifier && m.code !== 0)?.name;
      const parts = [mod, hidName(a.code), a.code2 ? hidName(a.code2) : undefined].filter(Boolean);
      return parts.join("+");
    }
    case "mouse": return MOUSE.find((m) => m.code === a.code && (m.code !== 0xf5 || m.arg === a.arg))?.name ?? "Mouse";
    case "system": return SYSTEM.find((s) => s.code === a.code)?.name ?? "System";
    case "consumer": return CONSUMER.find((c) => c.usage === a.usage)?.name ?? `Media 0x${a.usage.toString(16)}`;
    case "profile": return ["Profile", "Next profile", "Prev profile", "Cycle profiles", `Profile ${a.arg + 1}`][a.op] ?? "Profile";
    case "macro": return `Macro ${a.index + 1}`;
    case "fn": return "Fn";
    case "raw": return a.bytes[0] === 0x0d ? "Lighting control (the keyboard's own)" : "The keyboard's own function";
  }
}

/** Short names that fit on a keycap. */
const SHORT_KEY: Record<number, string> = {
  0x46: "PrtSc", 0x47: "ScrLk", 0x48: "Pause", 0x49: "Ins", 0x4a: "Home", 0x4b: "PgUp", 0x4c: "Del", 0x4d: "End", 0x4e: "PgDn",
  0x39: "Caps", 0x2a: "Bksp", 0x28: "Enter", 0x29: "Esc", 0x2c: "Space", 0x65: "Menu",
};
const SHORT_CONSUMER: Record<number, string> = {
  0xcd: "Play", 0xb7: "Stop", 0xb5: "Next", 0xb6: "Prev", 0xe2: "Mute", 0xe9: "Vol+", 0xea: "Vol−",
  0x183: "Media", 0x18a: "Mail", 0x192: "Calc", 0x194: "PC", 0x221: "Search", 0x223: "Web", 0x224: "Back", 0x227: "Reload",
  0x6f: "Bri+", 0x70: "Bri−",
};

/** A key action as a keycap legend: a few characters (the side panel names it in full). */
export function keyLegend(a: KeyAction): string {
  switch (a.kind) {
    case "key": {
      const mod = MODIFIERS.find((m) => m.code === a.modifier && m.code !== 0)?.name;
      const name = SHORT_KEY[a.code] ?? hidName(a.code);
      return a.code2 || mod ? actionLabel(a).replace("Left ", "").replace("Right ", "") : name;
    }
    case "consumer": return SHORT_CONSUMER[a.usage] ?? "Media";
    case "raw": return a.bytes[0] === 0x0d ? "Light" : "•";
    case "macro": return `M${a.index + 1}`;
    default: return actionLabel(a);
  }
}

/** Two key actions do the same thing (field order doesn't matter). */
export function sameAction(a: KeyAction | undefined, b: KeyAction | undefined): boolean {
  const canon = (x: KeyAction | undefined) => { const v = x ?? { kind: "disabled" }; return JSON.stringify(v, Object.keys(v).sort()); };
  return canon(a) === canon(b);
}

/** What a key does on the Fn layer by default (the layout's standard Fn layer). */
export const fnDefault = (layout: Layout, id: string): KeyAction => layout.fnLayer?.[id] ?? { kind: "disabled" };

/** The standard Fn layer, for every key on the board. */
export const standardFnLayer = (layout: Layout): Record<string, KeyAction> =>
  Object.fromEntries(layout.keys.map((k) => [k.id, fnDefault(layout, k.id)]));

/** Does a keyboard's Fn layer (as read from it) match the standard one? */
export const isStandardFn = (layout: Layout, map: Record<string, KeyAction>): boolean =>
  layout.keys.every((k) => sameAction(map[k.id], fnDefault(layout, k.id)));

/** What the standard Fn layer gives, in a few words. */
export function fnSummary(layout: Layout): string {
  const fn = layout.fnLayer ?? {};
  const parts = [
    fn["1"]?.kind === "key" && fn["equal"]?.kind === "key" ? "Fn + 1 … = for F1 … F12" : null,
    fn.z && fn.c ? "Fn + Z X C for media" : null,
    fn.comma && fn.slash ? "Fn + , . / for volume" : null,
  ].filter(Boolean);
  return parts.join(", ");
}

export const isDefaultAction = (a: KeyAction, hid: number, keyId: string): boolean =>
  keyId === "fn" ? a.kind === "fn" : a.kind === "key" && a.code === hid && a.modifier === 0 && a.code2 === 0;

/** Map a browser KeyboardEvent.code to a HID usage (for macro recording). */
export function codeToHid(code: string): number | undefined {
  if (/^Key[A-Z]$/.test(code)) return 0x04 + code.charCodeAt(3) - 65;
  if (/^Digit[0-9]$/.test(code)) return code === "Digit0" ? 0x27 : 0x1e + Number(code[5]) - 1;
  const fk = /^F(\d{1,2})$/.exec(code);
  if (fk) { const n = Number(fk[1]); return n <= 12 ? 0x3a + n - 1 : 0x68 + n - 13; }
  const table: Record<string, number> = {
    Enter: 0x28, Escape: 0x29, Backspace: 0x2a, Tab: 0x2b, Space: 0x2c, Minus: 0x2d, Equal: 0x2e,
    BracketLeft: 0x2f, BracketRight: 0x30, Backslash: 0x31, Semicolon: 0x33, Quote: 0x34, Backquote: 0x35,
    Comma: 0x36, Period: 0x37, Slash: 0x38, CapsLock: 0x39, PrintScreen: 0x46, ScrollLock: 0x47, Pause: 0x48,
    Insert: 0x49, Home: 0x4a, PageUp: 0x4b, Delete: 0x4c, End: 0x4d, PageDown: 0x4e, ArrowRight: 0x4f,
    ArrowLeft: 0x50, ArrowDown: 0x51, ArrowUp: 0x52, ContextMenu: 0x65,
    ControlLeft: 0xe0, ShiftLeft: 0xe1, AltLeft: 0xe2, MetaLeft: 0xe3,
    ControlRight: 0xe4, ShiftRight: 0xe5, AltRight: 0xe6, MetaRight: 0xe7,
  };
  return table[code];
}
