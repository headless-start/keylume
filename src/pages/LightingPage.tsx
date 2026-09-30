import { useEffect, useMemo, useRef, useState } from "react";
import { AnimatedBoard } from "../components/AnimatedBoard";
import { ColorField, SaveRow, Segmented, Slider, Toggle } from "../components/controls";
import { api } from "../lib/api";
import { MODES, modeInfo } from "../lib/modes";
import { attempt, useApp } from "../lib/store";
import type { Effect, Mode, Profile } from "../lib/types";

const SPEED = ["Slowest", "Slow", "Medium", "Fast", "Fastest"];

export const DEFAULT_EFFECT: Effect = { mode: "breathing", speed: 1, brightness: 4, direction: 0, rainbow: false, color: "#0040ff" };

/** Create > Animated: one of the keyboard's built-in animations, tuned and saved. */
export function EffectBuilder({ note }: { note: string }) {
  const { reloadProfiles, toast, layout, editing, edit, features } = useApp();
  const [effect, setEffect] = useState<Effect>(DEFAULT_EFFECT);
  const [name, setName] = useState("");
  const first = useRef(true);

  // Start from the profile being edited, else from what the keyboard shows now.
  useEffect(() => {
    if (editing?.lighting.kind === "effect") {
      setEffect(editing.lighting.effect);
      setName(editing.source === "user" ? editing.name : `${editing.name} (mine)`);
      edit(null);
      return;
    }
    api().then((a) => a.getEffect()).then((e) => {
      if (e.mode !== "user-picture" && !modeInfo(e.mode).hostDriven) setEffect(e);
    }).catch(() => {});
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // Apply as you tweak (the backend coalesces rapid changes).
  useEffect(() => {
    if (first.current) { first.current = false; return; }
    const t = setTimeout(() => attempt("Apply effect", async () => (await api()).applyEffect(effect)), 120);
    return () => clearTimeout(t);
  }, [effect]);

  const info = modeInfo(effect.mode);
  const set = (patch: Partial<Effect>) => setEffect((e) => ({ ...e, ...patch }));
  const pick = (mode: Mode) => {
    const i = modeInfo(mode);
    set({ mode, direction: Math.min(effect.direction, Math.max(0, i.directions.length - 1)), rainbow: i.hasRainbow ? effect.rainbow : false });
  };

  const save = async () => {
    const profile: Profile = {
      id: "",
      name: name.trim() || `${info.name} ${effect.rainbow ? "rainbow" : effect.color}`,
      category: "Mine",
      tags: ["animated", effect.mode],
      description: `${info.name} effect`,
      source: "user",
      lighting: { kind: "effect", effect },
    };
    if (await attempt("Save", async () => { await (await api()).saveProfile(profile); await reloadProfiles(); })) {
      toast(`Saved “${profile.name}” to your library`, "success");
      setName("");
    }
  };

  const selectable = MODES.filter((m) => !m.hostDriven && m.mode !== "user-picture" && features.effects.includes(m.mode));
  const preview: Profile = useMemo(() => ({
    id: "preview", name: "", category: "", tags: [], description: "", source: "user", lighting: { kind: "effect", effect },
  }), [effect]);

  return (
    <div className="builder">
      <div className="stage-card">
        {layout && <AnimatedBoard profile={preview} layout={layout} spill className="stage-board" />}
        <div className="mode-grid" role="listbox" aria-label="Effect">
          {selectable.map((m) => (
            <button key={m.mode} role="option" aria-selected={m.mode === effect.mode} className={`mode ${m.mode === effect.mode ? "on" : ""}`}
              onClick={() => pick(m.mode)} title={m.description}>{m.name}</button>
          ))}
        </div>
        <p className="stage-note">{note}</p>
      </div>
      <aside className="panel builder-panel">
        <div className="panel-title">
          <h2>{info.name}</h2>
          <p className="hint">{info.description}</p>
        </div>
        {info.hasColor && !effect.rainbow && <ColorField label="Colour" value={effect.color} onChange={(color) => set({ color })} />}
        {info.hasRainbow && <Toggle label="Rainbow" hint="Cycle through every colour instead" checked={effect.rainbow} onChange={(rainbow) => set({ rainbow })} />}
        {info.hasSpeed && <Slider label="Speed" min={0} max={4} value={effect.speed} onChange={(speed) => set({ speed })} format={(v) => SPEED[v]} />}
        <Slider label="Brightness" min={0} max={4} value={effect.brightness} onChange={(brightness) => set({ brightness })} format={(v) => `${v * 25}%`} />
        {info.directions.length > 0 && (
          <Segmented label="Direction" value={effect.direction} onChange={(direction) => set({ direction })} options={info.directions.map((d, i) => ({ value: i, label: d }))} />
        )}
        <div className="panel-foot"><SaveRow name={name} setName={setName} label="Save" onSave={save} /></div>
      </aside>
    </div>
  );
}
