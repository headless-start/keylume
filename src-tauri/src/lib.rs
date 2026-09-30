//! Keylume desktop app: wires keylume-core into Tauri (window, tray, hotkeys, events).

mod commands;
mod conflict;
mod inputs;
mod tray;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use keylume_core::backup::Backup;
use keylume_core::lighting::LightingState;
use keylume_core::service::{Connect, Listeners, Options, Service};
use keylume_core::Store;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub struct AppState {
    pub service: Arc<Service>,
    pub store: Arc<Mutex<Store>>,
    /// Id of the profile chosen most recently (what "lights on" and the side light go back to).
    pub current: Arc<Mutex<Option<String>>>,
    pub captures: Arc<inputs::Captures>,
    pub lights_off: AtomicBool,
    /// When the lighting last changed (for shuffle).
    pub changed_at: Mutex<Instant>,
    /// Files dropped on the window lately: the only paths `import_profiles` accepts.
    pub dropped: Mutex<Vec<(PathBuf, Instant)>>,
    /// A backup the user picked and Keylume checked, waiting for their go-ahead.
    pub staged_restore: Mutex<Option<(u64, Backup)>>,
    /// The window wants live frames until then (renewed while Home shows).
    pub frames_wanted_until: Mutex<Instant>,
    pub data_dir: PathBuf,
}

impl AppState {
    /// Draw the library for the keyboard that just connected, if it has another shape
    /// than the last one; the window reloads its layout and library on "library-changed".
    pub fn follow_keyboard(&self, app: &tauri::AppHandle) {
        let layout = self.service.layout();
        let changed = self.store.lock().unwrap().set_layout(&layout).unwrap_or(false);
        if changed {
            let _ = app.emit("library-changed", ());
            tray::refresh(app);
        }
    }
}

impl AppState {
    /// Apply a profile by id: lighting, live-input capture, "current" and settings.
    /// Returns the lighting request's id.
    pub fn apply_profile(&self, app: &tauri::AppHandle, id: &str) -> Result<u64, String> {
        let profile = self.store.lock().unwrap().get(id).cloned().ok_or_else(|| format!("no profile {id:?}"))?;
        let needs = match &profile.lighting {
            keylume_profiles::Lighting::Live { live } => live.needs(),
            _ => Default::default(),
        };
        self.captures.set_needs(needs);
        let request = self.service.apply_profile(&profile);
        self.lights_off.store(false, Ordering::Relaxed);
        *self.changed_at.lock().unwrap() = Instant::now();
        *self.current.lock().unwrap() = Some(profile.id.clone());
        let _ = self.store.lock().unwrap().update_settings(|s| s.last_profile = Some(profile.id.clone()));
        self.refresh_side();
        let _ = app.emit("current-profile", &profile.id);
        tray::refresh(app);
        Ok(request)
    }

    pub fn cycle(&self, app: &tauri::AppHandle, step: i32) {
        let next = {
            let cur = self.current.lock().unwrap().clone();
            self.store.lock().unwrap().cycle_favorite(cur.as_deref(), step)
        };
        if let Some(id) = next {
            let _ = self.apply_profile(app, &id);
        }
    }

    pub fn toggle_lights(&self, app: &tauri::AppHandle) {
        if self.lights_off.swap(true, Ordering::Relaxed) {
            self.lights_off.store(false, Ordering::Relaxed);
            let last = self.current.lock().unwrap().clone();
            if let Some(id) = last {
                let _ = self.apply_profile(app, &id);
            }
        } else {
            self.captures.set_needs(Default::default());
            self.service.lights_off();
            self.service.set_side(keylume_proto::SideLight::off());
        }
    }

    /// After a restore or a factory reset the keyboard shows what it holds itself: forget
    /// the chosen profile, so reconnecting doesn't put it back over the top.
    pub fn forget_current(&self, app: &tauri::AppHandle) {
        *self.current.lock().unwrap() = None;
        self.lights_off.store(false, Ordering::Relaxed);
        self.captures.set_needs(Default::default());
        let _ = self.store.lock().unwrap().update_settings(|s| s.last_profile = None);
        let _ = app.emit("settings-changed", ());
        tray::refresh(app);
    }

    /// Put the side strip in the right state: matching the current profile, or the
    /// user's own choice when it doesn't follow.
    pub fn refresh_side(&self) {
        let (follow, custom) = {
            let s = self.store.lock().unwrap();
            (s.settings().side_follow, s.settings().side_custom)
        };
        if !follow {
            self.service.set_side(custom);
            return;
        }
        let current = self.current.lock().unwrap().clone();
        let lighting = current.and_then(|id| self.store.lock().unwrap().get(&id).map(|p| p.lighting.clone()));
        if let Some(l) = lighting {
            self.service.set_side(keylume_profiles::side::matching(&l));
        }
    }

    /// Was `path` dropped on the window in the last couple of minutes? (Each drop
    /// allows one import.)
    pub fn take_dropped(&self, path: &std::path::Path) -> bool {
        let mut dropped = self.dropped.lock().unwrap();
        dropped.retain(|(_, at)| at.elapsed() < Duration::from_secs(120));
        match dropped.iter().position(|(p, _)| p == path) {
            Some(i) => {
                dropped.remove(i);
                true
            }
            None => false,
        }
    }
}

/// The app's state, once setup has put it in place (the device worker can report a
/// connection a moment before that).
fn managed(app: &tauri::AppHandle) -> Option<tauri::State<'_, AppState>> {
    for _ in 0..250 {
        if let Some(s) = app.try_state::<AppState>() {
            return Some(s);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

fn data_dir(app: &tauri::App) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir().join("keylume"))
}

/// Keylume never competes with games for the CPU: run below normal priority.
/// (Typing itself never passes through Keylume: keys go straight from the keyboard
/// to Windows; Keylume only sends lighting commands.)
fn lower_own_priority() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Threading::{GetCurrentProcess, SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS};
        let _ = SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
    }
}

/// Hand live frames to the window while it asks for them (Home renews the request every
/// second). The worker never waits on this thread; if the window is slow or hidden,
/// frames are skipped, never queued.
fn publish_frames(app: tauri::AppHandle, service: Arc<Service>) {
    let tap = service.frames();
    let _ = std::thread::Builder::new().name("keylume-frames".into()).spawn(move || {
        let mut last = (0, 0);
        loop {
            let Some(frame) = tap.next(last, Duration::from_millis(500)) else { continue };
            last = (frame.request, frame.seq);
            let state = app.state::<AppState>();
            let wanted = Instant::now() < *state.frames_wanted_until.lock().unwrap();
            let visible = app.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(false);
            if wanted && visible {
                let _ = app.emit("lighting-frame", &frame);
            }
        }
    });
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    let st = app.state::<AppState>();
                    if !st.store.lock().unwrap().settings().hotkeys {
                        return;
                    }
                    let mods = Modifiers::CONTROL | Modifiers::ALT;
                    if shortcut.matches(mods, Code::ArrowRight) {
                        st.cycle(app, 1);
                    } else if shortcut.matches(mods, Code::ArrowLeft) {
                        st.cycle(app, -1);
                    } else if shortcut.matches(mods, Code::KeyL) {
                        st.toggle_lights(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            lower_own_priority();
            let layout = keylume_proto::Layout::tk68();
            let dir = data_dir(app);
            let store = Store::open(&dir, &layout).map_err(|e| e.to_string())?;
            let settings = store.settings().clone();
            let connect = if settings.demo_mode { Connect::HardwareOrSimulator } else { Connect::Hardware };

            // Status events -> UI; re-apply the last profile whenever a keyboard (re)connects.
            let handle = app.handle().clone();
            let was_connected = Arc::new(AtomicBool::new(false));
            let on_status = move |st: &keylume_core::Status| {
                let _ = handle.emit("status", st);
                let now = st.connected;
                if now && !was_connected.swap(true, Ordering::SeqCst) {
                    let h = handle.clone();
                    // defer: we're on the device thread, and the app may still be starting
                    std::thread::spawn(move || {
                        let Some(state) = managed(&h) else { return };
                        state.follow_keyboard(&h);
                        let (restore, last) = {
                            let s = state.store.lock().unwrap();
                            (s.settings().restore_on_connect, s.settings().last_profile.clone())
                        };
                        match (restore, last) {
                            (true, Some(id)) => {
                                let _ = state.apply_profile(&h, &id);
                            }
                            _ => state.refresh_side(),
                        }
                    });
                } else if !now {
                    was_connected.store(false, Ordering::SeqCst);
                }
            };
            // What the keys show -> UI; the tray's check mark follows it.
            let handle = app.handle().clone();
            let shown_profile = Mutex::new(None::<String>);
            let on_lighting = move |st: &LightingState| {
                let _ = handle.emit("lighting", st);
                let now = st.shown.as_ref().and_then(|s| s.profile_id.clone());
                let mut last = shown_profile.lock().unwrap();
                if *last != now {
                    *last = now;
                    let h = handle.clone();
                    std::thread::spawn(move || tray::refresh(&h)); // off the device thread
                }
            };
            let service = Arc::new(Service::start_with(
                connect,
                Options { live_layer: settings.live_layer, true_colors: settings.true_colors, demo: settings.demo_board },
                Listeners { status: Some(Box::new(on_status)), lighting: Some(Box::new(on_lighting)) },
            ));

            let captures = Arc::new(inputs::Captures::new());
            let c = captures.clone();
            service.set_inputs_provider(Arc::new(move || c.snapshot()));

            app.manage(AppState {
                service: service.clone(),
                store: Arc::new(Mutex::new(store)),
                current: Arc::new(Mutex::new(settings.last_profile.clone())),
                captures,
                lights_off: AtomicBool::new(false),
                changed_at: Mutex::new(Instant::now()),
                dropped: Mutex::new(Vec::new()),
                staged_restore: Mutex::new(None),
                frames_wanted_until: Mutex::new(Instant::now()),
                data_dir: dir,
            });
            publish_frames(app.handle().clone(), service);

            // Shuffle: move to the next favourite every N minutes (not while the lights are off).
            let h = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(15));
                let state = h.state::<AppState>();
                let minutes = state.store.lock().unwrap().settings().shuffle_minutes;
                if minutes == 0 || state.lights_off.load(Ordering::Relaxed) || state.service.status().busy.is_some() {
                    continue;
                }
                if state.changed_at.lock().unwrap().elapsed().as_secs() >= minutes as u64 * 60 {
                    state.cycle(&h, 1);
                }
            });

            let gs = app.global_shortcut();
            for code in [Code::ArrowRight, Code::ArrowLeft, Code::KeyL] {
                let _ = gs.register(Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), code));
            }

            tray::create(app.handle())?;

            // Pause while the vendor driver runs (two programs on the config channel wedge it).
            conflict::watch(app.state::<AppState>().service.clone());

            // KEYLUME_SMOKE_TEST=1: start, report, exit. Used by CI and the Linux check.
            if std::env::var_os("KEYLUME_SMOKE_TEST").is_some() {
                let h = app.handle().clone();
                std::thread::spawn(move || {
                    let state = h.state::<AppState>();
                    for _ in 0..40 {
                        if state.service.status().connected {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(250));
                    }
                    let report = serde_json::json!({
                        "status": state.service.status(),
                        "lighting": state.service.lighting(),
                        "profiles": state.store.lock().unwrap().profiles().len(),
                        "effect": state.service.call(|b| b.effect()).ok(),
                    });
                    println!("KEYLUME_SMOKE_TEST {report}");
                    h.exit(0);
                });
            }

            let minimized = std::env::args().any(|a| a == "--minimized") || settings.start_minimized;
            if let Some(w) = app.get_webview_window("main") {
                if !minimized {
                    let _ = w.show();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Closing the window keeps Keylume running in the tray (live effects need it).
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            // Files dropped on the window are the ones the user chose to upload.
            tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) => {
                let state = window.state::<AppState>();
                let mut dropped = state.dropped.lock().unwrap();
                dropped.retain(|(_, at)| at.elapsed() < Duration::from_secs(120));
                dropped.extend(paths.iter().take(32).map(|p| (p.clone(), Instant::now())));
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::get_status,
            commands::get_lighting,
            commands::watch_lighting,
            commands::get_layout,
            commands::list_profiles,
            commands::save_profile,
            commands::delete_profile,
            commands::import_profiles,
            commands::list_packs,
            commands::remove_pack,
            commands::maker_info,
            commands::make_pack,
            commands::open_pack_page,
            commands::upload_profiles,
            commands::apply_profile,
            commands::current_profile,
            commands::apply_effect,
            commands::preview_keys,
            commands::stop_live,
            commands::toggle_lights,
            commands::get_settings,
            commands::set_settings,
            commands::get_effect,
            commands::read_layer,
            commands::write_layer,
            commands::get_keymap,
            commands::set_keymap,
            commands::get_macro,
            commands::set_macro,
            commands::get_device_settings,
            commands::set_device_settings,
            commands::factory_reset,
            commands::backup_to_file,
            commands::choose_backup,
            commands::restore_backup,
            commands::close_conflicting_apps,
            commands::get_side_light,
            commands::preview_live,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Keylume");
}
