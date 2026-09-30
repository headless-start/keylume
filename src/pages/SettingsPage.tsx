import { useEffect, useRef, useState } from "react";
import { Busy, Group, PageHead, Row, Segmented, Toggle } from "../components/controls";
import logo from "../assets/logo.svg";
import { api } from "../lib/api";
import { attempt, useApp } from "../lib/store";
import { fnSummary, isStandardFn, standardFnLayer } from "../lib/keys";
import type { DeviceSettings, KeyAction } from "../lib/types";

const minutes = (s: number) => (s === 0 ? "Never" : s < 60 ? `${s}s` : `${Math.round(s / 60)} min`);
const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** Every setting in one place: the keyboard's first (it stores them itself: they're written
 * with Save to keyboard, which shows once something changed), then Keylume's own (they apply
 * at once). */
export function SettingsPage() {
  const { settings, saveSettings, toast, status, features, layout } = useApp();
  // the keyboard's Fn layer, to offer the standard one (F1 … F12 on Fn + the number row)
  const [fnMap, setFnMap] = useState<Record<string, KeyAction> | null>(null);
  const [version, setVersion] = useState("");
  const [dev, setDev] = useState<DeviceSettings | null>(null);
  const [saved, setSaved] = useState("");
  const [busy, setBusy] = useState<string | null>(features.settings ? "Reading the keyboard's settings" : null);
  // a restore that failed says what happened and what to do: it stays until dismissed
  const [problem, setProblem] = useState<string | null>(null);
  const problemBox = useRef<HTMLDivElement>(null);
  useEffect(() => { if (problem) problemBox.current?.scrollIntoView?.({ block: "nearest" }); }, [problem]);
  useEffect(() => { api().then((a) => a.appInfo()).then((i) => setVersion(i.version)); }, []);
  // nothing else may use the keyboard during a backup, restore or reset
  const locked = !!busy || !!status?.busy;
  const device = status?.device ?? null;

  const load = async () => {
    if (!features.settings) return;
    setBusy("Reading the keyboard's settings");
    await attempt("Read settings", async () => {
      const a = await api();
      const d = await a.getDeviceSettings();
      setDev(d);
      setSaved(JSON.stringify(d));
      if (features.keymap) setFnMap(await a.getKeymap("fn", 0));
    });
    setBusy(null);
  };
  useEffect(() => { load(); }, [features.settings, device?.board]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!settings) return null;
  const setApp = (patch: Partial<typeof settings>) => saveSettings({ ...settings, ...patch });
  const dirty = dev !== null && JSON.stringify(dev) !== saved;
  const set = (patch: Partial<DeviceSettings>) => dev && setDev({ ...dev, ...patch });
  const opt = (patch: Partial<DeviceSettings["options"]>) => dev && setDev({ ...dev, options: { ...dev.options, ...patch } });

  const apply = async () => {
    if (!dev) return;
    setBusy("Saving to the keyboard");
    const ok = await attempt("Save settings", async () => (await api()).setDeviceSettings(dev));
    setBusy(null);
    if (ok) { setSaved(JSON.stringify(dev)); toast("Saved on the keyboard", "success"); }
  };

  const useStandardFn = async () => {
    if (!layout) return;
    const a = await api();
    if (!(await a.confirm(`Put the standard Fn layer on the keyboard? ${fnSummary(layout)}. Changes you made to the Fn layer are replaced.`))) return;
    setBusy("Writing the Fn layer");
    const map = standardFnLayer(layout);
    const ok = await attempt("Write the Fn layer", () => a.setKeymap("fn", 0, map));
    // what the keyboard holds now, not what was sent
    const now = await a.getKeymap("fn", 0).catch(() => null);
    setBusy(null);
    if (now) setFnMap(now);
    if (ok && now && isStandardFn(layout, now)) toast("The standard Fn layer is on the keyboard", "success");
  };

  const backup = async () => {
    setBusy("Backing up everything (about 15 s)");
    await attempt("Backup", async () => {
      const file = await (await api()).backupToFile();
      if (file) toast(`Backup saved as ${file}: lighting layers, key maps, macros and settings`, "success");
    });
    setBusy(null);
  };

  const restore = async () => {
    const a = await api();
    setProblem(null);
    let staged;
    try {
      staged = await a.chooseBackup(); // read and checked in full; nothing written yet
    } catch (e) {
      return setProblem(message(e));
    }
    if (!staged) return;
    const macros = staged.macros ? `${staged.macros} macro${staged.macros === 1 ? "" : "s"}` : "macros";
    if (!(await a.confirm(`Restore ${staged.file}? It replaces the keyboard's picture layers, key maps, ${macros} and settings. Keylume saves what the keyboard holds now first, so you can go back.`))) return;
    setBusy("Restoring (about a minute; leave the keyboard plugged in)");
    try {
      const before = await a.restoreBackup(staged.token);
      toast(`Backup restored. What the keyboard held before is saved in ${before}`, "success");
      await load();
    } catch (e) {
      setProblem(message(e));
    }
    setBusy(null);
  };

  const reset = async () => {
    const a = await api();
    if (!(await a.confirm("Factory reset the keyboard? Key maps, macros, lighting and settings go back to defaults. Tip: make a backup first."))) return;
    setBusy("Resetting");
    const ok = await attempt("Factory reset", () => a.factoryReset());
    setBusy(null);
    if (ok) { toast("Keyboard reset to factory defaults", "success"); load(); }
  };

  const f = device?.features ?? features;
  const verified = device?.support === "verified";
  const slider = (label: string, value: number, max: number, step: number, onChange: (v: number) => void, show: (v: number) => string) => (
    <Row label={label}>
      <span className="inline-slider">
        <input type="range" min={0} max={max} step={step} value={value} aria-label={label} onChange={(e) => onChange(Number(e.target.value))} />
        <span className="val">{show(value)}</span>
      </span>
    </Row>
  );

  return (
    <section className="page settings">
      <div className="page-column">
        <PageHead title="Settings" />

        <Group title="Keyboard">
          {device ? (
            <Row label={device.name} hint={[device.maker, device.simulated ? "simulated" : ""].filter(Boolean).join(", ")}>
              <span className={`pill ${verified ? "ok" : "warn"}`} title={verified ? "Checked with Keylume on this model" : undefined}>{verified ? "Verified" : "Experimental"}</span>
            </Row>
          ) : <p className="hint row-note">Connect your keyboard to see its settings.</p>}
          {device && !verified && <p className="hint row-note">Keylume follows the keyboard's own description of its lights, but hasn't been tried on this model yet.</p>}
          {device && f.hostDriven && <p className="hint row-note">Keylume drives its lights while it runs (from the tray too). When Keylume closes, the keyboard shows its own lighting again.</p>}
          {device && f.pictureLayers > 1 && !f.hostDriven && (
            <Row label="Layer for library designs" hint="The keyboard keeps a design in each layer; the others stay yours">
              <Segmented value={settings.liveLayer} onChange={(liveLayer) => setApp({ liveLayer })}
                options={Array.from({ length: f.pictureLayers }, (_, l) => ({ value: l, label: `${l + 1}` }))} />
            </Row>
          )}
          {dev && (
            <>
              <Row label="Polling rate" hint="1000 Hz is the quickest">
                <Segmented value={dev.reportRate} onChange={(reportRate) => set({ reportRate })} options={[125, 250, 500, 1000].map((v) => ({ value: v, label: `${v}` }))} />
              </Row>
              {slider("Debounce", dev.debounce, 10, 1, (debounce) => set({ debounce }), (v) => `${v} ms`)}
              {slider("Lights off after", dev.sleep.bluetooth, 3600, 60, (v) => set({ sleep: { ...dev.sleep, bluetooth: v, wireless: v } }), minutes)}
              {slider("Deep sleep after", dev.sleep.deepBluetooth, 3600, 60, (v) => set({ sleep: { ...dev.sleep, deepBluetooth: v, deepWireless: v } }), minutes)}
            </>
          )}
          {device && features.settings && !dev && (busy ? <Busy text={`${busy}…`} /> : <p className="hint row-note">Couldn't read the keyboard's settings.</p>)}
        </Group>

        {dev && (
          <Group title="Keys">
            <Toggle label="Lock the Windows key" checked={dev.options.winKeyLock} onChange={(winKeyLock) => opt({ winKeyLock })} />
            <Toggle label="Swap WASD and arrows" checked={dev.options.wasdArrowsSwap} onChange={(wasdArrowsSwap) => opt({ wasdArrowsSwap })} />
            <Toggle label="Mac layout" checked={dev.options.macMode} onChange={(macMode) => opt({ macMode })} />
            <Toggle label="Lock typing" hint="Until you turn it off again" checked={dev.options.keyboardLock} onChange={(keyboardLock) => opt({ keyboardLock })} />
            {f.onboardProfiles > 1 && (
              <Row label="Onboard profile" hint={`Which of the ${f.onboardProfiles} stored key maps is active`}>
                <Segmented value={dev.profile} onChange={(profile) => set({ profile })}
                  options={Array.from({ length: f.onboardProfiles }, (_, p) => ({ value: p, label: `${p + 1}` }))} />
              </Row>
            )}
            {fnMap && layout && fnSummary(layout) && (
              <Row label="Fn layer" hint={fnSummary(layout)}>
                {isStandardFn(layout, fnMap)
                  ? <span className="pill ok">In use</span>
                  : <button className="btn small" onClick={useStandardFn} disabled={locked}>Use it</button>}
              </Row>
            )}
          </Group>
        )}

        <Group title="Keylume">
          <Toggle label="True colours" hint="Keys closer to the colours you see here" checked={settings.trueColors} onChange={(trueColors) => setApp({ trueColors })} />
          <Toggle label="Restore the last look" hint="When the keyboard connects" checked={settings.restoreOnConnect} onChange={(restoreOnConnect) => setApp({ restoreOnConnect })} />
          <Toggle label={navigator.userAgent.includes("Windows") ? "Start with Windows" : "Start when you log in"} checked={settings.launchAtLogin} onChange={(launchAtLogin) => setApp({ launchAtLogin })} />
          <Toggle label="Start in the tray" checked={settings.startMinimized} onChange={(startMinimized) => setApp({ startMinimized })} />
          <Toggle label="Hotkeys" hint="Ctrl+Alt+Left or Right for favourites, Ctrl+Alt+L for the lights" checked={settings.hotkeys} onChange={(hotkeys) => setApp({ hotkeys })} />
          <Row label="Shuffle favourites">
            <select value={settings.shuffleMinutes} aria-label="Shuffle favourites" onChange={(e) => setApp({ shuffleMinutes: Number(e.target.value) })}>
              {[[0, "Off"], [5, "Every 5 min"], [10, "Every 10 min"], [30, "Every 30 min"], [60, "Every hour"]].map(([v, l]) => <option key={v} value={v}>{l}</option>)}
            </select>
          </Row>
          <Toggle label="Demo keyboard" hint="Shown when none is plugged in, after a restart" checked={settings.demoMode} onChange={(demoMode) => setApp({ demoMode })} />
          {settings.demoMode && (
            <Row label="Demo plays" hint="A TK68, or a full-size keyboard Keylume lights key by key">
              <Segmented value={settings.demoBoard} onChange={(demoBoard) => setApp({ demoBoard })}
                options={[{ value: "tk68" as const, label: "TK68" }, { value: "lamparray" as const, label: "Full size" }]} />
            </Row>
          )}
        </Group>

        {device && f.backup && (
          <Group title="Backup">
            <Row label="Back up the keyboard" hint="Layers, key maps, macros and settings, in one file">
              <span className="row-buttons">
                <button className="btn small" onClick={backup} disabled={locked}>Back up…</button>
                <button className="btn small ghost" onClick={restore} disabled={locked}>Restore…</button>
              </span>
            </Row>
            <Row label="Factory reset" hint="Everything on the keyboard back to how it came">
              <button className="btn small danger" onClick={reset} disabled={locked}>Reset…</button>
            </Row>
          </Group>
        )}
        {problem && (
          <div className="device-problem" role="alert" ref={problemBox}>
            <p>{problem}</p>
            <button className="btn small" onClick={() => setProblem(null)}>Dismiss</button>
          </div>
        )}
        {(busy || status?.busy) && dev && <Busy text={`${busy ?? `Busy: ${status?.busy}`}…`} />}

        <div className="about-row">
          <img src={logo} alt="" className="about-logo" />
          <span className="row-text">
            <span className="lbl">Keylume {version}</span>
            <span className="hint">Lighting and keys for your keyboard</span>
          </span>
        </div>
        {dirty && (
          <div className="save-bar" role="region" aria-label="Unsaved keyboard settings">
            <span>The keyboard's settings changed</span>
            <button className="btn ghost" onClick={() => setDev(JSON.parse(saved))} disabled={locked}>Discard</button>
            <button className="btn primary" onClick={apply} disabled={locked}>Save to keyboard</button>
          </div>
        )}
      </div>
    </section>
  );
}
