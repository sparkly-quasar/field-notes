// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! The dose reference lives in `field_notes_core::pw` (shared with the phone's
//! offline checker); this adds loading it from the app bundle.

pub use field_notes_core::pw::*;

/// Path of the bundled reference file, relative to the Tauri resource dir.
pub const RESOURCE_PATH: &str = "resources/dosewiki.json";

/// Load the whole bundled reference from the app's resource directory.
pub fn load_bundled<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<Vec<PwInfo>, String> {
    parse_slim(&read_bundled(app)?)
}

/// The bundled reference file as shipped, for the phone to cache.
pub fn read_bundled<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<String, String> {
    use tauri::Manager;
    let path = app
        .path()
        .resolve(RESOURCE_PATH, tauri::path::BaseDirectory::Resource)
        .map_err(|e| format!("Couldn't locate the bundled dose reference: {e}"))?;
    std::fs::read_to_string(&path)
        .map_err(|e| format!("Couldn't read the bundled dose reference at {}: {e}", path.display()))
}
