// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Field Notes — an offline harm-reduction journal & trip-sitting workstation.
//! All data stays on-device in a local SQLite database.

mod commands;
mod db;
mod interactions;

use std::sync::Mutex;
use tauri::Manager;

/// The single on-device journal connection, shared across commands.
pub struct Db(pub Mutex<rusqlite::Connection>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir().expect("no app data dir");
            let conn = db::init(&dir.join("journal.db")).expect("failed to open journal database");
            app.manage(Db(Mutex::new(conn)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::interaction_classes,
            commands::list_substances,
            commands::add_substance,
            commands::check_combo,
            commands::create_experience,
            commands::list_experiences,
            commands::get_experience,
            commands::end_experience,
            commands::log_dose,
            commands::add_timeline_event,
            commands::usage_by_substance,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Field Notes");
}
