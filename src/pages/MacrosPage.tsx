import { useEffect, useRef, useState } from "react";
import { Busy } from "../components/controls";
import { Icon } from "../components/Icon";
import { api } from "../lib/api";
import { codeToHid, hidName, HID_KEYS } from "../lib/keys";
import { encodedSize, MACRO_BYTES, normalize, Recorder } from "../lib/macro";
import { attempt, useApp } from "../lib/store";
import type { Macro, MacroEvent } from "../lib/types";

/** Keys > Macros: recorded key sequences stored on the keyboard (16 slots). */
export function MacrosView() {
  const { toast } = useApp();
  const [slot, setSlot] = useState(0);
  const [macro, setMacro] = useState<Macro>({ repeat: 1, events: [] });
  const [dirty, setDirty] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [recording, setRecording] = useState(false);
  const rec = useRef<Recorder | null>(null);

  useEffect(() => {
    let alive = true;
    setBusy("Reading macro");
    api().then((a) => a.getMacro(slot)).then((m) => { if (alive) { setMacro(m.repeat ? m : { ...m, repeat: 1 }); setDirty(false); } })
      .catch((e) => toast(String(e), "error")).finally(() => alive && setBusy(null));
    return () => { alive = false; };
  }, [slot, toast]);

  useEffect(() => {
    if (!recording) return;
    const handler = (e: KeyboardEvent) => {
      const code = codeToHid(e.code);
      if (code === undefined) return;
      e.preventDefault();
      rec.current?.push(code, e.type === "keydown", e.timeStamp);
    };
    window.addEventListener("keydown", handler, true);
    window.addEventListener("keyup", handler, true);
    return () => { window.removeEventListener("keydown", handler, true); window.removeEventListener("keyup", handler, true); };
  }, [recording]);

  const toggleRecord = () => {
    if (!recording) { rec.current = new Recorder(); setRecording(true); return; }
    const events = rec.current!.finish(performance.now());
    setRecording(false);
    if (events.length) { setMacro({ ...macro, events }); setDirty(true); }
  };

  const update = (events: MacroEvent[]) => { setMacro({ ...macro, events }); setDirty(true); };
  const size = encodedSize(macro);
  const tooBig = size > MACRO_BYTES;

  const write = async () => {
    setBusy("Writing macro");
    const ok = await attempt("Write macro", async () => (await api()).setMacro(slot, { ...macro, events: normalize(macro.events) }));
    setBusy(null);
    if (ok) { setDirty(false); toast(`Macro ${slot + 1} saved on the keyboard`, "success"); }
  };

  return (
    <div className="builder">
      <div className="stage-card">
        <div className="stage-bar">
          <button className={`btn ${recording ? "danger" : ""}`} onClick={toggleRecord}>
            <span className={`rec-dot ${recording ? "on" : ""}`} aria-hidden />{recording ? "Stop recording" : "Record"}
          </button>
          <label className="inline">Repeat
            <input type="number" min={1} max={65535} value={macro.repeat} onChange={(e) => { setMacro({ ...macro, repeat: Math.max(1, Number(e.target.value) || 1) }); setDirty(true); }} />
          </label>
          <span className="spacer" />
          <span className={`bytes ${tooBig ? "over" : ""}`}>{size} / {MACRO_BYTES} bytes</span>
          <button className="btn ghost small" onClick={() => update([])} disabled={!macro.events.length}>Clear</button>
        </div>
        {recording && <p className="recording">Recording… type the sequence now (every key is captured, including Enter). Click Stop when done.</p>}

        {macro.events.length ? (
          <ol className="events">
            {macro.events.map((e, i) => (
              <li key={i} className={e.kind}>
                {e.kind === "key" ? (
                  <>
                    <span className={`arrow ${e.down ? "down" : "up"}`}>{e.down ? "↓" : "↑"}</span>
                    <select value={e.code} onChange={(ev) => update(macro.events.map((x, j) => (j === i ? { ...e, code: Number(ev.target.value) } : x)))} aria-label="key">
                      {HID_KEYS.map((k) => <option key={k.code} value={k.code}>{k.name}</option>)}
                    </select>
                    <span className="hint">{e.down ? "press" : "release"} {hidName(e.code)}</span>
                  </>
                ) : (
                  <>
                    <span className="arrow">⏱</span>
                    <input type="number" min={1} max={65535} value={e.ms} onChange={(ev) => update(macro.events.map((x, j) => (j === i ? { kind: "delay", ms: Math.max(1, Number(ev.target.value) || 1) } : x)))} aria-label="delay in milliseconds" />
                    <span className="hint">ms</span>
                  </>
                )}
                <button className="icon" onClick={() => update(macro.events.filter((_, j) => j !== i))} aria-label="remove step"><Icon name="close" size={14} /></button>
              </li>
            ))}
          </ol>
        ) : !recording && (
          <div className="panel-empty">
            <Icon name="macro" size={26} />
            <b>Macro {slot + 1} is empty</b>
            <span>Press Record and type, or add steps below.</span>
          </div>
        )}

        <div className="stage-bar">
          <button className="btn ghost small" onClick={() => update([...macro.events, { kind: "key", code: 0x04, down: true }, { kind: "delay", ms: 20 }, { kind: "key", code: 0x04, down: false }])}>+ Key press</button>
          <button className="btn ghost small" onClick={() => update([...macro.events, { kind: "delay", ms: 100 }])}>+ Delay</button>
        </div>
        <p className="stage-note">Recorded key sequences, stored on the keyboard. Give one to a key under Remap keys.</p>
      </div>

      <aside className="panel builder-panel">
        <div className="panel-title">
          <h2>Slots</h2>
          <p className="hint">The keyboard keeps 16 macros.</p>
        </div>
        <div className="slots" role="radiogroup" aria-label="Macro slots">
          {Array.from({ length: 16 }, (_, i) => (
            <button key={i} role="radio" aria-checked={i === slot} aria-label={`Macro ${i + 1}`} className={`slot ${i === slot ? "on" : ""}`} onClick={() => !recording && setSlot(i)}>{i + 1}</button>
          ))}
        </div>
        <div className="panel-foot">
          {tooBig && <p className="error-text">Too long for the keyboard's 256-byte macro memory: remove some steps.</p>}
          <button className="btn primary wide" disabled={!dirty || tooBig || !!busy || recording} onClick={write}>Write to keyboard</button>
          {busy && <Busy text={`${busy}…`} />}
        </div>
      </aside>
    </div>
  );
}
