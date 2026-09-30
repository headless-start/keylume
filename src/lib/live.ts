// Helpers for live effects shared by the library, the previews and the mock.
import type { Hex, LiveEffect } from "./types";

/** The colour a live effect shows first (for thumbnails). */
export function liveColor(l: LiveEffect): Hex {
  switch (l.kind) {
    case "paletteFlow": case "trip": case "lava": case "breathe": case "rave": case "beatFlash": case "aurora": case "musicFlow": case "steps":
      return l.colors[0] ?? "#0040ff";
    case "heartbeat": case "flicker": case "spectrum": case "sineBars": case "rainBars": case "scanner": case "equalizer":
    case "morse": case "bomb": case "flashbang": case "bars": case "pacedBreathing": return l.color;
    case "focus": return l.work;
    case "tide": return l.b;
    case "storm": return l.sky;
    case "strobe": case "siren": return l.a;
    case "audioGlow": return l.loud;
    case "cpuHeat": return l.cool;
    case "screenAmbient": return "#6080ff";
    case "sunrise": return "#ff8a30";
    case "countdown": return l.color;
    case "glitter": return l.base;
  }
}
