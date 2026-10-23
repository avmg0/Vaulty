//! The tray (menu bar) icon, the quick-add shortcut and keeping Vaulty running when its window is
//! closed. These are settings of the device, kept in the registry (they work before anyone signs in).
//!
//! The shortcut and the tray's "Quick add" bring the window forward and tell it to open quick add;
//! when no one is signed in, the window shows the sign-in screen first and opens quick add after.
use crate::{poisoned, AppState};
use std::sync::Mutex;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use vaulty_core::{Error, Result};

const TRAY_ID: &str = "vaulty";

/// The shortcuts offered. Only these, so Vaulty never takes a key another app relies on.
pub const SHORTCUTS: &[&str] = &["CommandOrControl+Shift+J", "CommandOrControl+Shift+Space", "CommandOrControl+Alt+V", "CommandOrControl+Shift+Y"];

/// The tray menu's words, in the window's language (sent by the window; English until then).
pub struct TrayLabels(pub Mutex<[String; 4]>);
impl Default for TrayLabels {
    fn default() -> Self { Self(Mutex::new(["Quick add".into(), "Open Vaulty".into(), "Lock now".into(), "Quit Vaulty".into()])) }
}

pub fn show_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn quick_add<R: Runtime>(app: &AppHandle<R>) {
    show_window(app);
    let _ = app.emit("vaulty://quick-add", ());
}

fn lock<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    if let Ok(mut p) = state.pending.lock() { *p = None; }
    if let Ok(mut h) = state.helping.lock() { *h = None; }
    if let Ok(mut s) = state.session.lock() { *s = None; }
    let _ = app.emit("vaulty://locked", ());
}

fn menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let l = app.state::<TrayLabels>().0.lock().map(|l| l.clone()).unwrap_or_default();
    Menu::with_items(app, &[
        &MenuItem::with_id(app, "quick", &l[0], true, None::<&str>)?,
        &MenuItem::with_id(app, "open", &l[1], true, None::<&str>)?,
        &PredefinedMenuItem::separator(app)?,
        &MenuItem::with_id(app, "lock", &l[2], true, None::<&str>)?,
        &MenuItem::with_id(app, "quit", &l[3], true, None::<&str>)?,
    ])
}

/// Shows or removes the tray icon.
pub fn set_tray<R: Runtime>(app: &AppHandle<R>, on: bool) -> Result<()> {
    let fail = |e: tauri::Error| Error::Invalid(format!("the menu bar icon couldn't be set up ({e})"));
    if !on {
        app.remove_tray_by_id(TRAY_ID);
        return Ok(());
    }
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(menu(app).map_err(fail)?)).map_err(fail)?;
        return Ok(());
    }
    let icon = Image::from_bytes(include_bytes!("../icons/tray.png")).map_err(fail)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        // One colour on macOS, so the menu bar tints it for light and dark.
        .icon_as_template(true)
        .tooltip("Vaulty")
        .menu(&menu(app).map_err(fail)?)
        // macOS shows the menu on a click; elsewhere a click opens the window and the menu is on right-click.
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, e| match e.id().as_ref() {
            "quick" => quick_add(app),
            "open" => show_window(app),
            "lock" => lock(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                if !cfg!(target_os = "macos") { show_window(tray.app_handle()); }
            }
        })
        .build(app)
        .map_err(fail)?;
    Ok(())
}

/// Registers the quick-add shortcut ("" for none), replacing any earlier one.
pub fn set_shortcut<R: Runtime>(app: &AppHandle<R>, keys: &str) -> Result<()> {
    if !keys.is_empty() && !SHORTCUTS.contains(&keys) { return Err(Error::Invalid("choose one of the shortcuts offered".into())); }
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    if !keys.is_empty() {
        gs.register(keys).map_err(|_| Error::Invalid("another app is already using that shortcut; choose a different one".into()))?;
    }
    Ok(())
}

/// The global-shortcut plugin, opening quick add when the shortcut is pressed.
pub fn shortcut_plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| { if event.state() == ShortcutState::Pressed { quick_add(app); } })
        .build()
}

/// On start: the tray (on unless turned off) and the shortcut (off unless chosen).
pub fn apply_saved<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let (tray, keys) = state.registry(|r| Ok((r.app_setting("tray")?, r.app_setting("shortcut")?))).unwrap_or_default();
    let _ = set_tray(app, tray.as_deref() != Some("off"));
    if let Some(k) = keys.filter(|k| !k.is_empty()) { let _ = set_shortcut(app, &k); }
}

/// Whether closing the window should hide it instead of quitting (only with the tray icon showing).
pub fn keep_running<R: Runtime>(app: &AppHandle<R>) -> bool {
    let state = app.state::<AppState>();
    let on = state.registry.lock().map_err(poisoned).ok()
        .and_then(|r| r.app_setting("keep_running").ok().flatten())
        .is_some_and(|v| v == "on");
    on && app.tray_by_id(TRAY_ID).is_some()
}
