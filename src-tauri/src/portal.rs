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
//!    the LAN, a coffee-shop network, or anywhere else — the only way in is through
//!    Tailscale, which fronts loopback with HTTPS on your tailnet: the Tailscale
//!    built into Field Notes (`tailnet.rs`, which never uses Funnel), or the
//!    Tailscale app's `tailscale serve`. Change this line and you have published a
//!    substance journal to the local network.
//! 2. **A token on every API request, tailnet or not.** A tailnet is *not* a trust
//!    boundary for this data — every device you ever added to it is on it, forever.
//!    Each paired device has its own token (see `devices.rs`): compared in constant
//!    time, stored only as a hash, and revocable one device at a time.
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

use crate::{commands, devices::{Devices, OWNER}, people::People, Db, Knowledge};
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
pub(crate) const PORT_RANGE: std::ops::Range<u16> = 8787..8807;

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
    stopping: Arc<AtomicBool>,
    /// Set the first time a request arrives carrying a paired device's token since
    /// the portal was turned on. Goes false again only by stopping the portal.
    paired: Arc<AtomicBool>,
}

#[derive(serde::Serialize)]
pub struct PortalStatus {
    pub running: bool,
    pub port: Option<u16>,
    /// A paired device has made a request since the portal was turned on. It
    /// says nothing about whether that device is still connected right now.
    pub paired: bool,
}

impl Default for Portal {
    fn default() -> Self {
        // Off until the desktop says otherwise: the Companion is opt-in, and a phone
        // that asks before the desktop page has pushed the real setting should not
        // be offered a chat the user never turned on.
        Portal { inner: Mutex::new(None), companion_enabled: AtomicBool::new(false) }
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
                paired: r.paired.load(Ordering::SeqCst),
            },
            None => PortalStatus { running: false, port: None, paired: false },
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

/// Start the portal. Fails if the journal is locked (rule 3).
pub fn start<R: Runtime>(app: &AppHandle<R>) -> Result<PortalStatus, String> {
    let portal = app.state::<Portal>();
    if portal.status().running {
        return Ok(portal.status());
    }
    if !app.state::<Db>().is_unlocked() {
        return Err("Unlock the journal before turning on device access.".into());
    }
    // A computer whose journal lives on another one has nothing of its own to serve —
    // its local journal is only a cache and an outbox. Chaining servers isn't a thing.
    if crate::remote::is_connected(app) {
        return Err("This computer uses another one as its journal, so it can't serve one too. Disconnect first.".into());
    }

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
        std::thread::spawn(move || {
            while let Ok(req) = server.recv() {
                if stopping.load(Ordering::SeqCst) {
                    break;
                }
                handle(&app, &paired, req);
            }
        });
    }

    *portal.inner.lock().unwrap() = Some(Running { server, port, stopping, paired });
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

fn handle<R: Runtime>(app: &AppHandle<R>, paired: &AtomicBool, req: Request) {
    let url = req.url().to_string();
    let path = url.split('?').next().unwrap_or("/").to_string();

    if let Some(command) = path.strip_prefix("/api/") {
        let command = command.to_string();
        return api(app, paired, &command, req);
    }
    assets(app, &path, req);
}

fn api<R: Runtime>(app: &AppHandle<R>, paired: &AtomicBool, command: &str, mut req: Request) {
    // Rule 2: token first, before we even look at the body.
    let given = req
        .headers()
        .iter()
        .find(|h| h.field.equiv("Authorization"))
        .and_then(|h| h.value.as_str().strip_prefix("Bearer ").map(str::to_string))
        .unwrap_or_default();
    let Some(device) = app.state::<Devices>().verify(&given) else {
        let _ = req.respond(json_response(401, json!({ "error": "Not paired with this journal." })));
        return;
    };
    // Whose journal: from the device record and nothing else (people.rs, rule 1).
    let who = Caller { person: device.person, device: device.id };
    let people = app.try_state::<People>();
    if who.person != OWNER && !people.as_ref().is_some_and(|p| p.exists(who.person)) {
        // Their person was removed; the device record should be gone too.
        let _ = req.respond(json_response(401, json!({ "error": "Not paired with this journal." })));
        return;
    }

    // The token checked out, so a paired device is on the other end. Tell the
    // desktop — the pairing screen has no other way to know a scan worked, and
    // "did it take?" is the whole question the user is holding.
    paired.store(true, Ordering::SeqCst);
    let _ = app.emit("portal-paired", device.id);

    // Rule 3: re-check on every request, not just at startup.
    if who.person == OWNER {
        if !app.state::<Db>().is_unlocked() {
            let _ = req.respond(json_response(
                503,
                json!({ "error": "The journal is locked on the desktop." }),
            ));
            return;
        }
    } else {
        // Another person unlocks their own journal from their own device, so the
        // unlock itself, and everything that needs no journal (Help, the
        // combination checker), still answers while it's locked.
        let locked = people.as_ref().and_then(|p| p.db(who.person)).is_none_or(|d| !d.is_unlocked());
        // Owner-only commands fall through to dispatch's 403, locked or not.
        if locked && !LOCKED_OK.contains(&command) && !OWNER_ONLY.contains(&command) {
            let _ = req.respond(json_response(
                503,
                json!({ "error": "Your journal is locked. Unlock it with your password.", "locked": true }),
            ));
            return;
        }
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

    let resp = match dispatch_as(app, who, command, args) {
        Ok(Value::String(raw)) if RAW_FILES.contains(&command) => {
            let hdr = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
            Response::from_string(raw).with_header(hdr)
        }
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

fn owner_auth<R: Runtime>(app: &AppHandle<R>) -> Result<tauri::State<'_, crate::owner_auth::OwnerAuth>, DispatchError> {
    app.try_state::<crate::owner_auth::OwnerAuth>()
        .ok_or_else(|| DispatchError::Failed("Pairing from a phone isn't available on this server.".into()))
}

/// The owner's phone pairing or un-pairing: only with the journal's password or
/// the phone PIN (`owner_auth.rs`). Other people's devices don't come here.
fn owner_check<R: Runtime>(app: &AppHandle<R>, journal: &std::path::Path, args: &Value) -> Result<(), DispatchError> {
    let given: String = arg::<Option<String>>(args, "password")?.unwrap_or_default();
    owner_auth(app)?.verify(journal, &given).map_err(DispatchError::Failed)
}

pub enum DispatchError {
    /// The command exists but is deliberately not reachable from a phone.
    NotExposed,
    Failed(String),
}

/// The caller's own devices, and only theirs, with the one asking marked `this`
/// so the phone can say which is itself.
fn mine<R: Runtime>(app: &AppHandle<R>, who: Caller) -> Vec<Value> {
    app.state::<Devices>()
        .list()
        .into_iter()
        .filter(|d| d.person == who.person)
        .map(|d| {
            let this = d.id == who.device;
            let mut v = json!(d);
            v["this"] = json!(this);
            v
        })
        .collect()
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
///
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
    // Read-only aggregation over the same rows as `usage_by_substance`.
    "usage_stats",
    // Updating the server. Status is a read. Install is the one exposed command
    // that installs software: it's off until enabled at the computer, refuses
    // unless the server will come back by itself, and installs only a signed
    // release from the app's own update URL. See `server_update.rs`. The switch
    // that enables it, `set_phone_can_update`, is deliberately absent.
    "server_update_status",
    "server_update_install",
    "list_substances",
    "db_status",
    "companion_enabled",
    // Whether to offer the discreet-mode toggle. Read only; the switch itself,
    // `set_discreet_available`, lives in Settings on the computer.
    "discreet_available",
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
    "list_unit_kinds",
    "save_unit_kind",
    "delete_unit_kind",
    "set_bedtime",
    "kind_question",
    "answer_kind_question",
    "set_substance_kind",
    // Only ever removes an entry that is an exact copy of another. A laptop runs it
    // here at the end of "Sync journal to server".
    "remove_duplicate_entries",
    "check_combo",
    "interaction_classes",
    "crisis_scan",
    "emergency_resources",
    "pw_status",
    "pw_lookup",
    // Names and street names from the (public) dose reference, for pasted logs.
    "pw_names",
    // Which substance a typed name means ("acid" → LSD), for saving a dose.
    "canonical_name",
    "knowledge_search",
    "knowledge_status",
    // The bundled reference files as shipped (public CC0 data, no journal), for a
    // phone to keep so its combination checker and lookup work offline.
    "offline_reference",
    "offline_corpus",
    "companion_chat",
    "companion_chat_start",
    "companion_chat_poll",
    // Read-only: load the model ahead of the first message, and report whether this
    // machine can run it comfortably. A laptop using this computer as its server
    // shows the *server's* answer, since that's where the model runs.
    "companion_warm",
    "compute_status",
    "ai_status",
    "ollama_up",
    "ollama_models",
    "ai_start",
    // Another person on this server (people.rs), from their own device only: unlock
    // (or, the first time, create) their journal with their password, keep it
    // unlocked across restarts if they choose, and manage their own devices. None
    // of these take a person: it comes from the device.
    "person_unlock",
    "person_remember",
    "my_devices",
    "pair_own_device",
    "unpair_my_device",
    "person_change_password",
    "export_my_journal",
    // The owner's phone, managing the owner's own devices: listing is free, but
    // pairing or un-pairing asks for the journal's password or the phone PIN
    // (owner_auth.rs, owner's request 2026-10-03). This says which to ask for.
    "owner_device_auth",
];

/// Exposed to the owner's devices only. Installing an update restarts everyone's
/// server, so only the owner may do it from a phone.
pub const OWNER_ONLY: &[&str] = &["server_update_status", "server_update_install", "owner_device_auth"];

/// Exposed to other people's devices only. The owner's journal unlocks at the desk,
/// as it always has. (The owner may pair from a phone, but only with the journal's
/// password or the phone PIN: see `owner_auth.rs`.)
pub const OTHERS_ONLY: &[&str] = &[
    "person_unlock",
    "person_remember",
    "person_change_password",
    "export_my_journal",
];

/// What another person's device may still do while their journal is locked:
/// unlock it, and anything that holds no journal data. Help must never wait on a
/// password.
pub const LOCKED_OK: &[&str] = &[
    "db_status",
    "person_unlock",
    "check_combo",
    "interaction_classes",
    "emergency_resources",
    "knowledge_search",
    "knowledge_status",
    "offline_reference",
    "offline_corpus",
    "discreet_available",
    "companion_enabled",
];

/// Commands whose answer is a whole bundled file, sent as it is rather than
/// wrapped in JSON again.
const RAW_FILES: &[&str] = &["offline_reference", "offline_corpus"];

/// Who is asking: whose journal, from which device. Built from the device record
/// in [`api`]; never from anything in the request.
#[derive(Clone, Copy, Debug)]
pub struct Caller {
    pub person: u32,
    pub device: u64,
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn dispatch<R: Runtime>(app: &AppHandle<R>, command: &str, args: Value) -> Result<Value, DispatchError> {
    dispatch_as(app, Caller { person: OWNER, device: 0 }, command, args)
}

/// Run one exposed command for `who`, against their journal and nothing else.
pub fn dispatch_as<R: Runtime>(app: &AppHandle<R>, who: Caller, command: &str, args: Value) -> Result<Value, DispatchError> {
    if !EXPOSED.contains(&command) {
        return Err(DispatchError::NotExposed);
    }
    let is_owner = who.person == OWNER;
    if (is_owner && OTHERS_ONLY.contains(&command)) || (!is_owner && OWNER_ONLY.contains(&command)) {
        return Err(DispatchError::NotExposed);
    }
    // Whose journal. The owner's is the app's own `Db`; anyone else's comes from
    // `People`, by the person on the device record.
    let owner_db = app.state::<Db>();
    let theirs: Option<std::sync::Arc<Db>> = if is_owner {
        None
    } else {
        Some(
            app.try_state::<People>()
                .and_then(|p| p.db(who.person))
                .ok_or_else(|| DispatchError::Failed("This device's person is no longer on this server.".into()))?,
        )
    };
    let db: &Db = match &theirs {
        Some(d) => d,
        None => owner_db.inner(),
    };
    match command {
        // --- reading the journal ---
        "list_experiences" => done(commands::list_experiences_in(db)),
        "get_experience" => done(commands::get_experience_in(db, arg(&args, "id")?)),
        "export_experience_markdown" => {
            done(commands::export_experience_markdown_in(db, arg(&args, "id")?))
        }
        "usage_by_substance" => done(commands::usage_by_substance_in(db)),
        "usage_stats" => done(commands::usage_stats_in(db, arg(&args, "since")?)),
        "offline_reference" => crate::pw::read_bundled(app).map(Value::String).map_err(DispatchError::Failed),
        "offline_corpus" => crate::knowledge::read_bundled(app).map(Value::String).map_err(DispatchError::Failed),
        "server_update_status" => ok(crate::server_update::status(app)),
        "server_update_install" => {
            done(crate::server_update::install(app, arg::<Option<bool>>(&args, "anyway")?.unwrap_or(false)))
        }
        "list_substances" => done(commands::list_substances_in(db)),
        "db_status" => match &theirs {
            None => ok(commands::db_status(owner_db)),
            Some(_) => ok(app.state::<People>().status(who.person)),
        },
        "companion_enabled" => ok(app.state::<Portal>().companion_enabled()),
        "discreet_available" => ok(app.state::<crate::prefs::Prefs>().get().discreet_available),

        // --- writing to the journal: the whole point of the portal ---
        "create_experience" => done(commands::create_experience_in(db, arg(&args, "input")?)),
        "end_experience" => done(commands::end_experience_in(
            db,
            arg(&args, "id")?,
            arg(&args, "endedAt")?,
            arg(&args, "rating")?,
            arg(&args, "notes")?,
        )),
        "log_dose" => done(commands::log_dose_in(db, arg(&args, "input")?)),
        "add_timeline_event" => done(commands::add_timeline_event_in(db, arg(&args, "input")?)),
        "add_substance" => done(commands::add_substance_in(db, arg(&args, "input")?)),
        "update_experience" => {
            done(commands::update_experience_in(db, arg(&args, "id")?, arg(&args, "update")?))
        }
        "set_writeup_skipped" => {
            done(commands::set_writeup_skipped_in(db, arg(&args, "id")?, arg(&args, "skipped")?))
        }
        "update_dose" => done(commands::update_dose_in(db, arg(&args, "id")?, arg(&args, "update")?)),
        "update_timeline_event" => {
            done(commands::update_timeline_event_in(db, arg(&args, "id")?, arg(&args, "update")?))
        }
        "delete_experience" => done(commands::delete_experience_in(db, arg(&args, "id")?)),
        "delete_dose" => done(commands::delete_dose_in(db, arg(&args, "id")?)),
        "list_unit_kinds" => done(commands::list_unit_kinds_in(db, arg(&args, "substance")?)),
        "save_unit_kind" => {
            done(commands::save_unit_kind_in(db, arg(&args, "substance")?, arg(&args, "kind")?))
        }
        "delete_unit_kind" => done(commands::delete_unit_kind_in(db, arg(&args, "id")?)),
        "set_bedtime" => done(commands::set_bedtime_in(db, arg(&args, "value")?)),
        "kind_question" => done(commands::kind_question_in(db, arg(&args, "substance")?)),
        "answer_kind_question" => done(commands::answer_kind_question_in(
            db,
            arg(&args, "substance")?,
            arg(&args, "ask")?,
            arg(&args, "answer")?,
        )),
        "set_substance_kind" => {
            done(commands::set_substance_kind_in(db, arg(&args, "substance")?, arg(&args, "kind")?))
        }
        "delete_timeline_event" => done(commands::delete_timeline_event_in(db, arg(&args, "id")?)),
        "delete_substance" => done(commands::delete_substance_in(db, arg(&args, "id")?)),
        "remove_duplicate_entries" => done(commands::remove_duplicate_entries_in(db)),

        // --- safety: the same deterministic layers the desktop uses ---
        "check_combo" => ok(commands::check_combo_in(db, arg(&args, "names")?, arg(&args, "doses")?)),
        "interaction_classes" => ok(commands::interaction_classes()),
        "crisis_scan" => ok(commands::crisis_scan_text(arg(&args, "text")?, arg(&args, "recent")?)),
        "emergency_resources" => ok(commands::emergency_resources()),

        // --- reference ---
        "pw_status" => done(commands::pw_status_in(db)),
        "pw_lookup" => done(commands::pw_lookup_in(db, arg(&args, "name")?)),
        "pw_names" => done(commands::pw_names_in(db)),
        "canonical_name" => done(commands::canonical_name_in(db, arg(&args, "name")?)),
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
                db,
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
            let theirs = theirs.clone();
            std::thread::spawn(move || {
                let owner_db = app.state::<Db>();
                let db: &Db = match &theirs {
                    Some(d) => d,
                    None => owner_db.inner(),
                };
                let kb = app.state::<Knowledge>();
                let result = commands::companion_chat_inner(
                    db,
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
        "companion_warm" => done(crate::ollama::warm(&arg::<String>(&args, "model")?)),
        "compute_status" => ok(crate::compute::status(
            &arg::<String>(&args, "model")?,
            arg(&args, "measuredTps")?,
        )),
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

        // --- another person on this server: their own journal, their own devices ---
        "person_unlock" => {
            let people = app.state::<People>();
            let password: String = arg(&args, "password")?;
            let opened = people.unlock(who.person, who.device, &password).map_err(DispatchError::Failed)?;
            // The dose reference lives inside each journal; load it into theirs.
            commands::refresh_dose_reference(app, &opened);
            ok(people.status(who.person))
        }
        "person_remember" => {
            let people = app.state::<People>();
            let remember: bool = arg(&args, "remember")?;
            let password: Option<String> = arg(&args, "password")?;
            people
                .set_remember(who.person, who.device, remember, password.as_deref())
                .map_err(DispatchError::Failed)?;
            ok(people.status(who.person))
        }
        "person_change_password" => {
            let people = app.state::<People>();
            let current: String = arg(&args, "current")?;
            let new: String = arg(&args, "new")?;
            people.change_password(who.person, who.device, &current, &new).map_err(DispatchError::Failed)?;
            ok(people.status(who.person))
        }
        // Their journal as a file, still encrypted with their password. Base64 so it
        // rides in the same JSON as everything else; the phone saves it as a download.
        "export_my_journal" => {
            use base64::Engine;
            let bytes = app.state::<People>().backup(who.person).map_err(DispatchError::Failed)?;
            ok(json!({ "data": base64::engine::general_purpose::STANDARD.encode(bytes) }))
        }
        "my_devices" => ok(mine(app, who)),
        "owner_device_auth" => ok(owner_auth(app)?.method(&owner_db.path)),
        "pair_own_device" => {
            if is_owner {
                owner_check(app, &owner_db.path, &args)?;
            }
            let name: String = arg(&args, "name")?;
            // The phone says where it reached us (its own address bar), so the new
            // device's link and code point at the same place. Only drawn into the
            // code; nothing here is stored.
            let origin: Option<String> = arg(&args, "origin")?;
            let (device, token) =
                app.state::<Devices>().pair_for(&name, who.person).map_err(DispatchError::Failed)?;
            let qr = origin
                .filter(|o| o.starts_with("https://") || o.starts_with("http://"))
                .and_then(|o| commands::portal_qr(format!("{}/m#t={token}", o.trim_end_matches('/'))).ok());
            ok(json!({ "device": device, "token": token, "qr": qr }))
        }
        "unpair_my_device" => {
            if is_owner {
                owner_check(app, &owner_db.path, &args)?;
            }
            app.state::<Devices>().revoke_own(who.person, arg(&args, "id")?).map_err(DispatchError::Failed)?;
            ok(mine(app, who))
        }

        _ => Err(DispatchError::NotExposed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            // Nor sign the built-in Tailscale in or out, or swap it for the app.
            "tailnet_connect",
            "tailnet_sign_out",
            "tailnet_use_app",
            // Nor pair new devices, list them, or revoke anyone — including itself.
            "portal_pair",
            "portal_devices",
            "portal_revoke",
            // Nor read the remembered passphrase's state or change it.
            "keychain_status",
            "keychain_remember",
            "keychain_forget",
            "server_prefs",
            "set_server_prefs",
            "set_menu_bar",
            // A server must never act as a client of another server on a device's say-so.
            "remote_status",
            "remote_connect",
            "remote_disconnect",
            "remote_call",
            "remote_flush",
            "remote_discard",
            "remote_upload_local",
            // Writes a file to this computer's disk.
            "save_markdown_file",
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

        app.manage(Db::new(Some(conn), path.clone()));
        app.manage(Knowledge(None));
        app.manage(Portal::default());
        app.manage(CompanionJobs::default());
        // The phone asks for these preferences on every load (discreet mode);
        // without them each load panicked a worker until the portal stopped
        // answering, which cut `dev_portal` short.
        let _ = std::fs::remove_file(path.with_extension("server.json"));
        app.manage(crate::prefs::Prefs::load(path.with_extension("server.json")));
        let _ = std::fs::remove_file(path.with_extension("devices.json"));
        let devices = Devices::load(path.with_extension("devices.json"));
        let (_, token) = devices.pair("Test phone").unwrap();
        app.manage(devices);
        let _ = std::fs::remove_file(path.with_extension("pin.json"));
        app.manage(crate::owner_auth::OwnerAuth::new(path.with_extension("pin.json")));

        let handle = app.handle().clone();
        let status = start(&handle).expect("portal starts");
        let port = status.port.unwrap();
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

    // ---- Two people on one server (people.rs) ----

    /// A running portal with the owner's journal and a second person, Sam, whose
    /// journal doesn't exist yet. Returns the owner's token and Sam's.
    fn serving_two() -> (tauri::AppHandle<tauri::test::MockRuntime>, u16, String, String) {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("mock app");
        let dir = std::env::temp_dir().join(format!("fn-people-portal-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("journal.db");
        let conn = crate::db::open(&path, None).unwrap();
        app.manage(Db::new(Some(conn), path.clone()));
        app.manage(Knowledge(None));
        app.manage(Portal::default());
        app.manage(CompanionJobs::default());
        app.manage(crate::prefs::Prefs::load(dir.join("server.json")));
        app.manage(crate::owner_auth::OwnerAuth::new(dir.join("phone_pin.json")));
        let people = People::load(&dir);
        let sam = people.add("Sam").unwrap();
        app.manage(people);
        let devices = Devices::load(dir.join("devices.json"));
        let (_, owner_token) = devices.pair("Owner phone").unwrap();
        let (_, sam_token) = devices.pair_for("Sam phone", sam.id).unwrap();
        app.manage(devices);
        let handle = app.handle().clone();
        let port = start(&handle).expect("portal starts").port.unwrap();
        (handle, port, owner_token, sam_token)
    }

    fn new_session(title: &str) -> Value {
        json!({ "input": { "kind": "session", "title": title, "intention": "", "setting": "", "started_at": "2026-09-01T20:00:00Z" } })
    }

    #[test]
    fn another_persons_journal_is_locked_until_they_choose_a_password_but_help_still_answers() {
        let (_app, port, _owner, sam) = serving_two();
        let (status, body) = post(port, "db_status", Some(&sam), json!({}));
        assert_eq!(status, 200, "{body}");
        let st: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(st["new_journal"], true);
        assert_eq!(st["unlocked"], false);
        assert_eq!(st["person_name"], "Sam");

        // Journal commands wait for the password...
        let (status, body) = post(port, "list_experiences", Some(&sam), json!({}));
        assert_eq!(status, 503, "{body}");
        assert!(body.contains("locked"));
        // ...but Help and the combination checker never do.
        assert_eq!(post(port, "emergency_resources", Some(&sam), json!({})).0, 200);
        assert_eq!(post(port, "check_combo", Some(&sam), json!({ "names": ["MDMA", "tramadol"] })).0, 200);

        // Too short a password is refused; a real one creates the journal.
        let (status, body) = post(port, "person_unlock", Some(&sam), json!({ "password": "short" }));
        assert_eq!(status, 400, "{body}");
        let (status, body) = post(port, "person_unlock", Some(&sam), json!({ "password": "correct horse battery" }));
        assert_eq!(status, 200, "{body}");
        assert_eq!(post(port, "list_experiences", Some(&sam), json!({})).0, 200);
    }

    /// The core promise, against the real request path: everything one person's
    /// device can call never returns a word of the other person's journal.
    #[test]
    fn one_persons_device_never_sees_the_other_persons_entries() {
        let (_app, port, owner, sam) = serving_two();
        assert_eq!(post(port, "person_unlock", Some(&sam), json!({ "password": "correct horse battery" })).0, 200);

        assert_eq!(post(port, "create_experience", Some(&owner), new_session("OWNERSECRET evening")).0, 200);
        assert_eq!(post(port, "create_experience", Some(&sam), new_session("SAMSECRET night")).0, 200);

        let (_, mine) = post(port, "list_experiences", Some(&owner), json!({}));
        assert!(mine.contains("OWNERSECRET") && !mine.contains("SAMSECRET"), "{mine}");
        let (_, theirs) = post(port, "list_experiences", Some(&sam), json!({}));
        assert!(theirs.contains("SAMSECRET") && !theirs.contains("OWNERSECRET"), "{theirs}");

        // Every exposed command, as each person, with a spread of plausible ids.
        for (token, other) in [(&sam, "OWNERSECRET"), (&owner, "SAMSECRET")] {
            for cmd in EXPOSED {
                if ["person_unlock", "person_remember", "server_update_status", "server_update_install", "ai_start", "companion_warm", "companion_chat", "companion_chat_start"].contains(cmd) {
                    continue; // hold no journal data: they change state, reach for a model or the updater
                }
                for id in 1..=3 {
                    let (_, body) = post(port, cmd, Some(token), json!({ "id": id, "since": null, "names": ["LSD"], "text": "hi", "query": "x", "name": "LSD", "experienceId": id }));
                    assert!(!body.contains(other), "`{cmd}` leaked the other person's journal: {body}");
                }
            }
        }
    }

    #[test]
    fn the_owner_pairs_and_unpairs_from_a_phone_only_with_the_pin() {
        let (app, port, owner, _sam) = serving_two();
        let (_, how) = post(port, "owner_device_auth", Some(&owner), json!({}));
        assert_eq!(how, "\"off\"");
        app.state::<crate::owner_auth::OwnerAuth>().set_pin(Some("482913")).unwrap();
        let (_, how) = post(port, "owner_device_auth", Some(&owner), json!({}));
        assert_eq!(how, "\"pin\"");

        let (code, _) = post(port, "pair_own_device", Some(&owner), json!({ "name": "Tablet", "password": "000000" }));
        assert_eq!(code, 400, "a wrong PIN pairs nothing");
        let (code, body) = post(port, "pair_own_device", Some(&owner), json!({ "name": "Tablet", "password": "482913" }));
        assert_eq!(code, 200, "{body}");
        let tablet = app.state::<Devices>().list().into_iter().find(|d| d.name == "Tablet").unwrap();
        assert_eq!(tablet.person, OWNER);

        // Listing needs no PIN, and shows only the owner's devices.
        let (_, list) = post(port, "my_devices", Some(&owner), json!({}));
        assert!(list.contains("Tablet") && !list.contains("Sam phone"), "{list}");

        assert_eq!(post(port, "unpair_my_device", Some(&owner), json!({ "id": tablet.id })).0, 400);
        assert_eq!(post(port, "unpair_my_device", Some(&owner), json!({ "id": tablet.id, "password": "482913" })).0, 200);
        assert!(app.state::<Devices>().list().iter().all(|d| d.name != "Tablet"));
    }

    #[test]
    fn people_reach_only_their_own_commands_and_their_own_devices() {
        let (app, port, owner, sam) = serving_two();
        // Only the owner may install an update; only others unlock from a phone.
        assert_eq!(post(port, "server_update_install", Some(&sam), json!({})).0, 403);
        assert_eq!(post(port, "server_update_status", Some(&sam), json!({})).0, 403);
        assert_eq!(post(port, "person_unlock", Some(&owner), json!({ "password": "correct horse battery" })).0, 403);
        // The owner may pair from a phone, but only with the journal's password or
        // the phone PIN (owner_auth.rs); none is set here, so it's refused.
        let (code, body) = post(port, "pair_own_device", Some(&owner), json!({ "name": "x" }));
        assert_eq!(code, 400, "{body}");
        assert!(app.state::<Devices>().list().iter().all(|d| d.name != "x"));
        assert_eq!(post(port, "owner_device_auth", Some(&sam), json!({})).0, 403);

        assert_eq!(post(port, "person_unlock", Some(&sam), json!({ "password": "correct horse battery" })).0, 200);
        let (_, body) = post(port, "my_devices", Some(&sam), json!({}));
        assert!(body.contains("Sam phone") && !body.contains("Owner phone"), "{body}");
        let list: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(list[0]["this"], true, "the asking device is marked as itself");

        // Sam can't un-pair the owner's phone, however the id is guessed.
        let owner_id = app.state::<Devices>().list().iter().find(|d| d.person == OWNER).unwrap().id;
        let (status, _) = post(port, "unpair_my_device", Some(&sam), json!({ "id": owner_id }));
        assert_eq!(status, 400);
        assert_eq!(post(port, "list_experiences", Some(&owner), json!({})).0, 200);

        // A second device of Sam's opens Sam's journal.
        let (status, body) =
            post(port, "pair_own_device", Some(&sam), json!({ "name": "Sam laptop", "origin": "https://box.example.ts.net" }));
        assert_eq!(status, 200, "{body}");
        let paired: Value = serde_json::from_str(&body).unwrap();
        assert!(paired["qr"].as_str().is_some_and(|q| q.contains("<svg")), "{body}");
        let token2 = paired["token"].as_str().unwrap().to_string();
        let (_, body) = post(port, "my_devices", Some(&token2), json!({}));
        assert!(body.contains("Sam laptop") && body.contains("Sam phone") && !body.contains("Owner phone"), "{body}");
        assert_eq!(post(port, "list_experiences", Some(&token2), json!({})).0, 200);

        // Removing Sam un-pairs every device of theirs at once.
        let people = app.state::<People>();
        app.state::<Devices>().revoke_person(2).unwrap();
        people.remove(2).unwrap();
        assert_eq!(post(port, "list_experiences", Some(&sam), json!({})).0, 401);
        assert_eq!(post(port, "list_experiences", Some(&token2), json!({})).0, 401);
        assert_eq!(post(port, "list_experiences", Some(&owner), json!({})).0, 200);
    }

    #[test]
    fn a_person_changes_their_password_and_exports_their_journal_from_their_own_device() {
        use base64::Engine;
        let (_app, port, owner, sam) = serving_two();
        // Neither works for the owner's phone, nor before Sam's journal is open.
        assert_eq!(post(port, "export_my_journal", Some(&owner), json!({})).0, 403);
        assert_eq!(post(port, "person_change_password", Some(&owner), json!({ "current": "x", "new": "y" })).0, 403);
        assert_eq!(post(port, "export_my_journal", Some(&sam), json!({})).0, 503);

        assert_eq!(post(port, "person_unlock", Some(&sam), json!({ "password": "correct horse battery" })).0, 200);
        assert_eq!(post(port, "create_experience", Some(&sam), new_session("SAMSECRET night")).0, 200);
        let (status, body) = post(port, "export_my_journal", Some(&sam), json!({}));
        assert_eq!(status, 200, "{body}");
        let data = serde_json::from_str::<Value>(&body).unwrap()["data"].as_str().unwrap().to_string();
        let bytes = base64::engine::general_purpose::STANDARD.decode(data).unwrap();
        assert!(!bytes.windows(9).any(|w| w == b"SAMSECRET"), "the backup is encrypted");

        let (status, body) =
            post(port, "person_change_password", Some(&sam), json!({ "current": "wrong one", "new": "a brand new password" }));
        assert_eq!(status, 400, "{body}");
        let (status, body) = post(
            port,
            "person_change_password",
            Some(&sam),
            json!({ "current": "correct horse battery", "new": "a brand new password" }),
        );
        assert_eq!(status, 200, "{body}");
        let (_, body) = post(port, "list_experiences", Some(&sam), json!({}));
        assert!(body.contains("SAMSECRET"), "still open after the change: {body}");
    }

    #[test]
    fn no_exposed_command_lets_a_phone_name_a_person() {
        // The person comes from the device. If a command ever grows a `person`
        // argument, this source check fails before it ships.
        let src = include_str!("portal.rs");
        let start = src.find("pub fn dispatch_as").unwrap();
        let dispatch = &src[start..start + src[start..].find("\n}\n").unwrap()];
        assert!(dispatch.contains("person_unlock"), "the slice must cover dispatch_as");
        assert!(!dispatch.contains("\"person\""), "dispatch_as must never read a person from the request");
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

        // Turning the portal off resets the light; the pairing itself persists.
        portal.stop();
        assert!(!portal.status().paired);
    }

    /// Not a test: a real portal on a throwaway journal, for driving the phone UI by
    /// hand or with a headless browser. Serves until killed. Prints the port and a
    /// device token. `cargo test --lib portal::tests::dev_portal -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn dev_portal() {
        let (app, port, token) = serving();
        let db = app.state::<Db>();
        // The real bundled dose reference, so interactions and street names behave as shipped.
        let subs = crate::pw::parse_slim(include_str!("../resources/dosewiki.json")).unwrap();
        db.with_mut(|c| crate::db::pw_replace_all(c, &subs)).unwrap();
        db.with(|c| {
            let day = |d: i64, h: i64| {
                let t = std::time::SystemTime::now() - std::time::Duration::from_secs((d * 86400) as u64);
                let secs = t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
                let midnight = secs - secs % 86400;
                let ts = midnight + h * 3600;
                c.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', ?1, 'unixepoch')", [ts], |r| r.get::<_, String>(0))
                    .unwrap()
            };
            let mk = |kind: &str, title: &str, start: String, end: Option<String>, notes: &str, rating: Option<i64>| {
                let e = crate::db::create_experience(c, &serde_json::from_value(json!({ "kind": kind, "title": title, "started_at": start })).unwrap()).unwrap();
                crate::db::update_experience(c, e.id, &serde_json::from_value(json!({ "title": title, "notes": notes, "rating": rating, "started_at": start, "ended_at": end, "intention": if kind == "session" { "Dance, stay with friends." } else { "" }, "setting": "" })).unwrap()).unwrap();
                e.id
            };
            let dose = |id: i64, s: &str, amt: f64, unit: &str, at: String| {
                crate::db::log_dose(c, &serde_json::from_value(json!({ "experience_id": id, "substance_name": s, "amount": amt, "unit": unit, "route": "oral", "taken_at": at })).unwrap()).unwrap();
            };
            let a = mk("session", "Festival day 2", day(40, 20), Some(day(39, 3)), "Long, warm night. Came up anxious, settled after an hour.\n\nWater every hour worked.", Some(7));
            dose(a, "MDMA", 120.0, "mg", day(40, 20));
            dose(a, "MDMA", 80.0, "mg", day(40, 22));
            crate::db::add_timeline_event(c, &serde_json::from_value(json!({ "experience_id": a, "at": day(40, 21), "note": "Coming up, a bit anxious", "intensity": 5 })).unwrap()).unwrap();
            let b = mk("session", "", day(6, 21), Some(day(6, 21)), "", None);
            dose(b, "Ketamine", 40.0, "mg", day(6, 21));
            mk("note", "Couldn't sleep", day(3, 2), None, "Thinking about the weekend. Want to plan it more carefully this time.", None);
            let d = mk("session", "", day(1, 22), Some(day(1, 22)), "", None);
            dose(d, "Cannabis", 0.3, "g", day(1, 22));
            Ok(())
        })
        .unwrap();
        println!("DEV_PORTAL port={port} token={token}");
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }

    /// Revoking a device shuts it out on its very next request, while the portal
    /// keeps running for everyone else.
    #[test]
    fn a_revoked_device_is_shut_out_immediately() {
        let (app, port, token) = serving();
        let (other, other_token) = app.state::<Devices>().pair("Laptop").unwrap();

        let (status, _) = post(port, "list_experiences", Some(&other_token), json!({}));
        assert_eq!(status, 200);
        app.state::<Devices>().revoke(other.id).unwrap();
        let (status, _) = post(port, "list_experiences", Some(&other_token), json!({}));
        assert_eq!(status, 401, "a revoked device must be refused");

        let (status, _) = post(port, "list_experiences", Some(&token), json!({}));
        assert_eq!(status, 200, "revoking one device must not affect another");
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
                ..Default::default()
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
        app.manage(Db::new(Some(conn), std::env::temp_dir().join("fn-export-test-unused.db")));

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
    fn the_companion_is_off_until_the_desktop_turns_it_on() {
        let portal = Portal::default();
        assert!(!portal.companion_enabled());
        portal.set_companion_enabled(true);
        assert!(portal.companion_enabled());
    }

    #[test]
    fn the_phone_can_read_the_companion_switch_but_not_flip_it() {
        assert!(EXPOSED.contains(&"companion_enabled"));
        assert!(!EXPOSED.contains(&"set_companion_enabled"));
        assert!(EXPOSED.contains(&"discreet_available"));
        assert!(!EXPOSED.contains(&"set_discreet_available"));
    }

    /// Both halves of the job flow must be on the allowlist, and the blocking call
    /// stays for phones running an older frontend.
    #[test]
    fn the_companion_job_commands_are_exposed() {
        assert!(EXPOSED.contains(&"companion_chat_start"));
        assert!(EXPOSED.contains(&"companion_chat_poll"));
        assert!(EXPOSED.contains(&"companion_chat"));
    }

    /// Usage stats is read-only, so it belongs on the phone: allowlisted on
    /// purpose, and routed for a desktop whose journal is on a server.
    #[test]
    fn usage_stats_is_reachable_from_a_phone_and_a_client() {
        assert!(EXPOSED.contains(&"usage_stats"));
        assert!(crate::remote::ROUTED.contains(&"usage_stats"));
    }

    /// A phone may install a server update, but may never turn that permission on.
    #[test]
    fn a_phone_can_install_updates_but_not_grant_itself_the_right() {
        assert!(EXPOSED.contains(&"server_update_status"));
        assert!(EXPOSED.contains(&"server_update_install"));
        assert!(!EXPOSED.contains(&"set_phone_can_update"));
        assert!(!EXPOSED.contains(&"set_server_prefs"));
        assert!(!EXPOSED.contains(&"set_menu_bar"));
    }
}
