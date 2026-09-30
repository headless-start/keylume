import { memo, useEffect, useId, useRef, useState } from "react";
import { luminance } from "../lib/color";
import type { Hex, Layout } from "../lib/types";

/** What an unlit key looks like: a dark keycap, not a hole. */
const OFF = [0x15, 0x17, 0x1e];
/** A key's colour on screen: its light, never darker than an unlit keycap. */
export function capColor(c: Hex | undefined): Hex {
  const n = c && c.length === 7 ? parseInt(c.slice(1), 16) : 0;
  const ch = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v, i) => Math.max(v, OFF[i]));
  return `#${((ch[0] << 16) | (ch[1] << 8) | ch[2]).toString(16).padStart(6, "0")}`;
}
const shown = capColor;

/** An unlit white keycap. */
const WHITE_CAP = [0xe9, 0xec, 0xf2];
/** A key's colour on a white keyboard: its light shining through a white keycap (softened a
 * little, never darker than the keycap itself). */
export function whiteCapColor(c: Hex | undefined): Hex {
  const n = c && c.length === 7 ? parseInt(c.slice(1), 16) : 0;
  const ch = [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  const peak = Math.max(...ch);
  const lit = peak / 255;
  const out = WHITE_CAP.map((base, i) => {
    const full = peak ? (ch[i] / peak) * 255 : 255;
    const tint = full + (255 - full) * 0.15;
    return Math.round(base * (1 - lit) + tint * lit);
  });
  return `#${((out[0] << 16) | (out[1] << 8) | out[2]).toString(16).padStart(6, "0")}`;
}

interface Props {
  layout: Layout;
  colors: Record<string, Hex>;
  /** Text drawn on each key (defaults to the key's own label); `legends={false}` hides it. */
  labels?: Record<string, string>;
  legends?: boolean;
  selected?: Set<string>;
  /** Keys that differ from the device (e.g. unsaved remaps). */
  marked?: Set<string>;
  /** A key was pressed (pointer down, or Enter / Space when it has focus). Makes the keys
   * focusable buttons; the arrow keys move between them. */
  onKeyDown?: (id: string) => void;
  onKeyEnter?: (id: string, e: React.PointerEvent) => void;
  /** What a screen reader says for a key, after its name (its colour, what it does). */
  describe?: (id: string) => string;
  unit?: number;
  /** The keys' light spilling around the board. */
  glow?: boolean;
  /** Size the board by its case alone and let the glow spill outside the box (so a lit
   * board and an unlit one line up); otherwise the box leaves room for the glow. */
  spill?: boolean;
  /** The keyboard's own colour (case and keycaps): dark unless its layout says white. */
  finish?: "dark" | "white";
  className?: string;
}

type LayoutKey = Layout["keys"][number];
const centre = (k: LayoutKey) => ({ x: k.x + k.w / 2, y: k.y + k.h / 2 });

/** The key an arrow key moves to: the nearest one that way, preferring the same row or column. */
function neighbour(keys: LayoutKey[], from: LayoutKey, arrow: string): LayoutKey | undefined {
  const c = centre(from);
  let best: LayoutKey | undefined;
  let score = Infinity;
  for (const k of keys) {
    const p = centre(k);
    const [dx, dy] = [p.x - c.x, p.y - c.y];
    const [along, across] = ({ ArrowRight: [dx, dy], ArrowLeft: [-dx, dy], ArrowDown: [dy, dx], ArrowUp: [-dy, dx] } as Record<string, number[]>)[arrow] ?? [0, 0];
    if (k === from || along <= 0.1) continue;
    const s = along + Math.abs(across) * 3;
    if (s < score) { score = s; best = k; }
  }
  return best;
}

/** The keyboard: a case, one lit keycap per key, and its underglow. */
export const Keyboard = memo(function Keyboard({
  layout, colors, labels, legends = true, selected, marked, onKeyDown, onKeyEnter, describe, unit = 54, glow = true, spill = false, finish = "dark", className,
}: Props) {
  const uid = useId().replace(/[^a-zA-Z0-9]/g, "");
  const svg = useRef<SVGSVGElement>(null);
  // one key at a time takes Tab (roving focus); the arrows move it
  const [focusId, setFocusId] = useState<string | undefined>(layout.keys[0]?.id);
  const interactive = !!onKeyDown;
  const keyNav = (k: LayoutKey, e: React.KeyboardEvent) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onKeyDown?.(k.id);
      return;
    }
    const to = e.key === "Home" ? layout.keys[0] : e.key === "End" ? layout.keys[layout.keys.length - 1] : neighbour(layout.keys, k, e.key);
    if (!to || !["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(e.key)) return;
    e.preventDefault();
    setFocusId(to.id);
    svg.current?.querySelector<SVGGElement>(`[data-key="${to.id}"]`)?.focus();
  };
  const pad = 16, gap = 5, bleed = glow && !spill ? unit * 0.9 : 0;
  const white = finish === "white";
  const face = white ? whiteCapColor : shown;
  const W = layout.width * unit + pad * 2;
  const H = layout.height * unit + pad * 2;
  const rect = (k: Layout["keys"][number]) => ({ x: pad + k.x * unit + gap / 2, y: pad + k.y * unit + gap / 2, w: k.w * unit - gap, h: k.h * unit - gap });
  return (
    <svg ref={svg} className={`keyboard finish-${finish} ${interactive ? "interactive" : ""} ${className ?? ""}`} viewBox={`${-bleed} ${-bleed} ${W + bleed * 2} ${H + bleed * 2}`}
      role={interactive ? "group" : "img"} aria-label={`${layout.name} keyboard`}>
      <defs>
        {white ? (
          <linearGradient id={`case${uid}`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor="#ffffff" />
            <stop offset="0.08" stopColor="#eef0f5" />
            <stop offset="1" stopColor="#c9ced8" />
          </linearGradient>
        ) : (
          <linearGradient id={`case${uid}`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor="#262a36" />
            <stop offset="0.08" stopColor="#1a1d27" />
            <stop offset="1" stopColor="#0c0e14" />
          </linearGradient>
        )}
        <linearGradient id={`shade${uid}`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#fff" stopOpacity="0.2" />
          <stop offset="0.4" stopColor="#fff" stopOpacity="0" />
          <stop offset="1" stopColor="#000" stopOpacity={white ? 0.14 : 0.3} />
        </linearGradient>
        {glow && (
          <filter id={`under${uid}`} x="-20%" y="-30%" width="140%" height="160%">
            <feGaussianBlur stdDeviation={unit * 0.42} />
          </filter>
        )}
      </defs>
      {glow && (
        <g className="underglow" filter={`url(#under${uid})`}>
          {layout.keys.map((k) => {
            const c = colors[k.id];
            if (!c || c === "#000000") return null;
            const r = rect(k);
            return <rect key={k.id} x={r.x - 6} y={r.y - 6} width={r.w + 12} height={r.h + 12} fill={c} />;
          })}
        </g>
      )}
      <rect className="case" x={0} y={0} width={W} height={H} rx={18} fill={`url(#case${uid})`} />
      {layout.keys.map((k) => {
        const c = face(colors[k.id]);
        const r = rect(k);
        const text = legends ? labels?.[k.id] ?? k.label : "";
        return (
          <g key={k.id} data-key={k.id}
            className={`key ${selected?.has(k.id) ? "sel" : ""} ${marked?.has(k.id) ? "marked" : ""}`}
            {...(interactive && {
              tabIndex: k.id === focusId ? 0 : -1,
              role: "button",
              "aria-label": `${k.label || k.id}${describe ? `, ${describe(k.id)}` : ""}`,
              "aria-pressed": selected ? selected.has(k.id) : undefined,
              onFocus: () => setFocusId(k.id),
              onKeyDown: (e: React.KeyboardEvent) => keyNav(k, e),
            })}
            onPointerDown={onKeyDown ? (e) => { (e.target as Element).releasePointerCapture?.(e.pointerId); onKeyDown(k.id); } : undefined}
            onPointerEnter={onKeyEnter ? (e) => onKeyEnter(k.id, e) : undefined}>
            <rect className="cap" x={r.x} y={r.y} width={r.w} height={r.h} rx={7} fill={c} />
            <rect className="cap-shade" x={r.x} y={r.y} width={r.w} height={r.h} rx={7} fill={`url(#shade${uid})`} />
            {text && (
              <text x={r.x + r.w / 2} y={r.y + r.h / 2 + 1} textAnchor="middle" dominantBaseline="middle" className="klabel"
                fill={luminance(c) > 0.55 ? "#0b1020" : "#e8eeff"} fontSize={text.length > 6 ? 9 : text.length > 3 ? 10.5 : 13}>
                {text}
              </text>
            )}
          </g>
        );
      })}
    </svg>
  );
});

/** Small, cheap preview (canvas, drawn once). */
export const MiniBoard = memo(function MiniBoard({ layout, colors, width = 220 }: { layout: Layout; colors: Record<string, Hex>; width?: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const u = (width - 8) / layout.width;
  const height = Math.round(layout.height * u + 8);
  useEffect(() => {
    const cv = ref.current;
    if (!cv) return;
    const dpr = window.devicePixelRatio || 1;
    cv.width = width * dpr;
    cv.height = height * dpr;
    const g = cv.getContext("2d");
    if (!g) return;
    g.scale(dpr, dpr);
    g.clearRect(0, 0, width, height);
    for (const k of layout.keys) {
      g.fillStyle = shown(colors[k.id]);
      g.beginPath();
      g.roundRect(4 + k.x * u + 1, 4 + k.y * u + 1, k.w * u - 2, k.h * u - 2, 2);
      g.fill();
    }
  }, [layout, colors, width, height, u]);
  return <canvas ref={ref} className="miniboard" style={{ width, height }} aria-hidden />;
});
