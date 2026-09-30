import type { Mode } from "./types";

// Mirrors keylume_proto::led::Mode::info(). Kept in sync by src/lib/modes.test.ts.
export interface ModeInfo {
  mode: Mode;
  name: string;
  description: string;
  hasColor: boolean;
  hasSpeed: boolean;
  hasRainbow: boolean;
  directions: string[];
  hostDriven: boolean;
}

const m = (
  mode: Mode, name: string, description: string, hasColor: boolean, hasSpeed: boolean,
  hasRainbow: boolean, directions: string[] = [], hostDriven = false,
): ModeInfo => ({ mode, name, description, hasColor, hasSpeed, hasRainbow, directions, hostDriven });

export const MODES: ModeInfo[] = [
  m("static", "Static", "One steady colour", true, false, true),
  m("breathing", "Breathing", "Slow fade in and out", true, true, true),
  m("spectrum", "Spectrum Cycle", "The whole board fades through the rainbow", false, true, false),
  m("wave", "Wave", "A wave of light sweeping across", true, true, true, ["Right", "Left", "Down", "Up"]),
  m("ripple", "Ripple", "Rings spread from every key you press", true, true, true),
  m("raindrop", "Starlight", "Random keys twinkle", true, true, true),
  m("snake", "Snake", "A trail snakes through the keys", true, true, true, ["Zig-zag", "Return"]),
  m("reactive", "Reactive", "Keys light up as you type", true, true, true),
  m("converge", "Converge", "Light closes in from both sides", true, true, true),
  m("sine-wave", "Sine Wave", "A rolling sine wave", true, true, true),
  m("kaleidoscope", "Kaleidoscope", "Colour blooms outward or inward", true, true, true, ["Out", "In"]),
  m("line-wave", "Line Wave", "Straight bands sweep across", true, true, true, ["Right", "Left"]),
  m("laser", "Laser", "Beams fire from pressed keys", true, true, true),
  m("circle-wave", "Circle Wave", "A spinning vortex", true, true, true, ["Anticlockwise", "Clockwise"]),
  m("dazzle", "Dazzle", "Glittering colour flashes", true, true, true),
  m("rain-down", "Rain", "Light falls like rain", true, true, true),
  m("meteor", "Meteor", "Shooting streaks of light", true, true, true),
  m("reactive-off", "Reactive Fade", "Board lit; pressed keys go dark then return", true, true, true),
  m("user-picture", "Custom Picture", "Your per-key design from a picture layer", false, false, false, ["Layer 1", "Layer 2", "Layer 3"]),
  m("music-bars", "Music Bars", "Audio bars (driven by Keylume Live)", true, false, true, ["Upright", "Separate", "Intersect"], true),
  m("music-pulse", "Music Pulse", "Audio pulse (driven by Keylume Live)", true, false, true, ["Upright", "Separate", "Intersect"], true),
  m("screen-sync", "Screen Sync", "Whole-board colour (driven by Keylume Live)", false, false, false, [], true),
  m("off", "Off", "Lights off", false, false, false),
];

export const modeInfo = (mode: Mode): ModeInfo => MODES.find((x) => x.mode === mode) ?? MODES[0];
