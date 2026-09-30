import { useEffect, useState } from "react";
import { deviceState, LightsButton } from "./components/Device";
import { Icon, type IconName } from "./components/Icon";
import logo from "./assets/logo.svg";
import { api } from "./lib/api";
import { attempt, useApp, type Page } from "./lib/store";
import type { Features } from "./lib/types";
import { CreatePage } from "./pages/CreatePage";
import { HomePage } from "./pages/HomePage";
import { KeysPage } from "./pages/KeysPage";
import { LibraryPage } from "./pages/LibraryPage";
import { SettingsPage } from "./pages/SettingsPage";
import { SidePage } from "./pages/SidePage";

/** Every place past Home, in one rail down the left (Settings at its foot): one way to get
 * around, the same everywhere. */
const PLACES: { page: Page; label: string; icon: IconName }[] = [
  { page: "library", label: "Library", icon: "library" },
  { page: "create", label: "Create", icon: "paint" },
  { page: "side", label: "Side light", icon: "strip" },
  { page: "keys", label: "Keys", icon: "keyboard" },
];

/** Pages the connected keyboard has a use for (no Side light without a side light…). */
export function available(page: Page, f: Features): boolean {
  if (page === "side") return f.sideLight;
  if (page === "keys") return f.keymap || f.macros;
  return true;
}

export function ConflictBanner({ reason }: { reason: string }) {
  const toast = useApp((s) => s.toast);
  const close = async () => {
    const a = await api();
    if (!(await a.confirm(`${reason}. Close it so Keylume can control the keyboard? Any unsaved changes in it are lost.`))) return;
    await attempt("Close conflicting app", async () => {
      const n = await a.closeConflictingApps();
      toast(n ? "Closed. Keylume will reconnect in a moment." : "Nothing to close.", "success");
    });
  };
  return (
    <div className="banner" role="alert">
      <span className="banner-icon" aria-hidden>⚠</span>
      <span>
        <b>{reason}.</b> Keylume has paused so the two apps don't talk to the keyboard at once
        (that can freeze its settings until you replug it). It resumes by itself when the other app closes.
      </span>
      <button className="btn" onClick={close}>Close it</button>
    </div>
  );
}

/** Drop profile files anywhere on the window to add them. */
function DropZone() {
  const upload = useApp((s) => s.upload);
  const [over, setOver] = useState(false);
  useEffect(() => {
    let off: (() => void) | undefined;
    let dead = false;
    api().then((a) => {
      if (dead) return;
      off = a.onFileDrop((e) => {
        setOver(e.kind === "over");
        if (e.kind === "drop" && e.paths.length) upload(e.paths);
      });
    });
    return () => { dead = true; off?.(); };
  }, [upload]);
  if (!over) return null;
  return (
    <div className="drop-zone" aria-hidden>
      <div><Icon name="upload" size={30} /><b>Drop to add to your library</b><span>Keylume profile files (.json) and design packs (.keylumepack)</span></div>
    </div>
  );
}

/** The bar across the top: back to Home, which keyboard and how it is, and the lights. */
function TopBar() {
  const { go, status, layout } = useApp();
  const state = deviceState(status);
  return (
    <header className="topbar">
      <div className="top-left">
        <button className="icon back-home" onClick={() => go("home")} aria-label="Back" title="Back to Home"><Icon name="left" size={18} /></button>
        <span className="top-device">
          <b>{layout?.name ?? "Keyboard"}</b>
          <small><span className={`dot ${state.tone}`} />{state.text}</small>
        </span>
      </div>
      <div className="top-right"><LightsButton /></div>
    </header>
  );
}

/** The rail: every place, and Settings at the foot. */
function Rail() {
  const { page, go, features } = useApp();
  const item = (p: Page, label: string, icon: IconName) => (
    <button key={p} className={`rail-item ${page === p ? "on" : ""}`} aria-current={page === p ? "page" : undefined} onClick={() => go(p)} title={label}>
      <Icon name={icon} size={20} /><span>{label}</span>
    </button>
  );
  return (
    <nav className="rail-nav" aria-label="Places">
      {PLACES.filter((p) => available(p.page, features)).map((p) => item(p.page, p.label, p.icon))}
      <span className="rail-spacer" />
      {item("settings", "Settings", "settings")}
    </nav>
  );
}

export function App() {
  const { ready, init, page, go, toasts, dismiss, status, backend, features } = useApp();
  useEffect(() => { init().catch((e) => console.error(e)); }, [init]);
  // another keyboard connected that has no use for this page
  useEffect(() => { if (!available(page, features)) go("library"); }, [page, features, go]);

  if (!ready) return <div className="boot"><span className="logo-mark big" aria-hidden><img src={logo} alt="" /></span><span className="spinner" /></div>;
  const onDevice = page !== "home";
  return (
    <div className={`app ${page === "home" ? "no-top" : ""}`}>
      {page !== "home" && <TopBar />}
      <div className={`body ${onDevice ? "with-rail" : ""}`}>
        {onDevice && <Rail />}
        <main className="main">
          {status?.paused && <ConflictBanner reason={status.paused} />}
          {page === "home" && <HomePage />}
          {page === "library" && <LibraryPage />}
          {page === "create" && <CreatePage />}
          {page === "side" && <SidePage />}
          {page === "keys" && <KeysPage />}
          {page === "settings" && <SettingsPage />}
        </main>
      </div>
      {backend === "mock" && <span className="pill warn preview-pill" title="Running in a browser against a simulated keyboard">Browser preview</span>}
      <DropZone />
      <div className="toasts" role="status" aria-live="polite">
        {toasts.map((t) => (
          <div key={t.id} className={`toast ${t.kind}`} onClick={() => dismiss(t.id)}>{t.text}</div>
        ))}
      </div>
    </div>
  );
}
