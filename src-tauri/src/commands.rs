//! Tauri commands: the UI's only way into the app. Device calls run on a blocking
//! thread (the device worker may be waiting out a firmware settle period).
//!
//! Every command checks its own arguments: the window is trusted to be Keylume's, but a
//! command never relies on the UI having checked. No command takes a file path to write
//! to or read from on the window's say-so: backups, restores and uploads open their file
//! dialogs here, and a dropped file is accepted only if the OS reported that drop.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use keylume_core::lighting::{LightingState, LiveFrameEvent};
use keylume_core::service::{Status, Task};
use keylume_core::store::Imported;
use keylume_core::AppSettings;
use keylume_core::{backup, board};
use keylume_device::ONBOARD_PROFILES;
use keylume_profiles::{Lighting, Profile};
use keylume_proto::keymap::KeyAction;
use keylume_proto::macros::{Macro, MACRO_SLOTS};
use keylume_proto::picture::Frame;
use keylume_proto::settings::{KeyboardOptions, ReportRate, SleepTimers, MAX_DEBOUNCE};
use keylume_proto::{Effect, Layout, Rgb};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::{tray, AppState};

type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Run a closure on a blocking thread (device calls can take seconds).
async fn blocking<R: Send + 'static>(f: impl FnOnce() -> Res<R> + Send + 'static) -> Res<R> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(err)?
}

fn layer_index(layer: u8) -> Res<u8> {
    if layer < 3 {
        Ok(layer)
    } else {
        Err("picture layers are 1, 2 and 3".into())
    }
}

fn onboard_profile(profile: u8) -> Res<u8> {
    if profile < ONBOARD_PROFILES {
        Ok(profile)
    } else {
        Err(format!("the keyboard has {ONBOARD_PROFILES} onboard profiles"))
    }
}

fn macro_slot(index: u8) -> Res<u8> {
    if index < MACRO_SLOTS {
        Ok(index)
    } else {
        Err(format!("the keyboard has {MACRO_SLOTS} macro slots"))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    /// Problems found while loading settings and profiles (files kept, not lost).
    notices: Vec<String>,
}

/// Text for the window with the user's own folders written as `%APPDATA%` (Windows) or
/// `~` (Linux) instead: paths show which folder, never the account name in it.
pub fn private_path(text: &str) -> String {
    let mut out = text.to_string();
    let vars: &[(&str, &str)] = if cfg!(windows) {
        &[("LOCALAPPDATA", "%LOCALAPPDATA%"), ("APPDATA", "%APPDATA%"), ("USERPROFILE", "%USERPROFILE%")]
    } else {
        &[("HOME", "~")]
    };
    for (var, shown) in vars {
        if let Some(base) = std::env::var_os(var).map(|v| v.to_string_lossy().into_owned()).filter(|v| v.len() > 3) {
            out = out.replace(&base, shown);
        }
    }
    out
}

#[tauri::command]
pub fn app_info(app: AppHandle, state: State<AppState>) -> AppInfo {
    let store = state.store.lock().unwrap();
    AppInfo { version: app.package_info().version.to_string(), notices: store.notices().iter().map(|n| private_path(n)).collect() }
}

/// Close the vendor driver (user clicked "Close it" on the conflict banner).
#[tauri::command]
pub fn close_conflicting_apps() -> usize {
    crate::conflict::close_all()
}

#[tauri::command]
pub fn get_status(state: State<AppState>) -> Status {
    state.service.status()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightingSnapshot {
    state: LightingState,
    /// The newest live frame sent, if a live effect ran.
    frame: Option<LiveFrameEvent>,
}

/// What the keys show now, with the newest live frame (a fresh start for Home).
#[tauri::command]
pub fn get_lighting(state: State<AppState>) -> LightingSnapshot {
    LightingSnapshot { state: state.service.lighting(), frame: state.service.frames().latest() }
}

/// The window wants live frames for the next few seconds (Home renews this while it shows).
#[tauri::command]
pub fn watch_lighting(state: State<AppState>) {
    *state.frames_wanted_until.lock().unwrap() = Instant::now() + Duration::from_secs(3);
}

/// The keyboard the library is drawn for (the connected one, or the last one seen).
#[tauri::command]
pub fn get_layout(state: State<AppState>) -> Layout {
    state.store.lock().unwrap().layout().clone()
}

// ---- profiles -----------------------------------------------------------------

#[tauri::command]
/// Every profile, per-key colours packed (see `keylume_profiles::to_wire`).
pub fn list_profiles(state: State<AppState>) -> Vec<serde_json::Value> {
    let store = state.store.lock().unwrap();
    store.iter().map(|p| keylume_profiles::to_wire(p, store.layout())).collect()
}

#[tauri::command]
pub fn save_profile(app: AppHandle, state: State<AppState>, profile: Profile) -> Res<Profile> {
    let p = state.store.lock().unwrap().save(profile).map_err(err)?;
    tray::refresh(&app);
    Ok(p)
}

#[tauri::command]
pub fn delete_profile(app: AppHandle, state: State<AppState>, id: String) -> Res<()> {
    state.store.lock().unwrap().delete(&id).map_err(err)?;
    tray::refresh(&app);
    Ok(())
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Upload {
    /// Ids of the profiles added.
    added: Vec<String>,
    /// Design packs added (or updated).
    packs: Vec<keylume_core::packs::PackInfo>,
    /// Files that added nothing: (file name, why).
    failed: Vec<(String, String)>,
}

fn import_all(app: &AppHandle, state: &AppState, paths: Vec<PathBuf>) -> Upload {
    let mut out = Upload::default();
    for path in paths {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        match state.store.lock().unwrap().import_file(&path) {
            Ok(Imported::Profiles(ids)) => out.added.extend(ids),
            Ok(Imported::Pack(info)) => out.packs.push(info),
            Err(e) => out.failed.push((name, private_path(&e.to_string()))),
        }
    }
    if !out.packs.is_empty() {
        let _ = app.emit("library-changed", ());
    }
    if !out.added.is_empty() || !out.packs.is_empty() {
        tray::refresh(app);
    }
    out
}

/// The design packs added, for the Library.
#[tauri::command]
pub fn list_packs(state: State<AppState>) -> Vec<keylume_core::packs::PackInfo> {
    state.store.lock().unwrap().packs()
}

/// What the user put in the packs they made before (never their key).
#[tauri::command]
pub fn maker_info(state: State<AppState>) -> keylume_core::store::Maker {
    state.store.lock().unwrap().maker()
}

/// Make a pack of the user's own designs, signed with their maker key, and save it where
/// they choose. Returns the file's name, or None when they cancel.
#[tauri::command]
pub async fn make_pack(app: AppHandle, req: keylume_core::packs::MakeRequest) -> Res<Option<String>> {
    blocking(move || {
        let state = app.state::<AppState>();
        let (info, bytes) = state.store.lock().unwrap().make_pack(&req).map_err(err)?;
        let picked = app
            .dialog()
            .file()
            .set_title("Save your pack")
            .set_file_name(format!(
                "{}-{}.{}",
                keylume_core::packs::slug(&info.name, 40),
                keylume_core::packs::slug(&info.version, 20),
                keylume_core::packs::EXTENSION
            ))
            .add_filter("Keylume design pack", &[keylume_core::packs::EXTENSION])
            .blocking_save_file();
        let Some(path) = picked.and_then(|f| f.into_path().ok()) else { return Ok(None) };
        std::fs::write(&path, &bytes).map_err(|e| format!("couldn't save the pack ({e})"))?;
        state.store.lock().unwrap().made_pack(&req, &info).map_err(err)?;
        Ok(Some(path.file_name().unwrap_or_default().to_string_lossy().into_owned()))
    })
    .await
}

/// Open an installed pack's maker page in the browser. The window only names the pack:
/// the address comes from the pack, checked again here (https only).
#[tauri::command]
pub fn open_pack_page(app: AppHandle, state: State<AppState>, id: String) -> Res<()> {
    use tauri_plugin_opener::OpenerExt;
    let url = state
        .store
        .lock()
        .unwrap()
        .packs()
        .into_iter()
        .find(|p| p.id == id)
        .and_then(|p| p.url)
        .ok_or("that pack has no page")?;
    if !keylume_core::packs::valid_url(&url) {
        return Err("that pack's page isn't an https address".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(err)
}

/// Remove an added pack and its designs.
#[tauri::command]
pub fn remove_pack(app: AppHandle, state: State<AppState>, id: String) -> Res<()> {
    state.store.lock().unwrap().remove_pack(&id).map_err(err)?;
    let _ = app.emit("library-changed", ());
    tray::refresh(&app);
    Ok(())
}

/// Upload files dropped on the window (only paths the OS reported for a drop).
#[tauri::command]
pub fn import_profiles(app: AppHandle, state: State<AppState>, paths: Vec<String>) -> Upload {
    let (ok, refused): (Vec<PathBuf>, Vec<PathBuf>) = paths.into_iter().take(32).map(PathBuf::from).partition(|p| state.take_dropped(p));
    let mut out = import_all(&app, &state, ok);
    out.failed.extend(
        refused.into_iter().map(|p| {
            (p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), "drop the file on the window, or use Upload".into())
        }),
    );
    out
}

/// Upload: pick profile files in the system's dialog, then add them.
#[tauri::command]
pub async fn upload_profiles(app: AppHandle) -> Res<Upload> {
    blocking(move || {
        let picked = app
            .dialog()
            .file()
            .set_title("Add Keylume profiles or packs")
            .add_filter("Keylume profiles and packs", &["json", keylume_core::packs::EXTENSION])
            .blocking_pick_files();
        let paths: Vec<PathBuf> = picked.unwrap_or_default().into_iter().filter_map(|f| f.into_path().ok()).collect();
        let state = app.state::<AppState>();
        Ok(import_all(&app, &state, paths))
    })
    .await
}

/// Apply a profile; returns the lighting request's id (see `get_lighting`).
#[tauri::command]
pub fn apply_profile(app: AppHandle, state: State<AppState>, id: String) -> Res<u64> {
    state.apply_profile(&app, &id)
}

#[tauri::command]
pub fn current_profile(state: State<AppState>) -> Option<String> {
    state.current.lock().unwrap().clone()
}

/// Apply an effect from the editor without saving a profile (coalesced).
#[tauri::command]
pub fn apply_effect(state: State<AppState>, effect: Effect) -> Res<u64> {
    effect.validate().map_err(err)?;
    state.captures.set_needs(Default::default());
    let id = state.service.apply_effect(effect);
    if state.store.lock().unwrap().settings().side_follow {
        state.service.set_side(keylume_profiles::side::matching(&Lighting::Effect { effect }));
    }
    Ok(id)
}

/// Show per-key colours from the editor without saving a profile.
#[tauri::command]
pub fn preview_keys(state: State<AppState>, keys: BTreeMap<String, Rgb>, brightness: u8) -> Res<u64> {
    let lighting = Lighting::PerKey { keys, brightness };
    lighting.validate(&state.service.layout())?;
    state.captures.set_needs(Default::default());
    Ok(state.service.apply("Unsaved design", &lighting))
}

#[tauri::command]
pub fn stop_live(state: State<AppState>) {
    state.captures.set_needs(Default::default());
    state.service.stop_live();
}

#[tauri::command]
pub fn toggle_lights(app: AppHandle, state: State<AppState>) {
    state.toggle_lights(&app);
}

// ---- settings -----------------------------------------------------------------

/// The app's settings.
#[tauri::command]
pub fn get_settings(state: State<AppState>) -> AppSettings {
    state.store.lock().unwrap().settings().clone()
}

#[tauri::command]
pub fn set_settings(app: AppHandle, state: State<AppState>, settings: AppSettings) -> Res<()> {
    use tauri_plugin_autostart::ManagerExt;
    let old = state.store.lock().unwrap().settings().clone();
    // checked before anything else changes
    state.store.lock().unwrap().set_settings(settings.clone()).map_err(err)?;
    if old.launch_at_login != settings.launch_at_login {
        let al = app.autolaunch();
        let _ = if settings.launch_at_login { al.enable() } else { al.disable() };
    }
    state.service.set_live_layer(settings.live_layer);
    state.service.set_true_colors(settings.true_colors);
    tray::refresh(&app);
    if old.side_follow != settings.side_follow || old.side_custom != settings.side_custom {
        state.refresh_side();
    }
    Ok(())
}

/// A few seconds of a live effect, rendered by the real animator with stand-in inputs
/// (hover previews in the library).
#[tauri::command]
pub fn preview_live(live: keylume_live::LiveEffect, seconds: f32) -> Res<Vec<keylume_live::LiveFrame>> {
    live.validate()?;
    let seconds = if seconds.is_finite() { seconds.clamp(0.5, 12.0) } else { 8.0 };
    Ok(keylume_live::preview(live, seconds, 20.0))
}

/// What the side strip is showing right now (read from the keyboard).
#[tauri::command]
pub async fn get_side_light(state: State<'_, AppState>) -> Res<keylume_proto::SideLight> {
    let svc = state.service.clone();
    blocking(move || svc.call(|b| b.side_light()).map_err(err)).await
}

// ---- lighting on the device -----------------------------------------------------

#[tauri::command]
pub async fn get_effect(state: State<'_, AppState>) -> Res<Effect> {
    let svc = state.service.clone();
    blocking(move || svc.call(|b| b.effect()).map_err(err)).await
}

fn to_keys(layout: &Layout, f: &Frame) -> BTreeMap<String, Rgb> {
    layout.keys.iter().map(|k| (k.id.clone(), f.0[k.slot])).collect()
}

/// Read picture layer `layer` (0-based) as key id -> colour.
#[tauri::command]
pub async fn read_layer(state: State<'_, AppState>, layer: u8) -> Res<BTreeMap<String, Rgb>> {
    let layer = layer_index(layer)?;
    let svc = state.service.clone();
    let layout = svc.layout().clone();
    blocking(move || {
        let f = svc.read_layer(layer).map_err(err)?;
        Ok(to_keys(&layout, &f))
    })
    .await
}

#[tauri::command]
pub async fn write_layer(state: State<'_, AppState>, layer: u8, keys: BTreeMap<String, Rgb>, brightness: u8) -> Res<()> {
    let layer = layer_index(layer)?;
    let svc = state.service.clone();
    let lighting = Lighting::PerKey { keys, brightness };
    let layout = svc.layout();
    lighting.validate(&layout)?;
    let Lighting::PerKey { keys, .. } = lighting else { unreachable!() };
    let frame = layout.frame(|k| keys.get(&k.id).copied());
    blocking(move || svc.write_layer(layer, frame, brightness).map_err(err)).await
}

// ---- keys & macros ----------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyLayer {
    Base,
    Fn,
}

/// Key map as key id -> action (only keys that exist on the board).
#[tauri::command]
pub async fn get_keymap(state: State<'_, AppState>, layer: KeyLayer, profile: u8) -> Res<BTreeMap<String, KeyAction>> {
    let profile = onboard_profile(profile)?;
    let svc = state.service.clone();
    let layout = svc.layout().clone();
    blocking(move || {
        let km = svc
            .call(move |b| match layer {
                KeyLayer::Base => b.keymap(profile),
                KeyLayer::Fn => b.fn_keymap(0),
            })
            .map_err(err)?;
        Ok(layout.keys.iter().map(|k| (k.id.clone(), km.0[k.slot])).collect())
    })
    .await
}

/// Update only the given keys; everything else on the layer is preserved.
#[tauri::command]
pub async fn set_keymap(state: State<'_, AppState>, layer: KeyLayer, profile: u8, keys: BTreeMap<String, KeyAction>) -> Res<()> {
    let profile = onboard_profile(profile)?;
    let svc = state.service.clone();
    let layout = svc.layout().clone();
    let mut slots = Vec::with_capacity(keys.len());
    for (id, action) in &keys {
        let k = layout.key(id).ok_or_else(|| format!("the keyboard has no key {id:?}"))?;
        if !action.is_valid(ONBOARD_PROFILES) {
            return Err(format!("{id}: that macro or onboard profile doesn't exist"));
        }
        slots.push((k.slot, *action));
    }
    let lost = blocking(move || svc.call(move |b| board::write_keys(b, layer == KeyLayer::Fn, profile, &slots)).map_err(err)).await?;
    if lost.is_empty() {
        return Ok(());
    }
    let names: Vec<&str> = layout
        .keys
        .iter()
        .filter(|k| lost.contains(&k.slot))
        .map(|k| if k.label.is_empty() { k.id.as_str() } else { k.label.as_str() })
        .collect();
    Err(format!("The keyboard didn't keep the change to {}: it still has what it had there", names.join(", ")))
}

#[tauri::command]
pub async fn get_macro(state: State<'_, AppState>, index: u8) -> Res<Macro> {
    let index = macro_slot(index)?;
    let svc = state.service.clone();
    blocking(move || svc.call(move |b| b.macro_at(index)).map_err(err)).await
}

#[tauri::command]
pub async fn set_macro(state: State<'_, AppState>, index: u8, value: Macro) -> Res<()> {
    let index = macro_slot(index)?;
    value.to_bytes().map_err(err)?; // validate size before touching the device
    let svc = state.service.clone();
    blocking(move || svc.call(move |b| b.set_macro(index, &value)).map_err(err)).await
}

// ---- device settings --------------------------------------------------------------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSettings {
    report_rate: u16,
    debounce: u8,
    sleep: SleepTimers,
    options: KeyboardOptions,
    profile: u8,
    firmware: u16,
}

#[tauri::command]
pub async fn get_device_settings(state: State<'_, AppState>) -> Res<DeviceSettings> {
    let svc = state.service.clone();
    blocking(move || {
        svc.call(|b| {
            Ok(DeviceSettings {
                report_rate: b.report_rate()?.hz(),
                debounce: b.debounce()?,
                sleep: b.sleep_timers()?,
                options: b.options()?,
                profile: b.profile()?,
                firmware: b.firmware_version()?,
            })
        })
        .map_err(err)
    })
    .await
}

/// Write only the settings that changed.
#[tauri::command]
pub async fn set_device_settings(state: State<'_, AppState>, settings: DeviceSettings) -> Res<()> {
    let rate = ReportRate::from_hz(settings.report_rate).ok_or("report rate must be 125, 250, 500 or 1000 Hz")?;
    let p = onboard_profile(settings.profile)?;
    if settings.debounce > MAX_DEBOUNCE {
        return Err(format!("debounce goes up to {MAX_DEBOUNCE}"));
    }
    let svc = state.service.clone();
    blocking(move || {
        svc.call(move |b| {
            if b.profile()? != p {
                b.set_profile(p)?;
            }
            if b.report_rate()? != rate {
                b.set_report_rate(p, rate)?;
            }
            if b.debounce()? != settings.debounce {
                b.set_debounce(settings.debounce)?;
            }
            if b.sleep_timers()? != settings.sleep {
                b.set_sleep_timers(settings.sleep)?;
            }
            if b.options()? != settings.options {
                b.set_options(p, settings.options)?;
            }
            Ok(())
        })
        .map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn factory_reset(app: AppHandle) -> Res<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        state.service.maintain(Task::Reset, |b| b.factory_reset()).map_err(err)?.map_err(err)?;
        state.forget_current(&app);
        Ok(())
    })
    .await
}

/// Back up everything on the keyboard to a file the user picks. Returns its name, or
/// None when they cancel.
#[tauri::command]
pub async fn backup_to_file(app: AppHandle) -> Res<Option<String>> {
    blocking(move || {
        // named after the keyboard it's of (a board file's id), and the day
        let board = app.state::<AppState>().service.status().device.map(|d| d.board).unwrap_or_else(|| "keyboard".into());
        let name = format!("{board}-backup-{}.json", keylume_core::store::stamp_date());
        let picked = app
            .dialog()
            .file()
            .set_title("Back up the keyboard")
            .set_file_name(name)
            .add_filter("Keylume backup", &["json"])
            .blocking_save_file();
        let Some(path) = picked.and_then(|f| f.into_path().ok()) else { return Ok(None) };
        let state = app.state::<AppState>();
        let snap = state.service.maintain(Task::Backup, |b| backup::snapshot(b)).map_err(err)?.map_err(err)?;
        backup::write_file(&path, &snap).map_err(|e| format!("couldn't write the backup file ({e})"))?;
        Ok(Some(path.file_name().unwrap_or_default().to_string_lossy().into_owned()))
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedBackup {
    /// Pass back to `restore_backup` to go ahead.
    token: u64,
    file: String,
    firmware: String,
    /// Macros that have something in them.
    macros: usize,
    side_light: bool,
}

static NEXT_STAGE: AtomicU64 = AtomicU64::new(1);

/// Restore, step 1: pick a backup file. It's read (at most a few MB) and checked in full,
/// and against the keyboard when one is connected; nothing is written yet. None when the
/// user cancels.
#[tauri::command]
pub async fn choose_backup(app: AppHandle) -> Res<Option<StagedBackup>> {
    blocking(move || {
        let picked = app.dialog().file().set_title("Restore a backup").add_filter("Keylume backup", &["json"]).blocking_pick_file();
        let Some(path) = picked.and_then(|f| f.into_path().ok()) else { return Ok(None) };
        let file = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let b = backup::read_file(&path).map_err(|why| format!("{file} can't be restored: {why}."))?;
        let state = app.state::<AppState>();
        let status = state.service.status();
        if let (Some(device), Some(fw)) = (&status.device, status.firmware) {
            b.check_device(device, fw).map_err(|why| format!("{file} can't be restored: {why}."))?;
        }
        let token = NEXT_STAGE.fetch_add(1, Ordering::SeqCst);
        let staged = StagedBackup {
            token,
            file,
            firmware: backup::firmware_label(b.firmware),
            macros: b.macros.iter().filter(|m| !m.events.is_empty()).count(),
            side_light: b.side_light.is_some(),
        };
        *state.staged_restore.lock().unwrap() = Some((token, b));
        Ok(Some(staged))
    })
    .await
}

/// Restore, step 2 (after the user confirms): save what the keyboard holds now beside the
/// app's data, then write the backup. Returns where the safety copy went.
#[tauri::command]
pub async fn restore_backup(app: AppHandle, token: u64) -> Res<String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let staged = state.staged_restore.lock().unwrap().take();
        let b = match staged {
            Some((t, b)) if t == token => b,
            _ => return Err("pick the backup again".into()),
        };
        let dir = state.data_dir.join("backups");
        let safety = dir.join(format!("before-restore-{}.json", keylume_core::store::stamp()));
        let path = safety.clone();
        let result = state
            .service
            .maintain(Task::Restore, move |kb| {
                // everything is checked before the first write, including this safety copy
                backup::check(kb, &b).map_err(backup::RestoreError::Refused)?;
                let before =
                    backup::snapshot(kb).map_err(|e| backup::RestoreError::Refused(format!("couldn't read what the keyboard holds now ({e})")))?;
                std::fs::create_dir_all(&dir)
                    .and_then(|_| backup::write_file(&path, &before))
                    .map_err(|e| backup::RestoreError::Refused(format!("couldn't save what the keyboard holds now ({e})")))?;
                backup::restore(kb, &b)
            })
            .map_err(err)?;
        match result {
            Ok(()) => {
                state.forget_current(&app);
                let _ = app.emit("profiles-changed", ());
                Ok(private_path(&safety.display().to_string()))
            }
            Err(e @ backup::RestoreError::Refused(_)) => Err(e.to_string()),
            Err(e) => {
                Err(private_path(&format!("{e} What the keyboard held before is saved in {}: restore that file to go back.", safety.display())))
            }
        }
    })
    .await
}
