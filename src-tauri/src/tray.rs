//! System tray: favourites one click away, plus next/previous, lights off and quit.

use keylume_proto::{Rgb, SideLight, SideMode};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::AppState;

pub const TRAY_ID: &str = "keylume";

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let state = app.state::<AppState>();
    // the check mark goes to what the keyboard really shows
    let current = state.service.lighting().shown.and_then(|s| s.profile_id);
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(app, "open", "Open Keylume", true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    {
        let store = state.store.lock().unwrap();
        for id in &store.settings().favorites {
            if let Some(p) = store.get(id) {
                let checked = current.as_deref() == Some(id.as_str());
                menu.append(&CheckMenuItem::with_id(app, format!("fav:{id}"), &p.name, true, checked, None::<&str>)?)?;
            }
        }
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "next", "Next favourite\tCtrl+Alt+→", true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "prev", "Previous favourite\tCtrl+Alt+←", true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "lights", "Lights on / off\tCtrl+Alt+L", true, None::<&str>)?)?;
    {
        let s = state.store.lock().unwrap();
        let follow = s.settings().side_follow;
        let custom = s.settings().side_custom;
        let side = Submenu::with_id(app, "side", "Side light", true)?;
        side.append(&CheckMenuItem::with_id(app, "side:follow", "Match my lighting", true, follow, None::<&str>)?)?;
        side.append(&PredefinedMenuItem::separator(app)?)?;
        for (i, (name, preset)) in side_presets().iter().enumerate() {
            let on = !follow && custom == *preset;
            side.append(&CheckMenuItem::with_id(app, format!("side:{i}"), *name, true, on, None::<&str>)?)?;
        }
        menu.append(&side)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", "Quit Keylume", true, None::<&str>)?)?;
    Ok(menu)
}

/// Quick side-light looks for the tray (the Side light tab has full control).
fn side_presets() -> Vec<(&'static str, SideLight)> {
    let royal = Rgb(0x1F, 0x45, 0xFF);
    vec![
        ("Royal breathe", SideLight::new(SideMode::Breathing, royal)),
        ("Royal solid", SideLight::new(SideMode::Static, royal)),
        ("Electric wave", SideLight { speed: 3, ..SideLight::new(SideMode::Wave, Rgb(0x00, 0xC8, 0xFF)) }),
        ("Rainbow cycle", SideLight::new(SideMode::Neon, Rgb(255, 255, 255))),
        ("Gold", SideLight::new(SideMode::Static, Rgb(0xFF, 0xB0, 0x00))),
        ("Off", SideLight { brightness: 0, ..SideLight::new(SideMode::Off, Rgb(0, 0, 0)) }),
    ]
}

/// None: toggle "Match my lighting"; Some: a fixed look.
fn set_side(app: &AppHandle, preset: Option<SideLight>) {
    let state = app.state::<AppState>();
    {
        let mut store = state.store.lock().unwrap();
        let _ = store.update_settings(|s| match preset {
            None => s.side_follow = !s.side_follow,
            Some(p) => {
                s.side_follow = false;
                s.side_custom = p;
            }
        });
    }
    state.refresh_side();
    refresh(app);
    let _ = app.emit("settings-changed", ());
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Keylume")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref().to_string();
            let state = app.state::<AppState>();
            match id.as_str() {
                "open" => show_main(app),
                "next" => state.cycle(app, 1),
                "prev" => state.cycle(app, -1),
                "lights" => state.toggle_lights(app),
                "quit" => {
                    state.service.stop_live();
                    app.exit(0);
                }
                "side:follow" => set_side(app, None),
                other => {
                    if let Some(pid) = other.strip_prefix("fav:") {
                        let _ = state.apply_profile(app, pid);
                    } else if let Some(i) = other.strip_prefix("side:").and_then(|i| i.parse::<usize>().ok()) {
                        if let Some((_, preset)) = side_presets().get(i) {
                            set_side(app, Some(*preset));
                        }
                    }
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Rebuild the menu (favourites or the current profile changed).
pub fn refresh(app: &AppHandle) {
    if app.try_state::<AppState>().is_none() {
        return; // still starting: the tray is built with the state
    }
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}
