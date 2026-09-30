import type { AppSettings, DeviceSettings, Effect, Hex, KeyAction, KeyLayer, Layout, LightingSnapshot, LightingState, LiveEffect, LiveFrame, LiveFrameEvent, Macro, MakeRequest, Maker, PackInfo, Profile, SideLight, StagedBackup, Status, Upload } from "./types";
import { unpack } from "./wire";

export interface FileDrop { kind: "over" | "leave" | "drop"; paths: string[] }

/** Everything the UI can ask of the backend. Implemented by Tauri IPC and by a mock. */
export interface KeylumeApi {
  readonly kind: "tauri" | "mock";
  /** Version, data folder, and problems found while loading (files kept, not lost). */
  appInfo(): Promise<{ version: string; notices: string[] }>;
  getStatus(): Promise<Status>;
  onStatus(cb: (s: Status) => void): () => void;
  onCurrentProfile(cb: (id: string) => void): () => void;
  getLayout(): Promise<Layout>;

  /** What the keys show now, and the newest live frame. */
  getLighting(): Promise<LightingSnapshot>;
  onLighting(cb: (s: LightingState) => void): () => void;
  /** Live frames as the keyboard got them, while `watchLighting` is renewed. */
  onLightingFrame(cb: (f: LiveFrameEvent) => void): () => void;
  /** Ask for live frames for the next few seconds (renew about every second). */
  watchLighting(): Promise<void>;

  listProfiles(): Promise<Profile[]>;
  saveProfile(p: Profile): Promise<Profile>;
  deleteProfile(id: string): Promise<void>;
  /** Upload files dropped on the window (the app accepts only paths the OS reported for a drop). */
  importProfiles(paths: string[]): Promise<Upload>;
  /** Upload: pick profile files in the system's dialog and add them. */
  uploadProfiles(): Promise<Upload>;
  /** Design packs added (`.keylumepack` files, added through the same uploads). */
  listPacks(): Promise<PackInfo[]>;
  removePack(id: string): Promise<void>;
  /** What you put in the packs you made before. */
  makerInfo(): Promise<Maker>;
  /** Make a pack of your own designs, signed as yours, and save it where you choose: the file's name, or null if cancelled. */
  makePack(req: MakeRequest): Promise<string | null>;
  /** Open an installed pack's maker page in the browser. */
  openPackPage(id: string): Promise<void>;
  /** Files dragged over or dropped on the window. */
  onFileDrop(cb: (e: FileDrop) => void): () => void;
  /** Returns the lighting request's id. */
  applyProfile(id: string): Promise<number>;
  currentProfile(): Promise<string | null>;

  applyEffect(e: Effect): Promise<number>;
  previewKeys(keys: Record<string, Hex>, brightness: number): Promise<number>;
  stopLive(): Promise<void>;
  toggleLights(): Promise<void>;
  /** What the side light strip is showing (read from the keyboard). */
  getSideLight(): Promise<SideLight>;
  /** A few seconds of a live effect at 20 fps (hover previews). */
  previewLive(live: LiveEffect, seconds: number): Promise<LiveFrame[]>;
  /** The library changed on the backend. */
  onProfilesChanged(cb: () => void): () => void;
  /** A keyboard of another shape connected: the layout and library were redrawn for it. */
  onLibraryChanged(cb: () => void): () => void;
  /** Settings changed outside the window (tray). */
  onSettingsChanged(cb: () => void): () => void;

  getSettings(): Promise<AppSettings>;
  setSettings(s: AppSettings): Promise<void>;

  getEffect(): Promise<Effect>;
  readLayer(layer: number): Promise<Record<string, Hex>>;
  writeLayer(layer: number, keys: Record<string, Hex>, brightness: number): Promise<void>;

  getKeymap(layer: KeyLayer, profile: number): Promise<Record<string, KeyAction>>;
  setKeymap(layer: KeyLayer, profile: number, keys: Record<string, KeyAction>): Promise<void>;
  getMacro(index: number): Promise<Macro>;
  setMacro(index: number, value: Macro): Promise<void>;

  getDeviceSettings(): Promise<DeviceSettings>;
  setDeviceSettings(s: DeviceSettings): Promise<void>;
  factoryReset(): Promise<void>;
  /** Close the vendor driver when it conflicts. Returns how many processes were closed. */
  closeConflictingApps(): Promise<number>;
  /** Back up to a file picked in the system's dialog. Its name, or null if cancelled. */
  backupToFile(): Promise<string | null>;
  /** Restore, step 1: pick a backup; it's checked in full, nothing is written. Null if cancelled. */
  chooseBackup(): Promise<StagedBackup | null>;
  /** Restore, step 2: write it. Returns where what the keyboard held before was saved. */
  restoreBackup(token: number): Promise<string>;

  confirm(message: string): Promise<boolean>;
}

// ---- Tauri -----------------------------------------------------------------------

async function tauriApi(): Promise<KeylumeApi> {
  const { invoke } = await import("@tauri-apps/api/core");
  const { listen } = await import("@tauri-apps/api/event");
  const dialog = await import("@tauri-apps/plugin-dialog");
  const sub = <T,>(event: string, cb: (v: T) => void) => {
    let un: (() => void) | undefined;
    let dead = false;
    listen<T>(event, (e) => cb(e.payload)).then((u) => (dead ? u() : (un = u)));
    return () => { dead = true; un?.(); };
  };
  return {
    kind: "tauri",
    appInfo: () => invoke("app_info"),
    getStatus: () => invoke("get_status"),
    onStatus: (cb) => sub("status", cb),
    onCurrentProfile: (cb) => sub("current-profile", cb),
    getLayout: () => invoke("get_layout"),
    getLighting: () => invoke("get_lighting"),
    onLighting: (cb) => sub("lighting", cb),
    onLightingFrame: (cb) => sub("lighting-frame", cb),
    watchLighting: () => invoke("watch_lighting"),
    listProfiles: async () => {
      const [layout, list] = await Promise.all([invoke<Layout>("get_layout"), invoke<Profile[]>("list_profiles")]);
      return list.map((p) => unpack(p, layout));
    },
    saveProfile: (profile) => invoke("save_profile", { profile }),
    deleteProfile: (id) => invoke("delete_profile", { id }),
    importProfiles: (paths) => invoke("import_profiles", { paths }),
    uploadProfiles: () => invoke("upload_profiles"),
    listPacks: () => invoke("list_packs"),
    removePack: (id) => invoke("remove_pack", { id }),
    makerInfo: () => invoke("maker_info"),
    makePack: (req) => invoke("make_pack", { req }),
    openPackPage: (id) => invoke("open_pack_page", { id }),
    onFileDrop: (cb) => {
      let un: (() => void) | undefined;
      let dead = false;
      import("@tauri-apps/api/webview")
        .then(({ getCurrentWebview }) => getCurrentWebview().onDragDropEvent(({ payload: p }) => {
          if (p.type === "drop") cb({ kind: "drop", paths: p.paths });
          else cb({ kind: p.type === "leave" ? "leave" : "over", paths: [] });
        }))
        .then((u) => (dead ? u() : (un = u)));
      return () => { dead = true; un?.(); };
    },
    applyProfile: (id) => invoke("apply_profile", { id }),
    currentProfile: () => invoke("current_profile"),
    applyEffect: (effect) => invoke("apply_effect", { effect }),
    previewKeys: (keys, brightness) => invoke("preview_keys", { keys, brightness }),
    stopLive: () => invoke("stop_live"),
    toggleLights: () => invoke("toggle_lights"),
    getSideLight: () => invoke("get_side_light"),
    previewLive: (live, seconds) => invoke("preview_live", { live, seconds }),
    onProfilesChanged: (cb) => sub("profiles-changed", cb),
    onLibraryChanged: (cb) => sub("library-changed", cb),
    onSettingsChanged: (cb) => sub("settings-changed", cb),
    getSettings: () => invoke("get_settings"),
    setSettings: (settings) => invoke("set_settings", { settings }),
    getEffect: () => invoke("get_effect"),
    readLayer: (layer) => invoke("read_layer", { layer }),
    writeLayer: (layer, keys, brightness) => invoke("write_layer", { layer, keys, brightness }),
    getKeymap: (layer, profile) => invoke("get_keymap", { layer, profile }),
    setKeymap: (layer, profile, keys) => invoke("set_keymap", { layer, profile, keys }),
    getMacro: (index) => invoke("get_macro", { index }),
    setMacro: (index, value) => invoke("set_macro", { index, value }),
    getDeviceSettings: () => invoke("get_device_settings"),
    setDeviceSettings: (settings) => invoke("set_device_settings", { settings }),
    factoryReset: () => invoke("factory_reset"),
    closeConflictingApps: () => invoke("close_conflicting_apps"),
    backupToFile: () => invoke("backup_to_file"),
    chooseBackup: () => invoke("choose_backup"),
    restoreBackup: (token) => invoke("restore_backup", { token }),
    confirm: (message) => dialog.ask(message, { title: "Keylume", kind: "warning" }),
  };
}

let instance: Promise<KeylumeApi> | undefined;

/** The backend: Tauri inside the app, the in-browser mock everywhere else. */
export function api(): Promise<KeylumeApi> {
  if (!instance) {
    const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    instance = inTauri ? tauriApi() : import("./mock").then((m) => m.createMockApi());
  }
  return instance;
}

/** For tests: inject a backend. */
export function setApi(a: KeylumeApi) {
  instance = Promise.resolve(a);
}
