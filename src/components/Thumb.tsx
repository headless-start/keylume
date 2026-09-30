import { memo, useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { isDynamic, player } from "../lib/animate";
import { previewFrames } from "../lib/liveCache";
import type { Hex, Layout, Profile } from "../lib/types";
import { capColor } from "./Keyboard";

/** A colour's channels times `k` (0..1), for a keycap's shaded skirt. */
function shade(hex: Hex, k: number): string {
  const n = parseInt(hex.slice(1), 16);
  return `rgb(${Math.round(((n >> 16) & 255) * k)},${Math.round(((n >> 8) & 255) * k)},${Math.round((n & 255) * k)})`;
}

/** How lit a key is (0..1): its brightest channel. */
function lit(hex: Hex | undefined): number {
  if (!hex || hex.length !== 7) return 0;
  const n = parseInt(hex.slice(1), 16);
  return Math.max((n >> 16) & 255, (n >> 8) & 255, n & 255) / 255;
}

/**
 * A keyboard in `cols` on a `width`×`h` canvas: a dark case, the lit keys' colour glowing
 * on the plate between them, and keycaps with a shaded skirt and a lighter top face. The
 * glow is the lit keys drawn small on `glow` and scaled up (a soft blur every webview
 * does quickly), so a hovered design can repaint at 30 frames a second.
 */
export function paintBoard(g: CanvasRenderingContext2D, glow: HTMLCanvasElement, layout: Layout, cols: Record<string, Hex>, width: number, h: number) {
  const u = width / (layout.width + 0.4);
  const pad = 0.2 * u, gap = Math.max(1, u * 0.1), r = Math.max(1.5, u * 0.16);
  // the case
  const caseGrad = g.createLinearGradient(0, 0, 0, h);
  caseGrad.addColorStop(0, "#1c1f29");
  caseGrad.addColorStop(1, "#0c0e13");
  g.fillStyle = caseGrad;
  g.beginPath();
  g.roundRect(0.5, 0.5, width - 1, h - 1, Math.max(3, u * 0.32));
  g.fill();
  g.strokeStyle = "rgba(255,255,255,0.07)";
  g.lineWidth = 1;
  g.stroke();
  // the plate's glow
  const gw = Math.max(8, Math.ceil(width / 7)), gh = Math.max(4, Math.ceil(h / 7));
  if (glow.width !== gw || glow.height !== gh) {
    glow.width = gw;
    glow.height = gh;
  }
  const s = glow.getContext("2d");
  if (s) {
    s.clearRect(0, 0, gw, gh);
    const k = gw / width;
    for (const key of layout.keys) {
      const c = cols[key.id];
      if (lit(c) < 0.12) continue;
      s.fillStyle = c!;
      s.fillRect((pad + key.x * u) * k, (pad + key.y * u) * k, key.w * u * k, key.h * u * k);
    }
    g.save();
    g.globalCompositeOperation = "lighter";
    g.globalAlpha = 0.85;
    g.imageSmoothingEnabled = true;
    g.imageSmoothingQuality = "high";
    g.drawImage(glow, 0, 0, width, h);
    g.restore();
  }
  // keycaps: skirt, then the top face a little up and in
  const inset = Math.max(0.8, u * 0.11);
  for (const key of layout.keys) {
    const face = capColor(cols[key.id]); // unlit keys are dark keycaps
    const x = pad + key.x * u + gap / 2, y = pad + key.y * u + gap / 2, w = key.w * u - gap, kh = key.h * u - gap;
    g.fillStyle = shade(face, 0.62);
    g.beginPath();
    g.roundRect(x, y, w, kh, r);
    g.fill();
    g.fillStyle = face;
    g.beginPath();
    g.roundRect(x + inset, y + inset * 0.55, w - inset * 2, kh - inset * 1.9, Math.max(1, r * 0.8));
    g.fill();
    if (u >= 9) {
      g.fillStyle = "rgba(255,255,255,0.16)";
      g.fillRect(x + inset + r * 0.6, y + inset * 0.55 + 0.5, Math.max(0, w - inset * 2 - r * 1.2), Math.max(0.6, u * 0.035));
    }
  }
}

/** Hover this long before a dynamic profile starts playing. */
export const PREVIEW_DELAY_MS = 600;

/** Focus that came from the keyboard, not a click (keyboard focus previews like a hover). */
export function keyboardFocus(el: Element): boolean {
  try { return el.matches(":focus-visible"); } catch { return false; }
}

interface Props {
  layout: Layout;
  /** The still picture. */
  colors: Record<string, Hex>;
  /** When set with `playing`, the profile animates. */
  profile?: Profile;
  /** Parent says "the pointer is on me" (the whole card, not just the picture). */
  hovering?: boolean;
  onPlayingChange?: (playing: boolean) => void;
}

/**
 * A keyboard thumbnail drawn on a canvas that fills its container's width (crisp at
 * any size and pixel density). Dynamic profiles play after a short hover.
 */
export const Thumb = memo(function Thumb({ layout, colors, profile, hovering, onPlayingChange }: Props) {
  const wrap = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const [width, setWidth] = useState(0);
  const aspect = (layout.height + 0.4) / (layout.width + 0.4);

  useLayoutEffect(() => {
    const el = wrap.current;
    if (!el) return;
    const measure = () => setWidth(Math.round(el.clientWidth));
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const glowCanvas = useRef<HTMLCanvasElement | null>(null);
  const draw = useCallback((cols: Record<string, Hex>) => {
    const cv = canvas.current;
    if (!cv || width <= 0) return;
    const dpr = window.devicePixelRatio || 1;
    const h = width * aspect;
    const bw = Math.round(width * dpr), bh = Math.round(h * dpr);
    if (cv.width !== bw || cv.height !== bh) {
      cv.width = bw;
      cv.height = bh;
    }
    const g = cv.getContext("2d");
    if (!g) return;
    g.setTransform(bw / width, 0, 0, bh / h, 0, 0); // exact bitmap scale (no drift from rounding)
    g.clearRect(0, 0, width, h);
    glowCanvas.current ??= document.createElement("canvas");
    paintBoard(g, glowCanvas.current, layout, cols, width, h);
  }, [width, aspect, layout]);

  useEffect(() => draw(colors), [colors, draw]);

  // hover-to-play: you point at a design to see what it does, so it plays even when the
  // system asks for less motion
  useEffect(() => {
    if (!hovering || !profile || !isDynamic(profile)) return;
    let raf = 0, cancelled = false, last = 0;
    const timer = window.setTimeout(async () => {
      const frames = profile.lighting.kind === "live" ? await previewFrames(profile.lighting.live) : undefined;
      const play = player(profile, layout, frames);
      if (!play || cancelled) return;
      onPlayingChange?.(true);
      const start = performance.now();
      const tick = (now: number) => {
        if (now - last > 33) { // ~30 fps is plenty for a thumbnail
          last = now;
          draw(play((now - start) / 1000));
        }
        raf = requestAnimationFrame(tick);
      };
      raf = requestAnimationFrame(tick);
    }, PREVIEW_DELAY_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
      cancelAnimationFrame(raf);
      onPlayingChange?.(false);
      draw(colors);
    };
  }, [hovering, profile, layout, draw, colors, onPlayingChange]);

  return (
    // The canvas keeps the keyboard's proportions itself (width 100%, height auto from its
    // bitmap); the wrapper's aspect-ratio only reserves the space before the first draw.
    <div ref={wrap} className="thumb" style={{ aspectRatio: `${1 / aspect}` }}>
      <canvas ref={canvas} width={160} height={Math.round(160 * aspect)} aria-hidden />
    </div>
  );
});
