import { gradientBetween } from "./color";
import type { Hex, LayoutKey } from "./types";

// Pure state machine for the per-key editor (unit-tested in editor.test.ts).

export interface EditorState {
  keys: Record<string, Hex>;
  past: Record<string, Hex>[];
  future: Record<string, Hex>[];
}

export type EditorAction =
  | { type: "load"; keys: Record<string, Hex> }
  | { type: "paint"; ids: string[]; color: Hex; merge?: boolean }
  | { type: "fill"; color: Hex; ids?: string[] }
  | { type: "gradient"; all: LayoutKey[]; from: LayoutKey; to: LayoutKey; a: Hex; b: Hex; only?: string[] }
  | { type: "undo" }
  | { type: "redo" };

const HISTORY = 100;

export const initEditor = (keys: Record<string, Hex>): EditorState => ({ keys, past: [], future: [] });

function commit(s: EditorState, keys: Record<string, Hex>, merge = false): EditorState {
  // `merge` folds brush strokes (many paint events while dragging) into one undo step.
  const past = merge ? s.past : [...s.past, s.keys].slice(-HISTORY);
  return { keys, past, future: [] };
}

export function editorReducer(s: EditorState, a: EditorAction): EditorState {
  switch (a.type) {
    case "load":
      return commit(s, { ...a.keys });
    case "paint": {
      if (a.ids.every((id) => s.keys[id] === a.color)) return s;
      const keys = { ...s.keys };
      for (const id of a.ids) keys[id] = a.color;
      return commit(s, keys, a.merge);
    }
    case "fill": {
      const keys = { ...s.keys };
      for (const id of a.ids ?? Object.keys(keys)) keys[id] = a.color;
      return commit(s, keys);
    }
    case "gradient": {
      const g = gradientBetween(a.all, a.from, a.to, a.a, a.b, a.only ? new Set(a.only) : undefined);
      return commit(s, { ...s.keys, ...g });
    }
    case "undo": {
      if (!s.past.length) return s;
      return { keys: s.past[s.past.length - 1], past: s.past.slice(0, -1), future: [s.keys, ...s.future] };
    }
    case "redo": {
      if (!s.future.length) return s;
      return { keys: s.future[0], past: [...s.past, s.keys], future: s.future.slice(1) };
    }
  }
}
