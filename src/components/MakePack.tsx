import { useEffect, useState } from "react";
import { api } from "../lib/api";
import { kindLabel, stillColors } from "../lib/library";
import { useApp } from "../lib/store";
import type { Licence, MakeRequest, Maker } from "../lib/types";
import { Segmented } from "./controls";
import { Dialog } from "./Dialog";
import { Thumb } from "./Thumb";

/** The next version after `v`: its last number plus one ("1.0" → "1.1", "2" → "3"). */
export function nextVersion(v: string): string {
  const m = v.trim().match(/^(.*?)(\d+)$/);
  return m ? `${m[1]}${Number(m[2]) + 1}` : "1.0";
}

/** Make a pack of your own designs: named, described and signed as yours, saved as one
 * file to give away or sell anywhere. People add it by dropping it on Keylume. */
export function MakePack({ onClose }: { onClose: () => void }) {
  const { profiles, layout, toast } = useApp();
  const mine = profiles.filter((p) => p.source === "user");
  const [maker, setMaker] = useState<Maker | null>(null);
  const [form, setForm] = useState<MakeRequest>({ name: "", maker: "", version: "1.0", description: "", licence: "share", url: "", designs: mine.map((p) => p.id) });
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    api().then((a) => a.makerInfo()).then((m) => {
      setMaker(m);
      setForm((f) => ({ ...f, maker: f.maker || m.name, url: f.url || m.url, licence: m.licence }));
    }).catch(() => {});
  }, []);

  const set = (patch: Partial<MakeRequest>) => { setProblem(null); setForm((f) => ({ ...f, ...patch })); };
  // a pack you made before with this name: this is its next version
  const onName = (name: string) => {
    const before = maker?.made.find((m) => m.name.toLowerCase() === name.trim().toLowerCase());
    set({ name, ...(before ? { version: nextVersion(before.version), description: form.description || before.description } : {}) });
  };
  const picked = new Set(form.designs);
  const toggle = (id: string) => set({ designs: picked.has(id) ? form.designs.filter((d) => d !== id) : [...form.designs, id] });

  const save = async () => {
    const missing = !form.name.trim() ? "Give the pack a name" : !form.maker.trim() ? "Say who made it" : !form.description.trim() ? "Describe it in a few words"
      : !form.version.trim() ? "Give it a version" : !form.designs.length ? "Pick at least one design" : null;
    if (missing) return setProblem(missing);
    setBusy(true);
    let file: string | null = null;
    try {
      file = await (await api()).makePack(form);
    } catch (e) {
      setProblem(`That didn't work: ${e instanceof Error ? e.message : String(e)}`);
    }
    setBusy(false);
    if (!file) return;
    toast(`Saved ${file}. Share it or sell it anywhere: people add it by dropping it on Keylume.`, "success");
    onClose();
  };

  if (!layout) return null;
  return (
    <Dialog title="Make a pack" onClose={onClose} footer={
      <>
        <p className="hint">Keylume signs it as yours: people can tell it hasn't been changed, and only you can update it.</p>
        <button className="btn ghost" onClick={onClose}>Cancel</button>
        <button className="btn primary" onClick={save} disabled={busy}>Save pack…</button>
      </>
    }>
      <div className="form-grid">
        <label className="field span2">
          <span className="lbl">Name</span>
          <input value={form.name} maxLength={40} placeholder="Neon Nights" onChange={(e) => onName(e.target.value)} list="packs-made" />
          <datalist id="packs-made">{maker?.made.map((m) => <option key={m.id} value={m.name} />)}</datalist>
        </label>
        <label className="field span2">
          <span className="lbl">Description</span>
          <input value={form.description} maxLength={300} placeholder="What's in it, in a line" onChange={(e) => set({ description: e.target.value })} />
        </label>
        <label className="field">
          <span className="lbl">Made by</span>
          <input value={form.maker} maxLength={60} placeholder="Your name" onChange={(e) => set({ maker: e.target.value })} />
        </label>
        <label className="field">
          <span className="lbl">Version</span>
          <input value={form.version} maxLength={20} onChange={(e) => set({ version: e.target.value })} />
        </label>
        <label className="field span2">
          <span className="lbl">Your page (optional)</span>
          <input value={form.url} maxLength={200} placeholder="https://… where people find your packs" onChange={(e) => set({ url: e.target.value })} />
        </label>
        <div className="field span2">
          <Segmented<Licence> label="Who may have it" value={form.licence} onChange={(licence) => set({ licence })}
            options={[{ value: "share", label: "Anyone: free to pass on" }, { value: "personal", label: "Only who gets it from you" }]} />
        </div>
      </div>

      <div className="pick-head">
        <span className="lbl">Designs ({form.designs.length} of {mine.length})</span>
        <button className="link" onClick={() => set({ designs: mine.map((p) => p.id) })}>All</button>
        <button className="link" onClick={() => set({ designs: [] })}>None</button>
      </div>
      <ul className="pick-list" aria-label="Designs in the pack">
        {mine.map((p) => (
          <li key={p.id}>
            <label className={picked.has(p.id) ? "on" : ""}>
              <input type="checkbox" checked={picked.has(p.id)} onChange={() => toggle(p.id)} />
              <span className="pick-thumb"><Thumb layout={layout} colors={stillColors(p, layout)} /></span>
              <span className="pick-name">{p.name}</span>
              <span className={`kind k-${p.lighting.kind}`}>{kindLabel(p)}</span>
            </label>
          </li>
        ))}
      </ul>
      {problem && <p className="error-text" role="alert">{problem}</p>}
    </Dialog>
  );
}
