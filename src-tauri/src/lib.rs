// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Field Notes — an offline harm-reduction journal & trip-sitting workstation.
//! All data stays on-device in a local SQLite database, optionally encrypted at
//! rest with a passphrase (SQLCipher).

mod commands;
mod db;
mod interactions;
mod obsidian;
mod ollama;
mod pw;

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

/// The single on-device journal connection, shared across commands. The connection
/// is `None` while the database is encrypted and still locked (before the user has
/// entered their passphrase this session).
pub struct Db {
    pub conn: Mutex<Option<Connection>>,
    pub path: PathBuf,
}

impl Db {
    fn locked_err() -> String {
        "The journal is locked — unlock it with your passphrase.".to_string()
    }

    /// Run `f` against the open connection, or return a "locked" error if there
    /// isn't one yet.
    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Result<T, String> {
        let guard = self.conn.lock().unwrap();
        let conn = guard.as_ref().ok_or_else(Self::locked_err)?;
        f(conn).map_err(|e| e.to_string())
    }

    /// Like [`Db::with`], but for operations needing `&mut Connection` (transactions).
    pub fn with_mut<T>(&self, f: impl FnOnce(&mut Connection) -> rusqlite::Result<T>) -> Result<T, String> {
        let mut guard = self.conn.lock().unwrap();
        let conn = guard.as_mut().ok_or_else(Self::locked_err)?;
        f(conn).map_err(|e| e.to_string())
    }

    pub fn is_unlocked(&self) -> bool {
        self.conn.lock().unwrap().is_some()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let dir = app.path().app_data_dir().expect("no app data dir");
            let path = dir.join("journal.db");
            // If the journal is encrypted, leave it locked until the user unlocks
            // it with their passphrase; otherwise open it (creating on first run).
            let conn = if db::is_encrypted(&path) {
                None
            } else {
                Some(db::open(&path, None).expect("failed to open journal database"))
            };
            app.manage(Db { conn: Mutex::new(conn), path });

            // The bundled dose reference lives inside the journal DB, so it can
            // only be loaded once the DB is open (i.e. not locked).
            let db = app.state::<Db>();
            if db.is_unlocked() {
                commands::refresh_dose_reference(app.handle(), db.inner());
            }
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
            commands::update_experience,
            commands::update_dose,
            commands::delete_experience,
            commands::delete_dose,
            commands::delete_timeline_event,
            commands::delete_substance,
            commands::ai_status,
            commands::ai_recommended_models,
            commands::ai_install,
            commands::ai_start,
            commands::ai_pull,
            commands::ollama_up,
            commands::ollama_models,
            commands::companion_chat,
            commands::parse_experience,
            commands::import_experience,
            commands::pw_update,
            commands::pw_status,
            commands::pw_lookup,
            commands::db_status,
            commands::unlock_db,
            commands::enable_encryption,
            commands::disable_encryption,
            commands::change_passphrase,
            commands::export_backup,
            commands::import_backup,
            commands::obsidian_export,
            commands::obsidian_import,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Field Notes");
}
