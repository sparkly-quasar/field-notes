// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Tauri command surface exposed to the Svelte frontend. All journal data lives
//! in a single on-device SQLite connection guarded by a mutex.

use crate::db::{self, *};
use crate::interactions::{self, Warning};
use crate::ollama::{self, ChatMsg};
use crate::Db;
use serde::Serialize;
use std::collections::BTreeSet;
use tauri::State;

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

    let names: BTreeSet<String> = detail.doses.iter().map(|d| d.substance_name.clone()).collect();
    let subs: Vec<(String, Vec<String>)> =
        names.into_iter().map(|n| { let c = interactions::builtin_classes(&n); (n, c) }).collect();
    let warns = interactions::check(&subs);
    if !warns.is_empty() {
        s.push_str("Known interaction flags for this combination:\n");
        for w in &warns {
            s.push_str(&format!("- [{}] {} + {}: {}\n", w.severity, w.a, w.b, w.message));
        }
    }
    Some(s)
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
