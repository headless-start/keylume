import { useEffect, useMemo, useState } from "react";
import { AnimatedBoard } from "../components/AnimatedBoard";
import { ColorField, SaveRow, Slider, Toggle } from "../components/controls";
import { api } from "../lib/api";
import { attempt, useApp } from "../lib/store";
import type { BarStyle, Hex, LiveEffect, Profile } from "../lib/types";

/** Every live effect, grouped the way the picker shows them. */
const GROUPS = ["Calm", "Party", "Music", "Bars", "Useful", "Games"] as const;
const KINDS: { kind: LiveEffect["kind"]; group: (typeof GROUPS)[number]; name: string; about: string; input?: string }[] = [
  { kind: "paletteFlow", group: "Calm", name: "Palette flow", about: "Smoothly drift through your colours" },
  { kind: "heartbeat", group: "Calm", name: "Heartbeat", about: "A calm double pulse" },
  { kind: "flicker", group: "Calm", name: "Flicker", about: "Candle / flame flicker" },
  { kind: "tide", group: "Calm", name: "Tide", about: "Slow swell between two colours" },
  { kind: "storm", group: "Party", name: "Storm", about: "Dark sky with lightning" },
  { kind: "strobe", group: "Party", name: "Strobe", about: "Alternate two colours" },
  { kind: "audioGlow", group: "Music", name: "Music glow", about: "Brightness follows your music", input: "Listens to your PC's audio output (never recorded or sent anywhere)" },
  { kind: "spectrum", group: "Music", name: "Spectrum", about: "Real audio visualiser", input: "Listens to your PC's audio output (never recorded or sent anywhere)" },
  { kind: "cpuHeat", group: "Useful", name: "CPU heat", about: "Colour follows processor load", input: "Reads CPU usage" },
  { kind: "screenAmbient", group: "Useful", name: "Screen ambient", about: "Match your screen's colour", input: "Samples the average colour of your screen (one pixel, never saved)" },
  { kind: "sineBars", group: "Bars", name: "Sine waves", about: "Rolling waves" },
  { kind: "rainBars", group: "Bars", name: "Rainfall", about: "Falling drops" },
  { kind: "scanner", group: "Bars", name: "Scanner", about: "A sweeping beam" },
  { kind: "equalizer", group: "Bars", name: "Equalizer", about: "A lively fake EQ" },
  { kind: "bars", group: "Bars", name: "Bar patterns", about: "Plasma, fire, bouncing balls, DNA, heart monitor and more on the bars" },
  { kind: "trip", group: "Party", name: "Trip", about: "Colours drifting on out-of-sync clocks: hypnotic, never quite repeating" },
  { kind: "lava", group: "Calm", name: "Lava lamp", about: "Slow molten blobs of colour" },
  { kind: "breathe", group: "Calm", name: "Breathe", about: "Deep breaths, a new colour each time" },
  { kind: "rave", group: "Party", name: "Rave", about: "Hard colour cuts on the beat" },
  { kind: "beatFlash", group: "Music", name: "Beat flash", about: "Flashes on every beat of your music", input: "Listens to your PC's audio output (never recorded or sent anywhere)" },
  { kind: "morse", group: "Useful", name: "Morse", about: "Blink any message in Morse code" },
  { kind: "bomb", group: "Games", name: "C4", about: "Bomb planted: beeps speed up, then the blast" },
  { kind: "flashbang", group: "Games", name: "Flashbang", about: "Every so often, a white-out that fades back" },
  { kind: "focus", group: "Useful", name: "Focus timer", about: "Pomodoro: one colour while you work, another on your break" },
  { kind: "aurora", group: "Calm", name: "Aurora", about: "Slow curtains of colour, like the northern lights" },
  { kind: "sunrise", group: "Calm", name: "Day cycle", about: "Night, dawn, daylight and dusk on repeat" },
  { kind: "countdown", group: "Useful", name: "Timer", about: "Counts down, blinks at the end, then pulses" },
  { kind: "musicFlow", group: "Music", name: "Music drift", about: "Drifts through your colours, faster the louder the music", input: "Listens to your PC's audio output (never recorded or sent anywhere)" },
  { kind: "siren", group: "Party", name: "Siren", about: "Two colours in a police-style double flash" },
  { kind: "glitter", group: "Party", name: "Glitter", about: "A base colour with short random sparkles" },
  { kind: "pacedBreathing", group: "Calm", name: "Paced breathing", about: "A guided breath: in, hold, out, rest" },
  { kind: "steps", group: "Party", name: "Steps", about: "Hard cuts between colours, no fade, each held for a while" },
];

const BAR_STYLES: { style: BarStyle; name: string }[] = [
  { style: "plasma", name: "Plasma" }, { style: "bounce", name: "Bouncing balls" }, { style: "fire", name: "Fire" },
  { style: "fountain", name: "Fountain" }, { style: "helix", name: "DNA helix" }, { style: "ecg", name: "Heart monitor" },
  { style: "stacker", name: "Stacker" }, { style: "glitch", name: "Glitch" }, { style: "pump", name: "Kick drum" },
  { style: "fireflies", name: "Fireflies" }, { style: "terrain", name: "Terrain" },
  { style: "fireworks", name: "Fireworks" }, { style: "comet", name: "Comet" }, { style: "matrix", name: "Matrix rain" },
  { style: "snake", name: "Snake" }, { style: "vu", name: "VU meter" }, { style: "ripple", name: "Ripples" },
  { style: "pendulum", name: "Pendulum" }, { style: "lightning", name: "Lightning" }, { style: "rally", name: "Rally" },
  { style: "hourglass", name: "Hourglass" }, { style: "loading", name: "Loading" }, { style: "collide", name: "Collide" },
];

export function defaultLive(kind: LiveEffect["kind"]): LiveEffect {
  switch (kind) {
    case "bars": return { kind, style: "plasma", color: "#0060ff", rainbow: false, speed: 1 };
    case "trip": return { kind, colors: ["#1f45ff", "#00c8ff", "#0a1478", "#9fe8ff", "#5a2bff"], speed: 1 };
    case "lava": return { kind, colors: ["#0010d0", "#00c8ff", "#2f55ff", "#00fff0"], speed: 1 };
    case "breathe": return { kind, colors: ["#0040ff", "#00c8ff", "#6a2cff", "#00fff0"], bpm: 10 };
    case "rave": return { kind, colors: ["#0040ff", "#00ffff", "#6a2cff", "#ffffff"], bpm: 128 };
    case "beatFlash": return { kind, colors: ["#1f45ff", "#00f0ff", "#9fe8ff"] };
    case "morse": return { kind, message: "GG", color: "#00c8ff", background: "#020a3a", wpm: 10 };
    case "bomb": return { kind, color: "#ff1a1a", blast: "#ffb040", fuse: 40 };
    case "flashbang": return { kind, color: "#0030c0", every: 12 };
    case "focus": return { kind, work: "#1f45ff", rest: "#00ff60", minutes: 25, breakMinutes: 5 };
    case "aurora": return { kind, colors: ["#00ff9c", "#00c8ff", "#7a30ff", "#00ffd0"], speed: 1 };
    case "sunrise": return { kind, minutes: 10 };
    case "countdown": return { kind, color: "#0060ff", warn: "#ff1020", minutes: 5 };
    case "musicFlow": return { kind, colors: ["#0040ff", "#00c8ff", "#6a2cff", "#00fff0"] };
    case "paletteFlow": return { kind, colors: ["#00fff0", "#0080ff", "#0020ff", "#00c8ff"], period: 10 };
    case "heartbeat": return { kind, color: "#0050ff", bpm: 60 };
    case "flicker": return { kind, color: "#1e6bff", intensity: 0.45 };
    case "tide": return { kind, a: "#001aa0", b: "#00e5ff", period: 7 };
    case "storm": return { kind, sky: "#050a30", flash: "#e0f0ff", rate: 0.8 };
    case "strobe": return { kind, a: "#ff0010", b: "#0030ff", hz: 2 };
    case "audioGlow": return { kind, quiet: "#0020a0", loud: "#00f0ff" };
    case "cpuHeat": return { kind, cool: "#0040ff", hot: "#ff2000" };
    case "screenAmbient": return { kind, saturation: 1.4 };
    case "spectrum": return { kind, color: "#00a8ff", rainbow: false, mirror: false };
    case "sineBars": return { kind, color: "#0060ff", speed: 1 };
    case "rainBars": return { kind, color: "#00c8ff", rate: 0.9 };
    case "scanner": return { kind, color: "#00e5ff", speed: 1 };
    case "equalizer": return { kind, color: "#2962ff", speed: 1 };
    case "siren": return { kind, a: "#ff0010", b: "#0030ff", speed: 1 };
    case "glitter": return { kind, base: "#0a1050", spark: "#ffffff", density: 0.6 };
    case "pacedBreathing": return { kind, color: "#00c8a0", inhale: 4, hold: 4, exhale: 4, rest: 4 };
    case "steps": return { kind, colors: ["#00ff40", "#ffb000", "#ff1020"], hold: 2 };
  }
}

function Params({ fx, set }: { fx: LiveEffect; set: (f: LiveEffect) => void }) {
  const c = (label: string, value: Hex, patch: (v: Hex) => Partial<LiveEffect>) => (
    <ColorField key={label} label={label} value={value} onChange={(v) => set({ ...fx, ...patch(v) } as LiveEffect)} swatches={false} />
  );
  const s = (label: string, value: number, min: number, max: number, step: number, patch: (v: number) => Partial<LiveEffect>, fmt?: (v: number) => string) => (
    <Slider key={label} label={label} value={value} min={min} max={max} step={step} onChange={(v) => set({ ...fx, ...patch(v) } as LiveEffect)} format={fmt} />
  );
  const pal = (colors: Hex[]) => (
    <div className="palette-edit" key="palette">
      {colors.map((col, i) => (
        <div key={i} className="pal-item">
          <input type="color" value={col} onChange={(e) => set({ ...fx, colors: colors.map((x, j) => (j === i ? e.target.value : x)) } as LiveEffect)} aria-label={`colour ${i + 1}`} />
          {colors.length > 2 && <button className="pal-remove" onClick={() => set({ ...fx, colors: colors.filter((_, j) => j !== i) } as LiveEffect)} aria-label={`remove colour ${i + 1}`}>×</button>}
        </div>
      ))}
      {colors.length < 8 && <button className="pal-add" onClick={() => set({ ...fx, colors: [...colors, "#0040ff"] } as LiveEffect)} aria-label="add a colour" title="Add a colour">+</button>}
    </div>
  );
  const speed = (v: number) => s("Speed", v, 0.2, 3, 0.1, (speed) => ({ speed }), (x) => `${x.toFixed(1)}×`);
  switch (fx.kind) {
    case "paletteFlow": return <>{pal(fx.colors)}{s("Cycle length", fx.period, 2, 60, 1, (period) => ({ period }), (v) => `${v}s`)}</>;
    case "trip": case "lava": return <>{pal(fx.colors)}{speed(fx.speed)}</>;
    case "breathe": return <>{pal(fx.colors)}{s("Breaths per minute", fx.bpm, 3, 30, 1, (bpm) => ({ bpm }))}</>;
    case "rave": return <>{pal(fx.colors)}{s("Tempo", fx.bpm, 60, 200, 1, (bpm) => ({ bpm }), (v) => `${v} bpm`)}</>;
    case "beatFlash": return pal(fx.colors);
    case "morse":
      return (
        <>
          <label className="field" key="msg">
            <span>Message</span>
            <input value={fx.message} maxLength={40} onChange={(e) => set({ ...fx, message: e.target.value })} aria-label="Morse message" />
          </label>
          {c("Signal", fx.color, (color) => ({ color }))}{c("Background", fx.background, (background) => ({ background }))}
          {s("Speed", fx.wpm, 4, 25, 1, (wpm) => ({ wpm }), (v) => `${v} wpm`)}
        </>
      );
    case "aurora": return <>{pal(fx.colors)}{speed(fx.speed)}</>;
    case "musicFlow": return pal(fx.colors);
    case "sunrise": return s("A day takes", fx.minutes, 2, 60, 1, (minutes) => ({ minutes }), (v) => `${v} min`);
    case "countdown":
      return (
        <>
          {c("Counting", fx.color, (color) => ({ color }))}{c("Nearly up", fx.warn, (warn) => ({ warn }))}
          {s("Time", fx.minutes, 1, 90, 1, (minutes) => ({ minutes }), (v) => `${v} min`)}
        </>
      );
    case "bomb": return <>{c("Bomb light", fx.color, (color) => ({ color }))}{c("Blast", fx.blast, (blast) => ({ blast }))}{s("Fuse", fx.fuse, 10, 60, 1, (fuse) => ({ fuse }), (v) => `${v}s`)}</>;
    case "flashbang": return <>{c("Colour", fx.color, (color) => ({ color }))}{s("One every", fx.every, 3, 60, 1, (every) => ({ every }), (v) => `${v}s`)}</>;
    case "focus":
      return (
        <>
          {c("Focus", fx.work, (work) => ({ work }))}{c("Break", fx.rest, (rest) => ({ rest }))}
          {s("Focus length", fx.minutes, 5, 90, 5, (minutes) => ({ minutes }), (v) => `${v} min`)}
          {s("Break length", fx.breakMinutes, 1, 30, 1, (breakMinutes) => ({ breakMinutes }), (v) => `${v} min`)}
        </>
      );
    case "bars":
      return (
        <>
          <label className="field" key="style">
            <span>Pattern</span>
            <select value={fx.style} onChange={(e) => set({ ...fx, style: e.target.value as BarStyle })} aria-label="Bar pattern">
              {BAR_STYLES.map((b) => <option key={b.style} value={b.style}>{b.name}</option>)}
            </select>
          </label>
          {!fx.rainbow && c("Colour", fx.color, (color) => ({ color }))}
          <Toggle key="rainbow" label="Rainbow" checked={fx.rainbow} onChange={(rainbow) => set({ ...fx, rainbow })} />
          {speed(fx.speed)}
        </>
      );
    case "heartbeat": return <>{c("Colour", fx.color, (color) => ({ color }))}{s("Beats per minute", fx.bpm, 30, 180, 1, (bpm) => ({ bpm }))}</>;
    case "flicker": return <>{c("Colour", fx.color, (color) => ({ color }))}{s("Intensity", fx.intensity, 0, 1, 0.05, (intensity) => ({ intensity }), (v) => `${Math.round(v * 100)}%`)}</>;
    case "tide": return <>{c("Low tide", fx.a, (a) => ({ a }))}{c("High tide", fx.b, (b) => ({ b }))}{s("Swell length", fx.period, 2, 30, 1, (period) => ({ period }), (v) => `${v}s`)}</>;
    case "storm": return <>{c("Sky", fx.sky, (sky) => ({ sky }))}{c("Lightning", fx.flash, (flash) => ({ flash }))}{s("Strikes", fx.rate, 0.2, 3, 0.1, (rate) => ({ rate }), (v) => `${v.toFixed(1)}/s`)}</>;
    case "strobe": return <>{c("Colour A", fx.a, (a) => ({ a }))}{c("Colour B", fx.b, (b) => ({ b }))}{s("Speed", fx.hz, 0.5, 8, 0.5, (hz) => ({ hz }), (v) => `${v} Hz`)}</>;
    case "audioGlow": return <>{c("Quiet", fx.quiet, (quiet) => ({ quiet }))}{c("Loud", fx.loud, (loud) => ({ loud }))}</>;
    case "cpuHeat": return <>{c("Idle", fx.cool, (cool) => ({ cool }))}{c("Busy", fx.hot, (hot) => ({ hot }))}</>;
    case "screenAmbient": return s("Saturation boost", fx.saturation, 0.5, 2.5, 0.1, (saturation) => ({ saturation }), (v) => `${v.toFixed(1)}×`);
    case "spectrum":
      return (
        <>
          {!fx.rainbow && c("Colour", fx.color, (color) => ({ color }))}
          <Toggle label="Rainbow" checked={fx.rainbow} onChange={(rainbow) => set({ ...fx, rainbow })} />
          <Toggle label="Mirror (bass in the middle)" checked={fx.mirror} onChange={(mirror) => set({ ...fx, mirror })} />
        </>
      );
    case "sineBars": case "scanner": case "equalizer":
      return <>{c("Colour", fx.color, (color) => ({ color }))}{s("Speed", fx.speed, 0.2, 3, 0.1, (speed) => ({ speed }), (v) => `${v.toFixed(1)}×`)}</>;
    case "rainBars": return <>{c("Colour", fx.color, (color) => ({ color }))}{s("Rain", fx.rate, 0.2, 3, 0.1, (rate) => ({ rate }), (v) => `${v.toFixed(1)}×`)}</>;
    case "siren": return <>{c("Colour A", fx.a, (a) => ({ a }))}{c("Colour B", fx.b, (b) => ({ b }))}{speed(fx.speed)}</>;
    case "glitter":
      return (
        <>
          {c("Base", fx.base, (base) => ({ base }))}{c("Sparkle", fx.spark, (spark) => ({ spark }))}
          {s("Density", fx.density, 0.1, 2, 0.1, (density) => ({ density }), (v) => `${v.toFixed(1)}×`)}
        </>
      );
    case "pacedBreathing":
      return (
        <>
          {c("Colour", fx.color, (color) => ({ color }))}
          {s("Inhale", fx.inhale, 1, 15, 0.5, (inhale) => ({ inhale }), (v) => `${v}s`)}
          {s("Hold", fx.hold, 0, 15, 0.5, (hold) => ({ hold }), (v) => `${v}s`)}
          {s("Exhale", fx.exhale, 1, 15, 0.5, (exhale) => ({ exhale }), (v) => `${v}s`)}
          {s("Rest", fx.rest, 0, 15, 0.5, (rest) => ({ rest }), (v) => `${v}s`)}
        </>
      );
    case "steps": return <>{pal(fx.colors)}{s("Hold", fx.hold, 0.2, 10, 0.1, (hold) => ({ hold }), (v) => `${v.toFixed(1)}s`)}</>;
  }
}

/** Create > Live: a real-time effect streamed by Keylume, previewed before it's saved. */
export function LiveBuilder({ note }: { note: string }) {
  const { reloadProfiles, toast, apply, layout, editing, edit } = useApp();
  const [fx, setFx] = useState<LiveEffect>(defaultLive("paletteFlow"));
  const [name, setName] = useState("");
  const meta = KINDS.find((k) => k.kind === fx.kind) ?? KINDS[0];

  useEffect(() => {
    if (editing?.lighting.kind === "live") {
      setFx(editing.lighting.live);
      setName(editing.source === "user" ? editing.name : `${editing.name} (mine)`);
      edit(null);
    }
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const preview: Profile = useMemo(() => ({
    id: "preview", name: "", category: "", tags: [], description: "", source: "user", lighting: { kind: "live", live: fx },
  }), [fx]);

  const start = async () => {
    const p: Profile = {
      id: "", name: name.trim() || `My ${meta.name.toLowerCase()}`, category: "Mine", tags: ["live"],
      description: meta.about, source: "user", lighting: { kind: "live", live: fx },
    };
    await attempt("Start", async () => {
      const saved = await (await api()).saveProfile(p);
      await reloadProfiles();
      await apply(saved.id);
      toast(`Running “${saved.name}”, saved to your library`, "success");
    });
  };

  return (
    <div className="builder">
      <div className="stage-card">
        {layout && <AnimatedBoard profile={preview} layout={layout} spill className="stage-board" />}
        <p className="stage-note">{note}</p>
      </div>
      <aside className="panel builder-panel">
        <label className="field">
          <span className="lbl">Effect</span>
          <select value={fx.kind} onChange={(e) => setFx(defaultLive(e.target.value as LiveEffect["kind"]))} aria-label="Live effect type">
            {GROUPS.map((g) => (
              <optgroup key={g} label={g}>
                {KINDS.filter((k) => k.group === g).map((k) => <option key={k.kind} value={k.kind}>{k.name}</option>)}
              </optgroup>
            ))}
          </select>
        </label>
        <p className="hint">{meta.about}{meta.input ? `. ${meta.input}.` : ""}</p>
        <div className="params"><Params fx={fx} set={setFx} /></div>
        <div className="panel-foot"><SaveRow name={name} setName={setName} label="Save & start" onSave={start} /></div>
      </aside>
    </div>
  );
}
