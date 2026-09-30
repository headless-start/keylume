// What Home draws for the keyboard: exactly what the lighting state says is on the keys.
// Per-key designs and spells are exact (a spell shows the picture it has really reached).
// Live effects play the frames the keyboard was sent. The keyboard's own animations can't
// be read back frame by frame, so those are an approximation, labelled as one, and never
// show made-up key presses.
import { dimAll, effectFrame, liveFrameColors, REACTIVE } from "./animate";
import { livePreview } from "./preview";
import { spellFrames } from "./spell";
import type { Hex, Layout, LightingState, LiveFrameEvent } from "./types";

type Colors = Record<string, Hex>;

/** Modes whose colours come from somewhere Keylume can't see from here: a picture layer it
 * couldn't read, or the stream a live effect last sent. */
const UNSEEN: string[] = ["user-picture", "music-bars", "music-pulse", "screen-sync"];

/** How the mirror moves: not at all, with the clock (an approximation), or with live frames. */
export type Motion = "still" | "clock" | "frames";

export function mirrorMotion(s: LightingState | null): Motion {
  const l = s?.shown?.lighting;
  if (!l || !lit(s)) return "still";
  if (l.kind === "effect") return l.effect.mode === "off" || l.effect.mode === "static" || UNSEEN.includes(l.effect.mode) ? "still" : "clock";
  if (l.kind === "live") return "frames";
  return "still";
}

/** Is anything lit? (Not when disconnected, paused, off, dark or unknown.) */
function lit(s: LightingState | null): boolean {
  const sh = s?.shown;
  return !!sh && !!sh.lighting && sh.origin !== "off" && sh.brightness > 0 && s.phase !== "disconnected" && s.phase !== "paused";
}

/** The keys' colours at time `t`, given the newest live frame. Unlit keys are left out. */
export function mirrorColors(s: LightingState | null, layout: Layout, t: number, frame: LiveFrameEvent | null): Colors {
  if (!lit(s)) return {};
  const sh = s!.shown!;
  const l = sh.lighting!;
  let colors: Colors;
  switch (l.kind) {
    case "perKey": colors = l.keys; break;
    case "spell": {
      const frames = spellFrames(layout, l.words, l.background);
      const at = sh.progress ? Math.min(sh.progress.done, frames.length) : frames.length;
      colors = frames[Math.max(0, at - 1)];
      break;
    }
    case "effect":
      if (l.effect.mode === "off" || UNSEEN.includes(l.effect.mode)) return {};
      colors = effectFrame(l.effect, layout, t, { presses: false });
      break;
    case "live": {
      const mine = frame && frame.request === sh.request ? frame : null;
      colors = mine ? liveFrameColors(mine.frame, l.live, layout) : livePreview(l.live, layout);
      break;
    }
  }
  return dimAll(colors, sh.brightness);
}

/** A short note under the mirror, when what it shows needs one. */
export function mirrorNote(s: LightingState | null): string | null {
  const sh = s?.shown;
  if (!sh?.lighting || !lit(s)) return null;
  const l = sh.lighting;
  if (l.kind === "spell" && sh.running && sh.progress) return `Spelling: picture ${sh.progress.done} of ${sh.progress.total}`;
  if (l.kind === "live") return sh.running ? null : "Stopped: the keyboard keeps its last frame";
  if (l.kind === "effect" && l.effect.mode === "user-picture") return `Shows picture layer ${l.effect.direction + 1}, which couldn't be read`;
  if (l.kind === "effect" && UNSEEN.includes(l.effect.mode)) return "Shows what a live effect last sent; start one to see it here";
  if (l.kind === "effect" && l.effect.mode !== "off" && l.effect.mode !== "static") {
    const reacts = REACTIVE.includes(l.effect.mode) ? " It lights up as you type." : "";
    return `An approximation: the keyboard runs this animation itself.${reacts}`;
  }
  return null;
}
