// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Running from the menu bar (macOS) or the system tray (Windows, Linux), for a
//! computer that serves the journal and stays on.
//!
//! Opt-in (`menu_bar` in `server.json`). When it's on:
//! - a small icon sits next to the clock, with Open Field Notes and Quit;
//! - on macOS there's no Dock icon, and on Windows and Linux no taskbar button;
//! - closing the window hides it instead of quitting, so device access keeps
//!   running. Quit is in the icon's menu.
//!
//! Opened at login (the autostart plugin passes [`HIDDEN_ARG`]), it starts with
//! the window hidden.

use crate::prefs::Prefs;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};

const TRAY_ID: &str = "main";
/// Passed by the login item, so a server that starts at login doesn't put a window
/// in front of whoever just logged in.
pub const HIDDEN_ARG: &str = "--hidden";

pub fn enabled<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.state::<Prefs>().get().menu_bar
}

/// Bring the window back (from the icon, or when the option is turned off).
pub fn show_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Put the icon in (or take it out of) the menu bar / tray, and the app out of (or
/// back into) the Dock / taskbar, to match the setting.
pub fn apply<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let on = enabled(app);
    if on && app.tray_by_id(TRAY_ID).is_none() {
        build(app).map_err(|e| format!("Couldn't add the icon: {e}"))?;
    }
    if !on {
        let _ = app.remove_tray_by_id(TRAY_ID);
    }

    #[cfg(target_os = "macos")]
    {
        let policy = if on { tauri::ActivationPolicy::Accessory } else { tauri::ActivationPolicy::Regular };
        app.set_activation_policy(policy).map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "macos"))]
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_skip_taskbar(on);
    }

    // Leaving the menu bar must never strand a hidden window with no way back.
    if !on {
        show_window(app);
    }
    Ok(())
}

fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Field Notes", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Field Notes", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &quit])?;

    let builder = TrayIconBuilder::with_id(TRAY_ID).tooltip("Field Notes").menu(&menu);

    // macOS draws a template image in the menu bar's own colour (dark or light). The
    // tray on Windows and Linux can be either colour, so it gets the app's icon.
    #[cfg(target_os = "macos")]
    let builder = builder
        .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray-template.png"))?)
        .icon_as_template(true)
        .show_menu_on_left_click(true);
    #[cfg(not(target_os = "macos"))]
    let builder = match app.default_window_icon() {
        Some(i) => builder.icon(i.clone()),
        None => builder,
    }
    // A left click opens the window, the right click the menu, as tray icons do there.
    .show_menu_on_left_click(false);

    builder
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                #[cfg(not(target_os = "macos"))]
                show_window(tray.app_handle());
                #[cfg(target_os = "macos")]
                let _ = tray;
            }
        })
        .build(app)?;
    Ok(())
}
