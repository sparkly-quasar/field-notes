// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! The corpus search lives in `field_notes_core::knowledge` (shared with the
//! phone's offline lookup); this adds loading it from the app bundle.

pub use field_notes_core::knowledge::*;

/// Path of the bundled corpus, relative to the Tauri resource dir.
pub const RESOURCE_PATH: &str = "resources/dosewiki-corpus.json";

/// Load and index the bundled corpus from the app's resource directory.
pub fn load_bundled(app: &tauri::AppHandle) -> Result<Index, String> {
    load_str(&read_bundled(app)?)
}

/// The bundled corpus file as shipped, for the phone to cache.
pub fn read_bundled<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<String, String> {
    use tauri::Manager;
    let path = app
        .path()
        .resolve(RESOURCE_PATH, tauri::path::BaseDirectory::Resource)
        .map_err(|e| format!("knowledge corpus not found: {e}"))?;
    std::fs::read_to_string(&path).map_err(|e| format!("could not read knowledge corpus: {e}"))
}
