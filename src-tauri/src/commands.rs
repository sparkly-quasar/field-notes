// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Tauri command surface exposed to the Svelte frontend. All journal data lives
//! in a single on-device SQLite connection guarded by a mutex.

use crate::contribute;
use crate::db::{self, *};
use crate::interactions::{self, Warning};
use crate::knowledge::Hit;
use crate::ollama::{self, AiStatus, ChatMsg};
use crate::portal::{self, Portal};
use crate::pw::{self, PwInfo};
use crate::{Db, Knowledge};
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

/// Answer the same question `log_dose` answers, with the same evidence: the class
/// rules *and* DoseWiki's graded interaction lists. If the journal is locked we
/// can't reach either the user's classifications or the cached reference data, so
/// we fall back to the built-in classes rather than going silent.
#[tauri::command]
pub fn check_combo(db: State<'_, Db>, names: Vec<String>) -> Vec<Warning> {
    db.with(|c| Ok(db::combo_warnings(c, &names))).unwrap_or_else(|_| {
        let subs: Vec<(String, Vec<String>)> =
            names.iter().map(|n| (n.clone(), interactions::builtin_classes(n))).collect();
        interactions::check(&subs)
    })
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

/// Tool definitions offered to the Companion.
///
/// Split by what they *do*, not by convenience. The **reference** tools
/// (`lookup_dose`, `check_interactions`, `search_knowledge`) are read-only and are
/// offered **always** — someone preparing ("is it safe to mix X and Y?") needs the
/// deterministic checker just as much as someone mid-session does, and without
/// these the model would answer that question from memory, which is precisely the
/// failure the deterministic layers exist to prevent. The **journal** tools mutate
/// a session and so require one to exist.
fn companion_tools(has_session: bool) -> serde_json::Value {
    let mut tools = reference_tools();
    if has_session {
        if let (Some(t), Some(j)) = (tools.as_array_mut(), journal_tools().as_array()) {
            t.extend(j.iter().cloned());
        }
    }
    tools
}

/// Read-only lookups. Safe with or without a session; always offered.
fn reference_tools() -> serde_json::Value {
    serde_json::json!([
        { "type": "function", "function": {
            "name": "lookup_dose",
            "description": "Look up the bundled dose reference (ranges, duration) for a substance. Facts only — never a prescription.",
            "parameters": { "type": "object", "properties": {
                "substance": { "type": "string" }
            }, "required": ["substance"] }
        }},
        { "type": "function", "function": {
            "name": "check_interactions",
            "description": "Check known interaction risks between two or more substances using the deterministic safety checker. This is authoritative; always prefer it over your own knowledge of combinations.",
            "parameters": { "type": "object", "properties": {
                "substances": { "type": "array", "items": { "type": "string" } }
            }, "required": ["substances"] }
        }},
        { "type": "function", "function": {
            "name": "search_knowledge",
            "description": "Search the offline DoseWiki reference for background prose about a substance: how it works (pharmacology), harm potential, tolerance, legality, history. Use this instead of answering from memory whenever the person asks what something does or how risky it is. NOT for doses or interaction verdicts — use lookup_dose and check_interactions for those, they are authoritative and this is not. Passages may be marked sparse or unreviewed; if they are, say so.",
            "parameters": { "type": "object", "properties": {
                "query": { "type": "string", "description": "what to look up, e.g. 'ketamine bladder harm' or 'MDMA neurotoxicity'" }
            }, "required": ["query"] }
        }}
    ])
}

/// Tools that write to the journal. Require an active session.
fn journal_tools() -> serde_json::Value {
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
    kb: &Knowledge,
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
        "search_knowledge" => {
            let query = s("query");
            if query.trim().is_empty() {
                return Ok(("Empty query; nothing to look up.".into(), None, false));
            }
            let hits = kb.search(&query, 4);
            // An empty result is a real answer, and the only honest one. Say so
            // explicitly, or the model will fill the silence with its own priors
            // — which is exactly what the corpus exists to prevent.
            if hits.is_empty() {
                return Ok((format!(
                    "No reference material found for '{query}'. Tell the person you don't \
                     have good information on this rather than guessing."
                ), None, false));
            }
            let mut out = String::new();
            let mut caveats: Vec<&str> = Vec::new();
            for h in &hits {
                out.push_str(&format!("\n[DoseWiki — {} · {}]\n{}\n", h.title, h.section, h.text));
                // Coverage is worst for obscure substances — precisely where the
                // person has nowhere else to look. The flag must reach the model.
                if h.thin && !caveats.contains(&"thin") {
                    caveats.push("thin");
                }
                if !h.reviewed && !caveats.contains(&"unreviewed") {
                    caveats.push("unreviewed");
                }
            }
            if !caveats.is_empty() {
                out.push_str(
                    "\nNOTE ON SOURCE QUALITY: some passages above come from DoseWiki entries \
                     that are sparse and/or not editorially reviewed. Say so plainly when you \
                     use them — do not present thin material with full confidence. Dose and \
                     interaction facts must still come from lookup_dose / check_interactions, \
                     never from this prose.",
                );
            }
            Ok((out, None, false))
        }
        other => Ok((format!("Unknown tool '{other}'."), None, false)),
    }
}

/// Search the offline knowledge corpus directly (the UI's reference search).
///
/// Hits carry `thin` / `reviewed` — the UI must show them. See `knowledge.rs`.
#[tauri::command]
pub fn knowledge_search(kb: State<'_, Knowledge>, query: String, limit: Option<usize>) -> Vec<Hit> {
    kb.search(&query, limit.unwrap_or(8).clamp(1, 25))
}

#[derive(Serialize)]
pub struct KnowledgeStatus {
    /// False if the bundled corpus failed to load — the UI should hide search.
    pub available: bool,
    pub chunks: usize,
}

#[tauri::command]
pub fn knowledge_status(kb: State<'_, Knowledge>) -> KnowledgeStatus {
    KnowledgeStatus {
        available: kb.0.is_some(),
        chunks: kb.0.as_ref().map(|i| i.len()).unwrap_or(0),
    }
}

#[tauri::command]
pub fn companion_chat(
    db: State<'_, Db>,
    kb: State<'_, Knowledge>,
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

    // Reference lookups are always offered; journal writes need a session.
    let tools = companion_tools(experience_id.is_some());
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
            let (result, desc, did_change) =
                run_companion_tool(db.inner(), kb.inner(), experience_id, &name, &args)?;
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
            .map_err(|_| "Incorrect password.".to_string())?;
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
        db::open(path, from).map_err(|_| "Incorrect password.".to_string())?;
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
        return Err("Choose a password.".to_string());
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
        return Err("Choose a new password.".to_string());
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
/// save dialog). The backup keeps the source's encryption state; if the journal
/// is plaintext, an optional `password` encrypts just the backup file.
#[tauri::command]
pub fn export_backup(db: State<'_, Db>, path: String, password: Option<String>) -> Result<(), String> {
    let dest = Path::new(&path);
    // VACUUM INTO copies the journal with the same encryption/key as the source.
    db.with(|c| db::backup_to(c, dest))?;
    // If the journal is plaintext but the user wants an encrypted backup, encrypt
    // the copy in place with the chosen password. (An already-encrypted journal's
    // backup inherits its password, so there's nothing more to do.)
    if let Some(pw) = password.filter(|p| !p.is_empty()) {
        if !db::is_encrypted(&db.path) {
            db::convert(dest, None, Some(&pw))?;
        }
    }
    Ok(())
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

// ---------- erase all data / uninstall ----------

/// The on-device directory holding the journal and its sidecar files.
#[tauri::command]
pub fn data_dir(db: State<'_, Db>) -> String {
    db.path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()
}

/// Open the data directory in the OS file manager, so the user can inspect or
/// delete it manually when uninstalling.
#[tauri::command]
pub fn reveal_data_dir(db: State<'_, Db>) -> Result<(), String> {
    let dir = db.path.parent().ok_or("No data directory.")?;
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(dir).spawn().map_err(err)?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(dir).spawn().map_err(err)?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = dir;
        return Err("Opening the data folder isn't supported on this platform.".to_string());
    }
    Ok(())
}

/// Permanently erase all journal data on this device: the journal database and
/// its WAL/SHM sidecars are deleted, then a fresh empty (unencrypted) journal is
/// created so the app stays usable. Irreversible — the caller must confirm first.
#[tauri::command]
pub fn wipe_all_data(app: AppHandle, db: State<'_, Db>) -> Result<(), String> {
    {
        let mut guard = db.conn.lock().unwrap();
        *guard = None; // close the connection so the files can be removed
        for ext in ["db", "db-wal", "db-shm"] {
            let _ = std::fs::remove_file(db.path.with_extension(ext));
        }
        // Recreate a clean, empty, unencrypted journal.
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

// ---------- the phone portal (optional; off by default) ----------
//
// These are desktop-only by construction: `portal.rs` does not put them on its
// allowlist, so the portal cannot be used to reconfigure or disable itself.

#[tauri::command]
pub fn portal_status(portal: State<'_, Portal>) -> portal::PortalStatus {
    portal.status()
}

/// Turn on phone access. Requires an unlocked journal; binds `127.0.0.1` only.
#[tauri::command]
pub fn portal_enable(app: AppHandle) -> Result<portal::PortalStatus, String> {
    portal::start(&app)
}

#[tauri::command]
pub fn portal_disable(portal: State<'_, Portal>) -> portal::PortalStatus {
    portal.stop();
    portal.status()
}

/// The pairing QR, as an inline SVG. It encodes the bearer token, so it is only
/// ever rendered on the desktop screen — it is a key, and it is shown to a camera.
#[tauri::command]
pub fn portal_qr(portal: State<'_, Portal>, url: Option<String>) -> Result<String, String> {
    let status = portal.status();
    let target = url
        .filter(|u| !u.trim().is_empty())
        .or(status.pair_url)
        .ok_or("The portal isn't running.")?;
    let code = qrcode::QrCode::new(target.as_bytes()).map_err(err)?;
    Ok(code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(220, 220)
        .quiet_zone(true)
        .build())
}

#[derive(Serialize)]
pub struct TailscaleStatus {
    /// The `tailscale` CLI is on this machine.
    pub installed: bool,
    /// The tailnet hostname to reach this machine on, if we could read one.
    pub host: Option<String>,
    /// Tailscale is already proxying the tailnet to the portal's port.
    pub serving: bool,
    /// The tailnet URL the phone reaches, once we're serving.
    pub url: Option<String>,
    /// The equivalent command, for anyone who would rather run it themselves or
    /// wants to see what the button does. `portal_serve` runs exactly this.
    pub serve_command: Option<String>,
}

/// Where Tailscale's CLI actually lives. The Mac App Store build hides it inside
/// the .app, which is why `which tailscale` finds nothing on plenty of machines.
fn tailscale_bin() -> Option<String> {
    const PATHS: &[&str] = &[
        "/usr/local/bin/tailscale",
        "/opt/homebrew/bin/tailscale",
        "/usr/bin/tailscale",
        "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
    ];
    PATHS.iter().find(|p| Path::new(p).exists()).map(|p| p.to_string())
}

/// Run a `tailscale` subcommand, returning its stderr as the error. Tailscale's own
/// messages are the useful ones here ("HTTPS must be enabled in the admin console",
/// "not logged in"), and a generic "failed to publish" would throw them away.
fn tailscale_run(bin: &str, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new(bin)
        .args(args)
        .output()
        .map_err(|e| format!("Couldn't run Tailscale: {e}"))?;
    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).trim().to_string());
    }
    let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
    Err(if msg.is_empty() { "Tailscale refused, without saying why.".into() } else { msg })
}

#[tauri::command]
pub fn portal_tailscale(portal: State<'_, Portal>) -> TailscaleStatus {
    let Some(bin) = tailscale_bin() else {
        return TailscaleStatus {
            installed: false,
            host: None,
            serving: false,
            url: None,
            serve_command: None,
        };
    };

    let host = std::process::Command::new(&bin)
        .args(["status", "--json"])
        .output()
        .ok()
        .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok())
        .and_then(|v| {
            let dns = v["Self"]["DNSName"].as_str()?.trim_end_matches('.').to_string();
            (!dns.is_empty()).then_some(dns)
        });

    let port = portal.status().port;

    // Are we already proxying to *our* port? Tailscale may well be serving something
    // else entirely; that isn't us, and turning it off isn't ours to do.
    let serving = port.is_some_and(|p| {
        tailscale_run(&bin, &["serve", "status", "--json"])
            .map(|s| s.contains(&format!("127.0.0.1:{p}")))
            .unwrap_or(false)
    });

    TailscaleStatus {
        installed: true,
        host: host.clone(),
        serving,
        url: (serving && host.is_some()).then(|| format!("https://{}/m", host.unwrap())),
        serve_command: port.map(|p| format!("{bin} serve --bg {p}")),
    }
}

/// Publish the portal to the tailnet: Tailscale terminates HTTPS on the tailnet and
/// proxies to our loopback port. This is the one button that makes the journal
/// reachable from another device, so it stays an explicit, reversible act — and it
/// refuses if the portal isn't actually running, rather than serving a dead port.
#[tauri::command]
pub fn portal_serve(portal: State<'_, Portal>) -> Result<TailscaleStatus, String> {
    let bin = tailscale_bin().ok_or("Tailscale isn't installed on this Mac.")?;
    let port = portal.status().port.ok_or("Turn on phone access first.")?;
    tailscale_run(&bin, &["serve", "--bg", &port.to_string()])?;
    Ok(portal_tailscale(portal))
}

/// Stop publishing. The portal itself keeps running on loopback — this only removes
/// the tailnet's route to it.
#[tauri::command]
pub fn portal_unserve(portal: State<'_, Portal>) -> Result<TailscaleStatus, String> {
    let bin = tailscale_bin().ok_or("Tailscale isn't installed on this Mac.")?;
    tailscale_run(&bin, &["serve", "--https=443", "off"])?;
    Ok(portal_tailscale(portal))
}

// ---------- upstream contribution drafts ----------
//
// Every command here is local. None of them make a network call, and none of them
// may be changed to — see `contribute.rs`. `contribution_save` writes a file to a
// path the user picked in a save dialog; that is the whole of the "export".

/// The user-added substances, flagged with whether they're a genuine gap upstream.
#[tauri::command]
pub fn contribution_candidates(db: State<'_, Db>) -> Result<Vec<contribute::Candidate>, String> {
    db.with(contribute::candidates)
}

/// Build a DoseWiki-shaped draft for one substance, for the user to read before
/// they decide to do anything with it. Building a draft sends nothing.
#[tauri::command]
pub fn contribution_draft(db: State<'_, Db>, id: i64) -> Result<contribute::Draft, String> {
    let conn = db.conn.lock().unwrap();
    let conn = conn.as_ref().ok_or("The journal is locked — unlock it with your password.")?;
    contribute::draft(conn, id)
}

/// Write a reviewed draft to a file the user chose. This is the only "export"
/// there is: it lands on their disk, and submitting it upstream is something they
/// do themselves, by hand.
#[tauri::command]
pub fn contribution_save(db: State<'_, Db>, id: i64, path: String) -> Result<(), String> {
    let draft = contribution_draft(db.clone(), id)?;
    std::fs::write(Path::new(&path), draft.json).map_err(err)?;
    db.with(|c| contribute::mark_contributed(c, id))
}
