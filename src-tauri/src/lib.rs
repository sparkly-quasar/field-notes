// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Field Notes — an offline harm-reduction journal & trip-sitting workstation.
//! All data stays on-device in a local SQLite database, optionally encrypted at
//! rest with a passphrase (SQLCipher).

// These are `pub` so the offline evaluation harness (`examples/companion_eval.rs`)
// can drive the Companion exactly as the app does, without a Tauri runtime. The
// crate is `publish = false`; this is an internal seam, not a supported API.
pub mod commands;
pub mod compute;
mod contribute;
pub mod crisis;
pub mod db;
mod devices;
mod interactions;
mod keychain;
pub mod knowledge;
mod obsidian;
pub mod ollama;
mod portal;
mod prefs;
pub mod pw;
mod remote;
mod server_update;
mod stats;

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
    /// Still opening at launch (see `run`'s setup). Commands answer "locked" until
    /// it's done; the window waits on `db_status`.
    pub opening: std::sync::atomic::AtomicBool,
    /// Why opening an unencrypted journal failed, shown instead of freezing.
    pub open_error: Mutex<Option<String>>,
}

impl Db {
    /// A journal that's already open (or locked), with nothing still in progress.
    pub fn new(conn: Option<Connection>, path: PathBuf) -> Self {
        Db { conn: Mutex::new(conn), path, opening: false.into(), open_error: Mutex::new(None) }
    }

    pub fn is_opening(&self) -> bool {
        self.opening.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn locked_err() -> String {
        "The journal is locked — unlock it with your password.".to_string()
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

/// The bundled DoseWiki prose corpus, indexed for offline BM25 search.
///
/// Deliberately independent of [`Db`]: it's public CC0 reference data, not user
/// data, so it stays searchable while the journal is locked. `None` if the corpus
/// resource is missing or malformed — searching then returns no hits, which
/// callers already have to handle (see `knowledge.rs`).
pub struct Knowledge(pub Option<knowledge::Index>);

impl Knowledge {
    pub fn search(&self, query: &str, limit: usize) -> Vec<knowledge::Hit> {
        self.0.as_ref().map(|i| i.search(query, limit)).unwrap_or_default()
    }

    pub fn entry(&self, slug: &str) -> Vec<knowledge::Hit> {
        self.0.as_ref().map(|i| i.entry(slug)).unwrap_or_default()
    }

    pub fn entries(&self) -> Vec<knowledge::Entry> {
        self.0.as_ref().map(|i| i.entries()).unwrap_or_default()
    }
}

/// The launch-time half of opening the journal, off the main thread.
///
/// If the journal is encrypted, it stays locked until the user unlocks it with their
/// passphrase, unless they chose to remember it in the system keychain
/// (`keychain.rs`), so a computer serving the journal can come back on its own after
/// a reboot. A remembered passphrase that no longer fits just leaves it locked.
/// Otherwise it's opened (and created on first run).
fn open_journal(app: tauri::AppHandle, path: PathBuf) {
    use std::sync::atomic::Ordering;
    let db = app.state::<Db>();
    let opened = if db::is_encrypted(&path) {
        Ok(keychain::get().and_then(|p| db::open(&path, Some(&p)).ok()))
    } else {
        db::open(&path, None).map(Some).map_err(|e| format!("Couldn't open the journal at {}: {e}", path.display()))
    };
    match opened {
        Ok(conn) => {
            let mut guard = db.conn.lock().unwrap();
            // The user may have unlocked it by hand while the keychain was slow.
            if guard.is_none() {
                *guard = conn;
            }
        }
        Err(e) => {
            eprintln!("{e}");
            *db.open_error.lock().unwrap() = Some(e);
        }
    }
    // The bundled dose reference lives inside the journal DB, so it can only be
    // loaded once the DB is open (i.e. not locked).
    if db.is_unlocked() {
        commands::refresh_dose_reference(&app, db.inner());
    }
    db.opening.store(false, Ordering::SeqCst);
    // A computer set up as the server starts serving as soon as the journal is open.
    // With the journal locked this waits for `unlock_db`.
    commands::bring_up_server(&app);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // Open at login — off until the user turns it on, for a computer that
        // serves the journal to their other devices.
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let dir = app.path().app_data_dir().expect("no app data dir");
            let path = dir.join("journal.db");
            // Open the journal in the background, so the window always appears at
            // once. Everything in this hook runs before macOS will show the window,
            // so anything slow here (a Keychain prompt hidden behind other windows,
            // a disk that won't answer) used to leave the Dock icon bouncing with no
            // way to tell why. The window shows "Opening your journal…" until this
            // finishes, and a failure as a message rather than a freeze.
            app.manage(Db {
                conn: Mutex::new(None),
                path: path.clone(),
                opening: true.into(),
                open_error: Mutex::new(None),
            });
            // The knowledge corpus is bundled reference data held in memory, so it
            // loads regardless of the journal's lock state. A failure here is not
            // fatal — the app simply has no prose search.
            let index = match knowledge::load_bundled(app.handle()) {
                Ok(i) => Some(i),
                Err(e) => {
                    eprintln!("knowledge corpus unavailable: {e}");
                    None
                }
            };
            app.manage(Knowledge(index));

            // The phone portal is **off**. It only ever starts because the user
            // asked it to, in Settings, on this machine. See `portal.rs`.
            app.manage(portal::Portal::default());
            // Background Companion turns started from a phone (portal-only; the
            // desktop calls `companion_chat` directly and never needs a job).
            app.manage(portal::CompanionJobs::default());
            app.manage(devices::Devices::load(dir.join("devices.json")));
            app.manage(prefs::Prefs::load(dir.join("server.json")));
            app.manage(server_update::ServerUpdate::default());
            // Using another computer as the server: sends queued entries and notices
            // when it comes back. Idle unless this computer is connected to one.
            app.manage(remote::Remote::default());
            remote::start_background(app.handle());

            // Last, once every piece of state it touches is managed.
            let handle = app.handle().clone();
            std::thread::spawn(move || open_journal(handle, path));
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
            commands::usage_stats,
            commands::set_writeup_skipped,
            commands::discreet_available,
            commands::set_discreet_available,
            commands::server_update_status,
            commands::server_update_install,
            commands::set_phone_can_update,
            commands::update_experience,
            commands::update_dose,
            commands::update_timeline_event,
            commands::delete_experience,
            commands::remove_duplicate_entries,
            commands::delete_dose,
            commands::delete_timeline_event,
            commands::delete_substance,
            commands::ai_status,
            commands::ai_recommended_models,
            commands::ai_install,
            commands::ai_start,
            commands::ai_pull,
            commands::ai_remove,
            commands::ai_switch_model,
            commands::ai_preferred_model,
            commands::ollama_up,
            commands::ollama_models,
            commands::companion_chat,
            commands::companion_warm,
            commands::compute_status,
            commands::parse_experience,
            commands::import_experience,
            commands::pw_update,
            commands::pw_status,
            commands::pw_lookup,
            commands::pw_names,
            commands::db_status,
            commands::unlock_db,
            commands::enable_encryption,
            commands::disable_encryption,
            commands::change_passphrase,
            commands::export_backup,
            commands::import_backup,
            commands::obsidian_export,
            commands::obsidian_import,
            commands::export_experience_markdown,
            commands::export_experience_file,
            commands::save_markdown_file,
            commands::contribution_candidates,
            commands::contribution_draft,
            commands::contribution_save,
            commands::portal_status,
            commands::set_companion_enabled,
            commands::portal_enable,
            commands::portal_disable,
            commands::portal_qr,
            commands::portal_tailscale,
            commands::portal_serve,
            commands::portal_unserve,
            commands::portal_pair,
            commands::portal_devices,
            commands::portal_revoke,
            commands::server_prefs,
            commands::set_server_prefs,
            commands::keychain_status,
            commands::keychain_remember,
            commands::keychain_forget,
            commands::remote_status,
            commands::remote_connect,
            commands::remote_disconnect,
            commands::remote_discard,
            commands::remote_flush,
            commands::remote_upload_local,
            commands::remote_call,
            commands::crisis_scan,
            commands::knowledge_search,
            commands::knowledge_entry,
            commands::knowledge_entries,
            commands::knowledge_status,
            commands::emergency_resources,
            commands::data_dir,
            commands::reveal_data_dir,
            commands::wipe_all_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Field Notes");
}
