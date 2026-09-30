import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { Keyboard } from "../components/Keyboard";
import { Busy, ColorField, SaveRow, Segmented, Slider } from "../components/controls";
import { Icon } from "../components/Icon";
import { api } from "../lib/api";
import { editorReducer, initEditor } from "../lib/editor";
import { previewColors } from "../lib/library";
import { attempt, outcome, useApp } from "../lib/store";
import type { Hex, Profile } from "../lib/types";

type Tool = "brush" | "fill" | "picker" | "gradient" | "select";

const TOOLS: { value: Tool; label: string; hint: string }[] = [
  { value: "brush", label: "Brush", hint: "Click or drag across keys to paint (or move with the arrow keys and press Enter)" },
  { value: "select", label: "Select", hint: "Click keys to select; then fill or gradient only them" },
  { value: "gradient", label: "Gradient", hint: "Click a start key, then an end key" },
  { value: "picker", label: "Eyedropper", hint: "Click a key to pick up its colour" },
  { value: "fill", label: "Fill", hint: "Click anywhere to fill every key (or the selection)" },
];

/** Create > Paint keys: a per-key design, colour by colour. */
export function PaintEditor() {
  const { layout, editing, edit, reloadProfiles, toast, settings, lighting, features } = useApp();
  const blank = useCallback(() => Object.fromEntries((layout?.keys ?? []).map((k) => [k.id, "#000000"])), [layout]);
  const [state, dispatch] = useReducer(editorReducer, undefined, () => initEditor(blank()));
  const [tool, setTool] = useState<Tool>("brush");
  const [color, setColor] = useState<Hex>("#0060ff");
  const [color2, setColor2] = useState<Hex>("#00fff0");
  const [brightness, setBrightness] = useState(4);
  const [layer, setLayer] = useState(settings?.liveLayer ?? 2);
  const [selection, setSelection] = useState<Set<string>>(new Set());
  const [gradFrom, setGradFrom] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [sourceId, setSourceId] = useState<string | null>(null);
  const [sent, setSent] = useState<number | null>(null); // the preview request, to follow it
  const painting = useRef(false);

  // Opened from the library's "edit a copy".
  useEffect(() => {
    if (editing && layout && editing.lighting.kind === "perKey") {
      dispatch({ type: "load", keys: { ...blank(), ...previewColors(editing, layout) } });
      if (editing.lighting.kind === "perKey") setBrightness(editing.lighting.brightness);
      setName(editing.source === "user" ? editing.name : `${editing.name} (edited)`);
      setSourceId(editing.source === "user" ? editing.id : null);
      edit(null);
    }
  }, [editing, layout, blank, edit]);

  useEffect(() => {
    const up = () => (painting.current = false);
    window.addEventListener("pointerup", up);
    const keys = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") { e.preventDefault(); dispatch({ type: e.shiftKey ? "redo" : "undo" }); }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "y") { e.preventDefault(); dispatch({ type: "redo" }); }
    };
    window.addEventListener("keydown", keys);
    return () => { window.removeEventListener("pointerup", up); window.removeEventListener("keydown", keys); };
  }, []);

  if (!layout) return null;
  const sel = [...selection];

  const onDown = (id: string) => {
    switch (tool) {
      case "brush": painting.current = true; dispatch({ type: "paint", ids: [id], color }); break;
      case "fill": dispatch({ type: "fill", color, ids: sel.length ? sel : undefined }); break;
      case "picker": setColor(state.keys[id]); setTool("brush"); break;
      case "select": {
        const s = new Set(selection);
        if (s.has(id)) s.delete(id); else s.add(id);
        setSelection(s);
        break;
      }
      case "gradient": {
        if (!gradFrom) { setGradFrom(id); break; }
        const from = layout.keys.find((k) => k.id === gradFrom)!;
        const to = layout.keys.find((k) => k.id === id)!;
        dispatch({ type: "gradient", all: layout.keys, from, to, a: color, b: color2, only: sel.length ? sel : undefined });
        setGradFrom(null);
        break;
      }
    }
  };
  const onEnter = (id: string, e: React.PointerEvent) => {
    if (tool === "brush" && painting.current && e.buttons === 1) dispatch({ type: "paint", ids: [id], color, merge: true });
  };

  const run = async (label: string, fn: () => Promise<void>, done?: string) => {
    setBusy(label);
    const ok = await attempt(label, fn);
    setBusy(null);
    if (ok && done) toast(done, "success");
  };

  const preview = () => run("Previewing", async () => setSent(await (await api()).previewKeys(state.keys, brightness)));
  const sentState = outcome(lighting, sent);
  const write = () => run(`Writing layer ${layer + 1}`, async () => (await api()).writeLayer(layer, state.keys, brightness), `Saved to onboard layer ${layer + 1} (works without Keylume running)`);
  const load = () => run(`Reading layer ${layer + 1}`, async () => dispatch({ type: "load", keys: await (await api()).readLayer(layer) }));
  const save = () => run("Saving", async () => {
    const p: Profile = {
      id: sourceId ?? "",
      name: name.trim() || "My design",
      category: "Mine",
      tags: ["per-key"],
      description: "Made in the per-key editor",
      source: "user",
      lighting: { kind: "perKey", keys: state.keys, brightness },
    };
    const saved = await (await api()).saveProfile(p);
    setSourceId(saved.id);
    await reloadProfiles();
  }, "Saved to your library");

  const hint = tool === "gradient" && gradFrom ? `Start: ${gradFrom.toUpperCase()} — now click the end key` : TOOLS.find((t) => t.value === tool)!.hint;

  return (
    <div className="builder">
      <div className="stage-card">
        <div className="stage-bar">
          <Segmented value={tool} onChange={(t) => { setTool(t); setGradFrom(null); }} options={TOOLS.map((t) => ({ value: t.value, label: t.label }))} />
          <span className="bar-actions">
            {selection.size > 0 && <button className="btn ghost small" onClick={() => setSelection(new Set())}>Clear selection ({selection.size})</button>}
            <button className="btn ghost small icon-btn" onClick={() => dispatch({ type: "undo" })} disabled={!state.past.length} title="Undo (Ctrl+Z)" aria-label="Undo"><Icon name="undo" size={16} /></button>
            <button className="btn ghost small icon-btn" onClick={() => dispatch({ type: "redo" })} disabled={!state.future.length} title="Redo (Ctrl+Y)" aria-label="Redo"><Icon name="redo" size={16} /></button>
            <button className="btn ghost small" onClick={() => dispatch({ type: "fill", color: "#000000" })}>Clear all</button>
          </span>
        </div>
        <div className="stage-board paint-board">
          <Keyboard layout={layout} colors={state.keys} selected={selection} marked={gradFrom ? new Set([gradFrom]) : undefined} onKeyDown={onDown} onKeyEnter={onEnter} glow={false}
            describe={(id) => state.keys[id] ?? "unlit"} />
        </div>
        <p className="stage-note">{hint}</p>
      </div>
      <aside className="panel builder-panel">
        <ColorField label={tool === "gradient" ? "Start colour" : "Colour"} value={color} onChange={setColor} />
        {tool === "gradient" && <ColorField label="End colour" value={color2} onChange={setColor2} swatches={false} />}
        <Slider label="Brightness" min={1} max={4} value={brightness} onChange={setBrightness} format={(v) => `${v * 25}%`} />
        <button className="btn wide" onClick={preview}><Icon name="keyboard" size={16} />Show on keyboard</button>
        {sentState && (
          <p className={`hint status-line ${sentState === "failed" ? "error-text" : ""}`} role="status">
            {sentState === "applying" ? "Applying…" : sentState === "shown" ? "On the keyboard" : `Couldn't show it: ${lighting?.error ?? "the keyboard isn't connected"}`}
          </p>
        )}
        {features.pictureLayers > 1 && !features.hostDriven && (
          <details className="advanced">
            <summary>Onboard layers</summary>
            <p className="hint">The keyboard stores {features.pictureLayers} designs itself; they keep working with Keylume closed.</p>
            <Segmented value={layer} onChange={setLayer} options={Array.from({ length: features.pictureLayers }, (_, l) => ({ value: l, label: `Layer ${l + 1}` }))} />
            <div className="row-actions">
              <button className="btn ghost small" onClick={load}>Load from keyboard</button>
              <button className="btn small" onClick={write}>Write to keyboard</button>
            </div>
          </details>
        )}
        <div className="panel-foot">
          <SaveRow name={name} setName={setName} label={sourceId ? "Update" : "Save"} onSave={save} placeholder="Profile name" />
          {busy && <Busy text={`${busy}…`} />}
        </div>
      </aside>
    </div>
  );
}
