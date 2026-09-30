import { useEffect, useMemo, useState } from "react";
import { Keyboard } from "../components/Keyboard";
import { Busy, PageHead, Segmented } from "../components/controls";
import { api } from "../lib/api";
import { actionLabel, CONSUMER, fnDefault, HID_KEYS, keyLegend, isDefaultAction, MACRO_MODES, MODIFIERS, MOUSE, sameAction, SYSTEM } from "../lib/keys";
import { attempt, useApp } from "../lib/store";
import { Icon } from "../components/Icon";
import { MacrosView } from "./MacrosPage";
import type { KeyAction, KeyLayer, LayoutKey } from "../lib/types";

type Tab = "key" | "media" | "mouse" | "system" | "macro" | "profile" | "other";

function ActionPicker({ keyDef, value, onChange, onMacros }: { keyDef: LayoutKey; value: KeyAction; onChange: (a: KeyAction) => void; onMacros: () => void }) {
  const initialTab: Tab = { key: "key", consumer: "media", mouse: "mouse", system: "system", macro: "macro", profile: "profile" }[value.kind as string] as Tab ?? "other";
  const [tab, setTab] = useState<Tab>(initialTab);
  const [q, setQ] = useState("");
  const keyVal = value.kind === "key" ? value : { kind: "key" as const, code: keyDef.hid || 0x04, modifier: 0, code2: 0 };
  const groups = useMemo(() => {
    const list = HID_KEYS.filter((k) => !q || k.name.toLowerCase().includes(q.toLowerCase()));
    return [...new Set(list.map((k) => k.group))].map((g) => ({ g, keys: list.filter((k) => k.group === g) }));
  }, [q]);

  return (
    <div className="picker">
      <Segmented className="grid4" value={tab} onChange={setTab} options={[
        { value: "key", label: "Key" }, { value: "media", label: "Media" }, { value: "mouse", label: "Mouse" },
        { value: "macro", label: "Macro" }, { value: "system", label: "System" }, { value: "profile", label: "Profile" }, { value: "other", label: "Other" },
      ]} />
      {tab === "key" && (
        <div className="picker-body key-tab">
          <div className="picker-row">
            <select value={keyVal.modifier} onChange={(e) => onChange({ ...keyVal, modifier: Number(e.target.value) })} aria-label="Modifier">
              {MODIFIERS.map((m) => <option key={m.code} value={m.code}>{m.code ? m.name : "No modifier"}</option>)}
            </select>
            <input type="search" placeholder="Find a key…" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Find a key" />
          </div>
          <div className="keylist">
            {groups.map(({ g, keys }) => (
              <div key={g}>
                <h4>{g}</h4>
                <div className="keychips">
                  {keys.map((k) => (
                    <button key={k.code} className={`chip ${value.kind === "key" && value.code === k.code ? "on" : ""}`} onClick={() => onChange({ ...keyVal, code: k.code })}>{k.name}</button>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
      {tab === "media" && (
        <div className="keychips">
          {CONSUMER.map((c) => <button key={c.usage} className={`chip ${value.kind === "consumer" && value.usage === c.usage ? "on" : ""}`} onClick={() => onChange({ kind: "consumer", usage: c.usage })}>{c.name}</button>)}
        </div>
      )}
      {tab === "mouse" && (
        <div className="keychips">
          {MOUSE.map((m) => <button key={m.name} className={`chip ${value.kind === "mouse" && value.code === m.code && value.arg === m.arg ? "on" : ""}`} onClick={() => onChange({ kind: "mouse", code: m.code, arg: m.arg })}>{m.name}</button>)}
        </div>
      )}
      {tab === "system" && (
        <div className="keychips">
          {SYSTEM.map((s) => <button key={s.code} className={`chip ${value.kind === "system" && value.code === s.code ? "on" : ""}`} onClick={() => onChange({ kind: "system", code: s.code })}>{s.name}</button>)}
        </div>
      )}
      {tab === "macro" && (
        <div className="picker-body">
          <div className="keychips">
            {Array.from({ length: 16 }, (_, i) => (
              <button key={i} className={`chip ${value.kind === "macro" && value.index === i ? "on" : ""}`} onClick={() => onChange({ kind: "macro", index: i, mode: value.kind === "macro" ? value.mode : 0 })}>Macro {i + 1}</button>
            ))}
          </div>
          {value.kind === "macro" && (
            <Segmented label="Playback" value={value.mode} onChange={(mode) => onChange({ ...value, mode })} options={MACRO_MODES.map((m, i) => ({ value: i, label: m }))} />
          )}
          <p className="hint">Record them in <a onClick={onMacros}>Macros</a>.</p>
        </div>
      )}
      {tab === "profile" && (
        <div className="keychips">
          <button className="chip" onClick={() => onChange({ kind: "profile", op: 1, arg: 0 })}>Next onboard profile</button>
          <button className="chip" onClick={() => onChange({ kind: "profile", op: 2, arg: 0 })}>Previous onboard profile</button>
          <button className="chip" onClick={() => onChange({ kind: "profile", op: 3, arg: 0 })}>Cycle profiles</button>
          {[0, 1, 2].map((p) => <button key={p} className="chip" onClick={() => onChange({ kind: "profile", op: 4, arg: p })}>Profile {p + 1}</button>)}
        </div>
      )}
      {tab === "other" && (
        <div className="keychips">
          <button className={`chip ${value.kind === "disabled" ? "on" : ""}`} onClick={() => onChange({ kind: "disabled" })}>Disable key</button>
          <button className={`chip ${value.kind === "fn" ? "on" : ""}`} onClick={() => onChange({ kind: "fn" })}>Fn</button>
          {value.kind === "raw" && <span className="hint">Now: {actionLabel(value).toLowerCase()} (kept as it is unless you pick something else)</span>}
        </div>
      )}
    </div>
  );
}

/** Keys: what each key does (stored on the keyboard), and the macros keys can play. */
export function KeysPage() {
  const [view, setView] = useState<"remap" | "macros">("remap");
  const switcher = (
    <div className="segmented big" role="tablist" aria-label="Keys">
      <button role="tab" aria-selected={view === "remap"} className={view === "remap" ? "on" : ""} onClick={() => setView("remap")}>Remap keys</button>
      <button role="tab" aria-selected={view === "macros"} className={view === "macros" ? "on" : ""} onClick={() => setView("macros")}>Macros</button>
    </div>
  );
  return (
    <section className="page keys">
      <div className="page-wide">
        <PageHead title="Keys">{switcher}</PageHead>
        {view === "remap" ? <RemapView onMacros={() => setView("macros")} /> : <MacrosView />}
      </div>
    </section>
  );
}

function RemapView({ onMacros }: { onMacros: () => void }) {
  const { layout, toast } = useApp();
  const [layer, setLayer] = useState<KeyLayer>("base");
  const [profile, setProfile] = useState(0);
  const [device, setDevice] = useState<Record<string, KeyAction> | null>(null);
  const [pending, setPending] = useState<Record<string, KeyAction>>({});
  const [selected, setSelected] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>("Reading key map");

  useEffect(() => {
    let alive = true;
    setBusy("Reading key map");
    setPending({});
    api().then((a) => a.getKeymap(layer, profile)).then((m) => alive && setDevice(m)).catch((e) => toast(String(e), "error")).finally(() => alive && setBusy(null));
    return () => { alive = false; };
  }, [layer, profile, toast]);

  if (!layout) return null;
  const actions = { ...(device ?? {}), ...pending };
  const labels = Object.fromEntries(layout.keys.map((k) => {
    const a = actions[k.id];
    if (!a) return [k.id, k.label];
    if (layer === "fn" && a.kind === "disabled") return [k.id, ""];
    return [k.id, isDefaultAction(a, k.hid, k.id) ? k.label : keyLegend(a)];
  }));
  const colors = Object.fromEntries(layout.keys.map((k) => {
    const a = actions[k.id];
    const changed = a && (layer === "fn" ? !sameAction(a, fnDefault(layout, k.id)) : !isDefaultAction(a, k.hid, k.id));
    return [k.id, k.id === selected ? "#3b64ff" : changed ? "#1d3a8f" : "#1a1d26"];
  }));
  const selKey = layout.keys.find((k) => k.id === selected);
  const count = Object.keys(pending).length;

  const write = async () => {
    setBusy("Writing to keyboard");
    const ok = await attempt("Write key map", async () => (await api()).setKeymap(layer, profile, pending));
    setBusy(null);
    if (ok) {
      setDevice({ ...(device ?? {}), ...pending });
      setPending({});
      toast(`Saved ${count} key${count === 1 ? "" : "s"} to ${layer === "fn" ? "the Fn layer" : `onboard profile ${profile + 1}`}`, "success");
    } else {
      // show what the keyboard holds now; what it didn't keep stays to be written
      api().then((a) => a.getKeymap(layer, profile)).then(setDevice).catch(() => {});
    }
  };

  // back to what the key does by default: its own key on the base layer, the standard Fn
  // layer's function on the Fn layer (F1 … F12 on the number row of a board without an F-row)
  const reset = (k: LayoutKey): KeyAction => (layer === "fn" ? fnDefault(layout, k.id) : k.id === "fn" ? { kind: "fn" } : { kind: "key", code: k.hid, modifier: 0, code2: 0 });
  // a change counts only when it differs from what the keyboard holds
  const stage = (changes: Record<string, KeyAction>) => {
    const next = { ...pending };
    for (const [id, a] of Object.entries(changes)) {
      if (device && sameAction(a, device[id])) delete next[id];
      else next[id] = a;
    }
    setPending(next);
  };
  const resetAll = () => stage(Object.fromEntries(layout.keys.map((k) => [k.id, reset(k)])));

  return (
    <div className="builder fit-stage">
      <div className="stage-card">
        <div className="stage-bar">
          <Segmented value={layer} onChange={setLayer} options={[{ value: "base", label: "Base layer" }, { value: "fn", label: "Fn layer" }]} />
          {layer === "base" && <Segmented value={profile} onChange={setProfile} options={[0, 1, 2].map((p) => ({ value: p, label: `Profile ${p + 1}` }))} />}
          <span className="bar-actions">
            <button className="btn ghost small" onClick={resetAll} title="Every key on this layer back to what it does by default">Reset layer</button>
          </span>
        </div>
        <div className="stage-board keys-board">
          <Keyboard layout={layout} colors={colors} labels={labels} glow={false} selected={selected ? new Set([selected]) : undefined} marked={new Set(Object.keys(pending))} onKeyDown={(id) => setSelected(id)}
            describe={(id) => (actions[id] ? actionLabel(actions[id]) : "")} />
        </div>
        <div className="stage-note with-legend">
          <span>Changes are stored on the keyboard itself, so they work on any computer, with or without Keylume.</span>
          <span className="legend" aria-hidden><i className="lg-changed" />Changed<i className="lg-pending" />Not written yet</span>
        </div>
      </div>

      <div className="panel-cell">
      <aside className="panel builder-panel">
        {selKey ? (
          <>
            <div className="panel-title with-action">
              <span>
                <h2>{selKey.label || selKey.id}</h2>
                <p className="hint">Now: {actions[selKey.id] ? actionLabel(actions[selKey.id]) : "…"}</p>
              </span>
              <button className="btn ghost small" onClick={() => stage({ [selKey.id]: reset(selKey) })} title="Back to what this key does by default">Reset this key</button>
            </div>
            <ActionPicker key={`${selKey.id}-${layer}`} keyDef={selKey} value={actions[selKey.id] ?? { kind: "disabled" }} onChange={(a) => stage({ [selKey.id]: a })} onMacros={onMacros} />
          </>
        ) : (
          <div className="panel-empty">
            <Icon name="keyboard" size={26} />
            <b>Pick a key</b>
            <span>Click a key on the keyboard to choose what it does.</span>
          </div>
        )}
        <div className="panel-foot">
          <div className="foot-line">
            <span>{count ? `${count} unsaved change${count === 1 ? "" : "s"}` : "No unsaved changes"}</span>
            <button className="link" disabled={!count} onClick={() => setPending({})}>Discard</button>
          </div>
          <button className="btn primary wide" disabled={!count || !!busy} onClick={write}>Write to keyboard</button>
          {busy && <Busy text={`${busy}…`} />}
        </div>
      </aside>
      </div>
    </div>
  );
}
