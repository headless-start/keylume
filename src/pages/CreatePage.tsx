import { useEffect, useMemo, useState } from "react";
import { AnimatedBoard } from "../components/AnimatedBoard";
import { ColorField, PageHead } from "../components/controls";
import { Icon, type IconName } from "../components/Icon";
import { api } from "../lib/api";
import { attempt, useApp } from "../lib/store";
import type { Features, Hex, Profile, SpellWord } from "../lib/types";
import { PaintEditor } from "./EditorPage";
import { EffectBuilder } from "./LightingPage";
import { LiveBuilder } from "./LivePage";

type Tab = "paint" | "animated" | "live" | "spell";

const TABS: { tab: Tab; label: string; icon: IconName; about: string; hostAbout?: string; needs: (f: Features) => boolean }[] = [
  {
    tab: "paint", label: "Paint keys", icon: "paint", about: "Colour each key yourself. Designs are stored on the keyboard.",
    hostAbout: "Colour each key yourself. Keylume shows your design while it runs.", needs: (f) => f.perKey,
  },
  {
    tab: "animated", label: "Animated", icon: "wand", about: "One of the keyboard's own animations: it keeps running without Keylume. Changes show as you make them.",
    hostAbout: "An animation Keylume draws on the keys while it runs. Changes show as you make them.", needs: (f) => f.effects.some((m) => m !== "off"),
  },
  {
    tab: "live", label: "Live", icon: "bolt", about: "Real-time lighting streamed by Keylume. It keeps running from the tray when you close the window.",
    needs: (f) => f.live,
  },
  { tab: "spell", label: "Spell", icon: "text", about: "Words lit letter by letter on their own keys, then held.", needs: (f) => f.perKey },
];

const tabFor = (p: Profile | null): Tab =>
  p?.lighting.kind === "effect" ? "animated" : p?.lighting.kind === "live" ? "live" : p?.lighting.kind === "spell" ? "spell" : "paint";

/** Everything for making your own profiles, in one place. They're saved under Mine. */
export function CreatePage() {
  const { editing, features } = useApp();
  const tabs = TABS.filter((t) => t.needs(features));
  const [picked, setTab] = useState<Tab>(() => tabFor(editing));
  const tab = tabs.some((t) => t.tab === picked) ? picked : tabs[0]?.tab ?? "paint";
  const meta = TABS.find((t) => t.tab === tab)!;
  const about = features.hostDriven ? meta.hostAbout ?? meta.about : meta.about;
  return (
    <section className="page create">
      <div className="page-wide">
        <PageHead title="Create">
          <div className="segmented big" role="tablist" aria-label="What to create">
            {tabs.map((t) => (
              <button key={t.tab} role="tab" aria-selected={t.tab === tab} className={t.tab === tab ? "on" : ""} onClick={() => setTab(t.tab)}>
                <Icon name={t.icon} size={15} />{t.label}
              </button>
            ))}
          </div>
        </PageHead>
        {tab === "paint" && <PaintEditor />}
        {tab === "animated" && <EffectBuilder note={about} />}
        {tab === "live" && <LiveBuilder note={about} />}
        {tab === "spell" && <SpellMaker note={about} />}
      </div>
    </section>
  );
}

/** Create > Spell: words lit letter by letter on their keys, then held. */
function SpellMaker({ note }: { note: string }) {
  const { layout, editing, edit, reloadProfiles, apply, toast } = useApp();
  const initial = editing?.lighting.kind === "spell" ? editing.lighting : null;
  const [words, setWords] = useState<SpellWord[]>(initial?.words ?? [{ text: "", color: "#00c8ff" }]);
  const [bg, setBg] = useState<Hex>(initial?.background ?? "#020a3a");
  useEffect(() => { if (initial) edit(null); }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const preview: Profile = useMemo(() => ({
    id: "preview", name: "", category: "", tags: [], description: "", source: "user",
    lighting: { kind: "spell", words: words.filter((w) => w.text.trim()), background: bg, brightness: 4 },
  }), [words, bg]);
  const setWord = (i: number, w: Partial<SpellWord>) => setWords(words.map((x, j) => (j === i ? { ...x, ...w } : x)));

  const save = async () => {
    const cleaned = words.filter((w) => w.text.trim());
    if (!cleaned.length) return toast("Type a word first", "error");
    const p: Profile = {
      id: "", name: `Spell ${cleaned.map((w) => w.text.trim()).join(" & ")}`, category: "Mine", tags: ["spell"],
      description: "Your spell", source: "user", lighting: { kind: "spell", words: cleaned, background: bg, brightness: 4 },
    };
    await attempt("Spell", async () => {
      const saved = await (await api()).saveProfile(p);
      await reloadProfiles();
      await apply(saved.id);
      toast("Spelling on your keyboard (about 2 s per letter), then it stays lit", "success");
    });
  };

  if (!layout) return null;
  return (
    <div className="builder">
      <div className="stage-card">
        <AnimatedBoard profile={preview} layout={layout} legends spill className="stage-board" />
        <p className="stage-note">{note} The keyboard takes about 2 s per letter, so a spell plays once each time you start it.</p>
      </div>
      <aside className="panel builder-panel">
        <h2>Words</h2>
        <div className="spell-words">
          {words.map((w, i) => (
            <div key={i} className="spell-word">
              <input value={w.text} maxLength={16} placeholder="Word" aria-label={`Word ${i + 1}`} onChange={(e) => setWord(i, { text: e.target.value })} />
              <input type="color" value={w.color} aria-label={`Word ${i + 1} colour`} onChange={(e) => setWord(i, { color: e.target.value })} />
              {words.length > 1 && <button className="icon" aria-label={`Remove word ${i + 1}`} onClick={() => setWords(words.filter((_, j) => j !== i))}><Icon name="close" size={14} /></button>}
            </div>
          ))}
          {words.length < 4 && <button className="btn ghost small add-word" onClick={() => setWords([...words, { text: "", color: "#ffb000" }])}>+ Word</button>}
        </div>
        <ColorField label="Background" value={bg} onChange={setBg} swatches={false} />
        <div className="panel-foot"><button className="btn primary wide" onClick={save}>Save & spell it</button></div>
      </aside>
    </div>
  );
}
