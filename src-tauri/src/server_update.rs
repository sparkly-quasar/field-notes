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
//!   access at the worst moment. That goes for the owner's journal and for anyone
//!   else's that's open, as long as one of their devices has been in touch lately
//!   (`CONNECTED_WITHIN`). A session nobody has touched in hours was most likely
//!   just never ended, and the owner can't end it for them.
//! - **Other people's journals locking is the owner's call.** A journal kept
//!   unlocked only in memory locks on restart. That's an inconvenience, not a
//!   danger, so the phone names who it would affect and offers "Install anyway".
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

/// How recently one of a person's devices must have made a request for their open
/// session to count as going on now. Phones don't check in by themselves, so this
/// is generous: someone deep in a sit can go a long while without touching theirs.
const CONNECTED_WITHIN: u64 = 2 * 60 * 60;

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
    /// The phone may install anyway after saying so: what `blocked` describes only
    /// locks other people's journals, and nobody is mid-session.
    pub can_override: bool,
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
    /// People whose journals are unlocked only in memory: a restart would lock them
    /// until they unlock them again from their own device.
    pub others_would_lock: Vec<String>,
    /// Someone else has a session open and one of their devices was in touch lately.
    pub others_live_session: bool,
    /// Someone else has a session open but none of their devices has been in touch
    /// for a while: most likely a session nobody ended.
    pub others_stale_session: bool,
}

pub(crate) struct Blocked {
    pub why: String,
    pub can_override: bool,
}

fn hard(why: impl Into<String>) -> Option<Blocked> {
    Some(Blocked { why: why.into(), can_override: false })
}

/// "Sam", "Sam and Alex", "Sam, Alex and Jo".
fn names(n: &[String]) -> String {
    match n {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

pub(crate) fn blocked_reason(g: &Gates) -> Option<Blocked> {
    if !g.allowed {
        return hard("Installing from a phone is turned off. It can be turned on in Settings on the computer.");
    }
    if !g.serve_on_launch {
        return hard("The computer isn't set to turn device access on when Field Notes opens, so this phone would lose access after the restart. Install it at the computer.");
    }
    if g.encrypted && !g.remembered {
        return hard("The journal is encrypted and its password isn't saved on the computer, so it would stay locked after the restart. Install it at the computer.");
    }
    if let Some(t) = &g.open_session {
        let t = if t.is_empty() { "untitled" } else { t.as_str() };
        return hard(format!("A live trip report (\"{t}\") is still open. End it first: the restart would cut off every device."));
    }
    if g.others_live_session {
        return hard("Someone else on this server has a live trip report going. Wait until it's ended: the restart would cut them off.");
    }
    if g.others_would_lock.is_empty() && !g.others_stale_session {
        return None;
    }
    let mut why = Vec::new();
    match g.others_would_lock.as_slice() {
        [] => {}
        [one] => why.push(format!("{one}'s journal is unlocked on this server, and restarting would lock it until they unlock it again from their phone.")),
        many => why.push(format!("{}'s journals are unlocked on this server, and restarting would lock them until they unlock them again from their phones.", names(many))),
    }
    if g.others_stale_session {
        why.push("Someone else has a live trip report that was never ended, but nobody has used it in over two hours.".into());
    }
    why.push("Nobody has a live trip report going right now.".into());
    Some(Blocked { why: why.join(" "), can_override: true })
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
    // Someone else's open session, split by whether they've been in touch lately.
    let people = app.try_state::<crate::people::People>();
    let (mut live, mut stale) = (false, false);
    if let Some(p) = &people {
        let devices = app.state::<crate::devices::Devices>().list();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        for id in p.in_session() {
            let recent = devices
                .iter()
                .any(|d| d.person == id && d.last_seen.is_some_and(|t| now.saturating_sub(t) < CONNECTED_WITHIN));
            if recent {
                live = true;
            } else {
                stale = true;
            }
        }
    }
    Gates {
        allowed: p.phone_can_update,
        serve_on_launch: p.serve_on_launch,
        encrypted: db::is_encrypted(&db.path),
        remembered: keychain::remembered(),
        open_session,
        others_would_lock: people.as_ref().map(|p| p.would_lock()).unwrap_or_default(),
        others_live_session: live,
        others_stale_session: stale,
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
        can_override: blocked.as_ref().is_some_and(|b| b.can_override),
        blocked: blocked.map(|b| b.why),
        installing: s.installing,
        checking: s.checking,
        error: s.error.clone(),
    }
}

/// Download, install, restart. Returns as soon as it has started; the phone sees
/// `installing`, then the server going away, then the new version. `anyway` is the
/// owner having read who it would lock out; it never gets past a hard guard, and
/// the guards are checked again here, not taken from what the phone last saw.
pub fn install<R: Runtime>(app: &AppHandle<R>, anyway: bool) -> Result<ServerUpdateStatus, String> {
    if let Some(b) = blocked_reason(&gates(app)) {
        if !(b.can_override && anyway) {
            return Err(b.why);
        }
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
        Gates {
            allowed: true,
            serve_on_launch: true,
            encrypted: true,
            remembered: true,
            open_session: None,
            others_would_lock: vec![],
            others_live_session: false,
            others_stale_session: false,
        }
    }

    fn why(g: Gates) -> (String, bool) {
        let b = blocked_reason(&g).expect("blocked");
        (b.why, b.can_override)
    }

    #[test]
    fn installs_only_when_the_server_will_come_back_on_its_own() {
        assert!(blocked_reason(&ready()).is_none());
        assert!(blocked_reason(&Gates { encrypted: false, remembered: false, ..ready() }).is_none());
    }

    #[test]
    fn each_guard_blocks_on_its_own() {
        assert!(why(Gates { allowed: false, ..ready() }).0.contains("turned off"));
        assert!(why(Gates { serve_on_launch: false, ..ready() }).0.contains("lose access"));
        assert!(why(Gates { remembered: false, ..ready() }).0.contains("stay locked"));
        let open = why(Gates { open_session: Some("Autumn sit".into()), ..ready() }).0;
        assert!(open.contains("Autumn sit"), "{open}");
        assert!(why(Gates { others_live_session: true, ..ready() }).0.contains("trip report going"));
    }

    #[test]
    fn locking_someone_out_is_the_owners_call_but_a_live_session_never_is() {
        let (w, can) = why(Gates { others_would_lock: vec!["Sam".into()], ..ready() });
        assert!(can && w.contains("Sam's journal"), "{w}");
        let (w, _) = why(Gates { others_would_lock: vec!["Sam".into(), "Alex".into(), "Jo".into()], ..ready() });
        assert!(w.contains("Sam, Alex and Jo's journals"), "{w}");
        // A session nobody has touched in hours doesn't hold the server hostage.
        let (w, can) = why(Gates { others_stale_session: true, ..ready() });
        assert!(can && w.contains("never ended"), "{w}");
        // But someone in a session right now always wins, as do the hard guards.
        let live = Gates { others_would_lock: vec!["Sam".into()], others_live_session: true, ..ready() };
        assert!(!why(live).1);
        let off = Gates { others_would_lock: vec!["Sam".into()], serve_on_launch: false, ..ready() };
        assert!(!why(off).1);
    }
}
