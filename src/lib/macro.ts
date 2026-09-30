import type { Macro, MacroEvent } from "./types";

export const MACRO_BYTES = 256;

/** Encoded size in bytes — mirrors keylume_proto::macros::Macro::to_bytes. */
export function encodedSize(m: Macro): number {
  let n = 2; // repeat count
  const ev = m.events;
  for (let i = 0; i < ev.length; ) {
    const e = ev[i];
    if (e.kind === "delay") { i++; continue; }
    let delay = 0;
    let j = i + 1;
    while (j < ev.length && ev[j].kind === "delay") { delay += (ev[j] as { ms: number }).ms; j++; }
    n += delay >= 1 && delay <= 127 ? 2 : 4;
    i = j;
  }
  return n + 4; // terminator
}

/** Turn raw recorded events into a tidy list: merge consecutive delays, drop zero ones. */
export function normalize(events: MacroEvent[]): MacroEvent[] {
  const out: MacroEvent[] = [];
  for (const e of events) {
    const last = out[out.length - 1];
    if (e.kind === "delay") {
      if (e.ms <= 0) continue;
      if (last?.kind === "delay") last.ms = Math.min(65535, last.ms + e.ms);
      else out.push({ kind: "delay", ms: Math.min(65535, Math.round(e.ms)) });
    } else out.push({ ...e });
  }
  while (out[0]?.kind === "delay") out.shift();
  while (out[out.length - 1]?.kind === "delay") out.pop();
  return out;
}

/** Records key presses with timing from browser keyboard events. */
export class Recorder {
  private events: MacroEvent[] = [];
  private last = 0;
  private held = new Set<number>();

  push(code: number, down: boolean, t: number) {
    if (down && this.held.has(code)) return; // key auto-repeat
    if (down) this.held.add(code); else this.held.delete(code);
    if (this.events.length) this.events.push({ kind: "delay", ms: Math.round(t - this.last) });
    this.events.push({ kind: "key", code, down });
    this.last = t;
  }

  /** Finish: release anything still held so the macro never leaves keys stuck. */
  finish(t: number): MacroEvent[] {
    for (const code of this.held) this.push(code, false, t);
    return normalize(this.events);
  }
}
