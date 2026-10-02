// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Using another computer as the journal — the **client** half of "one computer
//! is the server". A laptop connected this way reads and writes the journal on the
//! server over its portal, exactly as a phone does, instead of its own database.
//!
//! ## What happens when the server can't be reached
//!
//! **New entries queue; nothing else does.** Starting a session, logging a dose,
//! adding a timeline note and ending a session are appended to an outbox and sent,
//! in order, the moment the server answers again. Edits and deletes are refused
//! while offline: queueing them re-opens conflict resolution, which the roadmap
//! rules out (see ROADMAP.md, Phase 3b — the same rule, for the same reason).
//!
//! **The safety layers keep running.** This is the laptop, not a browser: it has
//! the same `interactions.rs`, `crisis.rs` and bundled DoseWiki data as the server.
//! A dose logged offline is checked against the session as this computer last saw
//! it plus everything queued since, and the answer is shown just as it would be
//! online. Offline must never mean "silently unchecked".
//!
//! **Nothing leaves the encrypted journal.** The outbox, the read cache and the
//! pairing token all live in *this* computer's journal database — SQLCipher, if the
//! user turned encryption on — not in browser storage.
//!
//! ## IDs
//!
//! Something created offline has no server ID yet, so it gets a **negative** one
//! locally. When it's sent, the server's real ID is recorded against it, and every
//! later call carrying the temporary ID — a queued dose into a queued session, or
//! the UI still holding the old number — is translated on the way out.

use crate::{commands, db, Db};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Runtime};

/// Everything the desktop is allowed to send to the server. It's the portal's
/// `EXPOSED` list minus what's better answered locally (bundled reference data
/// that is identical on both machines). The server enforces its own allowlist
/// regardless; this one keeps the desktop from even trying anything else.
pub const ROUTED: &[&str] = &[
    "list_experiences",
    "get_experience",
    "export_experience_markdown",
    "usage_by_substance",
    "usage_stats",
    "list_substances",
    "create_experience",
    "end_experience",
    "log_dose",
    "add_timeline_event",
    "add_substance",
    "update_experience",
    "set_writeup_skipped",
    "update_dose",
    "update_timeline_event",
    "delete_experience",
    "delete_dose",
    "delete_timeline_event",
    "delete_substance",
    "check_combo",
    "crisis_scan",
    "companion_chat",
    "companion_warm",
    "compute_status",
    "ai_status",
    "ollama_up",
    "ollama_models",
    "ai_start",
];

/// Writes that may wait in the outbox. Only ever *new* things (plus ending a
/// session, which is setting a field nobody else can be setting on a session this
/// device started) — see the module docs.
const QUEUEABLE: &[&str] = &["create_experience", "log_dose", "add_timeline_event", "end_experience"];

/// Reads whose last answer is kept for offline use.
const CACHED: &[&str] = &["list_experiences", "get_experience", "list_substances", "usage_by_substance"];

/// After a failed attempt, go straight to the offline path for this long rather
/// than making every click wait out a connect timeout. The background loop keeps
/// probing in the meantime.
const OFFLINE_BACKOFF: Duration = Duration::from_secs(15);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// A Companion reply from a slow local model can take minutes.
const COMPANION_TIMEOUT: Duration = Duration::from_secs(15 * 60);

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS remote_config (k TEXT PRIMARY KEY, v TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS remote_cache (key TEXT PRIMARY KEY, body TEXT NOT NULL, at TEXT NOT NULL DEFAULT (datetime('now')));
CREATE TABLE IF NOT EXISTS remote_outbox (
    seq      INTEGER PRIMARY KEY AUTOINCREMENT,
    cmd      TEXT NOT NULL,
    args     TEXT NOT NULL,
    temp_id  INTEGER,
    preview  TEXT NOT NULL,
    failed   TEXT
);
CREATE TABLE IF NOT EXISTS remote_idmap (temp_id INTEGER PRIMARY KEY, real_id INTEGER NOT NULL);
-- This computer's own entries already copied to a server (upload_local), per server,
-- so syncing again only sends what's new. Survives a disconnect on purpose: a
-- reconnect must not copy everything a second time.
CREATE TABLE IF NOT EXISTS remote_synced (
    local_id  INTEGER NOT NULL,
    server    TEXT NOT NULL,
    server_id INTEGER NOT NULL,
    PRIMARY KEY (local_id, server)
);
-- Half-copied entries a failed sync couldn't take back off the server (the
-- connection was already gone). Removed at the start of the next sync.
CREATE TABLE IF NOT EXISTS remote_orphans (
    server    TEXT NOT NULL,
    server_id INTEGER NOT NULL,
    PRIMARY KEY (server, server_id)
);
";

#[derive(Default)]
pub struct Remote {
    online: AtomicBool,
    last_failure: Mutex<Option<Instant>>,
    /// One flush at a time, or two threads would send the same queued dose twice.
    flushing: Mutex<()>,
    /// One "Sync journal to server" at a time, or two would copy the same entries.
    syncing: Mutex<()>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Failed {
    pub seq: i64,
    pub cmd: String,
    pub error: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct RemoteStatus {
    /// This computer uses another as its journal.
    pub connected: bool,
    /// The server's address, e.g. `https://isaac.tailnet.ts.net`.
    pub server: Option<String>,
    /// The server's short name for the UI ("isaac").
    pub server_name: Option<String>,
    /// The last attempt to reach it succeeded.
    pub online: bool,
    /// New entries waiting to be sent.
    pub pending: i64,
    /// Queued entries the server refused, with its reason. Never dropped silently.
    pub failed: Vec<Failed>,
    /// The server no longer recognises this computer (it was un-paired there).
    pub unpaired: bool,
    /// Entries in this computer's own journal that haven't been copied to the
    /// server yet — what "Sync journal to server" would send.
    pub local_unsynced: i64,
}

enum HttpErr {
    /// Couldn't talk to it at all: asleep, off the tailnet, no route — or it's
    /// there but not serving: Tailscale answers 502 when Field Notes isn't running
    /// on the server, and a portal mid-restart can answer 500. Retry later.
    Net(String),
    /// The journal is locked on the server. Also "retry later".
    Locked,
    /// It answered, and said no.
    Status(u16, String),
}

// ---------- storage ----------

fn ensure(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(SCHEMA)
}

fn cfg(conn: &Connection, k: &str) -> rusqlite::Result<Option<String>> {
    ensure(conn)?;
    conn.query_row("SELECT v FROM remote_config WHERE k = ?1", [k], |r| r.get(0)).optional()
}

fn set_cfg(conn: &Connection, k: &str, v: &str) -> rusqlite::Result<()> {
    ensure(conn)?;
    conn.execute(
        "INSERT INTO remote_config (k, v) VALUES (?1, ?2) ON CONFLICT(k) DO UPDATE SET v = excluded.v",
        params![k, v],
    )?;
    Ok(())
}

/// (base URL, token), if this computer is connected to a server.
fn endpoint(conn: &Connection) -> rusqlite::Result<Option<(String, String)>> {
    Ok(cfg(conn, "url")?.zip(cfg(conn, "token")?))
}

fn cache_key(cmd: &str, args: &Value) -> String {
    match cmd {
        "get_experience" => format!("get_experience:{}", args["id"]),
        _ => cmd.to_string(),
    }
}

fn cache_get(conn: &Connection, key: &str) -> Option<Value> {
    ensure(conn).ok()?;
    conn.query_row("SELECT body FROM remote_cache WHERE key = ?1", [key], |r| r.get::<_, String>(0))
        .optional()
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn cache_put(conn: &Connection, key: &str, body: &Value) -> rusqlite::Result<()> {
    ensure(conn)?;
    conn.execute(
        "INSERT INTO remote_cache (key, body) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET body = excluded.body, at = datetime('now')",
        params![key, body.to_string()],
    )?;
    Ok(())
}

fn pending_count(conn: &Connection) -> rusqlite::Result<i64> {
    ensure(conn)?;
    conn.query_row("SELECT COUNT(*) FROM remote_outbox WHERE failed IS NULL", [], |r| r.get(0))
}

/// The next temporary (negative) ID, below every one already handed out.
fn next_temp_id(conn: &Connection) -> rusqlite::Result<i64> {
    let lowest: i64 = conn.query_row(
        "SELECT MIN(v) FROM (
            SELECT COALESCE(MIN(temp_id), 0) AS v FROM remote_outbox
            UNION ALL SELECT COALESCE(MIN(temp_id), 0) FROM remote_idmap
            UNION ALL SELECT COALESCE(CAST(v AS INTEGER), 0) FROM remote_config WHERE k = 'lowest_temp'
         )",
        [],
        |r| r.get(0),
    )?;
    let id = lowest.min(0) - 1;
    // Remembered separately so an ID is never reused, even after the outbox and
    // map have been cleared of it — the UI may still be holding the old number.
    set_cfg(conn, "lowest_temp", &id.to_string())?;
    Ok(id)
}

fn now_sqlite(conn: &Connection) -> String {
    conn.query_row("SELECT datetime('now')", [], |r| r.get(0)).unwrap_or_default()
}

/// Replace temporary IDs in `args` with real ones where the server has assigned
/// them. Returns false if any temporary ID is still waiting for its entry to be sent.
fn translate(conn: &Connection, args: &mut Value) -> rusqlite::Result<bool> {
    let mut resolved = true;
    for ptr in ["/id", "/input/experience_id", "/experienceId"] {
        let Some(slot) = args.pointer_mut(ptr) else { continue };
        let Some(id) = slot.as_i64() else { continue };
        if id >= 0 {
            continue;
        }
        let real: Option<i64> = conn
            .query_row("SELECT real_id FROM remote_idmap WHERE temp_id = ?1", [id], |r| r.get(0))
            .optional()?;
        match real {
            Some(real) => *slot = json!(real),
            None => resolved = false,
        }
    }
    Ok(resolved)
}

// ---------- the network ----------

fn http(base: &str, token: &str, cmd: &str, args: &Value) -> Result<Value, HttpErr> {
    let timeout = if cmd == "companion_chat" { COMPANION_TIMEOUT } else { REQUEST_TIMEOUT };
    let agent = ureq::AgentBuilder::new().timeout_connect(CONNECT_TIMEOUT).timeout(timeout).build();
    match agent
        .post(&format!("{base}/api/{cmd}"))
        .set("Authorization", &format!("Bearer {token}"))
        .send_json(args.clone())
    {
        Ok(r) => r.into_json::<Value>().map_err(|e| HttpErr::Net(e.to_string())),
        Err(ureq::Error::Status(503, _)) => Err(HttpErr::Locked),
        Err(ureq::Error::Status(code, _)) if code >= 500 => {
            Err(HttpErr::Net(format!("The server answered {code}.")))
        }
        Err(ureq::Error::Status(code, r)) => {
            let msg = r
                .into_json::<Value>()
                .ok()
                .and_then(|v| v["error"].as_str().map(str::to_string))
                .unwrap_or_else(|| format!("The server answered {code}."));
            Err(HttpErr::Status(code, msg))
        }
        Err(e) => Err(HttpErr::Net(e.to_string())),
    }
}

/// Split a pairing link into (server address, token). The link is what the server's
/// "Pair a device" screen shows: `https://host[:port]/m#t=<64 hex>`.
pub fn parse_link(link: &str) -> Result<(String, String), String> {
    let link = link.trim();
    let bad = || "That doesn't look like a Field Notes pairing link. Copy it from \"Pair a device\" on the server.".to_string();
    let (base, frag) = link.split_once('#').ok_or_else(bad)?;
    let token = frag
        .split('&')
        .find_map(|kv| kv.strip_prefix("t="))
        .filter(|t| t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit()))
        .ok_or_else(bad)?
        .to_ascii_lowercase();
    let base = base.trim_end_matches('/');
    let base = base.strip_suffix("/m").unwrap_or(base).trim_end_matches('/').to_string();

    let loopback = base.starts_with("http://127.0.0.1") || base.starts_with("http://localhost");
    if !base.starts_with("https://") && !loopback {
        // Tailscale serves HTTPS. Plain HTTP to anything but this machine would send
        // the token, and then the journal, across the network in the clear.
        return Err("The server's address must start with https:// — use the tailnet link from the server.".into());
    }
    if base.len() <= "https://".len() {
        return Err(bad());
    }
    Ok((base, token))
}

fn short_name(base: &str) -> String {
    let host = base.split("://").nth(1).unwrap_or(base);
    let host = host.split([':', '/']).next().unwrap_or(host);
    host.split('.').next().unwrap_or(host).to_string()
}

// ---------- state ----------

impl Remote {
    fn mark(&self, online: bool) {
        self.online.store(online, Ordering::SeqCst);
        *self.last_failure.lock().unwrap() = if online { None } else { Some(Instant::now()) };
    }

    fn backing_off(&self) -> bool {
        self.last_failure.lock().unwrap().is_some_and(|t| t.elapsed() < OFFLINE_BACKOFF)
    }
}

pub fn status<R: Runtime>(app: &AppHandle<R>) -> RemoteStatus {
    let db = app.state::<Db>();
    let remote = app.state::<Remote>();
    db.with(|c| {
        let ep = endpoint(c)?;
        let mut stmt = c.prepare("SELECT seq, cmd, failed FROM remote_outbox WHERE failed IS NOT NULL ORDER BY seq")?;
        let failed = stmt
            .query_map([], |r| Ok(Failed { seq: r.get(0)?, cmd: r.get(1)?, error: r.get(2)? }))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RemoteStatus {
            connected: ep.is_some(),
            server_name: ep.as_ref().map(|(b, _)| short_name(b)),
            server: ep.clone().map(|(b, _)| b),
            online: remote.online.load(Ordering::SeqCst),
            pending: pending_count(c)?,
            failed,
            unpaired: cfg(c, "unpaired")?.is_some(),
            local_unsynced: match &ep {
                Some((base, _)) => unsynced_local(c, base)?.len() as i64,
                None => 0,
            },
        })
    })
    .unwrap_or(RemoteStatus {
        connected: false,
        server: None,
        server_name: None,
        online: false,
        pending: 0,
        failed: vec![],
        unpaired: false,
        local_unsynced: 0,
    })
}

/// Is this computer using a server for its journal? False while the local journal
/// is locked — the pairing lives inside it.
pub fn is_connected<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.state::<Db>().with(endpoint).ok().flatten().is_some()
}

fn notify<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit("remote-status", status(app));
}

/// Connect to a server using a pairing link. Tries it before saving anything, so a
/// typo or a revoked link fails here, not at the next dose.
pub fn connect<R: Runtime>(app: &AppHandle<R>, link: &str) -> Result<RemoteStatus, String> {
    let (base, token) = parse_link(link)?;
    match http(&base, &token, "list_experiences", &json!({})) {
        Ok(list) => {
            app.state::<Db>().with(|c| {
                ensure(c)?;
                c.execute_batch("DELETE FROM remote_config; DELETE FROM remote_cache; DELETE FROM remote_idmap;")?;
                set_cfg(c, "url", &base)?;
                set_cfg(c, "token", &token)?;
                cache_put(c, "list_experiences", &list)
            })?;
            app.state::<Remote>().mark(true);
            notify(app);
            Ok(status(app))
        }
        Err(HttpErr::Status(401, _)) => {
            Err("The server didn't recognise that link — it may have been revoked. Pair again on the server.".into())
        }
        Err(HttpErr::Locked) => {
            Err("The server's journal is locked. Unlock it there (or let it unlock from the keychain), then try again.".into())
        }
        Err(HttpErr::Status(_, msg)) => Err(msg),
        Err(HttpErr::Net(e)) => Err(format!(
            "Couldn't reach {base}. Is that computer awake, running Field Notes, and on your tailnet? ({e})"
        )),
    }
}

/// Stop using the server. Refuses while entries are still waiting to be sent unless
/// `discard` is set — losing a logged dose has to be something the user chose.
pub fn disconnect<R: Runtime>(app: &AppHandle<R>, discard: bool) -> Result<RemoteStatus, String> {
    let pending = app.state::<Db>().with(pending_count)?;
    if pending > 0 && !discard {
        return Err(format!(
            "{pending} new {} still waiting to reach the server. Reconnect to send {}, or disconnect anyway to discard {}.",
            if pending == 1 { "entry is" } else { "entries are" },
            if pending == 1 { "it" } else { "them" },
            if pending == 1 { "it" } else { "them" },
        ));
    }
    app.state::<Db>().with(|c| {
        ensure(c)?;
        c.execute_batch(
            "DELETE FROM remote_config; DELETE FROM remote_cache; DELETE FROM remote_outbox; DELETE FROM remote_idmap;",
        )
    })?;
    app.state::<Remote>().mark(false);
    notify(app);
    Ok(status(app))
}

/// Throw away one queued entry the server refused.
pub fn discard_failed<R: Runtime>(app: &AppHandle<R>, seq: i64) -> Result<RemoteStatus, String> {
    app.state::<Db>().with(|c| {
        ensure(c)?;
        c.execute("DELETE FROM remote_outbox WHERE seq = ?1 AND failed IS NOT NULL", [seq])
    })?;
    notify(app);
    Ok(status(app))
}

// ---------- syncing this computer's own journal up ----------

/// This computer's own entries not yet copied to `server`, oldest first.
fn unsynced_local(conn: &Connection, server: &str) -> rusqlite::Result<Vec<i64>> {
    ensure(conn)?;
    let mut stmt = conn.prepare(
        "SELECT id FROM experiences
         WHERE id NOT IN (SELECT local_id FROM remote_synced WHERE server = ?1)
         ORDER BY started_at, id",
    )?;
    let ids = stmt.query_map([server], |r| r.get(0))?.collect();
    ids
}

#[derive(Debug, Serialize)]
pub struct UploadResult {
    /// Entries copied this time.
    pub copied: i64,
    /// Entries not copied because the server already had an identical one.
    pub skipped: i64,
    /// Duplicate entries removed from the server's journal after syncing.
    pub removed: i64,
    /// Substances you'd added to this computer's catalogue that the server didn't have.
    pub substances: i64,
    pub status: RemoteStatus,
}

/// One request that must succeed, with the server's own words if it doesn't.
fn send(base: &str, token: &str, cmd: &str, args: Value) -> Result<Value, String> {
    http(base, token, cmd, &args).map_err(|e| match e {
        HttpErr::Net(_) => "Lost the connection to the server.".to_string(),
        HttpErr::Locked => "The server's journal is locked.".to_string(),
        HttpErr::Status(401, _) => "The server no longer recognises this computer.".to_string(),
        HttpErr::Status(_, msg) => msg,
    })
}

/// Copy one local entry to the server, whole: the entry, its doses and timeline,
/// then its write-up, rating and times. Returns the server's ID — or the reason it
/// failed, plus the half-copied entry's ID if it couldn't be taken back out.
fn upload_one(base: &str, token: &str, d: &db::ExperienceDetail) -> Result<i64, (String, Option<i64>)> {
    let e = &d.experience;
    let created = send(base, token, "create_experience", json!({ "input": {
        "kind": e.kind, "title": e.title, "intention": e.intention,
        "setting": e.setting, "started_at": e.started_at,
    }}))
    .map_err(|e| (e, None))?;
    let sid = created["id"].as_i64().ok_or(("The server didn't return an ID.".to_string(), None))?;

    let rest = (|| {
        for dose in &d.doses {
            send(base, token, "log_dose", json!({ "input": {
                "experience_id": sid, "substance_name": dose.substance_name, "amount": dose.amount,
                "unit": dose.unit, "route": dose.route, "taken_at": dose.taken_at, "note": dose.note,
            }}))?;
        }
        for ev in &d.timeline {
            send(base, token, "add_timeline_event", json!({ "input": {
                "experience_id": sid, "at": ev.at, "note": ev.note, "mood": ev.mood, "intensity": ev.intensity,
            }}))?;
        }
        // Last, so the entry ends up exactly as written here — including a blank
        // title the first dose would otherwise have filled in on the server.
        send(base, token, "update_experience", json!({ "id": sid, "update": {
            "title": e.title, "intention": e.intention, "setting": e.setting, "notes": e.notes,
            "rating": e.rating, "started_at": e.started_at, "ended_at": e.ended_at,
        }}))?;
        // Best effort: a server older than v0.15 doesn't know this command, and
        // that mustn't undo the copy of an entry that otherwise arrived whole.
        if e.writeup_skipped {
            let _ = send(base, token, "set_writeup_skipped", json!({ "id": sid, "skipped": true }));
        }
        Ok::<(), String>(())
    })();

    if let Err(msg) = rest {
        // Don't leave half an entry on the server: take it back out, so running the
        // sync again copies it cleanly instead of duplicating it. If even that
        // can't get through, say so, and the next sync removes it first.
        let gone = send(base, token, "delete_experience", json!({ "id": sid })).is_ok();
        return Err((msg, (!gone).then_some(sid)));
    }
    Ok(sid)
}

/// "Sync journal to server": copy this computer's own entries — the ones hidden
/// while it's connected — to the server. They stay here too. Entries already
/// copied are skipped, so this can be run again at any time and only sends what's
/// new. Needs the server: nothing about this is queued.
///
/// Every entry waiting to be synced is checked against the server's journal
/// first: one the server already has — identical in everything written, however
/// it got there — is recorded as synced and not sent again. Afterwards the server
/// is asked to remove any exact duplicates it holds.
pub fn upload_local<R: Runtime>(app: &AppHandle<R>) -> Result<UploadResult, String> {
    let remote = app.state::<Remote>();
    let _one_at_a_time = remote.syncing.lock().unwrap();
    let db = app.state::<Db>();
    let (base, token) = db.with(endpoint)?.ok_or("This computer isn't connected to a server.")?;
    let unreachable = |why: &str| format!("{why} Syncing needs the connection — try again once it's back.");

    // Queued entries first, so the server's journal reads in the order things happened.
    if flush(app)? != Flushed::Clear {
        return Err(unreachable("Can't reach your server."));
    }

    // Your own additions to the substance catalogue, so the copied doses keep their
    // classifications (and the interaction checker keeps seeing them) on the server.
    let mine: Vec<db::Substance> = db.with(db::list_substances)?.into_iter().filter(|s| s.user_added).collect();
    let mut substances = 0;
    if !mine.is_empty() {
        let theirs = send(&base, &token, "list_substances", json!({})).map_err(|e| unreachable(&e))?;
        let known: std::collections::HashSet<String> = theirs
            .as_array()
            .map(|a| a.iter().filter_map(|s| s["name"].as_str().map(str::to_lowercase)).collect())
            .unwrap_or_default();
        for sub in mine.iter().filter(|s| !known.contains(&s.name.to_lowercase())) {
            send(&base, &token, "add_substance", json!({ "input": {
                "name": sub.name, "aliases": sub.aliases, "category": sub.category,
                "classes": sub.classes, "dose_note": sub.dose_note, "notes": sub.notes,
            }}))?;
            substances += 1;
        }
    }

    // Half-copied entries an earlier sync couldn't take back out.
    let orphans: Vec<i64> = db.with(|c| {
        ensure(c)?;
        let mut stmt = c.prepare("SELECT server_id FROM remote_orphans WHERE server = ?1")?;
        let ids = stmt.query_map([&base], |r| r.get(0))?.collect();
        ids
    })?;
    for sid in orphans {
        send(&base, &token, "delete_experience", json!({ "id": sid })).map_err(|e| unreachable(&e))?;
        db.with(|c| c.execute("DELETE FROM remote_orphans WHERE server = ?1 AND server_id = ?2", params![base, sid]))?;
    }

    let local: Vec<db::ExperienceDetail> = db.with(|c| {
        unsynced_local(c, &base)?.into_iter().map(|id| db::get_experience(c, id)).collect()
    })?;
    let mut on_server = if local.is_empty() {
        std::collections::HashMap::new()
    } else {
        server_fingerprints(&base, &token, &local).map_err(|e| unreachable(&e))?
    };

    let record = |id: i64, sid: i64| {
        db.with(|c| {
            c.execute(
                "INSERT OR REPLACE INTO remote_synced (local_id, server, server_id) VALUES (?1, ?2, ?3)",
                params![id, base, sid],
            )
        })
    };
    let (mut copied, mut skipped) = (0, 0);
    for detail in &local {
        let id = detail.experience.id;
        let fp = db::fingerprint(&serde_json::to_value(detail).unwrap_or_default());
        // Already there — copied by an earlier sync this computer has forgotten, or
        // written on the server too, or a second copy in this journal. Don't send it.
        if let Some(&sid) = on_server.get(&fp) {
            record(id, sid)?;
            skipped += 1;
            continue;
        }
        match upload_one(&base, &token, detail) {
            Ok(sid) => {
                record(id, sid)?;
                on_server.insert(fp, sid);
                copied += 1;
            }
            Err((msg, orphan)) => {
                if let Some(sid) = orphan {
                    db.with(|c| {
                        c.execute(
                            "INSERT OR IGNORE INTO remote_orphans (server, server_id) VALUES (?1, ?2)",
                            params![base, sid],
                        )
                    })?;
                }
                notify(app);
                return Err(format!(
                    "Copied {copied} {} before stopping: {msg} Run the sync again to carry on — nothing is copied twice.",
                    if copied == 1 { "entry" } else { "entries" }
                ));
            }
        }
    }

    // Clear out duplicates already on the server — from an older version's sync, or
    // from another computer. Best effort: a server that predates this just says no.
    let removed = http(&base, &token, "remove_duplicate_entries", &json!({}))
        .ok()
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    remote.mark(true);
    notify(app);
    Ok(UploadResult { copied, skipped, removed, substances, status: status(app) })
}

/// The server's entries that could match one of `local`, by fingerprint → server ID.
/// Only entries of the same kind started on the same day are fetched in full.
fn server_fingerprints(
    base: &str,
    token: &str,
    local: &[db::ExperienceDetail],
) -> Result<std::collections::HashMap<String, i64>, String> {
    let day = |kind: &str, at: &str| format!("{kind}|{}", at.get(..10).unwrap_or(at));
    let wanted: std::collections::HashSet<String> =
        local.iter().map(|d| day(&d.experience.kind, &d.experience.started_at)).collect();
    let list = send(base, token, "list_experiences", json!({}))?;
    let mut out = std::collections::HashMap::new();
    for e in list.as_array().into_iter().flatten() {
        let (Some(sid), Some(kind), Some(at)) = (e["id"].as_i64(), e["kind"].as_str(), e["started_at"].as_str()) else {
            continue;
        };
        if !wanted.contains(&day(kind, at)) {
            continue;
        }
        let detail = send(base, token, "get_experience", json!({ "id": sid }))?;
        // Keep the oldest if the server itself has copies.
        out.entry(db::fingerprint(&detail)).and_modify(|v: &mut i64| *v = (*v).min(sid)).or_insert(sid);
    }
    Ok(out)
}

#[derive(Debug, PartialEq)]
enum Flushed {
    /// The outbox is empty: everything queued has reached the server.
    Clear,
    /// Couldn't reach it (or it's locked, or it no longer knows us). Try later.
    Blocked,
}

/// Send queued entries, oldest first. Stops at the first one that can't be
/// delivered for a reason that would also stop everything behind it.
fn flush<R: Runtime>(app: &AppHandle<R>) -> Result<Flushed, String> {
    let remote = app.state::<Remote>();
    let _one_at_a_time = remote.flushing.lock().unwrap();
    let db = app.state::<Db>();
    let Some((base, token)) = db.with(endpoint)? else { return Ok(Flushed::Clear) };

    loop {
        let next = db.with(|c| {
            ensure(c)?;
            c.query_row(
                "SELECT seq, cmd, args, temp_id FROM remote_outbox WHERE failed IS NULL ORDER BY seq LIMIT 1",
                [],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<i64>>(3)?)),
            )
            .optional()
        })?;
        let Some((seq, cmd, args, temp_id)) = next else { return Ok(Flushed::Clear) };

        let mut args: Value = serde_json::from_str(&args).unwrap_or(json!({}));
        if !db.with(|c| translate(c, &mut args))? {
            // Its session was never created on the server (that entry failed and is
            // listed as such), so this one can't land either. Say which.
            db.with(|c| {
                c.execute(
                    "UPDATE remote_outbox SET failed = ?2 WHERE seq = ?1",
                    params![seq, "Its session couldn't be saved on the server, so this couldn't either."],
                )
            })?;
            continue;
        }

        match http(&base, &token, &cmd, &args) {
            Ok(v) => {
                let real = v["id"].as_i64().or_else(|| v["dose"]["id"].as_i64());
                db.with_mut(|c| {
                    let tx = c.transaction()?;
                    if let (Some(temp), Some(real)) = (temp_id, real) {
                        tx.execute(
                            "INSERT OR REPLACE INTO remote_idmap (temp_id, real_id) VALUES (?1, ?2)",
                            params![temp, real],
                        )?;
                    }
                    tx.execute("DELETE FROM remote_outbox WHERE seq = ?1", [seq])?;
                    tx.execute("DELETE FROM remote_config WHERE k = 'unpaired'", [])?;
                    tx.commit()
                })?;
                remote.mark(true);
            }
            Err(HttpErr::Net(_)) | Err(HttpErr::Locked) => {
                remote.mark(false);
                return Ok(Flushed::Blocked);
            }
            Err(HttpErr::Status(401, _)) => {
                db.with(|c| set_cfg(c, "unpaired", "1"))?;
                remote.mark(false);
                return Ok(Flushed::Blocked);
            }
            Err(HttpErr::Status(_, msg)) => {
                // The server understood and refused (e.g. a dose into a plain note).
                // Keep it, visibly, rather than retrying forever or dropping it.
                db.with(|c| c.execute("UPDATE remote_outbox SET failed = ?2 WHERE seq = ?1", params![seq, msg]))?;
            }
        }
    }
}

/// Called by the background loop: send what's queued, or just check whether the
/// server is back.
pub fn tick<R: Runtime>(app: &AppHandle<R>) {
    if !is_connected(app) {
        return;
    }
    let before = status(app);
    let db = app.state::<Db>();
    let pending = db.with(pending_count).unwrap_or(0);
    if pending > 0 {
        let _ = flush(app);
    } else if let Ok(Some((base, token))) = db.with(endpoint) {
        match http(&base, &token, "db_status", &json!({})) {
            Ok(_) => {
                let _ = db.with(|c| c.execute("DELETE FROM remote_config WHERE k = 'unpaired'", []));
                app.state::<Remote>().mark(true)
            }
            Err(HttpErr::Status(401, _)) => {
                let _ = db.with(|c| set_cfg(c, "unpaired", "1"));
                app.state::<Remote>().mark(false);
            }
            Err(_) => app.state::<Remote>().mark(false),
        }
    }
    if status(app) != before {
        notify(app);
    }
}

pub fn start_background<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(OFFLINE_BACKOFF);
        tick(&app);
    });
}

// ---------- routing ----------

/// Run one journal command against the server — or, if it can't be reached, answer
/// it here as well as honestly possible.
pub fn call<R: Runtime>(app: &AppHandle<R>, cmd: &str, mut args: Value) -> Result<Value, String> {
    if !ROUTED.contains(&cmd) {
        return Err(format!("`{cmd}` isn't sent to the server."));
    }
    let db = app.state::<Db>();
    let remote = app.state::<Remote>();
    let (base, token) = db.with(endpoint)?.ok_or("This computer isn't connected to a server.")?;
    let before = status(app);

    // Anything queued goes first, so the server sees events in the order they happened
    // and a temporary ID in `args` can be translated.
    let reachable = !remote.backing_off() && flush(app)? == Flushed::Clear;
    let resolved = db.with(|c| translate(c, &mut args))?;

    let result = if reachable && resolved {
        match http(&base, &token, cmd, &args) {
            Ok(v) => {
                remote.mark(true);
                if CACHED.contains(&cmd) {
                    let _ = db.with(|c| cache_put(c, &cache_key(cmd, &args), &v));
                }
                if cmd == "list_experiences" {
                    prefetch_open_sessions(app, &v);
                }
                Ok(v)
            }
            Err(HttpErr::Net(_)) => {
                remote.mark(false);
                offline(app, cmd, args, "Can't reach your Field Notes server.")
            }
            Err(HttpErr::Locked) => {
                remote.mark(false);
                offline(app, cmd, args, "Your Field Notes server's journal is locked.")
            }
            Err(HttpErr::Status(401, _)) => {
                let _ = db.with(|c| set_cfg(c, "unpaired", "1"));
                remote.mark(false);
                Err("The server no longer recognises this computer. Pair it again in Settings → Server.".into())
            }
            Err(HttpErr::Status(_, msg)) => Err(msg),
        }
    } else {
        offline(app, cmd, args, "Can't reach your Field Notes server.")
    };

    if status(app) != before {
        notify(app);
    }
    result
}

/// Keep the sessions still in progress on hand, so that if the server drops out
/// mid-session the next dose is still checked against everything already taken.
fn prefetch_open_sessions<R: Runtime>(app: &AppHandle<R>, list: &Value) {
    let open: Vec<i64> = list
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|e| e["kind"] == "session" && e["ended_at"].is_null())
                .filter_map(|e| e["id"].as_i64())
                .take(5)
                .collect()
        })
        .unwrap_or_default();
    if open.is_empty() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let db = app.state::<Db>();
        let Ok(Some((base, token))) = db.with(endpoint) else { return };
        for id in open {
            if let Ok(v) = http(&base, &token, "get_experience", &json!({ "id": id })) {
                let _ = db.with(|c| cache_put(c, &format!("get_experience:{id}"), &v));
            }
        }
    });
}

/// Answer without the server.
fn offline<R: Runtime>(app: &AppHandle<R>, cmd: &str, args: Value, why: &str) -> Result<Value, String> {
    let db = app.state::<Db>();
    match cmd {
        "list_experiences" => db.with(|c| Ok(overlay_list(c))),
        "get_experience" => {
            let id = args["id"].as_i64().unwrap_or(0);
            db.with(|c| Ok(overlay_detail(c, id)))?.ok_or_else(|| {
                format!("{why} This entry isn't saved on this computer, so it can't be opened until the server is back.")
            })
        }
        "list_substances" | "usage_by_substance" => {
            let cached = db.with(|c| Ok(cache_get(c, cmd)))?;
            match (cmd, cached) {
                (_, Some(v)) => Ok(v),
                ("list_substances", None) => {
                    // The bundled catalogue is on this computer too; only the user's
                    // own additions live solely on the server.
                    serde_json::to_value(commands::list_substances(db)?).map_err(|e| e.to_string())
                }
                _ => Ok(json!([])),
            }
        }
        c if QUEUEABLE.contains(&c) => db.with_mut(|conn| queue(conn, cmd, &args)).and_then(|v| v),
        "check_combo" => {
            let names: Vec<String> = serde_json::from_value(args["names"].clone()).unwrap_or_default();
            serde_json::to_value(commands::check_combo(db, names)).map_err(|e| e.to_string())
        }
        "crisis_scan" => {
            let text = args["text"].as_str().unwrap_or_default().to_string();
            let recent: Option<Vec<String>> = serde_json::from_value(args["recent"].clone()).ok();
            serde_json::to_value(commands::crisis_scan_text(text, recent))
                .map_err(|e| e.to_string())
        }
        "ollama_up" => Ok(json!(false)),
        "ollama_models" => Ok(json!([])),
        "companion_warm" => Ok(Value::Null),
        "compute_status" => Err(why.to_string()),
        "companion_chat" | "ai_status" | "ai_start" => {
            Err(format!("{why} The Companion runs on the server, so it's unavailable until it's back."))
        }
        _ => Err(format!(
            "{why} Editing and deleting need the connection. New sessions, doses and notes still save here and are sent when it's back."
        )),
    }
}

fn session_names(detail: &Value) -> Vec<String> {
    detail["doses"]
        .as_array()
        .map(|d| d.iter().filter_map(|x| x["substance_name"].as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

/// Put a new entry in the outbox and return what the server would have: the same
/// shape, a temporary ID, and — for a dose — the interaction warnings, computed here.
fn queue(conn: &mut Connection, cmd: &str, args: &Value) -> rusqlite::Result<Result<Value, String>> {
    ensure(conn)?;
    let now = now_sqlite(conn);
    let input = &args["input"];
    let (temp_id, preview, result) = match cmd {
        "create_experience" => {
            let id = next_temp_id(conn)?;
            let kind = if input["kind"] == "note" { "note" } else { "session" };
            let exp = json!({
                "id": id,
                "kind": kind,
                "title": input["title"].as_str().unwrap_or(""),
                "intention": input["intention"].as_str().unwrap_or(""),
                "setting": input["setting"].as_str().unwrap_or(""),
                "notes": "",
                "rating": null,
                "started_at": input["started_at"].as_str().unwrap_or(&now),
                "ended_at": null,
                "created_at": now,
            });
            (Some(id), exp.clone(), exp)
        }
        "log_dose" => {
            let exp_id = input["experience_id"].as_i64().unwrap_or(0);
            let detail = overlay_detail(conn, exp_id);
            if detail.as_ref().is_some_and(|d| d["kind"] == "note") {
                return Ok(Err("This entry is a plain note, not a session — it can't have doses.".into()));
            }
            let id = next_temp_id(conn)?;
            let name = input["substance_name"].as_str().unwrap_or("").to_string();
            let dose = json!({
                "id": id,
                "experience_id": exp_id,
                "substance_id": null,
                "substance_name": name,
                "amount": input["amount"],
                "unit": input["unit"].as_str().unwrap_or("mg"),
                "route": input["route"].as_str().unwrap_or(""),
                "taken_at": input["taken_at"].as_str().unwrap_or(&now),
                "note": input["note"].as_str().unwrap_or(""),
            });
            // The same question the server answers on `log_dose`, over the same
            // evidence as far as this computer knows it: the session as last seen
            // plus everything queued since, plus this dose.
            let mut names = detail.as_ref().map(session_names).unwrap_or_default();
            names.push(name);
            names.sort();
            names.dedup();
            let warnings = db::combo_warnings(conn, &names);
            (Some(id), dose.clone(), json!({ "dose": dose, "warnings": warnings }))
        }
        "add_timeline_event" => {
            let id = next_temp_id(conn)?;
            let ev = json!({
                "id": id,
                "experience_id": input["experience_id"],
                "at": input["at"].as_str().unwrap_or(&now),
                "note": input["note"].as_str().unwrap_or(""),
                "mood": input["mood"].as_str().unwrap_or(""),
                "intensity": input["intensity"],
            });
            (Some(id), ev.clone(), ev)
        }
        "end_experience" => {
            let id = args["id"].as_i64().unwrap_or(0);
            let Some(mut exp) = overlay_detail(conn, id) else {
                return Ok(Err("Can't reach your Field Notes server, and this session isn't saved on this computer.".into()));
            };
            exp["ended_at"] = args["endedAt"].clone();
            exp["rating"] = args["rating"].clone();
            exp["notes"] = args["notes"].clone();
            if let Some(o) = exp.as_object_mut() {
                o.remove("doses");
                o.remove("timeline");
            }
            (None, exp.clone(), exp)
        }
        _ => return Ok(Err(format!("`{cmd}` can't wait for the server."))),
    };
    conn.execute(
        "INSERT INTO remote_outbox (cmd, args, temp_id, preview) VALUES (?1, ?2, ?3, ?4)",
        params![cmd, args.to_string(), temp_id, preview.to_string()],
    )?;
    Ok(Ok(result))
}

/// Every ID an entry has gone by: its real one and any temporary one it started as.
fn aliases(conn: &Connection, id: i64) -> Vec<i64> {
    let mut ids = vec![id];
    let other: Option<i64> = if id < 0 {
        conn.query_row("SELECT real_id FROM remote_idmap WHERE temp_id = ?1", [id], |r| r.get(0)).optional().ok().flatten()
    } else {
        conn.query_row("SELECT temp_id FROM remote_idmap WHERE real_id = ?1", [id], |r| r.get(0)).optional().ok().flatten()
    };
    ids.extend(other);
    ids
}

type Pending = Vec<(String, Value, Value)>; // (cmd, args, preview)

fn pending(conn: &Connection) -> Pending {
    let Ok(mut stmt) = conn.prepare("SELECT cmd, args, preview FROM remote_outbox WHERE failed IS NULL ORDER BY seq") else {
        return vec![];
    };
    stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
        .map(|rows| {
            rows.filter_map(Result::ok)
                .map(|(c, a, p)| {
                    (c, serde_json::from_str(&a).unwrap_or(Value::Null), serde_json::from_str(&p).unwrap_or(Value::Null))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// An entry as this computer knows it: the server's last answer (or, for one
/// created offline, what was queued), with everything queued since applied on top.
fn overlay_detail(conn: &Connection, id: i64) -> Option<Value> {
    let _ = ensure(conn);
    let ids = aliases(conn, id);
    let queue = pending(conn);

    let mut detail = ids
        .iter()
        .filter(|i| **i > 0)
        .find_map(|i| cache_get(conn, &format!("get_experience:{i}")))
        .or_else(|| {
            queue
                .iter()
                .find(|(c, _, p)| c == "create_experience" && p["id"].as_i64().is_some_and(|x| ids.contains(&x)))
                .map(|(_, _, p)| {
                    let mut d = p.clone();
                    d["doses"] = json!([]);
                    d["timeline"] = json!([]);
                    d
                })
        })?;

    let belongs = |v: &Value| v.as_i64().is_some_and(|x| ids.contains(&x));
    for (cmd, args, preview) in &queue {
        match cmd.as_str() {
            "log_dose" if belongs(&args["input"]["experience_id"]) => {
                if let Some(a) = detail["doses"].as_array_mut() {
                    a.push(preview.clone());
                }
            }
            "add_timeline_event" if belongs(&args["input"]["experience_id"]) => {
                if let Some(a) = detail["timeline"].as_array_mut() {
                    a.push(preview.clone());
                }
            }
            "end_experience" if belongs(&args["id"]) => {
                detail["ended_at"] = args["endedAt"].clone();
                detail["rating"] = args["rating"].clone();
                detail["notes"] = args["notes"].clone();
            }
            _ => {}
        }
    }
    Some(detail)
}

/// The journal list as this computer knows it: the server's last answer with
/// entries created since shown on top.
fn overlay_list(conn: &Connection) -> Value {
    let _ = ensure(conn);
    let mut list: Vec<Value> = cache_get(conn, "list_experiences")
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    let queue = pending(conn);
    for (cmd, _, preview) in queue.iter().filter(|(c, _, _)| c == "create_experience") {
        let _ = cmd;
        let id = preview["id"].as_i64().unwrap_or(0);
        let detail = overlay_detail(conn, id).unwrap_or_else(|| preview.clone());
        let mut names = session_names(&detail);
        let count = names.len();
        names.sort();
        names.dedup();
        let mut summary = preview.clone();
        summary["substances"] = json!(names);
        summary["dose_count"] = json!(count);
        summary["ended_at"] = detail["ended_at"].clone();
        list.insert(0, summary);
    }
    // Queued doses into sessions the server already has: keep the count honest.
    for item in list.iter_mut() {
        let Some(id) = item["id"].as_i64() else { continue };
        if id < 0 {
            continue;
        }
        let ids = aliases(conn, id);
        let extra = queue
            .iter()
            .filter(|(c, a, _)| c == "log_dose" && a["input"]["experience_id"].as_i64().is_some_and(|x| ids.contains(&x)))
            .count() as i64;
        if extra > 0 {
            item["dose_count"] = json!(item["dose_count"].as_i64().unwrap_or(0) + extra);
        }
        if let Some((_, a, _)) = queue.iter().rev().find(|(c, a, _)| c == "end_experience" && a["id"].as_i64().is_some_and(|x| ids.contains(&x))) {
            item["ended_at"] = a["endedAt"].clone();
        }
    }
    Value::Array(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let dir = std::env::temp_dir().join(format!("fn-remote-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{:?}.db", std::thread::current().id()));
        let _ = std::fs::remove_file(&path);
        let c = db::open(&path, None).unwrap();
        ensure(&c).unwrap();
        c
    }

    #[test]
    fn a_pairing_link_splits_into_server_and_token() {
        let t = "a".repeat(64);
        let (base, token) = parse_link(&format!("https://isaac.tail1234.ts.net/m#t={t}")).unwrap();
        assert_eq!(base, "https://isaac.tail1234.ts.net");
        assert_eq!(token, t);
        let (base, _) = parse_link(&format!(" https://isaac.tail1234.ts.net:8443/m/#t={t} ")).unwrap();
        assert_eq!(base, "https://isaac.tail1234.ts.net:8443");
        assert_eq!(short_name(&base), "isaac");
    }

    #[test]
    fn a_link_over_plain_http_to_another_machine_is_refused() {
        let t = "b".repeat(64);
        assert!(parse_link(&format!("http://isaac.tail1234.ts.net/m#t={t}")).is_err());
        assert!(parse_link(&format!("http://127.0.0.1:8787/m#t={t}")).is_ok(), "loopback is fine");
        assert!(parse_link("https://isaac.tail1234.ts.net/m").is_err(), "no token");
        assert!(parse_link("https://isaac.tail1234.ts.net/m#t=short").is_err());
    }

    #[test]
    fn temporary_ids_are_negative_and_never_reused() {
        let c = conn();
        let a = next_temp_id(&c).unwrap();
        let b = next_temp_id(&c).unwrap();
        assert!(a < 0 && b < a);
        c.execute("DELETE FROM remote_outbox", []).unwrap();
        assert!(next_temp_id(&c).unwrap() < b, "an ID the UI may still hold must not come back");
    }

    #[test]
    fn a_session_started_offline_can_be_opened_and_logged_into_offline() {
        let mut c = conn();
        let exp = queue(&mut c, "create_experience", &json!({ "input": { "started_at": "2026-09-25 20:00:00" } }))
            .unwrap()
            .unwrap();
        let id = exp["id"].as_i64().unwrap();
        assert!(id < 0);

        let r = queue(
            &mut c,
            "log_dose",
            &json!({ "input": { "experience_id": id, "substance_name": "Alcohol", "amount": 2, "unit": "drinks", "taken_at": "2026-09-25 20:05:00" } }),
        )
        .unwrap()
        .unwrap();
        assert!(r["dose"]["id"].as_i64().unwrap() < 0);

        let detail = overlay_detail(&c, id).expect("an offline session is readable offline");
        assert_eq!(detail["doses"].as_array().unwrap().len(), 1);
        let list = overlay_list(&c);
        assert_eq!(list[0]["id"], json!(id));
        assert_eq!(list[0]["dose_count"], json!(1));
    }

    /// The point of running the checker here: a dangerous combination logged while
    /// the server is unreachable is flagged exactly as it would be online.
    #[test]
    fn a_dangerous_combination_logged_offline_is_still_flagged() {
        let mut c = conn();
        // The session as last seen online: one dose already in it.
        cache_put(
            &c,
            "get_experience:7",
            &json!({ "id": 7, "kind": "session", "title": "", "ended_at": null,
                     "doses": [{ "id": 70, "experience_id": 7, "substance_name": "Alcohol" }], "timeline": [] }),
        )
        .unwrap();
        let r = queue(
            &mut c,
            "log_dose",
            &json!({ "input": { "experience_id": 7, "substance_name": "Heroin", "taken_at": "2026-09-25 21:00:00" } }),
        )
        .unwrap()
        .unwrap();
        let warnings = r["warnings"].as_array().unwrap();
        assert!(
            warnings.iter().any(|w| w["severity"] == "danger"),
            "alcohol + heroin must be flagged offline too: {warnings:?}"
        );
    }

    #[test]
    fn a_queued_dose_into_a_note_is_refused_up_front() {
        let mut c = conn();
        let note = queue(&mut c, "create_experience", &json!({ "input": { "kind": "note", "started_at": "2026-09-25" } }))
            .unwrap()
            .unwrap();
        let r = queue(
            &mut c,
            "log_dose",
            &json!({ "input": { "experience_id": note["id"], "substance_name": "Caffeine", "taken_at": "2026-09-25" } }),
        )
        .unwrap();
        assert!(r.is_err());
    }

    #[test]
    fn temporary_ids_are_translated_once_the_server_has_assigned_one() {
        let c = conn();
        c.execute("INSERT INTO remote_idmap (temp_id, real_id) VALUES (-3, 42)", []).unwrap();
        let mut args = json!({ "input": { "experience_id": -3 } });
        assert!(translate(&c, &mut args).unwrap());
        assert_eq!(args["input"]["experience_id"], json!(42));

        let mut waiting = json!({ "id": -9 });
        assert!(!translate(&c, &mut waiting).unwrap(), "an unsent entry can't be translated yet");

        // And an entry now known by its real ID still picks up doses queued against
        // its temporary one.
        assert_eq!(aliases(&c, 42), vec![42, -3]);
    }

    #[test]
    fn edits_are_never_queued() {
        for cmd in ["update_experience", "update_dose", "delete_experience", "delete_dose", "add_substance"] {
            assert!(!QUEUEABLE.contains(&cmd), "`{cmd}` must not wait in the outbox");
        }
    }

    /// A mock Tauri app with its own journal — one per "computer" in the test.
    fn computer(tag: &str) -> tauri::App<tauri::test::MockRuntime> {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        let dir = std::env::temp_dir().join(format!("fn-remote-e2e-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("journal.db");
        for ext in ["db", "db-wal", "db-shm", "devices.json"] {
            let _ = std::fs::remove_file(path.with_extension(ext));
        }
        let conn = db::open(&path, None).unwrap();
        app.manage(Db::new(Some(conn), path.clone()));
        app.manage(crate::Knowledge(None));
        app.manage(crate::portal::Portal::default());
        app.manage(crate::portal::CompanionJobs::default());
        app.manage(crate::devices::Devices::load(path.with_extension("devices.json")));
        app.manage(Remote::default());
        app
    }

    /// The whole promise, end to end over real HTTP: a laptop connected to a server
    /// loses it mid-session, keeps logging, and everything lands on the server, in
    /// order, with the offline session's doses attached to it, once it's back.
    #[test]
    fn entries_logged_while_the_server_is_away_arrive_when_it_returns() {
        let server = computer("server");
        let laptop = computer("laptop");
        let (s, l) = (server.handle(), laptop.handle());

        let port = crate::portal::start(s).unwrap().port.unwrap();
        let (_, token) = s.state::<crate::devices::Devices>().pair("Laptop").unwrap();
        connect(l, &format!("http://127.0.0.1:{port}/m#t={token}")).expect("connects");

        // Online: a session straight onto the server.
        let online = call(l, "create_experience", json!({ "input": { "started_at": "2026-09-25 20:00:00" } })).unwrap();
        let online_id = online["id"].as_i64().unwrap();
        assert!(online_id > 0);
        call(l, "get_experience", json!({ "id": online_id })).unwrap(); // cached

        // The server goes away.
        s.state::<crate::portal::Portal>().stop();
        std::thread::sleep(Duration::from_millis(200));

        let r = call(l, "log_dose", json!({ "input": { "experience_id": online_id, "substance_name": "Alcohol", "taken_at": "2026-09-25 20:10:00" } })).unwrap();
        assert!(r["dose"]["id"].as_i64().unwrap() < 0, "queued, not lost");
        let offline_session = call(l, "create_experience", json!({ "input": { "started_at": "2026-09-25 21:00:00" } })).unwrap();
        let temp = offline_session["id"].as_i64().unwrap();
        call(l, "log_dose", json!({ "input": { "experience_id": temp, "substance_name": "Caffeine", "taken_at": "2026-09-25 21:05:00" } })).unwrap();
        assert!(call(l, "delete_experience", json!({ "id": online_id })).is_err(), "edits wait for the server");
        assert_eq!(status(l).pending, 3);
        assert!(!status(l).online);

        // It comes back (perhaps on another loopback port).
        let port = crate::portal::start(s).unwrap().port.unwrap();
        l.state::<Db>().with(|c| set_cfg(c, "url", &format!("http://127.0.0.1:{port}"))).unwrap();
        *l.state::<Remote>().last_failure.lock().unwrap() = None;
        tick(l);
        let st = status(l);
        assert_eq!(st.pending, 0, "everything sent: {st:?}");
        assert!(st.failed.is_empty(), "{:?}", st.failed);

        // On the server itself: both sessions, each with its dose.
        let sdb = s.state::<Db>();
        let first = sdb.with(|c| db::get_experience(c, online_id)).unwrap();
        assert_eq!(first.doses.len(), 1);
        assert_eq!(first.doses[0].substance_name, "Alcohol");
        let all = sdb.with(db::list_experiences).unwrap();
        assert_eq!(all.len(), 2);

        // And the laptop, still holding the temporary ID, reaches the real entry.
        let d = call(l, "get_experience", json!({ "id": temp })).unwrap();
        assert!(d["id"].as_i64().unwrap() > 0);
        assert_eq!(d["doses"][0]["substance_name"], "Caffeine");
    }

    /// The frontend decides what to send to `remote_call`; the backend refuses
    /// anything else. The two lists must agree, or a command silently runs against
    /// the local (cache-only) journal instead of the server.
    #[test]
    fn the_frontend_routes_exactly_what_the_backend_accepts() {
        let ts = include_str!("../../src/lib/api.ts");
        let start = ts.find("const ROUTED = new Set([").expect("ROUTED in api.ts");
        let block = &ts[start..start + ts[start..].find("]);").unwrap()];
        let mut front: Vec<&str> = block.split('"').skip(1).step_by(2).collect();
        let mut back: Vec<&str> = ROUTED.to_vec();
        front.sort();
        back.sort();
        assert_eq!(front, back);
    }

    /// "Sync journal to server": a laptop's own entries arrive on the server whole —
    /// doses, timeline, write-up, a blank title kept blank — and a second sync sends
    /// nothing, even after disconnecting and reconnecting.
    #[test]
    fn syncing_copies_this_computers_journal_once() {
        let server = computer("sync-server");
        let laptop = computer("sync-laptop");
        let (s, l) = (server.handle(), laptop.handle());

        // Written on the laptop before it ever knew about the server.
        l.state::<Db>()
            .with(|c| {
                let e = db::create_experience(c, &serde_json::from_value(json!({ "started_at": "2026-08-01 20:00:00" })).unwrap())?;
                db::log_dose(c, &serde_json::from_value(json!({ "experience_id": e.id, "substance_name": "Caffeine", "amount": 100.0, "taken_at": "2026-08-01 20:05:00" })).unwrap())?;
                db::add_timeline_event(c, &serde_json::from_value(json!({ "experience_id": e.id, "at": "2026-08-01 20:30:00", "note": "focused" })).unwrap())?;
                db::update_experience(c, e.id, &serde_json::from_value(json!({ "title": "", "notes": "a good evening", "rating": 4, "started_at": "2026-08-01 20:00:00", "ended_at": "2026-08-01 23:00:00" })).unwrap())?;
                db::create_experience(c, &serde_json::from_value(json!({ "kind": "note", "title": "Thoughts", "started_at": "2026-08-02 09:00:00" })).unwrap())
            })
            .unwrap();

        let port = crate::portal::start(s).unwrap().port.unwrap();
        let (_, token) = s.state::<crate::devices::Devices>().pair("Laptop").unwrap();
        let link = format!("http://127.0.0.1:{port}/m#t={token}");
        connect(l, &link).unwrap();
        assert_eq!(status(l).local_unsynced, 2);

        let r = upload_local(l).expect("syncs");
        assert_eq!(r.copied, 2);
        assert_eq!(r.status.local_unsynced, 0);

        let sdb = s.state::<Db>();
        let all = sdb.with(db::list_experiences).unwrap();
        assert_eq!(all.len(), 2);
        let session = all.iter().find(|e| e.experience.kind == "session").unwrap();
        let d = sdb.with(|c| db::get_experience(c, session.experience.id)).unwrap();
        assert_eq!(d.doses.len(), 1);
        assert_eq!(d.timeline.len(), 1);
        assert_eq!(d.experience.notes, "a good evening");
        assert_eq!(d.experience.rating, Some(4));
        assert_eq!(d.experience.title, "", "a title left blank stays blank");
        assert!(d.experience.ended_at.is_some());

        // Again, and after a disconnect/reconnect: nothing new, nothing doubled.
        assert_eq!(upload_local(l).unwrap().copied, 0);
        disconnect(l, false).unwrap();
        connect(l, &link).unwrap();
        assert_eq!(status(l).local_unsynced, 0);
        assert_eq!(upload_local(l).unwrap().copied, 0);
        assert_eq!(sdb.with(db::list_experiences).unwrap().len(), 2);
    }

    /// The bug this guards against: a laptop whose journal already holds entries the
    /// server has (written on both, or copied by a sync this laptop has since
    /// forgotten) must not send them again — and duplicates already on the server
    /// from an older version are cleared away when syncing.
    #[test]
    fn syncing_never_copies_an_entry_the_server_already_has() {
        let server = computer("dedupe-server");
        let laptop = computer("dedupe-laptop");
        let (s, l) = (server.handle(), laptop.handle());

        let write = |c: &Connection, title: &str, note: &str| -> rusqlite::Result<i64> {
            let e = db::create_experience(c, &serde_json::from_value(json!({ "title": title, "started_at": "2026-08-01 20:00:00" })).unwrap())?;
            db::log_dose(c, &serde_json::from_value(json!({ "experience_id": e.id, "substance_name": "Caffeine", "amount": 100.0, "taken_at": "2026-08-01 20:05:00" })).unwrap())?;
            db::add_timeline_event(c, &serde_json::from_value(json!({ "experience_id": e.id, "at": "2026-08-01 20:30:00", "note": note })).unwrap())?;
            Ok(e.id)
        };
        // The same evening on both computers, twice on the server already (an older
        // sync), plus one entry that exists only on the laptop.
        s.state::<Db>().with(|c| { write(c, "Evening", "focused")?; write(c, "Evening", "focused") }).unwrap();
        l.state::<Db>().with(|c| { write(c, "Evening", "focused")?; write(c, "Evening", "focused")?; write(c, "Evening", "scattered") }).unwrap();

        let port = crate::portal::start(s).unwrap().port.unwrap();
        let (_, token) = s.state::<crate::devices::Devices>().pair("Laptop").unwrap();
        connect(l, &format!("http://127.0.0.1:{port}/m#t={token}")).unwrap();
        assert_eq!(status(l).local_unsynced, 3);

        let r = upload_local(l).expect("syncs");
        assert_eq!(r.copied, 1, "only the entry the server didn't have");
        assert_eq!(r.skipped, 2, "both local copies of the shared evening are recognised");
        assert_eq!(r.removed, 1, "the server's own duplicate is cleared");
        assert_eq!(r.status.local_unsynced, 0);

        let sdb = s.state::<Db>();
        let all = sdb.with(db::list_experiences).unwrap();
        assert_eq!(all.len(), 2, "one 'focused' evening and one 'scattered': {all:?}");

        // Forget every sync record (as a reconnect under a new address would) and
        // sync again: still nothing doubled.
        l.state::<Db>().with(|c| c.execute("DELETE FROM remote_synced", [])).unwrap();
        let r = upload_local(l).unwrap();
        assert_eq!((r.copied, r.skipped), (0, 3));
        assert_eq!(sdb.with(db::list_experiences).unwrap().len(), 2);
    }

    #[test]
    fn everything_routed_is_something_the_server_exposes() {
        for cmd in ROUTED {
            assert!(crate::portal::EXPOSED.contains(cmd), "`{cmd}` is routed but the server refuses it");
        }
    }
}
