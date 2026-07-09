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

#[tauri::command]
pub fn companion_chat(
    db: State<'_, Db>,
    model: String,
    history: Vec<ChatMsg>,
    experience_id: Option<i64>,
) -> Result<String, String> {
    let mut messages = vec![ChatMsg { role: "system".into(), content: ollama::SYSTEM_PROMPT.into() }];
    if let Some(id) = experience_id {
        let ctx = {
            let guard = db.conn.lock().unwrap();
            let conn = guard.as_ref().ok_or_else(Db::locked_err)?;
            session_context(conn, id)
        };
        if let Some(ctx) = ctx {
            messages.push(ChatMsg { role: "system".into(), content: ctx });
        }
    }
    messages.extend(history);
    ollama::chat(&model, &messages)
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
