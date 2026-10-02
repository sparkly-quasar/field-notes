// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Updating the server from a paired phone.
//!
//! Seeing that the server has an update waiting is always allowed. **Installing**
//! is the first thing a phone can do that installs software and restarts the
//! server, so it is guarded, and every guard is re-checked at install time:
//!
//! - **Off until someone turns it on at the computer** (`phone_can_update` in
//!   `server.json`). The setting itself is not on the portal's allowlist.
//! - **The server must come back by itself**, or a phone that pressed "Install"
//!   would lose its journal until someone walked to the desk: device access has to
//!   start at launch (`serve_on_launch`), and an encrypted journal has to be able to
//!   unlock itself (its password saved in the keychain).
//! - **Never during an open session.** A restart mid-sit drops every device's
//!   access at the worst moment.
//!
//! What gets installed is decided by the same Tauri updater as the desktop's own
//! "Install & restart": only a release signed with the app's key, from the app's own
//! update URL. A phone can choose *when*, never *what*.

use crate::{db, keychain, prefs::Prefs, Db};
use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_updater::UpdaterExt;

/// How long a check is reused. The phone asks whenever Today opens; GitHub
/// doesn't need to hear about every one of those.
const RECHECK_AFTER: Duration = Duration::from_secs(15 * 60);

#[derive(Default)]
pub struct ServerUpdate {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    checked: Option<Instant>,
    available: Option<Available>,
    installing: bool,
    checking: bool,
    error: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct Available {
    pub version: String,
    pub notes: String,
}

#[derive(Serialize)]
pub struct ServerUpdateStatus {
    /// The version running on the server.
    pub current: String,
    /// A newer version, if the last check found one.
    pub available: Option<Available>,
    /// Why a phone can't install it right now, in words for the person holding
    /// the phone. `None` means it can.
    pub blocked: Option<String>,
    pub installing: bool,
    /// A check is running in the background; ask again in a few seconds.
    pub checking: bool,
    /// The last check or install failure, if any.
    pub error: Option<String>,
}

/// Everything the install decision depends on, gathered so the rule is testable.
pub(crate) struct Gates {
    pub allowed: bool,
    pub serve_on_launch: bool,
    pub encrypted: bool,
    pub remembered: bool,
    /// Title of a session that hasn't been ended, if there is one.
    pub open_session: Option<String>,
}

pub(crate) fn blocked_reason(g: &Gates) -> Option<String> {
    if !g.allowed {
        return Some("Installing from a phone is turned off. It can be turned on in Settings on the computer.".into());
    }
    if !g.serve_on_launch {
        return Some("The computer isn't set to turn device access on when Field Notes opens, so this phone would lose access after the restart. Install it at the computer.".into());
    }
    if g.encrypted && !g.remembered {
        return Some("The journal is encrypted and its password isn't saved on the computer, so it would stay locked after the restart. Install it at the computer.".into());
    }
    if let Some(t) = &g.open_session {
        let t = if t.is_empty() { "untitled" } else { t.as_str() };
        return Some(format!("A session (\"{t}\") is still open. End it first: the restart would cut off every device."));
    }
    None
}

fn gates<R: Runtime>(app: &AppHandle<R>) -> Gates {
    let p = app.state::<Prefs>().get();
    let db = app.state::<Db>();
    let open_session = db
        .with(|c| {
            use rusqlite::OptionalExtension;
            c.query_row(
                "SELECT title FROM experiences WHERE kind = 'session' AND ended_at IS NULL
                 ORDER BY started_at DESC LIMIT 1",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()
        })
        .ok()
        .flatten();
    Gates {
        allowed: p.phone_can_update,
        serve_on_launch: p.serve_on_launch,
        encrypted: db::is_encrypted(&db.path),
        remembered: keychain::remembered(),
        open_session,
    }
}

async fn check_now<R: Runtime>(app: &AppHandle<R>) -> Result<Option<Available>, String> {
    let updater = app
        .updater_builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let found = updater.check().await.map_err(|e| e.to_string())?;
    Ok(found.map(|u| Available { version: u.version, notes: u.body.unwrap_or_default() }))
}

/// What the phone shows. Never waits on the network: the portal answers one
/// request at a time, so a slow check would stall the phone. When the last check is
/// older than 15 minutes, a new one starts in the background and this returns what
/// was known, with `checking` set.
pub fn status<R: Runtime>(app: &AppHandle<R>) -> ServerUpdateStatus {
    let state = app.state::<ServerUpdate>();
    let start = {
        let mut s = state.inner.lock().unwrap();
        let stale = s.checked.is_none_or(|t| t.elapsed() > RECHECK_AFTER);
        let start = stale && !s.installing && !s.checking;
        if start {
            s.checking = true;
        }
        start
    };
    if start {
        let handle = app.clone();
        std::thread::spawn(move || {
            let result = tauri::async_runtime::block_on(check_now(&handle));
            let state = handle.state::<ServerUpdate>();
            let mut s = state.inner.lock().unwrap();
            s.checking = false;
            s.checked = Some(Instant::now());
            match result {
                Ok(a) => {
                    s.available = a;
                    s.error = None;
                }
                // Offline is normal for this app; keep what we knew and say why.
                Err(e) => s.error = Some(format!("Couldn't check for updates: {e}")),
            }
        });
    }
    let s = state.inner.lock().unwrap();
    let blocked = if s.available.is_some() { blocked_reason(&gates(app)) } else { None };
    ServerUpdateStatus {
        current: app.package_info().version.to_string(),
        available: s.available.clone(),
        blocked,
        installing: s.installing,
        checking: s.checking,
        error: s.error.clone(),
    }
}

/// Download, install, restart. Returns as soon as it has started; the phone sees
/// `installing`, then the server going away, then the new version.
pub fn install<R: Runtime>(app: &AppHandle<R>) -> Result<ServerUpdateStatus, String> {
    if let Some(why) = blocked_reason(&gates(app)) {
        return Err(why);
    }
    {
        let state = app.state::<ServerUpdate>();
        let mut s = state.inner.lock().unwrap();
        if s.installing {
            drop(s);
            return Ok(status(app));
        }
        if s.available.is_none() {
            return Err("There's no update to install.".into());
        }
        s.installing = true;
        s.error = None;
    }
    let handle = app.clone();
    std::thread::spawn(move || {
        let result = tauri::async_runtime::block_on(async {
            let update = handle
                .updater()
                .map_err(|e| e.to_string())?
                .check()
                .await
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "The update is no longer available.".to_string())?;
            update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())
        });
        match result {
            Ok(()) => handle.restart(),
            Err(e) => {
                let state = handle.state::<ServerUpdate>();
                let mut s = state.inner.lock().unwrap();
                s.installing = false;
                s.error = Some(format!("The update didn't install: {e}"));
            }
        }
    });
    Ok(status(app))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> Gates {
        Gates { allowed: true, serve_on_launch: true, encrypted: true, remembered: true, open_session: None }
    }

    #[test]
    fn installs_only_when_the_server_will_come_back_on_its_own() {
        assert!(blocked_reason(&ready()).is_none());
        assert!(blocked_reason(&Gates { encrypted: false, remembered: false, ..ready() }).is_none());
    }

    #[test]
    fn each_guard_blocks_on_its_own() {
        assert!(blocked_reason(&Gates { allowed: false, ..ready() }).unwrap().contains("turned off"));
        assert!(blocked_reason(&Gates { serve_on_launch: false, ..ready() }).unwrap().contains("lose access"));
        assert!(blocked_reason(&Gates { remembered: false, ..ready() }).unwrap().contains("stay locked"));
        let open = blocked_reason(&Gates { open_session: Some("Autumn sit".into()), ..ready() }).unwrap();
        assert!(open.contains("Autumn sit"), "{open}");
    }
}
