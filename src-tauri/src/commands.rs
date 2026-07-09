// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Tauri command surface exposed to the Svelte frontend. All journal data lives
//! in a single on-device SQLite connection guarded by a mutex.

use crate::db::{self, *};
use crate::interactions::{self, Warning};
use crate::ollama::{self, AiStatus, ChatMsg};
use crate::pw::{self, PwInfo};
use crate::Db;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::Path;
use tauri::{AppHandle, State};

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
pub fn interaction_classes() -> Vec<&'static str> {
    interactions::CLASSES.to_vec()
}

#[tauri::command]
pub fn list_substances(db: State<'_, Db>) -> Result<Vec<Substance>, String> {
    db.with(|c| db::list_substances(c))
}

#[tauri::command]
pub fn add_substance(db: State<'_, Db>, input: SubstanceInput) -> Result<Substance, String> {
    db.with(|c| db::add_substance(c, &input))
}

#[tauri::command]
pub fn check_combo(names: Vec<String>) -> Vec<Warning> {
    let subs: Vec<(String, Vec<String>)> =
        names.into_iter().map(|n| { let c = interactions::builtin_classes(&n); (n, c) }).collect();
    interactions::check(&subs)
}

#[tauri::command]
pub fn create_experience(db: State<'_, Db>, input: ExperienceInput) -> Result<Experience, String> {
    db.with(|c| db::create_experience(c, &input))
}

#[tauri::command]
pub fn list_experiences(db: State<'_, Db>) -> Result<Vec<ExperienceSummary>, String> {
    db.with(|c| db::list_experiences(c))
}

#[tauri::command]
pub fn get_experience(db: State<'_, Db>, id: i64) -> Result<ExperienceDetail, String> {
    db.with(|c| db::get_experience(c, id))
}

#[tauri::command]
pub fn end_experience(
    db: State<'_, Db>,
    id: i64,
    ended_at: String,
    rating: Option<i64>,
    notes: String,
) -> Result<Experience, String> {
    db.with(|c| db::end_experience(c, id, &ended_at, rating, &notes))
}

#[derive(Serialize)]
pub struct LogDoseResult {
    pub dose: Dose,
    pub warnings: Vec<Warning>,
}

#[tauri::command]
pub fn log_dose(db: State<'_, Db>, input: DoseInput) -> Result<LogDoseResult, String> {
    let (dose, warnings) = db.with(|c| db::log_dose(c, &input))?;
    Ok(LogDoseResult { dose, warnings })
}

#[tauri::command]
pub fn add_timeline_event(db: State<'_, Db>, input: TimelineInput) -> Result<TimelineEvent, String> {
    db.with(|c| db::add_timeline_event(c, &input))
}

#[tauri::command]
pub fn usage_by_substance(db: State<'_, Db>) -> Result<Vec<SubstanceUsage>, String> {
    db.with(|c| db::usage_by_substance(c))
}

// ---------- edit & delete ----------

#[tauri::command]
pub fn update_experience(db: State<'_, Db>, id: i64, update: ExperienceUpdate) -> Result<Experience, String> {
    db.with(|c| db::update_experience(c, id, &update))
}

#[tauri::command]
pub fn update_dose(db: State<'_, Db>, id: i64, update: DoseUpdate) -> Result<Dose, String> {
    db.with(|c| db::update_dose(c, id, &update))
}

#[tauri::command]
pub fn delete_experience(db: State<'_, Db>, id: i64) -> Result<(), String> {
    db.with(|c| db::delete_experience(c, id))
}

#[tauri::command]
pub fn delete_dose(db: State<'_, Db>, id: i64) -> Result<(), String> {
    db.with(|c| db::delete_dose(c, id))
}

#[tauri::command]
pub fn delete_timeline_event(db: State<'_, Db>, id: i64) -> Result<(), String> {
    db.with(|c| db::delete_timeline_event(c, id))
}

#[tauri::command]
pub fn delete_substance(db: State<'_, Db>, id: i64) -> Result<(), String> {
    db.with(|c| db::delete_substance(c, id))
}

// ---------- DoseWiki reference cache ----------

#[derive(Serialize)]
pub struct PwStatus {
    pub count: i64,
    /// Date of the bundled DoseWiki snapshot (not a per-device fetch time).
    pub snapshot: &'static str,
}

/// (Re)load the reference cache from the bundled DoseWiki snapshot. This reads a
/// local resource file only — no network call is ever made.
#[tauri::command]
pub fn pw_update(app: AppHandle, db: State<'_, Db>) -> Result<usize, String> {
    let subs = pw::load_bundled(&app)?;
    db.with_mut(|c| db::pw_replace_all(c, &subs))
}

#[tauri::command]
pub fn pw_status(db: State<'_, Db>) -> Result<PwStatus, String> {
    let (count, _last_fetched) = db.with(|c| db::pw_status(c))?;
    Ok(PwStatus { count, snapshot: pw::DOSEWIKI_SNAPSHOT })
}

/// Reload the bundled dose reference into the (open) cache. Shared by startup and
/// the unlock flow. Silent on failure — the reference is non-critical.
pub(crate) fn refresh_dose_reference(app: &AppHandle, db: &Db) {
    match pw::load_bundled(app) {
        Ok(subs) => {
            if let Err(e) = db.with_mut(|c| db::pw_replace_all(c, &subs)) {
                eprintln!("failed to populate dose reference cache: {e}");
            }
        }
        Err(e) => eprintln!("failed to load bundled dose reference: {e}"),
    }
}

#[tauri::command]
pub fn pw_lookup(db: State<'_, Db>, name: String) -> Result<Option<PwInfo>, String> {
    db.with(|c| db::pw_lookup(c, &name))
}

// ---------- local AI setup (Ollama) ----------

#[tauri::command]
pub fn ai_status() -> AiStatus {
    ollama::status()
}

#[tauri::command]
pub fn ai_recommended_models() -> Vec<(String, String)> {
    ollama::RECOMMENDED_MODELS.iter().map(|(t, l)| (t.to_string(), l.to_string())).collect()
}

#[tauri::command]
pub async fn ai_install(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || ollama::install(&app))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn ai_start() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(ollama::ensure_serving)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn ai_pull(app: AppHandle, tag: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || ollama::pull(&app, &tag))
        .await
        .map_err(|e| e.to_string())?
}

// ---------- companion (local LLM) ----------

#[tauri::command]
pub fn ollama_up() -> bool {
    ollama::api_up()
}

#[tauri::command]
pub fn ollama_models() -> Vec<String> {
    ollama::list_models()
}

/// Build a read-only context string describing the doses + interaction flags of
/// an experience, so the companion is aware of the current session.
fn session_context(conn: &rusqlite::Connection, id: i64) -> Option<String> {
    let detail = db::get_experience(conn, id).ok()?;
    if detail.doses.is_empty() {
        return None;
    }
    let title = if detail.experience.title.is_empty() {
        "untitled".to_string()
    } else {
        detail.experience.title.clone()
    };
    let mut s = format!("CURRENT SESSION CONTEXT (from the user's private journal).\nExperience: \"{title}\".\nDoses logged so far:\n");
    for d in &detail.doses {
        let amt = d.amount.map(|a| a.to_string()).unwrap_or_else(|| "?".into());
        let route = if d.route.is_empty() { String::new() } else { format!(" {}", d.route) };
        s.push_str(&format!("- {} {}{}{}\n", d.substance_name, amt, d.unit, route));
    }

    let names: Vec<String> = detail
        .doses
        .iter()
        .map(|d| d.substance_name.clone())
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect();
    let subs: Vec<(String, Vec<String>)> =
        names.iter().map(|n| (n.clone(), interactions::builtin_classes(n))).collect();
    let mut warns = interactions::check(&subs);
    warns.extend(db::pw_interaction_warnings(conn, &names));
    let warns = interactions::dedup_pairs(warns);
    if !warns.is_empty() {
        s.push_str("Known interaction flags for this combination:\n");
        for w in &warns {
            s.push_str(&format!("- [{}] {} + {}: {}\n", w.severity, w.a, w.b, w.message));
        }
    }
    Some(s)
}

/// Parse a pasted free-text experience into a structured preview (nothing saved).
#[tauri::command]
pub fn parse_experience(model: String, text: String) -> Result<ollama::ParsedExperience, String> {
    ollama::parse_experience(&model, &text)
}

/// Commit a (reviewed) parsed experience to the journal: experience + doses +
/// timeline. Missing timestamps fall back to the experience start.
#[tauri::command]
pub fn import_experience(db: State<'_, Db>, parsed: ollama::ParsedExperience) -> Result<Experience, String> {
    let guard = db.conn.lock().unwrap();
    let conn = guard.as_ref().ok_or_else(Db::locked_err)?;
    let started = match parsed.started_at.as_deref() {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')", [], |r| r.get::<_, String>(0)).map_err(err)?,
    };

    let exp = db::create_experience(
        &conn,
        &ExperienceInput {
            title: if parsed.title.is_empty() { "Imported experience".into() } else { parsed.title.clone() },
            intention: parsed.intention.clone(),
            setting: parsed.setting.clone(),
            started_at: started.clone(),
        },
    ).map_err(err)?;

    if !parsed.notes.is_empty() {
        db::update_experience(&conn, exp.id, &ExperienceUpdate {
            title: exp.title.clone(),
            intention: exp.intention.clone(),
            setting: exp.setting.clone(),
            notes: parsed.notes.clone(),
            rating: None,
            started_at: started.clone(),
            ended_at: None,
        }).map_err(err)?;
    }

    for d in &parsed.doses {
        if d.substance.trim().is_empty() {
            continue;
        }
        let taken = d.taken_at.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| started.clone());
        let unit = if d.unit.is_empty() { "mg".to_string() } else { d.unit.clone() };
        db::log_dose(&conn, &DoseInput {
            experience_id: exp.id,
            substance_name: d.substance.clone(),
            amount: d.amount,
            unit,
            route: d.route.clone(),
            taken_at: taken,
            note: d.note.clone(),
        }).map_err(err)?;
    }

    for t in &parsed.timeline {
        if t.note.trim().is_empty() {
            continue;
        }
        let at = t.at.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| started.clone());
        db::add_timeline_event(&conn, &TimelineInput {
            experience_id: exp.id,
            at,
            note: t.note.clone(),
            mood: t.mood.clone(),
            intensity: t.intensity,
        }).map_err(err)?;
    }

    Ok(exp)
}

#[derive(Serialize)]
pub struct CompanionReply {
    pub reply: String,
    /// Human-readable descriptions of any journal actions the model took.
    pub actions: Vec<String>,
    /// True if the model changed the journal (so the UI should refresh).
    pub journal_changed: bool,
}

fn sys(content: impl Into<String>) -> serde_json::Value {
    serde_json::json!({ "role": "system", "content": content.into() })
}

/// Tool definitions offered to the Companion during an active session.
fn companion_tools() -> serde_json::Value {
    serde_json::json!([
        { "type": "function", "function": {
            "name": "log_dose",
            "description": "Record a dose the person reports having just taken, in the current session. Only call this when they clearly state they took something. Never suggest or initiate dosing.",
            "parameters": { "type": "object", "properties": {
                "substance": { "type": "string" },
                "amount": { "type": "number", "description": "amount taken; omit if unknown" },
                "unit": { "type": "string", "description": "e.g. mg, g, ug, ml" },
                "route": { "type": "string", "description": "e.g. oral, insufflated, sublingual" },
                "note": { "type": "string" }
            }, "required": ["substance"] }
        }},
        { "type": "function", "function": {
            "name": "add_note",
            "description": "Add a note/feeling to the session timeline at the current time.",
            "parameters": { "type": "object", "properties": {
                "note": { "type": "string" },
                "mood": { "type": "string" },
                "intensity": { "type": "integer", "description": "1-10 subjective intensity, if given" }
            }, "required": ["note"] }
        }},
        { "type": "function", "function": {
            "name": "session_status",
            "description": "Get a summary of the current session: doses logged so far and any known interaction flags. Use for 'how am I doing?'.",
            "parameters": { "type": "object", "properties": {} }
        }},
        { "type": "function", "function": {
            "name": "lookup_dose",
            "description": "Look up the bundled dose reference (ranges, duration) for a substance. Facts only — never a prescription.",
            "parameters": { "type": "object", "properties": {
                "substance": { "type": "string" }
            }, "required": ["substance"] }
        }},
        { "type": "function", "function": {
            "name": "check_interactions",
            "description": "Check known interaction risks between two or more substances using the deterministic safety checker.",
            "parameters": { "type": "object", "properties": {
                "substances": { "type": "array", "items": { "type": "string" } }
            }, "required": ["substances"] }
        }}
    ])
}

fn arg_obj(call: &serde_json::Value) -> serde_json::Value {
    match call.pointer("/function/arguments") {
        Some(serde_json::Value::String(s)) => {
            serde_json::from_str(s).unwrap_or_else(|_| serde_json::json!({}))
        }
        Some(v) => v.clone(),
        None => serde_json::json!({}),
    }
}

fn now_iso(conn: &rusqlite::Connection) -> rusqlite::Result<String> {
    conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')", [], |r| r.get(0))
}

/// Execute one Companion tool call against the journal. Returns (result text for
/// the model, optional human-readable action description, whether the journal changed).
fn run_companion_tool(
    db: &Db,
    experience_id: Option<i64>,
    name: &str,
    args: &serde_json::Value,
) -> Result<(String, Option<String>, bool), String> {
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    match name {
        "log_dose" => {
            let Some(id) = experience_id else {
                return Ok(("No active session to log into.".into(), None, false));
            };
            let substance = s("substance");
            if substance.trim().is_empty() {
                return Ok(("Missing substance name; nothing logged.".into(), None, false));
            }
            let amount = args.get("amount").and_then(|v| v.as_f64());
            let unit = { let u = s("unit"); if u.is_empty() { "mg".into() } else { u } };
            let route = s("route");
            let note = s("note");
            let (dose, warns) = db.with(|c| {
                let now = now_iso(c)?;
                db::log_dose(c, &DoseInput {
                    experience_id: id,
                    substance_name: substance.clone(),
                    amount,
                    unit: unit.clone(),
                    route: route.clone(),
                    taken_at: now,
                    note: note.clone(),
                })
            })?;
            let amt = dose.amount.map(|a| format!("{a} {}", dose.unit)).unwrap_or_else(|| dose.unit.clone());
            let desc = format!("Logged {amt} {}{}", dose.substance_name, if dose.route.is_empty() { String::new() } else { format!(" ({})", dose.route) });
            let mut result = format!("Logged: {desc}.");
            if !warns.is_empty() {
                result.push_str(" Interaction flags: ");
                result.push_str(&warns.iter().map(|w| format!("[{}] {} + {}: {}", w.severity, w.a, w.b, w.message)).collect::<Vec<_>>().join("; "));
            }
            Ok((result, Some(desc), true))
        }
        "add_note" => {
            let Some(id) = experience_id else {
                return Ok(("No active session to note into.".into(), None, false));
            };
            let note = s("note");
            if note.trim().is_empty() {
                return Ok(("Empty note; nothing added.".into(), None, false));
            }
            let mood = s("mood");
            let intensity = args.get("intensity").and_then(|v| v.as_i64());
            db.with(|c| {
                let now = now_iso(c)?;
                db::add_timeline_event(c, &TimelineInput {
                    experience_id: id,
                    at: now,
                    note: note.clone(),
                    mood: mood.clone(),
                    intensity,
                })
            })?;
            Ok(("Note added to the timeline.".into(), Some("Added a timeline note".into()), true))
        }
        "session_status" => {
            let Some(id) = experience_id else {
                return Ok(("No active session.".into(), None, false));
            };
            let ctx = db.with(|c| Ok(session_context(c, id)))?;
            Ok((ctx.unwrap_or_else(|| "No doses logged in this session yet.".into()), None, false))
        }
        "lookup_dose" => {
            let substance = s("substance");
            let info = db.with(|c| db::pw_lookup(c, &substance))?;
            match info {
                Some(pi) => {
                    let mut out = format!("Dose reference for {}:", pi.name);
                    for roa in &pi.roas {
                        let rng = |r: &pw::Range| match (r.min, r.max) {
                            (Some(a), Some(b)) => format!("{a}-{b}"),
                            (Some(a), None) => format!("{a}+"),
                            _ => "?".into(),
                        };
                        out.push_str(&format!(
                            " [{}] {} light {}, common {}, strong {}.",
                            roa.name, roa.units.clone().unwrap_or_default(), rng(&roa.light), rng(&roa.common), rng(&roa.strong)
                        ));
                    }
                    out.push_str(" Reference only, not a prescription.");
                    Ok((out, None, false))
                }
                None => Ok((format!("No dose reference found for '{substance}'."), None, false)),
            }
        }
        "check_interactions" => {
            let names: Vec<String> = args
                .get("substances")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default();
            if names.len() < 2 {
                return Ok(("Need at least two substances to check.".into(), None, false));
            }
            let subs: Vec<(String, Vec<String>)> =
                names.iter().map(|n| (n.clone(), interactions::builtin_classes(n))).collect();
            let mut warns = interactions::check(&subs);
            warns.extend(db.with(|c| Ok(db::pw_interaction_warnings(c, &names)))?);
            let warns = interactions::dedup_pairs(warns);
            if warns.is_empty() {
                Ok(("No known interaction flags for that combination. Absence of a flag does not mean it's safe.".into(), None, false))
            } else {
                let text = warns.iter().map(|w| format!("[{}] {} + {}: {}", w.severity, w.a, w.b, w.message)).collect::<Vec<_>>().join("; ");
                Ok((format!("Interaction flags: {text}"), None, false))
            }
        }
        other => Ok((format!("Unknown tool '{other}'."), None, false)),
    }
}

#[tauri::command]
pub fn companion_chat(
    db: State<'_, Db>,
    model: String,
    history: Vec<ChatMsg>,
    experience_id: Option<i64>,
    support_style: Option<String>,
) -> Result<CompanionReply, String> {
    let mut messages: Vec<serde_json::Value> = vec![sys(ollama::SYSTEM_PROMPT)];
    if let Some(style) = support_style.as_deref().filter(|s| !s.is_empty()) {
        messages.push(sys(format!(
            "The person has chosen this kind of support for now: \"{style}\". Honor it, and gently re-offer to adjust if it seems to change."
        )));
    }
    if let Some(id) = experience_id {
        let ctx = db.with(|c| Ok(session_context(c, id)))?;
        if let Some(ctx) = ctx {
            messages.push(sys(ctx));
        }
    }
    for m in &history {
        messages.push(serde_json::json!({ "role": m.role, "content": m.content }));
    }

    // Tools are only offered when there's a session to act on.
    let tools = if experience_id.is_some() { companion_tools() } else { serde_json::json!([]) };
    let mut actions: Vec<String> = Vec::new();
    let mut changed = false;
    let mut last_content = String::new();

    // Bounded tool loop: the model may call tools, we run them, feed results back.
    for _ in 0..5 {
        let msg = ollama::chat_tools(&model, &messages, &tools)?;
        last_content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string();
        let calls = msg.get("tool_calls").and_then(|t| t.as_array()).cloned().unwrap_or_default();
        if calls.is_empty() {
            return Ok(CompanionReply { reply: last_content, actions, journal_changed: changed });
        }
        // Record the assistant's tool-call turn, then answer each call.
        messages.push(msg.clone());
        for call in &calls {
            let name = call.pointer("/function/name").and_then(|n| n.as_str()).unwrap_or("").to_string();
            let args = arg_obj(call);
            let (result, desc, did_change) = run_companion_tool(db.inner(), experience_id, &name, &args)?;
            if let Some(d) = desc {
                actions.push(d);
            }
            changed |= did_change;
            messages.push(serde_json::json!({ "role": "tool", "tool_name": name, "content": result }));
        }
    }

    // Ran the loop out — return whatever text we have (or a gentle fallback).
    let reply = if last_content.is_empty() {
        "I've done what I can with that — how are you feeling now?".to_string()
    } else {
        last_content
    };
    Ok(CompanionReply { reply, actions, journal_changed: changed })
}

// ---------- crisis escalation (deterministic) ----------

/// Scan a message for crisis signals, independent of the language model. If a
/// session is active and its combination is flagged dangerous, elevate to medical.
#[tauri::command]
pub fn crisis_scan(db: State<'_, Db>, text: String, experience_id: Option<i64>) -> crate::crisis::CrisisResult {
    let mut result = crate::crisis::scan(&text);
    if let Some(id) = experience_id {
        let has_danger = db
            .with(|c| {
                let detail = db::get_experience(c, id)?;
                let names: Vec<String> = detail
                    .doses
                    .iter()
                    .map(|d| d.substance_name.clone())
                    .collect::<BTreeSet<String>>()
                    .into_iter()
                    .collect();
                let subs: Vec<(String, Vec<String>)> =
                    names.iter().map(|n| (n.clone(), interactions::builtin_classes(n))).collect();
                let mut warns = interactions::check(&subs);
                warns.extend(db::pw_interaction_warnings(c, &names));
                Ok(warns.iter().any(|w| w.severity == "danger"))
            })
            .unwrap_or(false);
        if has_danger {
            result = crate::crisis::escalate(result, crate::crisis::Level::Medical, "a dangerous interaction is flagged in this session");
        }
    }
    result
}

/// The full list of emergency/support resources — for the always-available panic screen.
#[tauri::command]
pub fn emergency_resources() -> Vec<crate::crisis::Resource> {
    crate::crisis::all_resources()
}

// ---------- encryption at rest & backups ----------

#[derive(Serialize)]
pub struct DbStatus {
    /// Is the journal file on disk encrypted (SQLCipher)?
    pub encrypted: bool,
    /// Is there a live, usable connection this session? (An encrypted journal is
    /// locked — `encrypted && !unlocked` — until the passphrase is entered.)
    pub unlocked: bool,
}

#[tauri::command]
pub fn db_status(db: State<'_, Db>) -> DbStatus {
    DbStatus { encrypted: db::is_encrypted(&db.path), unlocked: db.is_unlocked() }
}

/// Open a locked (encrypted) journal with the supplied passphrase.
#[tauri::command]
pub fn unlock_db(app: AppHandle, db: State<'_, Db>, passphrase: String) -> Result<(), String> {
    {
        let mut guard = db.conn.lock().unwrap();
        if guard.is_some() {
            return Ok(()); // already unlocked
        }
        let conn = db::open(&db.path, Some(&passphrase))
            .map_err(|_| "Incorrect passphrase.".to_string())?;
        *guard = Some(conn);
    }
    // Repopulate the (in-DB) dose reference from the bundled snapshot, as at startup.
    refresh_dose_reference(&app, db.inner());
    Ok(())
}

/// Close, re-key, and reopen the journal file in place. `from`/`to` are the
/// current/new SQLCipher keys (`None`/empty = plaintext). A wrong `from` key is
/// caught before the live connection is closed, so it can't lock the user out.
fn rekey(
    guard: &mut Option<rusqlite::Connection>,
    path: &Path,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<(), String> {
    if from.map(|k| !k.is_empty()).unwrap_or(false) {
        // Verify the current passphrase against the file before touching state.
        db::open(path, from).map_err(|_| "Incorrect passphrase.".to_string())?;
    }
    *guard = None; // release the live connection so the file can be rewritten
    if let Err(e) = db::convert(path, from, to) {
        // Conversion failed — reopen with the original key so we aren't left locked.
        if let Ok(c) = db::open(path, from) {
            *guard = Some(c);
        }
        return Err(e.to_string());
    }
    *guard = Some(db::open(path, to).map_err(err)?);
    Ok(())
}

/// Turn on encryption at rest for a currently-plaintext, unlocked journal.
#[tauri::command]
pub fn enable_encryption(db: State<'_, Db>, passphrase: String) -> Result<(), String> {
    if passphrase.is_empty() {
        return Err("Choose a passphrase.".to_string());
    }
    let mut guard = db.conn.lock().unwrap();
    if guard.is_none() {
        return Err(Db::locked_err());
    }
    if db::is_encrypted(&db.path) {
        return Err("The journal is already encrypted.".to_string());
    }
    rekey(&mut guard, &db.path, None, Some(&passphrase))
}

/// Turn encryption off, returning the journal to plaintext. Requires the passphrase.
#[tauri::command]
pub fn disable_encryption(db: State<'_, Db>, passphrase: String) -> Result<(), String> {
    let mut guard = db.conn.lock().unwrap();
    if guard.is_none() {
        return Err(Db::locked_err());
    }
    if !db::is_encrypted(&db.path) {
        return Err("The journal is not encrypted.".to_string());
    }
    rekey(&mut guard, &db.path, Some(&passphrase), None)
}

/// Change the passphrase of an encrypted journal.
#[tauri::command]
pub fn change_passphrase(
    db: State<'_, Db>,
    current: String,
    new_passphrase: String,
) -> Result<(), String> {
    if new_passphrase.is_empty() {
        return Err("Choose a new passphrase.".to_string());
    }
    let mut guard = db.conn.lock().unwrap();
    if guard.is_none() {
        return Err(Db::locked_err());
    }
    if !db::is_encrypted(&db.path) {
        return Err("The journal is not encrypted.".to_string());
    }
    rekey(&mut guard, &db.path, Some(&current), Some(&new_passphrase))
}

/// Write a single-file copy of the journal to `path` (chosen via the frontend's
/// save dialog). The backup keeps the source's encryption state.
#[tauri::command]
pub fn export_backup(db: State<'_, Db>, path: String) -> Result<(), String> {
    db.with(|c| db::backup_to(c, Path::new(&path)))
}

/// Replace the live journal with the file at `path` (chosen via the frontend's
/// open dialog). If the imported file is encrypted it stays locked until unlocked.
#[tauri::command]
pub fn import_backup(app: AppHandle, db: State<'_, Db>, path: String) -> Result<(), String> {
    let src = Path::new(&path);
    if !src.exists() {
        return Err("That backup file does not exist.".to_string());
    }
    {
        let mut guard = db.conn.lock().unwrap();
        *guard = None; // close the live connection before overwriting the file
        let _ = std::fs::remove_file(db.path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db.path.with_extension("db-shm"));
        std::fs::copy(src, &db.path).map_err(err)?;
        if db::is_encrypted(&db.path) {
            return Ok(()); // imported an encrypted journal — leave it locked
        }
        *guard = Some(db::open(&db.path, None).map_err(err)?);
    }
    refresh_dose_reference(&app, db.inner());
    Ok(())
}

// ---------- Obsidian vault sync ----------

/// Export every experience to the chosen Obsidian vault folder as Markdown notes.
#[tauri::command]
pub fn obsidian_export(db: State<'_, Db>, folder: String) -> Result<crate::obsidian::ExportResult, String> {
    db.with(|c| Ok(crate::obsidian::export_all(c, Path::new(&folder))))?
}

/// Import Field Notes notes from the chosen Obsidian vault folder back into the journal.
#[tauri::command]
pub fn obsidian_import(db: State<'_, Db>, folder: String) -> Result<crate::obsidian::ImportResult, String> {
    db.with(|c| Ok(crate::obsidian::import_all(c, Path::new(&folder))))?
}
