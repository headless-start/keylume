import { useCallback, useEffect, useState } from "react";
import { ColorField, PageHead, Segmented, Slider, Toggle } from "../components/controls";
import { api } from "../lib/api";
import { useApp } from "../lib/store";
import type { SideLight, SideMode } from "../lib/types";

const MODES: { value: SideMode; label: string }[] = [
  { value: "off", label: "Off" },
  { value: "static", label: "Solid" },
  { value: "breathing", label: "Breathe" },
  { value: "neon", label: "Spectrum" },
  { value: "wave", label: "Wave" },
  { value: "snake", label: "Snake" },
];

const DIRECTIONS: Partial<Record<SideMode, string[]>> = {
  wave: ["Right", "Left", "Down", "Up"],
  snake: ["Zig-zag", "Return"],
};

const side = (mode: SideMode, color: string, extra: Partial<SideLight> = {}): SideLight =>
  ({ mode, color, speed: 2, brightness: 4, direction: 0, rainbow: false, ...extra });

const PRESETS: { name: string; s: SideLight }[] = [
  { name: "Royal Breathe", s: side("breathing", "#1f45ff") },
  { name: "Royal Solid", s: side("static", "#1f45ff") },
  { name: "Electric Wave", s: side("wave", "#00c8ff", { speed: 3 }) },
  { name: "Rainbow Cycle", s: side("neon", "#ffffff") },
  { name: "Rainbow Wave", s: side("wave", "#ff0000", { rainbow: true, speed: 3 }) },
  { name: "Gold", s: side("static", "#ffb000") },
  { name: "Ice Snake", s: side("snake", "#9fe8ff", { speed: 3 }) },
  { name: "Off", s: side("off", "#000000", { brightness: 0 }) },
];

const SPEED = ["Slowest", "Slow", "Medium", "Fast", "Fastest"];

const MODE_LABEL = Object.fromEntries(MODES.map((m) => [m.value, m.label])) as Record<SideMode, string>;

/** A little animated strip that mimics the real one. */
function StripPreview({ s }: { s: SideLight }) {
  const on = s.mode !== "off" && s.brightness > 0;
  const rainbow = s.mode === "neon" || s.rainbow;
  return (
    <div
      className={`strip-preview m-${s.mode} ${rainbow ? "rainbow" : ""}`}
      style={{ ["--c" as string]: s.color, ["--dur" as string]: `${6 - s.speed}s`, opacity: on ? 0.35 + 0.65 * (s.brightness / 4) : 0.08 }}
      aria-label="Side light preview"
    />
  );
}

export function SidePage() {
  const { settings, saveSettings, lighting } = useApp();
  const [device, setDevice] = useState<SideLight | null>(null);

  const refresh = useCallback(() => {
    api().then((a) => a.getSideLight()).then(setDevice).catch(() => setDevice(null));
  }, []);
  // re-read what the keyboard shows after anything changes (writes settle ~1.1 s)
  useEffect(() => {
    const t = setTimeout(refresh, 1600);
    return () => clearTimeout(t);
  }, [refresh, settings?.sideFollow, settings?.sideCustom, lighting?.shown?.request]);

  if (!settings) return null;
  const c = settings.sideCustom;
  const setCustom = (patch: Partial<SideLight>) => {
    const next = { ...c, ...patch };
    // keep the direction valid for the new mode
    if (next.direction >= (DIRECTIONS[next.mode]?.length ?? 1)) next.direction = 0;
    saveSettings({ ...settings, sideFollow: false, sideCustom: next });
  };
  const dirs = DIRECTIONS[c.mode];
  const cur = lighting?.phase !== "disconnected" && lighting?.phase !== "paused" ? lighting?.shown : null;

  return (
    <section className="page side">
      <div className="page-wide">
        <PageHead title="Side light" />
        <div className="builder">
          <div className="stage-card">
            <div className="strip-frame"><StripPreview s={settings.sideFollow && device ? device : c} /></div>
            <p className="now-on">
              {device
                ? <>On the strip now: <b>{MODE_LABEL[device.mode]}</b>{device.mode !== "off" && device.mode !== "neon" && (device.rainbow ? " · rainbow" : ` · ${device.color}`)}</>
                : "\u00a0"}
            </p>
            <h3>Looks</h3>
            <div className="side-presets">
              {PRESETS.map((p) => (
                <button key={p.name} className={`side-preset ${!settings.sideFollow && JSON.stringify(c) === JSON.stringify(p.s) ? "on" : ""}`}
                  onClick={() => saveSettings({ ...settings, sideFollow: false, sideCustom: p.s })}>
                  <span className={`swatch m-${p.s.mode} ${p.s.mode === "neon" || p.s.rainbow ? "rainbow" : ""}`} style={{ ["--c" as string]: p.s.color }} aria-hidden />
                  {p.name}
                </button>
              ))}
            </div>
            <p className="stage-note">Picking a look turns matching off, so the strip keeps it whatever the keys do.</p>
          </div>

          <aside className="panel builder-panel">
            <Toggle
              label="Match my lighting"
              hint={`Follows whatever the keys show.${cur ? ` Now: ${cur.name}.` : ""}`}
              checked={settings.sideFollow}
              onChange={(sideFollow) => saveSettings({ ...settings, sideFollow })}
            />
            <div className={`fine-tune ${settings.sideFollow ? "dimmed" : ""}`}>
              <h3>Fine-tune</h3>
              <Segmented label="Effect" className="grid3" options={MODES} value={c.mode} onChange={(mode) => setCustom({ mode })} />
              {c.mode !== "off" && (
                <>
                  {c.mode !== "neon" && !c.rainbow && <ColorField label="Colour" value={c.color} onChange={(color) => setCustom({ color })} swatches={false} />}
                  {c.mode !== "neon" && <Toggle label="Rainbow" checked={c.rainbow} onChange={(rainbow) => setCustom({ rainbow })} />}
                  <Slider label="Brightness" value={c.brightness} min={0} max={4} onChange={(brightness) => setCustom({ brightness })} format={(v) => `${v * 25}%`} />
                  {c.mode !== "static" && <Slider label="Speed" value={c.speed} min={0} max={4} onChange={(speed) => setCustom({ speed })} format={(v) => SPEED[v]} />}
                  {dirs && <Segmented label="Direction" options={dirs.map((label, value) => ({ value, label }))} value={c.direction} onChange={(direction) => setCustom({ direction })} />}
                </>
              )}
            </div>
          </aside>
        </div>
      </div>
    </section>
  );
}
