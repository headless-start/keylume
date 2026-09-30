import { create } from "zustand";
import { api } from "./api";
import type { Filter } from "./library";
import { setLatestFrame } from "./frames";
import { ALL_FEATURES } from "./features";
import type { AppSettings, Features, Layout, LightingState, PackInfo, Profile, Status, Upload } from "./types";

/** Home (the keyboard as it is), the keyboard's pages, and the app's Settings. */
export type Page = "home" | "library" | "create" | "side" | "keys" | "settings";

export interface Toast { id: number; kind: "info" | "error" | "success"; text: string }

/** What the Library was showing, restored when you come back to it. */
export interface LibraryView {
  filter: Filter;
  limit: number;
  scroll: number;
  /** The profile whose design is open in the side panel. */
  selected: string | null;
}
export const LIBRARY_START: LibraryView = {
  filter: { scope: "all", kind: "any", section: "", category: "", query: "", hue: "" }, limit: 60, scroll: 0, selected: null,
};

interface AppState {
  ready: boolean;
  backend: "tauri" | "mock" | null;
  page: Page;
  layout: Layout | null;
  profiles: Profile[];
  status: Status | null;
  /** What the keyboard can do (the last one seen while none is connected). */
  features: Features;
  /** Design packs added. */
  packs: PackInfo[];
  settings: AppSettings | null;
  /** The profile chosen most recently (what the tray and "lights on" go back to). */
  current: string | null;
  /** What the keyboard shows, and the newest request: the only source for "On keyboard". */
  lighting: LightingState | null;
  toasts: Toast[];
  /** Profile the editor should open with (set by "Edit" in the library). */
  editing: Profile | null;
  library: LibraryView;

  init(): Promise<void>;
  go(page: Page): void;
  toast(text: string, kind?: Toast["kind"]): void;
  dismiss(id: number): void;
  reloadProfiles(): Promise<void>;
  apply(id: string): Promise<void>;
  saveSettings(s: AppSettings): Promise<void>;
  toggleFavorite(id: string): Promise<void>;
  edit(p: Profile | null): void;
  setLibrary(v: Partial<LibraryView>): void;
  /** Upload files dropped on the window, then show them under "Mine". */
  upload(paths: string[]): Promise<void>;
  /** Upload: pick files in the system's dialog. */
  uploadDialog(): Promise<void>;
  /** Take a lighting state, unless a newer one is already here. */
  setLighting(s: LightingState): void;
  /** Ask the backend for a fresh lighting state and live frame (Home, on opening). */
  refreshLighting(): Promise<void>;
}

let toastId = 0;

export const useApp = create<AppState>((set, get) => ({
  ready: false,
  backend: null,
  page: "home",
  layout: null,
  profiles: [],
  status: null,
  features: ALL_FEATURES,
  packs: [],
  settings: null,
  current: null,
  lighting: null,
  toasts: [],
  editing: null,
  library: LIBRARY_START,

  async init() {
    const a = await api();
    a.onStatus((s) => set(s.device ? { status: s, features: s.device.features } : { status: s }));
    a.onLibraryChanged(async () => {
      const [layout, profiles, packs] = await Promise.all([a.getLayout(), a.listProfiles(), a.listPacks()]);
      set({ layout, profiles, packs });
    });
    a.onLighting((s) => get().setLighting(s));
    a.onCurrentProfile((id) => set({ current: id }));
    a.onProfilesChanged(() => { get().reloadProfiles(); });
    a.onSettingsChanged(async () => set({ settings: await a.getSettings(), current: await a.currentProfile() }));
    const [layout, profiles, status, settings, current, lighting, info, packs] = await Promise.all([
      a.getLayout(), a.listProfiles(), a.getStatus(), a.getSettings(), a.currentProfile(), a.getLighting(), a.appInfo(), a.listPacks(),
    ]);
    setLatestFrame(lighting.frame);
    get().setLighting(lighting.state);
    set({ ready: true, backend: a.kind, layout, profiles, status, settings, current, packs, ...(status.device ? { features: status.device.features } : {}) });
    // files kept aside while loading: say so once, and where they are
    for (const n of info.notices) get().toast(n, "error");
  },

  setLighting(s) {
    const now = get().lighting;
    if (!now || s.seq >= now.seq) set({ lighting: s });
  },

  async refreshLighting() {
    try {
      const snap = await (await api()).getLighting();
      setLatestFrame(snap.frame);
      get().setLighting(snap.state);
    } catch { /* the next event brings it */ }
  },

  go: (page) => set({ page }),

  toast(text, kind = "info") {
    const id = ++toastId;
    set({ toasts: [...get().toasts, { id, kind, text }] });
    setTimeout(() => get().dismiss(id), kind === "error" ? 7000 : 3200);
  },

  dismiss: (id) => set({ toasts: get().toasts.filter((t) => t.id !== id) }),

  async reloadProfiles() {
    set({ profiles: await (await api()).listProfiles() });
  },

  async apply(id) {
    // "On keyboard" follows the lighting state: set only once the keyboard took it
    const busy = get().status?.busy;
    if (busy) return get().toast(`Wait a moment: the keyboard is busy (${busy})`, "info");
    try {
      await (await api()).applyProfile(id);
      set({ current: id });
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async saveSettings(s) {
    set({ settings: s });
    try {
      await (await api()).setSettings(s);
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async toggleFavorite(id) {
    const s = get().settings;
    if (!s) return;
    const favorites = s.favorites.includes(id) ? s.favorites.filter((f) => f !== id) : [...s.favorites, id];
    await get().saveSettings({ ...s, favorites });
  },

  edit(p) {
    set(p ? { editing: p, page: "create" } : { editing: null });
  },

  setLibrary: (v) => set({ library: { ...get().library, ...v } }),

  async upload(paths) {
    await attempt("Upload", async () => uploaded((await api()).importProfiles(paths)));
  },

  async uploadDialog() {
    await attempt("Upload", async () => uploaded((await api()).uploadProfiles()));
  },
}));

/** How one of our lighting requests went, from the lighting state (null: not known, or
 * a newer request took over before it showed). */
export function outcome(l: LightingState | null, id: number | null): "applying" | "shown" | "failed" | null {
  if (!l || id == null) return null;
  const reachable = l.phase !== "disconnected" && l.phase !== "paused";
  if (l.shown?.request === id && reachable) return "shown";
  if (l.request?.id !== id) return null;
  if (l.phase === "requested" || l.phase === "pending") return "applying";
  return l.phase === "failed" || !reachable ? "failed" : null;
}

/** Report an upload: each file that added nothing, then what was added (a pack opens
 * in its section, profiles under Mine). */
async function uploaded(result: Promise<Upload>) {
  const { added, packs, failed } = await result;
  const { toast, reloadProfiles } = useApp.getState();
  for (const [file, why] of failed) toast(`${file}: ${why}`, "error");
  if (packs.length) {
    const a = await api();
    const [profiles, all, layout] = await Promise.all([a.listProfiles(), a.listPacks(), a.getLayout()]);
    const first = packs[0];
    // open the pack's first collection, in the section it sits in (Themes, Games or Comics)
    const category = first.collections[0] ?? "";
    const section = profiles.find((p) => p.source === "builtin" && p.category === category)?.section ?? "Themes";
    useApp.setState({ profiles, packs: all, layout, page: "library", library: { ...LIBRARY_START, filter: { ...LIBRARY_START.filter, section, category } } });
    for (const p of packs) {
      const n = p.themes + p.designs;
      toast(`Added the ${p.name} pack: ${n} ${n === 1 ? "design" : "designs"}${p.official ? "" : " (community)"}`, "success");
    }
  }
  if (!added.length) return;
  await reloadProfiles();
  useApp.setState({ page: "library", library: { ...LIBRARY_START, filter: { ...LIBRARY_START.filter, scope: "mine" } } });
  toast(`Added ${added.length} profile${added.length === 1 ? "" : "s"} to Mine`, "success");
}

/** Run an async action with error toasts. Returns true on success. */
export async function attempt(what: string, fn: () => Promise<unknown>): Promise<boolean> {
  try {
    await fn();
    return true;
  } catch (e) {
    useApp.getState().toast(`${what}: ${e instanceof Error ? e.message : String(e)}`, "error");
    return false;
  }
}
