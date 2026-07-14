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

use crate::{commands, Db};
use serde_json::{json, Value};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, Runtime};
use tiny_http::{Header, Request, Response, Server};

/// Loopback only. See rule 1 — this is not a preference.
const BIND_ADDR: &str = "127.0.0.1";

/// First choice; we walk upward if it's taken (3000 is a popular port).
const PORT_RANGE: std::ops::Range<u16> = 8787..8807;

pub struct Portal {
    inner: Mutex<Option<Running>>,
}

struct Running {
    server: Arc<Server>,
    port: u16,
    token: String,
    stopping: Arc<AtomicBool>,
}

#[derive(serde::Serialize)]
pub struct PortalStatus {
    pub running: bool,
    pub port: Option<u16>,
    /// The pairing URL, token included. Only ever shown on the desktop screen.
    pub pair_url: Option<String>,
}

impl Default for Portal {
    fn default() -> Self {
        Portal { inner: Mutex::new(None) }
    }
}

impl Portal {
    pub fn status(&self) -> PortalStatus {
        let guard = self.inner.lock().unwrap();
        match guard.as_ref() {
            Some(r) => PortalStatus {
                running: true,
                port: Some(r.port),
                pair_url: Some(format!("http://{BIND_ADDR}:{}/m#t={}", r.port, r.token)),
            },
            None => PortalStatus { running: false, port: None, pair_url: None },
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

    // A small pool: a Companion reply blocks its thread for many seconds, and a
    // phone that can't load the timeline meanwhile looks broken.
    for _ in 0..4 {
        let server = Arc::clone(&server);
        let stopping = Arc::clone(&stopping);
        let app = app.clone();
        let token = token.clone();
        std::thread::spawn(move || {
            while let Ok(req) = server.recv() {
                if stopping.load(Ordering::SeqCst) {
                    break;
                }
                handle(&app, &token, req);
            }
        });
    }

    *portal.inner.lock().unwrap() = Some(Running { server, port, token, stopping });
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

fn handle<R: Runtime>(app: &AppHandle<R>, token: &str, req: Request) {
    let url = req.url().to_string();
    let path = url.split('?').next().unwrap_or("/").to_string();

    if let Some(command) = path.strip_prefix("/api/") {
        let command = command.to_string();
        return api(app, token, &command, req);
    }
    assets(app, &path, req);
}

fn api<R: Runtime>(app: &AppHandle<R>, token: &str, command: &str, mut req: Request) {
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
    "usage_by_substance",
    "list_substances",
    "db_status",
    "create_experience",
    "end_experience",
    "log_dose",
    "add_timeline_event",
    "add_substance",
    "update_experience",
    "update_dose",
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
    "ai_status",
    "ollama_up",
    "ollama_models",
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
        "usage_by_substance" => done(commands::usage_by_substance(db)),
        "list_substances" => done(commands::list_substances(db)),
        "db_status" => ok(commands::db_status(db)),

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
        "companion_chat" => done(commands::companion_chat(
            db,
            app.state(),
            arg(&args, "model")?,
            arg(&args, "history")?,
            arg(&args, "experienceId")?,
            arg(&args, "supportStyle")?,
        )),
        "ai_status" => ok(commands::ai_status()),
        "ollama_up" => ok(commands::ollama_up()),
        "ollama_models" => ok(commands::ollama_models()),

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
            "contribution_save",
            "data_dir",
            "reveal_data_dir",
            // Destroys the journal.
            "wipe_all_data",
            // Installs software / mutates app state.
            "ai_install",
            "ai_pull",
            "ai_start",
            "pw_update",
            // The portal may not reconfigure or disable itself.
            "portal_status",
            "portal_enable",
            "portal_disable",
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

    #[test]
    fn the_phone_cannot_reach_a_desktop_only_command() {
        let (_app, port, token) = serving();
        for cmd in ["wipe_all_data", "unlock_db", "export_backup", "portal_disable"] {
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
}
