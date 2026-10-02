// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Settings the **backend** has to act on before the window has loaded — which
//! rules out the browser storage the rest of the UI's settings live in.
//!
//! Today that's only the server role: "start device access when Field Notes opens",
//! and which tailnet HTTPS port we were published on, so a reboot republishes on
//! the same URL instead of stranding every paired device on a new one.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ServerPrefs {
    /// Turn device access on at launch (and on unlock), and republish to the tailnet
    /// if we were published before. For a computer that *is* the server.
    #[serde(default)]
    pub serve_on_launch: bool,
    /// The tailnet HTTPS port we last published on, if we're published. Set by
    /// `portal_serve`, cleared by `portal_unserve`. Not user-editable.
    #[serde(default)]
    pub served_https: Option<u16>,
    /// Paired phones may install a Field Notes update on this computer (and
    /// restart it). Off by default; only settable at the computer. See
    /// `server_update.rs` for the guards that apply even when it's on.
    #[serde(default)]
    pub phone_can_update: bool,
    /// Discreet mode is offered (the eye toggle next to Help, on this computer and
    /// its paired phones). Off by default. Whether names are hidden right now is a
    /// per-device choice kept in each browser; this only makes the toggle appear.
    #[serde(default)]
    pub discreet_available: bool,
}

pub struct Prefs {
    path: PathBuf,
    inner: Mutex<ServerPrefs>,
}

impl Prefs {
    pub fn load(path: PathBuf) -> Self {
        let p = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Prefs { path, inner: Mutex::new(p) }
    }

    pub fn get(&self) -> ServerPrefs {
        self.inner.lock().unwrap().clone()
    }

    pub fn update(&self, f: impl FnOnce(&mut ServerPrefs)) -> Result<ServerPrefs, String> {
        let mut p = self.inner.lock().unwrap();
        f(&mut p);
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&self.path, serde_json::to_vec_pretty(&*p).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        Ok(p.clone())
    }
}
