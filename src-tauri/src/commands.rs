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
use tauri::{AppHandle, Manager, State};

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
pub fn update_timeline_event(db: State<'_, Db>, id: i64, update: TimelineUpdate) -> Result<TimelineEvent, String> {
    db.with(|c| db::update_timeline_event(c, id, &update))
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

#[tauri::command]
pub async fn ai_remove(app: AppHandle, tag: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || ollama::remove(&app, &tag))
        .await
        .map_err(|e| e.to_string())?
}

/// One-button move to the recommended model: download it, then drop the old one.
#[tauri::command]
pub async fn ai_switch_model(app: AppHandle, from: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || ollama::switch_model(&app, &from))
        .await
        .map_err(|e| e.to_string())??;
    Ok(ollama::PREFERRED_MODEL.to_string())
}

/// Which model the app recommends, so the UI can spot an outdated choice without
/// hardcoding a tag of its own.
#[tauri::command]
pub fn ai_preferred_model() -> &'static str {
    ollama::PREFERRED_MODEL
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
    // A session the person attached for integration is over, not happening now.
    // Without this distinction the model reads a weeks-old experience as live and
    // offers stay-hydrated advice for doses long since worn off. An ended session
    // (has an `ended_at`) is framed as past; an open one stays current.
    let (header, doses_label) = if detail.experience.ended_at.is_some() {
        (
            "A PAST SESSION the person wants to talk through — for reflection or integration, \
             not happening now. Do not treat these doses as still active.",
            "Doses that were logged",
        )
    } else {
        ("CURRENT SESSION CONTEXT (from the user's private journal).", "Doses logged so far")
    };
    let mut s = format!("{header}\nExperience: \"{title}\".\n{doses_label}:\n");
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
            kind: "session".into(),
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
    /// Measured generation speed of the turn's text, in tokens/sec, from Ollama's
    /// own stats. `None` when nothing was generated to measure (e.g. a turn that
    /// was only tool calls). The compute watcher uses it to catch a machine that
    /// has the RAM but is still painfully slow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_per_sec: Option<f64>,
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
                    let mut any_duration = false;
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
                        // Timing matters as much as amount — "how much longer is
                        // this going to last" is one of the most common things
                        // asked mid-experience, and the answer is right here.
                        let timings: Vec<String> = [
                            ("onset", &roa.onset),
                            ("come-up", &roa.come_up),
                            ("peak", &roa.peak),
                            ("offset", &roa.offset),
                            ("total", &roa.total),
                            ("after-effects", &roa.after_effects),
                        ]
                        .iter()
                        .filter_map(|(label, v)| v.as_ref().map(|t| format!("{label} {t}")))
                        .collect();
                        if !timings.is_empty() {
                            any_duration = true;
                            out.push_str(&format!(" Duration — {}.", timings.join(", ")));
                        }
                    }
                    if !any_duration {
                        // Say the gap out loud. An unexplained silence about
                        // duration is exactly what the model fills from memory.
                        out.push_str(
                            " The reference has no duration data for this substance — say so \
                             plainly rather than estimating how long it lasts.",
                        );
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

/// Read one substance's entry whole, by slug (from a hit or the browse list).
///
/// Search shows excerpts; this is how a reader gets from an excerpt to the page
/// it came from. Same caveat as `knowledge_search`: prose only. Doses and combo
/// verdicts come from `pw_lookup` / `check_interactions`, never from here.
#[tauri::command]
pub fn knowledge_entry(kb: State<'_, Knowledge>, slug: String) -> Vec<Hit> {
    kb.entry(&slug)
}

/// Every substance in the corpus, alphabetical — for browsing without a query.
#[tauri::command]
pub fn knowledge_entries(kb: State<'_, Knowledge>) -> Vec<crate::knowledge::Entry> {
    kb.entries()
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

/// Pre-load the model into memory so the *first* message isn't slow.
///
/// Fired best-effort when the Companion comes into view, well before anyone types.
/// The cold load of an 8B model takes tens of seconds; paying it here, off the
/// main thread, means the first real reply arrives at warm speed. Any error is the
/// caller's to ignore — a failed warm-up just means the first message loads the
/// model the old, slow way.
/// Whether this machine has the memory (and, once a reply lands, the speed) to run
/// `model` comfortably. A read-only preflight the Companion setup and chat surface —
/// never a gate, just an honest heads-up. `measured_tps` is the last reply's
/// tokens/sec, or `None` before the first message. Runs off the UI thread because
/// it touches Ollama's `/api/ps`.
#[tauri::command]
pub async fn compute_status(model: String, measured_tps: Option<f64>) -> crate::compute::ComputeStatus {
    tauri::async_runtime::spawn_blocking(move || crate::compute::status(&model, measured_tps))
        .await
        .unwrap_or_else(|_| crate::compute::unknown())
}

#[tauri::command]
pub async fn companion_warm(model: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || ollama::warm(&model))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn companion_chat(
    app: AppHandle,
    model: String,
    history: Vec<ChatMsg>,
    experience_id: Option<i64>,
    support_style: Option<String>,
) -> Result<CompanionReply, String> {
    // A turn is a chain of blocking HTTP calls to the local model — and, on the
    // first message, a cold load of tens of seconds. As a *synchronous* command
    // this ran on Tauri's main thread and froze the whole window until it
    // returned, which reads as a crash. Run it on a blocking-safe thread instead
    // so the UI stays responsive; the phone portal already does this via
    // `companion_chat_start`. State is fetched inside the closure because `State`
    // borrows can't cross the `spawn_blocking` boundary.
    tauri::async_runtime::spawn_blocking(move || {
        let db = app.state::<Db>();
        let kb = app.state::<Knowledge>();
        companion_chat_inner(db.inner(), kb.inner(), model, history, experience_id, support_style)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The Companion's whole conversation turn, free of Tauri's `State` wrappers, so it
/// can run on any thread that can borrow the managed state — the desktop command
/// above, and the portal's background jobs (`portal.rs`), which must not hold an
/// HTTP request open for the minutes a slow local model can take.
pub fn companion_chat_inner(
    db: &Db,
    kb: &Knowledge,
    model: String,
    history: Vec<ChatMsg>,
    experience_id: Option<i64>,
    support_style: Option<String>,
) -> Result<CompanionReply, String> {
    companion_chat_traced(db, kb, model, history, experience_id, support_style, &mut Vec::new())
}

/// Phrases that claim a source, and the tools that license them. Saying "the dose
/// reference says..." is a promise about where a fact came from; making that promise
/// without having called the tool is worse than a plain mistake, because the app's
/// own honesty vocabulary is what makes the invention sound checked.
///
/// Observed in the wild from an 8B model: it cited "the dose reference" *and* the
/// prompt's own "thin and unreviewed, so hold it loosely" hedge, having called
/// neither `lookup_dose` nor `search_knowledge`.
const CITATION_CLAIMS: &[(&str, &[&str])] = &[
    ("dose reference", &["lookup_dose"]),
    ("dosewiki", &["search_knowledge"]),
    ("interaction checker", &["check_interactions"]),
    ("the checker", &["check_interactions"]),
];

/// Source claims in `reply` that none of the `called` tools back up.
pub fn fabricated_citations(reply: &str, called: &[String]) -> Vec<String> {
    let lower = reply.to_lowercase();
    CITATION_CLAIMS
        .iter()
        .filter(|(phrase, licensed_by)| {
            lower.contains(phrase) && !licensed_by.iter().any(|t| called.iter().any(|c| c == t))
        })
        .map(|(phrase, _)| (*phrase).to_string())
        .collect()
}

/// Did the model emit a tool call as ordinary message text instead of through the
/// tool-calling channel? Small models do this under pressure, and the result is raw
/// JSON delivered to someone who may be mid-experience — observed verbatim from an
/// 8B during a heat-stroke scenario, where the entire reply was
/// `assistant {"name": "check_interactions", ...}`.
pub fn looks_like_leaked_tool_call(reply: &str) -> bool {
    let compact: String = reply.chars().filter(|c| !c.is_whitespace()).collect();
    compact.contains("{\"name\":")
        && (compact.contains("\"parameters\":") || compact.contains("\"arguments\":"))
}

/// Remove whole sentences carrying a source claim the model never earned.
///
/// Sentence-level rather than phrase-level on purpose: excising "the dose
/// reference" from "the dose reference says 8-12 hours" leaves a sentence that
/// still asserts the fact, just with the attribution filed off. The claim and
/// its scaffolding go together or not at all.
pub fn strip_sourced_sentences(reply: &str, faked: &[String]) -> String {
    let kept: Vec<&str> = reply
        .split_inclusive(['.', '!', '?', '\n'])
        .filter(|s| {
            let lower = s.to_lowercase();
            !faked.iter().any(|f| lower.contains(f.as_str()))
        })
        .collect();
    kept.join("").trim().to_string()
}

/// Small models sometimes echo the chat template's role header into the message
/// body. It is meaningless to the reader and makes the Companion look broken.
fn strip_role_artifact(content: &str) -> &str {
    content
        .trim_start()
        .strip_prefix("assistant")
        // Only a bare header counts. "assistants can help" is prose, not an artifact.
        .filter(|rest| rest.starts_with(char::is_whitespace))
        .map(|rest| rest.trim_start())
        .filter(|rest| !rest.is_empty())
        .unwrap_or_else(|| content.trim())
}

/// Last line of defence on the way out: an empty reply, or one the model refused to
/// stop putting JSON in, becomes something a person can actually receive.
fn presentable(content: String) -> String {
    let content = strip_role_artifact(&content).to_string();
    if content.trim().is_empty() || looks_like_leaked_tool_call(&content) {
        return "I'm here with you. How are you feeling right now?".to_string();
    }
    content
}

/// A system note carrying the deterministic crisis verdict, when there is one
/// the model needs to act on. `None` for `none` and `peer`.
///
/// The wording is a brief, not a script: it says what was detected and what the
/// person is already being shown, and leaves the Companion to say it in its own
/// voice. Handing it a sentence to recite would produce exactly the flat
/// boilerplate this app is trying not to be.
pub fn crisis_context(history: &[ChatMsg]) -> Option<String> {
    let said: Vec<String> =
        history.iter().filter(|m| m.role == "user").map(|m| m.content.clone()).collect();
    if said.is_empty() {
        return None;
    }
    let result = crate::crisis::scan_recent(&said);
    let what = match result.level {
        crate::crisis::Level::Medical => {
            "signs of a physical medical emergency. Treat it as one. Say clearly and early \
             that this needs medical help now — emergency services, or festival medics if \
             they're at an event. Do not suggest waiting to see whether it improves, and do \
             not offer a timeframe before seeking help."
        }
        crate::crisis::Level::Psychiatric => {
            "risk of suicide or serious self-harm. Stay warm and stay with them; do not \
             lecture or recite hotline numbers at them. Make sure they know help is \
             reachable right now, and that reaching a person tonight matters more than \
             anything else you might discuss."
        }
        _ => return None,
    };
    Some(format!(
        "A deterministic safety check — not you, and not something you can turn off — has \
         detected {what} The person is already seeing crisis resources on screen alongside \
         this conversation; don't contradict them. Signals: {}.",
        result.matched.join(", ")
    ))
}

/// One tool invocation as it actually happened, for the evaluation harness.
///
/// The read-only reference tools deliberately return no user-facing action
/// description (they aren't journal changes, so the UI has nothing to show), which
/// means `CompanionReply.actions` cannot answer the question the harness most needs
/// to ask: *did the model look this up, or did it answer from memory?* This trace
/// does.
#[derive(Debug, Clone, Serialize)]
pub struct ToolTrace {
    pub name: String,
    pub args: serde_json::Value,
    pub result: String,
}

/// [`companion_chat_inner`], recording every tool call into `trace`. The two share
/// one body so the harness can never drift from what the app really runs.
pub fn companion_chat_traced(
    db: &Db,
    kb: &Knowledge,
    model: String,
    history: Vec<ChatMsg>,
    experience_id: Option<i64>,
    support_style: Option<String>,
    trace: &mut Vec<ToolTrace>,
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

    // Tell the model what the deterministic scanner already worked out.
    //
    // Without this the two halves disagree in front of someone in trouble: the
    // banner says get help now, while the chat — having re-derived the situation
    // from scratch — says see how you feel in half an hour. Measured on the
    // `overheating` scenario (heat stroke after MDMA), qwen3 gave wait-and-see
    // advice in 5 of 5 runs while the scan correctly returned `medical` in all 5.
    //
    // Only the two levels that mean "this needs someone who isn't me". `Peer` is
    // deliberately excluded: acute distress is a moment to sit with somebody, and
    // nudging the model toward resources there would trample the non-directive
    // stance the whole Companion is built around.
    if let Some(note) = crisis_context(&history) {
        messages.push(sys(note));
    }

    // Reference lookups are always offered; journal writes need a session.
    let tools = companion_tools(experience_id.is_some());
    let mut actions: Vec<String> = Vec::new();
    let mut changed = false;
    let mut last_content = String::new();
    // Speed of the most recent turn that actually generated text. Tool-only turns
    // report nothing to measure, so we keep the last real number.
    let mut last_tps: Option<f64> = None;

    // Bounded tool loop: the model may call tools, we run them, feed results back.
    let mut citations_corrected = false;
    let mut leak_corrected = false;
    for _ in 0..5 {
        let (msg, perf) = ollama::chat_tools(&model, &messages, &tools)?;
        if let Some(tps) = perf.tokens_per_sec() {
            last_tps = Some((tps * 10.0).round() / 10.0);
        }
        last_content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string();
        let calls = msg.get("tool_calls").and_then(|t| t.as_array()).cloned().unwrap_or_default();
        if calls.is_empty() {
            // Before this answer reaches someone who may be in no state to
            // audit it: did it claim a source it never consulted? Give it one
            // chance to either look the thing up or drop the claim. Costs a
            // round trip, and only when a false claim was actually detected.
            // A leaked tool call is unreadable to the person and can look alarming.
            // Give one chance to answer properly; never pass the JSON through.
            if looks_like_leaked_tool_call(&last_content) && !leak_corrected {
                leak_corrected = true;
                messages.push(sys(
                    "Your last reply contained a raw tool call in the message text. Call the \
                     tool through the tool-calling mechanism, or just reply in plain words. \
                     Never put JSON in a message the person reads.",
                ));
                continue;
            }

            let called: Vec<String> = trace.iter().map(|t| t.name.clone()).collect();
            let faked = fabricated_citations(&last_content, &called);
            if !faked.is_empty() {
                if !citations_corrected {
                    citations_corrected = true;
                    messages.push(msg.clone());
                    // Deliberately a single instruction. An earlier version also
                    // offered "or call the tool now and report what it returns",
                    // and small models took that as a template: they asserted the
                    // call and invented the result. Removing the option removed
                    // the failure. Rewriting is the only way out.
                    messages.push(sys(format!(
                        "Your last reply mentioned {}. You did not read it, so you cannot \
                         say anything about what it contains. Rewrite the reply with that \
                         claim removed entirely. Do not say you looked anything up.",
                        faked.join(" and ")
                    )));
                    continue;
                }
                // It fabricated again after being told not to. Do not ship a
                // false claim to someone who may be in no state to question it:
                // drop the sentences carrying it and send what survives.
                last_content = strip_sourced_sentences(&last_content, &faked);
            }
            return Ok(CompanionReply { reply: presentable(last_content), actions, journal_changed: changed, tokens_per_sec: last_tps });
        }
        // Record the assistant's tool-call turn, then answer each call.
        messages.push(msg.clone());
        for call in &calls {
            let name = call.pointer("/function/name").and_then(|n| n.as_str()).unwrap_or("").to_string();
            let args = arg_obj(call);
            let (result, desc, did_change) =
                run_companion_tool(db, kb, experience_id, &name, &args)?;
            trace.push(ToolTrace { name: name.clone(), args: args.clone(), result: result.clone() });
            if let Some(d) = desc {
                actions.push(d);
            }
            changed |= did_change;
            messages.push(serde_json::json!({ "role": "tool", "tool_name": name, "content": result }));
        }
    }

    // Ran the loop out — return whatever text we have (or a gentle fallback).
    Ok(CompanionReply { reply: presentable(last_content), actions, journal_changed: changed, tokens_per_sec: last_tps })
}

// ---------- crisis escalation (deterministic) ----------

/// Scan a message for crisis signals, independent of the language model. If a
/// session is active and its combination is flagged dangerous, elevate to medical.
#[tauri::command]
pub fn crisis_scan(
    db: State<'_, Db>,
    text: String,
    experience_id: Option<i64>,
    // Earlier messages from the person this conversation, oldest first. Lets
    // expressive distress be judged on repetition rather than on one sentence.
    recent: Option<Vec<String>>,
) -> crate::crisis::CrisisResult {
    let mut run = recent.unwrap_or_default();
    run.push(text);
    let mut result = crate::crisis::scan_recent(&run);
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
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer").arg(dir).spawn().map_err(err)?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
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

// ---------- single-entry export ----------

/// One experience rendered as Markdown, plus the filename it should land under.
#[derive(Serialize)]
pub struct ExportedNote {
    pub filename: String,
    pub markdown: String,
}

/// Render a single experience as a Markdown note (same rendering and filename
/// convention as the Obsidian vault export). Pure: touches no filesystem, so it
/// is safe to expose to the phone, which downloads the text in the browser.
#[tauri::command]
pub fn export_experience_markdown(db: State<'_, Db>, id: i64) -> Result<ExportedNote, String> {
    let detail = db.with(|c| db::get_experience(c, id))?;
    Ok(ExportedNote {
        filename: crate::obsidian::note_filename(&detail),
        markdown: crate::obsidian::render_note(&detail)?,
    })
}

/// Write a single experience's Markdown note to `dest` (a path the user chose in
/// the frontend's save dialog). Desktop-only: it writes to the desktop's
/// filesystem, so `portal.rs` must never allowlist it.
#[tauri::command]
pub fn export_experience_file(db: State<'_, Db>, id: i64, dest: String) -> Result<(), String> {
    let detail = db.with(|c| db::get_experience(c, id))?;
    let markdown = crate::obsidian::render_note(&detail)?;
    std::fs::write(Path::new(&dest), markdown).map_err(err)
}

// ---------- the phone portal (optional; off by default) ----------
//
// These are desktop-only by construction: `portal.rs` does not put them on its
// allowlist, so the portal cannot be used to reconfigure or disable itself.

#[tauri::command]
pub fn portal_status(portal: State<'_, Portal>) -> portal::PortalStatus {
    portal.status()
}

/// Mirror the desktop's companion-off preference so the phone can see it. Set
/// from the desktop only — `companion_enabled` is readable over the portal but
/// this setter is not exposed, per the rule above that the phone cannot
/// reconfigure the desktop.
#[tauri::command]
pub fn set_companion_enabled(portal: State<'_, Portal>, enabled: bool) {
    portal.set_companion_enabled(enabled);
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
/// The Windows installer puts it in Program Files without touching PATH either.
fn tailscale_bin() -> Option<String> {
    #[cfg(not(windows))]
    const PATHS: &[&str] = &[
        "/usr/local/bin/tailscale",
        "/opt/homebrew/bin/tailscale",
        "/usr/bin/tailscale",
        "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
    ];
    #[cfg(windows)]
    const PATHS: &[&str] = &[
        r"C:\Program Files\Tailscale\tailscale.exe",
        r"C:\Program Files (x86)\Tailscale IPN\tailscale.exe",
    ];
    PATHS.iter().find(|p| Path::new(p).exists()).map(|p| p.to_string())
}

/// Run a `tailscale` subcommand, returning its stderr as the error. Tailscale's own
/// messages are the useful ones here ("HTTPS must be enabled in the admin console",
/// "not logged in"), and a generic "failed to publish" would throw them away.
fn tailscale_run(bin: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = std::process::Command::new(bin);
    crate::ollama::hide_console(&mut cmd);
    let out = cmd
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

    let host = tailscale_run(&bin, &["status", "--json"])
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
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
    let bin = tailscale_bin().ok_or("Tailscale isn't installed on this computer.")?;
    let port = portal.status().port.ok_or("Turn on phone access first.")?;
    tailscale_run(&bin, &["serve", "--bg", &port.to_string()])?;
    Ok(portal_tailscale(portal))
}

/// Stop publishing. The portal itself keeps running on loopback — this only removes
/// the tailnet's route to it.
#[tauri::command]
pub fn portal_unserve(portal: State<'_, Portal>) -> Result<TailscaleStatus, String> {
    let bin = tailscale_bin().ok_or("Tailscale isn't installed on this computer.")?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn said(texts: &[&str]) -> Vec<ChatMsg> {
        texts
            .iter()
            .map(|t| ChatMsg { role: "user".into(), content: (*t).to_string() })
            .collect()
    }

    #[test]
    fn a_medical_emergency_is_handed_to_the_model_not_left_for_it_to_infer() {
        let note = crisis_context(&said(&["i'm really hot and i've stopped sweating and feel confused"]))
            .expect("heat stroke should produce a brief");
        assert!(note.contains("medical emergency"), "{note}");
        // The specific failure this exists to prevent: wait-and-see advice.
        assert!(note.contains("Do not suggest waiting"), "{note}");
    }

    #[test]
    fn acute_distress_does_not_push_the_companion_toward_resources() {
        // Peer-level is a moment to sit with someone. If this ever returns a brief,
        // the Companion starts steering people to hotlines when they wanted company.
        assert!(crisis_context(&said(&["i think i'm dying"])).is_none());
    }

    #[test]
    fn ordinary_talk_adds_nothing_to_the_prompt() {
        assert!(crisis_context(&said(&["the visuals are really pretty"])).is_none());
    }

    #[test]
    fn only_what_the_person_said_is_scanned() {
        // An assistant turn mentioning chest pain — asking about it, say — must not
        // be read back as the person reporting it.
        let history = vec![ChatMsg {
            role: "assistant".into(),
            content: "Any chest pain or trouble breathing?".into(),
        }];
        assert!(crisis_context(&history).is_none());
    }

    #[test]
    fn a_repeated_fabrication_loses_the_sentence_that_carried_it() {
        let reply = "I called the dose reference and it says 8-12 hours. \
                     How are you feeling right now?";
        let cleaned = strip_sourced_sentences(reply, &["dose reference".to_string()]);
        assert_eq!(cleaned, "How are you feeling right now?");
    }

    #[test]
    fn stripping_a_claim_never_leaves_the_fact_standing_alone() {
        // The danger is a half-excision that keeps the assertion and drops only
        // the attribution, which reads as the Companion's own knowledge.
        let cleaned = strip_sourced_sentences(
            "The dose reference says LSD lasts 3 hours.",
            &["dose reference".to_string()],
        );
        assert!(!cleaned.contains("3 hours"), "left the claim behind: {cleaned:?}");
    }

    #[test]
    fn a_reply_that_was_only_a_fabrication_falls_back_to_presence() {
        let cleaned = strip_sourced_sentences(
            "The dose reference says you'll be fine.",
            &["dose reference".to_string()],
        );
        assert_eq!(presentable(cleaned), "I'm here with you. How are you feeling right now?");
    }

    #[test]
    fn a_leaked_role_header_does_not_reach_the_person() {
        assert_eq!(presentable("assistant\n\nI'm here.".into()), "I'm here.");
    }

    #[test]
    fn prose_beginning_with_assistants_is_left_alone() {
        let reply = "assistants like me can't diagnose that.";
        assert_eq!(presentable(reply.into()), reply);
    }
}
