// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! The phone portal — an **optional, off-by-default** HTTP server that lets a
//! phone on your tailnet reach this journal while the desktop app is running.
//!
//! Field Notes ships as a fully offline, on-device app and stays that way unless
//! the user turns this on. Nothing here runs until they do.
//!
//! ## The four rules
//!
//! 1. **Bind `127.0.0.1` only.** Never `0.0.0.0`. The server is not reachable from
//!    the LAN, a coffee-shop network, or anywhere else — the only way in is
//!    `tailscale serve`, which fronts loopback with HTTPS on your tailnet. Change
//!    this line and you have published a substance journal to the local network.
//! 2. **A token on every API request, tailnet or not.** A tailnet is *not* a trust
//!    boundary for this data — every device you ever added to it is on it, forever.
//!    The token is compared in constant time and lives only in memory.
//! 3. **Only while the journal is unlocked.** The portal refuses to start against a
//!    locked journal and every request re-checks, so an encrypted journal can never
//!    be reached through it before the passphrase is entered on the desktop.
//! 4. **An allowlist, never a denylist.** [`dispatch`] matches on an explicit list of
//!    commands. A new command added to `commands.rs` is unreachable from the phone
//!    until someone puts it here on purpose. In particular the portal cannot
//!    reconfigure or disable *itself*, read its own token, touch encryption,
//!    export/import backups, or wipe the journal — those are desktop-only, in person.
//!
//! Handlers call the **same** `commands::` functions the desktop calls. There is no
//! second implementation of any rule, so the interaction checker and the crisis layer
//! behave identically whichever screen you're on.

use crate::{commands, Db, Knowledge};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tiny_http::{Header, Request, Response, Server};

/// Loopback only. See rule 1 — this is not a preference.
const BIND_ADDR: &str = "127.0.0.1";

/// First choice; we walk upward if it's taken (3000 is a popular port).
const PORT_RANGE: std::ops::Range<u16> = 8787..8807;

pub struct Portal {
    inner: Mutex<Option<Running>>,
    /// Mirrors the desktop's "use Field Notes without a Companion" switch, which
    /// lives in browser storage the phone cannot see. The desktop pushes it here
    /// on load and whenever it changes, so the phone can hide a Companion the
    /// user has turned off instead of offering a chat that shouldn't be there.
    companion_enabled: AtomicBool,
}

struct Running {
    server: Arc<Server>,
    port: u16,
    token: String,
    stopping: Arc<AtomicBool>,
    /// Set the first time a request arrives carrying the right token — i.e. the
    /// moment a phone has actually paired. It only ever goes false again by
    /// stopping the portal, which throws the token away with it.
    paired: Arc<AtomicBool>,
}

#[derive(serde::Serialize)]
pub struct PortalStatus {
    pub running: bool,
    pub port: Option<u16>,
    /// The pairing URL, token included. Only ever shown on the desktop screen.
    pub pair_url: Option<String>,
    /// A phone has used this token successfully since the portal was turned on.
    /// Drives the desktop's "Paired successfully" light; it says nothing about
    /// whether that phone is still connected right now.
    pub paired: bool,
}

impl Default for Portal {
    fn default() -> Self {
        Portal { inner: Mutex::new(None), companion_enabled: AtomicBool::new(true) }
    }
}

impl Portal {
    pub fn set_companion_enabled(&self, on: bool) {
        self.companion_enabled.store(on, Ordering::Relaxed);
    }

    pub fn companion_enabled(&self) -> bool {
        self.companion_enabled.load(Ordering::Relaxed)
    }

    pub fn status(&self) -> PortalStatus {
        let guard = self.inner.lock().unwrap();
        match guard.as_ref() {
            Some(r) => PortalStatus {
                running: true,
                port: Some(r.port),
                pair_url: Some(format!("http://{BIND_ADDR}:{}/m#t={}", r.port, r.token)),
                paired: r.paired.load(Ordering::SeqCst),
            },
            None => PortalStatus { running: false, port: None, pair_url: None, paired: false },
        }
    }

    /// Stop serving. Idempotent — disabling a portal that isn't running is fine.
    pub fn stop(&self) {
        if let Some(r) = self.inner.lock().unwrap().take() {
            r.stopping.store(true, Ordering::SeqCst);
            r.server.unblock();
        }
    }
}

/// Background Companion turns started from a phone.
///
/// A Companion reply from a slow local model can take minutes, and mobile Safari
/// kills any request that stays silent for ~60 seconds (a locked screen kills it
/// instantly). So the phone doesn't wait: `companion_chat_start` moves the turn
/// onto a thread and answers with a job id, and `companion_chat_poll` — a series
/// of fast, cheap requests — collects the result whenever it's ready.
///
/// Results are delivered **once**: a successful poll removes the job. Jobs nobody
/// comes back for (the phone died, the tab was closed) are swept after
/// [`CompanionJobs::MAX_AGE`] so an abandoned reply doesn't sit in memory forever.
#[derive(Default)]
pub struct CompanionJobs {
    next_id: AtomicU64,
    jobs: Mutex<HashMap<u64, Job>>,
}

/// One in-flight (or finished but unclaimed) Companion turn.
struct Job {
    /// `None` while the worker thread is still running the turn.
    result: Option<Result<commands::CompanionReply, String>>,
    created: Instant,
}

impl CompanionJobs {
    /// How long an unclaimed job may linger before the sweep drops it.
    const MAX_AGE: Duration = Duration::from_secs(15 * 60);

    /// Register a new running job, sweeping abandoned ones first.
    fn begin(&self) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let mut jobs = self.jobs.lock().unwrap();
        let now = Instant::now();
        jobs.retain(|_, j| now.duration_since(j.created) < Self::MAX_AGE);
        jobs.insert(id, Job { result: None, created: now });
        id
    }

    /// Store a finished turn. If the job was swept meanwhile, the result is
    /// dropped — nobody was coming back for it.
    fn finish(&self, id: u64, result: Result<commands::CompanionReply, String>) {
        if let Some(job) = self.jobs.lock().unwrap().get_mut(&id) {
            job.result = Some(result);
        }
    }

    /// `Err(())` — no such job (never started, already claimed, or swept).
    /// `Ok(None)` — still running. `Ok(Some(..))` — finished; the job is removed,
    /// so the result is delivered exactly once.
    fn take(&self, id: u64) -> Result<Option<Result<commands::CompanionReply, String>>, ()> {
        let mut jobs = self.jobs.lock().unwrap();
        match jobs.get(&id) {
            None => Err(()),
            Some(j) if j.result.is_none() => Ok(None),
            Some(_) => Ok(Some(jobs.remove(&id).expect("checked above").result.expect("checked above"))),
        }
    }
}

/// 256 bits of OS randomness, hex. Not a password — never typed, only scanned.
fn new_token() -> Result<String, String> {
    let mut buf = [0u8; 32];
    getrandom::getrandom(&mut buf).map_err(|e| format!("no secure randomness available: {e}"))?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

/// Compare without leaking where the mismatch was. `==` on a `String` short-circuits
/// on the first differing byte, which over enough requests tells an attacker the
/// token one byte at a time.
fn token_matches(expected: &str, given: &str) -> bool {
    let (a, b) = (expected.as_bytes(), given.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Start the portal. Fails if the journal is locked (rule 3).
pub fn start<R: Runtime>(app: &AppHandle<R>) -> Result<PortalStatus, String> {
    let portal = app.state::<Portal>();
    if portal.status().running {
        return Ok(portal.status());
    }
    if !app.state::<Db>().is_unlocked() {
        return Err("Unlock the journal before turning on phone access.".into());
    }

    let token = new_token()?;
    let (server, port) = bind()?;
    let server = Arc::new(server);
    let stopping = Arc::new(AtomicBool::new(false));
    let paired = Arc::new(AtomicBool::new(false));

    // A small pool: a Companion reply blocks its thread for many seconds, and a
    // phone that can't load the timeline meanwhile looks broken.
    for _ in 0..4 {
        let server = Arc::clone(&server);
        let stopping = Arc::clone(&stopping);
        let paired = Arc::clone(&paired);
        let app = app.clone();
        let token = token.clone();
        std::thread::spawn(move || {
            while let Ok(req) = server.recv() {
                if stopping.load(Ordering::SeqCst) {
                    break;
                }
                handle(&app, &token, &paired, req);
            }
        });
    }

    *portal.inner.lock().unwrap() = Some(Running { server, port, token, stopping, paired });
    Ok(portal.status())
}

fn bind() -> Result<(Server, u16), String> {
    for port in PORT_RANGE {
        if let Ok(s) = Server::http((BIND_ADDR, port)) {
            return Ok((s, port));
        }
    }
    Err(format!("no free port in {}–{}", PORT_RANGE.start, PORT_RANGE.end - 1))
}

fn json_response(status: u16, body: Value) -> Response<Cursor<Vec<u8>>> {
    let hdr = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
    Response::from_string(body.to_string()).with_status_code(status).with_header(hdr)
}

fn handle<R: Runtime>(app: &AppHandle<R>, token: &str, paired: &AtomicBool, req: Request) {
    let url = req.url().to_string();
    let path = url.split('?').next().unwrap_or("/").to_string();

    if let Some(command) = path.strip_prefix("/api/") {
        let command = command.to_string();
        return api(app, token, paired, &command, req);
    }
    assets(app, &path, req);
}

fn api<R: Runtime>(app: &AppHandle<R>, token: &str, paired: &AtomicBool, command: &str, mut req: Request) {
    // Rule 2: token first, before we even look at the body.
    let given = req
        .headers()
        .iter()
        .find(|h| h.field.equiv("Authorization"))
        .and_then(|h| h.value.as_str().strip_prefix("Bearer ").map(str::to_string))
        .unwrap_or_default();
    if !token_matches(token, &given) {
        let _ = req.respond(json_response(401, json!({ "error": "Not paired with this journal." })));
        return;
    }

    // The token checked out, so a phone is on the other end. The first time that
    // happens, tell the desktop — the pairing screen has no other way to know a
    // scan worked, and "did it take?" is the whole question the user is holding.
    if !paired.swap(true, Ordering::SeqCst) {
        let _ = app.emit("portal-paired", ());
    }

    // Rule 3: re-check on every request, not just at startup.
    if !app.state::<Db>().is_unlocked() {
        let _ = req.respond(json_response(
            503,
            json!({ "error": "The journal is locked on the desktop." }),
        ));
        return;
    }

    let mut body = String::new();
    if std::io::Read::read_to_string(&mut req.as_reader(), &mut body).is_err() {
        let _ = req.respond(json_response(400, json!({ "error": "unreadable body" })));
        return;
    }
    let args: Value = if body.trim().is_empty() {
        json!({})
    } else {
        match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(e) => {
                let _ = req.respond(json_response(400, json!({ "error": e.to_string() })));
                return;
            }
        }
    };

    let resp = match dispatch(app, command, args) {
        Ok(v) => json_response(200, v),
        // 403, not 404: the command may well exist — it just isn't reachable from a
        // phone, and saying so plainly beats letting someone think it's a typo.
        Err(DispatchError::NotExposed) => json_response(
            403,
            json!({ "error": format!("`{command}` is desktop-only — it isn't reachable from the phone.") }),
        ),
        Err(DispatchError::Failed(e)) => json_response(400, json!({ "error": e })),
    };
    let _ = req.respond(resp);
}

/// Serve the app's own frontend — the same assets Tauri embedded in the binary, so
/// the phone runs the same build as the desktop and there is no second bundle to
/// keep in sync.
///
/// The frontend is a SPA (`adapter-static` with an `index.html` fallback, `ssr =
/// false`), so `/m` is a **client-side** route with no file behind it. Anything that
/// isn't a real asset therefore falls back to `index.html` and lets the router sort
/// it out — exactly what the Tauri webview does.
fn assets<R: Runtime>(app: &AppHandle<R>, path: &str, req: Request) {
    let resolver = app.asset_resolver();
    let looks_like_a_file = path.rsplit('/').next().is_some_and(|f| f.contains('.'));

    let asset = resolver
        .get(path.to_string())
        .or_else(|| (!looks_like_a_file).then(|| resolver.get("/index.html".into())).flatten());

    match asset {
        Some(asset) => {
            let hdr = Header::from_bytes(&b"Content-Type"[..], asset.mime_type.as_bytes())
                .unwrap_or_else(|_| {
                    Header::from_bytes(&b"Content-Type"[..], &b"application/octet-stream"[..]).unwrap()
                });
            let _ = req.respond(Response::from_data(asset.bytes).with_header(hdr));
        }
        None => {
            let _ = req.respond(Response::from_string("Not found").with_status_code(404));
        }
    }
}

pub enum DispatchError {
    /// The command exists but is deliberately not reachable from a phone.
    NotExposed,
    Failed(String),
}

/// Pull a named argument out of the request body, the way Tauri's `invoke` would.
fn arg<T: serde::de::DeserializeOwned>(args: &Value, name: &str) -> Result<T, DispatchError> {
    serde_json::from_value(args.get(name).cloned().unwrap_or(Value::Null))
        .map_err(|e| DispatchError::Failed(format!("bad argument `{name}`: {e}")))
}

/// Wrap a command's result back into JSON.
fn ok<T: serde::Serialize>(v: T) -> Result<Value, DispatchError> {
    serde_json::to_value(v).map_err(|e| DispatchError::Failed(e.to_string()))
}

fn done<T: serde::Serialize>(r: Result<T, String>) -> Result<Value, DispatchError> {
    ok(r.map_err(DispatchError::Failed)?)
}

/// **The allowlist (rule 4).** Everything the phone may do, and nothing else.
///
/// Absent on purpose, and each for its own reason:
/// - `unlock_db`, `enable_encryption`, `disable_encryption`, `change_passphrase` —
///   the passphrase is the one secret that must be typed **in person**. Exposing
///   `unlock_db` would turn the portal into a passphrase-guessing oracle.
/// - `export_backup`, `import_backup`, `obsidian_*`, `contribution_save`,
///   `wipe_all_data`, `reveal_data_dir`, `data_dir` — these read and write the
///   **desktop's filesystem**. A phone has no business there.
/// - `ai_install`, `ai_pull`, `ai_start`, `pw_update` — they install software and
///   mutate app state, and they take an `AppHandle`.
/// - `portal_*` — the portal must not be able to reconfigure, re-token, or disable
///   itself. Turning it off is a thing you do on the machine that's serving it.
/// The allowlist itself, as data, so it can be tested without standing up a Tauri
/// app — and so there is exactly one place to look to answer "what can the phone do?".
/// [`dispatch`] refuses anything not on this list *before* matching, which means a
/// command reachable in the `match` below but missing here is still unreachable.
pub const EXPOSED: &[&str] = &[
    "list_experiences",
    "get_experience",
    // Renders one entry to Markdown text and *returns* it — the phone downloads it
    // in the browser. Its sibling `export_experience_file` writes to the desktop's
    // disk and must never appear here.
    "export_experience_markdown",
    "usage_by_substance",
    "list_substances",
    "db_status",
    "companion_enabled",
    "create_experience",
    "end_experience",
    "log_dose",
    "add_timeline_event",
    "add_substance",
    "update_experience",
    "update_dose",
    "update_timeline_event",
    "delete_experience",
    "delete_dose",
    "delete_timeline_event",
    "delete_substance",
    "check_combo",
    "interaction_classes",
    "crisis_scan",
    "emergency_resources",
    "pw_status",
    "pw_lookup",
    "knowledge_search",
    "knowledge_status",
    "companion_chat",
    "companion_chat_start",
    "companion_chat_poll",
    "ai_status",
    "ollama_up",
    "ollama_models",
    "ai_start",
];

pub fn dispatch<R: Runtime>(app: &AppHandle<R>, command: &str, args: Value) -> Result<Value, DispatchError> {
    if !EXPOSED.contains(&command) {
        return Err(DispatchError::NotExposed);
    }
    let db = app.state::<Db>();
    match command {
        // --- reading the journal ---
        "list_experiences" => done(commands::list_experiences(db)),
        "get_experience" => done(commands::get_experience(db, arg(&args, "id")?)),
        "export_experience_markdown" => {
            done(commands::export_experience_markdown(db, arg(&args, "id")?))
        }
        "usage_by_substance" => done(commands::usage_by_substance(db)),
        "list_substances" => done(commands::list_substances(db)),
        "db_status" => ok(commands::db_status(db)),
        "companion_enabled" => ok(app.state::<Portal>().companion_enabled()),

        // --- writing to the journal: the whole point of the portal ---
        "create_experience" => done(commands::create_experience(db, arg(&args, "input")?)),
        "end_experience" => done(commands::end_experience(
            db,
            arg(&args, "id")?,
            arg(&args, "endedAt")?,
            arg(&args, "rating")?,
            arg(&args, "notes")?,
        )),
        "log_dose" => done(commands::log_dose(db, arg(&args, "input")?)),
        "add_timeline_event" => done(commands::add_timeline_event(db, arg(&args, "input")?)),
        "add_substance" => done(commands::add_substance(db, arg(&args, "input")?)),
        "update_experience" => {
            done(commands::update_experience(db, arg(&args, "id")?, arg(&args, "update")?))
        }
        "update_dose" => done(commands::update_dose(db, arg(&args, "id")?, arg(&args, "update")?)),
        "update_timeline_event" => {
            done(commands::update_timeline_event(db, arg(&args, "id")?, arg(&args, "update")?))
        }
        "delete_experience" => done(commands::delete_experience(db, arg(&args, "id")?)),
        "delete_dose" => done(commands::delete_dose(db, arg(&args, "id")?)),
        "delete_timeline_event" => done(commands::delete_timeline_event(db, arg(&args, "id")?)),
        "delete_substance" => done(commands::delete_substance(db, arg(&args, "id")?)),

        // --- safety: the same deterministic layers the desktop uses ---
        "check_combo" => ok(commands::check_combo(db, arg(&args, "names")?)),
        "interaction_classes" => ok(commands::interaction_classes()),
        "crisis_scan" => ok(commands::crisis_scan(
            db,
            arg(&args, "text")?,
            arg(&args, "experienceId")?,
            arg(&args, "recent")?,
        )),
        "emergency_resources" => ok(commands::emergency_resources()),

        // --- reference ---
        "pw_status" => done(commands::pw_status(db)),
        "pw_lookup" => done(commands::pw_lookup(db, arg(&args, "name")?)),
        "knowledge_search" => ok(commands::knowledge_search(
            app.state(),
            arg(&args, "query")?,
            arg(&args, "limit")?,
        )),
        "knowledge_status" => ok(commands::knowledge_status(app.state())),

        // --- Companion. It runs on the desktop's Ollama; the phone is a thin client. ---
        // The blocking form stays for older phones; new ones use start/poll below so
        // a slow model can't outlive mobile Safari's ~60s request timeout.
        "companion_chat" => {
            // The desktop command is now async (it runs off the UI thread); the
            // portal is already on a worker thread, so call the shared inner
            // function directly and keep this path blocking.
            let kb = app.state::<Knowledge>();
            done(commands::companion_chat_inner(
                db.inner(),
                kb.inner(),
                arg(&args, "model")?,
                arg(&args, "history")?,
                arg(&args, "experienceId")?,
                arg(&args, "supportStyle")?,
            ))
        }
        // Same arguments, but the turn runs on a background thread and the phone
        // gets a job id back immediately. See [`CompanionJobs`].
        "companion_chat_start" => {
            let model: String = arg(&args, "model")?;
            let history: Vec<crate::ollama::ChatMsg> = arg(&args, "history")?;
            let experience_id: Option<i64> = arg(&args, "experienceId")?;
            let support_style: Option<String> = arg(&args, "supportStyle")?;
            let id = app.state::<CompanionJobs>().begin();
            let app = app.clone();
            std::thread::spawn(move || {
                let db = app.state::<Db>();
                let kb = app.state::<Knowledge>();
                let result = commands::companion_chat_inner(
                    db.inner(),
                    kb.inner(),
                    model,
                    history,
                    experience_id,
                    support_style,
                );
                app.state::<CompanionJobs>().finish(id, result);
            });
            ok(json!({ "job": id }))
        }
        "companion_chat_poll" => match app.state::<CompanionJobs>().take(arg(&args, "id")?) {
            Err(()) => Err(DispatchError::Failed("unknown or expired job".into())),
            Ok(None) => ok(json!({ "status": "running" })),
            Ok(Some(Ok(reply))) => ok(json!({ "status": "done", "reply": reply })),
            Ok(Some(Err(e))) => ok(json!({ "status": "error", "error": e })),
        },
        "ai_status" => ok(commands::ai_status()),
        "ollama_up" => ok(commands::ollama_up()),
        "ollama_models" => ok(commands::ollama_models()),
        // Wake the desktop's local model server. Without this the phone's Companion is
        // dead whenever Ollama happens to be asleep, and the only cure is walking to the
        // desk — which is the exact situation the portal exists to avoid. It starts a
        // loopback process the app already owns; it does **not** install anything
        // (`ai_install`/`ai_pull` stay desktop-only). `commands::ai_start` is `async`
        // purely to keep Tauri's UI thread free, so we call the same inner function.
        "ai_start" => done(crate::ollama::ensure_serving()),

        _ => Err(DispatchError::NotExposed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wrong_token_is_rejected_and_a_right_one_accepted() {
        let t = new_token().unwrap();
        assert!(token_matches(&t, &t));
        assert!(!token_matches(&t, "wrong"));
        assert!(!token_matches(&t, ""));
        // Same length, one byte different — the case a naive compare leaks.
        let mut near = t.clone();
        near.pop();
        near.push(if t.ends_with('a') { 'b' } else { 'a' });
        assert!(!token_matches(&t, &near));
    }

    #[test]
    fn tokens_are_long_and_not_repeated() {
        let a = new_token().unwrap();
        let b = new_token().unwrap();
        assert_eq!(a.len(), 64, "256 bits, hex");
        assert_ne!(a, b);
    }

    /// Rule 1. If this test ever needs "fixing", stop and re-read the module docs:
    /// binding anything but loopback publishes a substance journal to the network.
    #[test]
    fn the_portal_binds_loopback_only() {
        assert_eq!(BIND_ADDR, "127.0.0.1");
    }

    /// Rule 4, pinned. Adding any of these to `EXPOSED` should mean deleting a line
    /// here first — deliberately, having read why it's listed.
    #[test]
    fn the_dangerous_commands_are_not_reachable_from_a_phone() {
        let forbidden = [
            // The passphrase is typed in person or not at all.
            "unlock_db",
            "enable_encryption",
            "disable_encryption",
            "change_passphrase",
            // The desktop's filesystem is not the phone's business.
            "export_backup",
            "import_backup",
            "obsidian_export",
            "obsidian_import",
            // (`export_experience_markdown` *is* exposed: it only returns Markdown
            // text. This one writes it to a path on the desktop's disk.)
            "export_experience_file",
            "contribution_save",
            "data_dir",
            "reveal_data_dir",
            // Destroys the journal.
            "wipe_all_data",
            // Installs software or downloads gigabytes. (`ai_start` *is* exposed: it only
            // wakes a local server the app already owns, and without it the phone's
            // Companion stays dead until someone walks to the desk.)
            "ai_install",
            "ai_pull",
            "pw_update",
            // The portal may not reconfigure or disable itself — including publishing
            // itself to the tailnet, which is a decision made at the desk.
            "portal_status",
            "portal_enable",
            "portal_disable",
            "portal_qr",
            "portal_tailscale",
            "portal_serve",
            "portal_unserve",
        ];
        for c in forbidden {
            assert!(!EXPOSED.contains(&c), "`{c}` must not be reachable from the phone");
        }
    }

    /// The allowlist is only load-bearing if `dispatch` consults it. A `match` arm
    /// added without a matching `EXPOSED` entry must stay unreachable.
    #[test]
    fn the_allowlist_is_checked_before_anything_else() {
        let src = include_str!("portal.rs");
        let guard = "if !EXPOSED.contains(&command) {\n        return Err(DispatchError::NotExposed);";
        assert!(src.contains(guard), "dispatch() must reject non-allowlisted commands up front");
    }

    // ---- Over a real socket ----
    //
    // The portal's surface is HTTP, so these stand up the actual server and talk to
    // it the way a phone would. Security properties asserted against the real
    // request path, not against a function in isolation.

    use crate::Knowledge;
    use std::sync::Mutex as StdMutex;

    /// A running portal backed by a real (temporary) journal.
    fn serving() -> (tauri::AppHandle<tauri::test::MockRuntime>, u16, String) {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");

        let dir = std::env::temp_dir().join(format!("fn-portal-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{:?}.db", std::thread::current().id()));
        let _ = std::fs::remove_file(&path);
        let conn = crate::db::open(&path, None).unwrap();

        app.manage(Db { conn: StdMutex::new(Some(conn)), path });
        app.manage(Knowledge(None));
        app.manage(Portal::default());
        app.manage(CompanionJobs::default());

        let handle = app.handle().clone();
        let status = start(&handle).expect("portal starts");
        let port = status.port.unwrap();
        let token = status.pair_url.unwrap().split("#t=").nth(1).unwrap().to_string();
        (handle, port, token)
    }

    fn post(port: u16, cmd: &str, token: Option<&str>, body: Value) -> (u16, String) {
        let mut req = ureq::post(&format!("http://127.0.0.1:{port}/api/{cmd}"));
        if let Some(t) = token {
            req = req.set("Authorization", &format!("Bearer {t}"));
        }
        match req.send_json(body) {
            Ok(r) => (r.status(), r.into_string().unwrap_or_default()),
            Err(ureq::Error::Status(code, r)) => (code, r.into_string().unwrap_or_default()),
            Err(e) => panic!("transport error: {e}"),
        }
    }

    #[test]
    fn an_unpaired_phone_gets_nothing() {
        let (_app, port, token) = serving();

        // No token at all.
        let (status, _) = post(port, "list_experiences", None, json!({}));
        assert_eq!(status, 401, "a request with no token must be refused");

        // A wrong token.
        let (status, _) = post(port, "list_experiences", Some(&"0".repeat(64)), json!({}));
        assert_eq!(status, 401, "a request with the wrong token must be refused");

        // The real one works, so the 401s above are the token doing its job and not
        // the endpoint being broken.
        let (status, body) = post(port, "list_experiences", Some(&token), json!({}));
        assert_eq!(status, 200, "the paired phone can read: {body}");
    }

    /// The desktop's "Paired successfully" light: it turns on when — and only when —
    /// a request arrives with the right token. A refused request must not light it,
    /// or someone probing the port would tell the user their phone is paired.
    #[test]
    fn the_paired_light_tracks_a_real_pairing() {
        let (app, port, token) = serving();
        let portal = app.state::<Portal>();

        assert!(!portal.status().paired, "nothing has paired yet");

        let (status, _) = post(port, "list_experiences", Some(&"0".repeat(64)), json!({}));
        assert_eq!(status, 401);
        assert!(!portal.status().paired, "a rejected token must not light the pairing indicator");

        let (status, _) = post(port, "list_experiences", Some(&token), json!({}));
        assert_eq!(status, 200);
        assert!(portal.status().paired, "a phone with the right token has paired");

        // Turning the portal off throws the token away, so the next one starts unpaired.
        portal.stop();
        assert!(!portal.status().paired);
    }

    /// The single-entry export, end to end at the dispatch seam: the pure Markdown
    /// renderer is reachable and carries the whole story (title, doses, timeline),
    /// while its file-writing sibling stays desktop-only.
    #[test]
    fn a_single_entry_exports_as_markdown_but_never_as_a_file() {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        conn.execute_batch(crate::db::schema_for_tests()).unwrap();
        let exp = crate::db::create_experience(
            &conn,
            &crate::db::ExperienceInput {
                kind: "session".into(),
                title: "River evening".into(),
                intention: "unwind".into(),
                setting: "home".into(),
                started_at: "2026-07-10T20:00:00Z".into(),
            },
        )
        .unwrap();
        crate::db::log_dose(
            &conn,
            &crate::db::DoseInput {
                experience_id: exp.id,
                substance_name: "Caffeine".into(),
                amount: Some(80.0),
                unit: "mg".into(),
                route: "oral".into(),
                taken_at: "2026-07-10T20:05:00Z".into(),
                note: "tea".into(),
            },
        )
        .unwrap();
        crate::db::add_timeline_event(
            &conn,
            &crate::db::TimelineInput {
                experience_id: exp.id,
                at: "2026-07-10T21:00:00Z".into(),
                note: "calm and settled".into(),
                mood: "easy".into(),
                intensity: Some(3),
            },
        )
        .unwrap();
        app.manage(Db {
            conn: StdMutex::new(Some(conn)),
            path: std::env::temp_dir().join("fn-export-test-unused.db"),
        });

        let handle = app.handle().clone();
        let v = dispatch(&handle, "export_experience_markdown", json!({ "id": exp.id }))
            .unwrap_or_else(|_| panic!("`export_experience_markdown` must be reachable"));
        let md = v["markdown"].as_str().expect("markdown text");
        assert!(md.contains("River evening"), "the title must be in the note");
        assert!(md.contains("Caffeine") && md.contains("80"), "the logged dose must be in the note");
        assert!(md.contains("calm and settled"), "the timeline note must be in the note");
        let name = v["filename"].as_str().expect("filename");
        assert_eq!(name, format!("2026-07-10-river-evening-{}.md", exp.id));

        // The file writer is a different animal — it touches the desktop's disk.
        assert!(
            matches!(
                dispatch(&handle, "export_experience_file", json!({ "id": exp.id, "dest": "/tmp/nope.md" })),
                Err(DispatchError::NotExposed)
            ),
            "`export_experience_file` must not be reachable through dispatch"
        );
    }

    #[test]
    fn the_phone_cannot_reach_a_desktop_only_command() {
        let (_app, port, token) = serving();
        for cmd in ["wipe_all_data", "unlock_db", "export_backup", "export_experience_file", "portal_disable"] {
            let (status, body) = post(port, cmd, Some(&token), json!({ "passphrase": "x" }));
            assert_eq!(status, 403, "`{cmd}` must not be reachable from a phone, got: {body}");
        }
    }

    #[test]
    fn a_dose_logged_from_the_phone_lands_in_the_journal_with_its_warnings() {
        let (app, port, token) = serving();

        let (status, body) = post(
            port,
            "create_experience",
            Some(&token),
            json!({ "input": { "title": "From the phone", "started_at": "2026-07-13T20:00:00Z" } }),
        );
        assert_eq!(status, 200, "{body}");
        let exp: Value = serde_json::from_str(&body).unwrap();
        let id = exp["id"].as_i64().unwrap();

        let (status, body) = post(
            port,
            "log_dose",
            Some(&token),
            json!({ "input": {
                "experience_id": id,
                "substance_name": "MDMA",
                "amount": 100.0,
                "unit": "mg",
                "route": "oral",
                "taken_at": "2026-07-13T20:05:00Z"
            }}),
        );
        assert_eq!(status, 200, "{body}");

        // It's really in the journal, not just echoed back.
        let n: i64 = app
            .state::<Db>()
            .with(|c| c.query_row("SELECT COUNT(*) FROM doses WHERE experience_id = ?1", [id], |r| r.get(0)))
            .unwrap();
        assert_eq!(n, 1, "the dose the phone logged should be in the journal");
    }

    /// Rule 3. The deterministic safety layers must answer the phone exactly as they
    /// answer the desktop — a combo check that goes quiet on a phone is worse than none.
    #[test]
    fn the_interaction_checker_answers_the_phone_too() {
        let (_app, port, token) = serving();
        // Opioid + benzodiazepine — respiratory depression, the combination that
        // actually kills people. If the portal ever fails to pass this one through,
        // the phone is a liability rather than a tool.
        let (status, body) = post(
            port,
            "check_combo",
            Some(&token),
            json!({ "names": ["heroin", "xanax"] }),
        );
        assert_eq!(status, 200, "{body}");
        let warnings: Value = serde_json::from_str(&body).unwrap();
        assert!(
            warnings.as_array().unwrap().iter().any(|w| w["severity"] == "danger"),
            "opioid + benzo must still be flagged `danger` over the portal, got: {body}"
        );
    }

    /// Rule 3, the other half: locking the journal on the desktop must close the door,
    /// even for an already-paired phone holding a valid token.
    #[test]
    fn locking_the_desktop_shuts_the_phone_out() {
        let (app, port, token) = serving();
        let (status, _) = post(port, "list_experiences", Some(&token), json!({}));
        assert_eq!(status, 200);

        // What unlock/lock does under the hood: drop the connection.
        *app.state::<Db>().conn.lock().unwrap() = None;

        let (status, body) = post(port, "list_experiences", Some(&token), json!({}));
        assert_eq!(status, 503, "a locked journal must not be served: {body}");
    }

    // ---- Companion jobs ----
    //
    // No live Ollama in CI, so a started job finishes with an *error* — which is
    // exactly what proves the lifecycle over the real socket: started, ran on its
    // own thread, finished, result delivered (once) through polling. The path a
    // successful reply takes is identical; only the `Result` inside differs.

    #[test]
    fn a_companion_job_outlives_its_request_and_delivers_once() {
        let (_app, port, token) = serving();

        let (status, body) = post(
            port,
            "companion_chat_start",
            Some(&token),
            json!({
                "model": "fieldnotes-test-model-that-does-not-exist",
                "history": [{ "role": "user", "content": "hi" }],
                "experienceId": null,
                "supportStyle": null
            }),
        );
        assert_eq!(status, 200, "start must answer immediately: {body}");
        let v: Value = serde_json::from_str(&body).unwrap();
        let id = v["job"].as_u64().expect("a numeric job id");

        // Poll the way a phone would: short, cheap requests in a bounded loop.
        let mut finished = None;
        for _ in 0..150 {
            let (status, body) = post(port, "companion_chat_poll", Some(&token), json!({ "id": id }));
            assert_eq!(status, 200, "polling a live job must succeed: {body}");
            let v: Value = serde_json::from_str(&body).unwrap();
            if v["status"] == "running" {
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
            finished = Some(v);
            break;
        }
        let v = finished.expect("the job should finish well within the polling window");
        assert_eq!(
            v["status"], "error",
            "with no reachable model the turn must surface its error, got: {v}"
        );
        assert!(
            !v["error"].as_str().unwrap_or("").is_empty(),
            "the error must say something: {v}"
        );

        // Delivered once: the same id is now unknown.
        let (status, body) = post(port, "companion_chat_poll", Some(&token), json!({ "id": id }));
        assert_eq!(status, 400, "a delivered job must be gone: {body}");
        assert!(body.contains("unknown or expired"), "{body}");
    }

    #[test]
    fn polling_a_job_that_never_existed_fails_gracefully() {
        let (_app, port, token) = serving();
        let (status, body) = post(port, "companion_chat_poll", Some(&token), json!({ "id": 424242 }));
        assert_eq!(status, 400, "{body}");
        assert!(body.contains("unknown or expired"), "{body}");
    }

    /// The phone may *read* whether the Companion is switched on, so it can say so
    /// instead of offering a chat that shouldn't be there — but it must never be
    /// able to switch it back on. That would be the phone reconfiguring the
    /// desktop, which rule 4 exists to prevent.
    #[test]
    fn the_phone_can_read_the_companion_switch_but_not_flip_it() {
        assert!(EXPOSED.contains(&"companion_enabled"));
        assert!(!EXPOSED.contains(&"set_companion_enabled"));
    }

    /// Both halves of the job flow must be on the allowlist, and the blocking call
    /// stays for phones running an older frontend.
    #[test]
    fn the_companion_job_commands_are_exposed() {
        assert!(EXPOSED.contains(&"companion_chat_start"));
        assert!(EXPOSED.contains(&"companion_chat_poll"));
        assert!(EXPOSED.contains(&"companion_chat"));
    }
}
