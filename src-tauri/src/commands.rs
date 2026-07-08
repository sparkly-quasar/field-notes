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
    db::list_substances(&db.0.lock().unwrap()).map_err(err)
}

#[tauri::command]
pub fn add_substance(db: State<'_, Db>, input: SubstanceInput) -> Result<Substance, String> {
    db::add_substance(&db.0.lock().unwrap(), &input).map_err(err)
}

#[tauri::command]
pub fn check_combo(names: Vec<String>) -> Vec<Warning> {
    let subs: Vec<(String, Vec<String>)> =
        names.into_iter().map(|n| { let c = interactions::builtin_classes(&n); (n, c) }).collect();
    interactions::check(&subs)
}

#[tauri::command]
pub fn create_experience(db: State<'_, Db>, input: ExperienceInput) -> Result<Experience, String> {
    db::create_experience(&db.0.lock().unwrap(), &input).map_err(err)
}

#[tauri::command]
pub fn list_experiences(db: State<'_, Db>) -> Result<Vec<ExperienceSummary>, String> {
    db::list_experiences(&db.0.lock().unwrap()).map_err(err)
}

#[tauri::command]
pub fn get_experience(db: State<'_, Db>, id: i64) -> Result<ExperienceDetail, String> {
    db::get_experience(&db.0.lock().unwrap(), id).map_err(err)
}

#[tauri::command]
pub fn end_experience(
    db: State<'_, Db>,
    id: i64,
    ended_at: String,
    rating: Option<i64>,
    notes: String,
) -> Result<Experience, String> {
    db::end_experience(&db.0.lock().unwrap(), id, &ended_at, rating, &notes).map_err(err)
}

#[derive(Serialize)]
pub struct LogDoseResult {
    pub dose: Dose,
    pub warnings: Vec<Warning>,
}

#[tauri::command]
pub fn log_dose(db: State<'_, Db>, input: DoseInput) -> Result<LogDoseResult, String> {
    let (dose, warnings) = db::log_dose(&db.0.lock().unwrap(), &input).map_err(err)?;
    Ok(LogDoseResult { dose, warnings })
}

#[tauri::command]
pub fn add_timeline_event(db: State<'_, Db>, input: TimelineInput) -> Result<TimelineEvent, String> {
    db::add_timeline_event(&db.0.lock().unwrap(), &input).map_err(err)
}

#[tauri::command]
pub fn usage_by_substance(db: State<'_, Db>) -> Result<Vec<SubstanceUsage>, String> {
    db::usage_by_substance(&db.0.lock().unwrap()).map_err(err)
}

// ---------- edit & delete ----------

#[tauri::command]
pub fn update_experience(db: State<'_, Db>, id: i64, update: ExperienceUpdate) -> Result<Experience, String> {
    db::update_experience(&db.0.lock().unwrap(), id, &update).map_err(err)
}

#[tauri::command]
pub fn update_dose(db: State<'_, Db>, id: i64, update: DoseUpdate) -> Result<Dose, String> {
    db::update_dose(&db.0.lock().unwrap(), id, &update).map_err(err)
}

#[tauri::command]
pub fn delete_experience(db: State<'_, Db>, id: i64) -> Result<(), String> {
    db::delete_experience(&db.0.lock().unwrap(), id).map_err(err)
}

#[tauri::command]
pub fn delete_dose(db: State<'_, Db>, id: i64) -> Result<(), String> {
    db::delete_dose(&db.0.lock().unwrap(), id).map_err(err)
}

#[tauri::command]
pub fn delete_timeline_event(db: State<'_, Db>, id: i64) -> Result<(), String> {
    db::delete_timeline_event(&db.0.lock().unwrap(), id).map_err(err)
}

#[tauri::command]
pub fn delete_substance(db: State<'_, Db>, id: i64) -> Result<(), String> {
    db::delete_substance(&db.0.lock().unwrap(), id).map_err(err)
}

// ---------- PsychonautWiki reference cache ----------

#[derive(Serialize)]
pub struct PwStatus {
    pub count: i64,
    pub last_fetched: Option<String>,
}

/// Fetch fresh reference data from PsychonautWiki and replace the local cache.
/// This is the only outbound network call the app makes, and only on request.
#[tauri::command]
pub fn pw_update(db: State<'_, Db>) -> Result<usize, String> {
    let subs = pw::fetch_all()?;
    let mut conn = db.0.lock().unwrap();
    db::pw_replace_all(&mut conn, &subs).map_err(err)
}

#[tauri::command]
pub fn pw_status(db: State<'_, Db>) -> Result<PwStatus, String> {
    let (count, last_fetched) = db::pw_status(&db.0.lock().unwrap()).map_err(err)?;
    Ok(PwStatus { count, last_fetched })
}

#[tauri::command]
pub fn pw_lookup(db: State<'_, Db>, name: String) -> Result<Option<PwInfo>, String> {
    db::pw_lookup(&db.0.lock().unwrap(), &name).map_err(err)
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
    let conn = db.0.lock().unwrap();
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

#[tauri::command]
pub fn companion_chat(
    db: State<'_, Db>,
    model: String,
    history: Vec<ChatMsg>,
    experience_id: Option<i64>,
) -> Result<String, String> {
    let mut messages = vec![ChatMsg { role: "system".into(), content: ollama::SYSTEM_PROMPT.into() }];
    if let Some(id) = experience_id {
        if let Some(ctx) = session_context(&db.0.lock().unwrap(), id) {
            messages.push(ChatMsg { role: "system".into(), content: ctx });
        }
    }
    messages.extend(history);
    ollama::chat(&model, &messages)
}
