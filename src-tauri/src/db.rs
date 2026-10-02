// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Local SQLite store for the harm-reduction journal: substances the user
//! catalogues, experiences, the doses taken during them, and a live timeline.
//! Everything stays on-device. Timestamps are ISO-8601 strings supplied by the
//! frontend; `created_at` columns default to SQLite's clock.

use crate::pw::PwInfo;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Open (creating if needed) the journal database at `path` and run migrations.
/// `key` is the SQLCipher passphrase; pass `None` (or an empty string) for an
/// unencrypted database. Existing plaintext journals keep working with `None`.
pub fn open(path: &Path, key: Option<&str>) -> rusqlite::Result<Connection> {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let conn = Connection::open(path)?;
    if let Some(k) = key {
        if !k.is_empty() {
            // SQLCipher: the key must be applied before any other DB access.
            conn.pragma_update(None, "key", k)?;
        }
    }
    // Validate the key (and that this is a database) before migrating: a wrong
    // key surfaces here as "file is not a database".
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.execute_batch(SCHEMA)?;
    // Migration: `kind` postdates v0.5 journals. CREATE TABLE IF NOT EXISTS won't
    // touch an existing table, so add the column when it's missing.
    let has_kind: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('experiences') WHERE name = 'kind'",
        [],
        |r| r.get(0),
    )?;
    if has_kind == 0 {
        conn.execute_batch("ALTER TABLE experiences ADD COLUMN kind TEXT NOT NULL DEFAULT 'session'")?;
    }
    // Migration (v0.15): "doesn't need a write-up".
    let has_skip: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('experiences') WHERE name = 'writeup_skipped'",
        [],
        |r| r.get(0),
    )?;
    if has_skip == 0 {
        conn.execute_batch("ALTER TABLE experiences ADD COLUMN writeup_skipped INTEGER NOT NULL DEFAULT 0")?;
    }
    // One-time cleanup (v0.13.3): "Sync journal to server" in v0.12–v0.13.2 could
    // copy an entry the server already had, leaving it in the journal twice. The
    // first open after updating removes exact copies; later syncs check first.
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version < 1 {
        remove_duplicate_experiences(&conn)?;
        conn.pragma_update(None, "user_version", 1)?;
    }
    Ok(conn)
}

/// Is the database file at `path` encrypted? A plaintext (or absent) DB opens and
/// reads its schema without a key; an encrypted one fails to until keyed.
pub fn is_encrypted(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    match Connection::open(path) {
        Ok(conn) => conn
            .query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))
            .is_err(),
        Err(_) => true,
    }
}

fn sql_quote(s: &str) -> String {
    s.replace('\'', "''")
}

/// Convert the journal file between plaintext and encrypted (or change its key)
/// using SQLCipher's `sqlcipher_export`. `from_key`/`to_key` are `None`/empty for
/// plaintext. The connection to `path` must be closed before calling this.
pub fn convert(path: &Path, from_key: Option<&str>, to_key: Option<&str>) -> Result<(), String> {
    let stringify = |e: rusqlite::Error| e.to_string();
    let src = Connection::open(path).map_err(stringify)?;
    if let Some(k) = from_key {
        if !k.is_empty() {
            src.pragma_update(None, "key", k).map_err(stringify)?;
        }
    }
    // Validate we can read the source (correct current key).
    src.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0))
        .map_err(stringify)?;

    let tmp = path.with_extension("convert-tmp");
    let _ = std::fs::remove_file(&tmp);
    let tmp_q = sql_quote(&tmp.to_string_lossy());
    let key_q = sql_quote(to_key.unwrap_or(""));
    src.execute_batch(&format!("ATTACH DATABASE '{tmp_q}' AS target KEY '{key_q}';"))
        .map_err(stringify)?;
    src.query_row("SELECT sqlcipher_export('target')", [], |_| Ok(())).map_err(stringify)?;
    src.execute_batch("DETACH DATABASE target;").map_err(stringify)?;
    drop(src);

    // Replace the original with the freshly-exported file, clearing stale WAL/SHM.
    let _ = std::fs::remove_file(path.with_extension("db-wal"));
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}

/// Write a clean single-file copy of the live database to `dest` (same encryption
/// state and key as the source). Uses VACUUM INTO so committed WAL data is included.
pub fn backup_to(conn: &Connection, dest: &Path) -> rusqlite::Result<()> {
    let _ = std::fs::remove_file(dest);
    conn.execute("VACUUM INTO ?1", params![dest.to_string_lossy()])?;
    Ok(())
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS substances (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    aliases     TEXT NOT NULL DEFAULT '[]',   -- JSON array
    category    TEXT NOT NULL DEFAULT '',
    classes     TEXT NOT NULL DEFAULT '[]',   -- JSON array of interaction classes
    dose_note   TEXT NOT NULL DEFAULT '',
    notes       TEXT NOT NULL DEFAULT '',
    user_added  INTEGER NOT NULL DEFAULT 1,
    contributed INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS experiences (
    id          INTEGER PRIMARY KEY,
    -- 'session' (a drug session: doses, timeline, the works) or 'note' (a plain
    -- journal entry: title, body, date — nothing else). Explicit, never inferred:
    -- a session with no doses logged *yet* is still a session.
    kind        TEXT NOT NULL DEFAULT 'session',
    title       TEXT NOT NULL DEFAULT '',
    intention   TEXT NOT NULL DEFAULT '',
    setting     TEXT NOT NULL DEFAULT '',
    notes       TEXT NOT NULL DEFAULT '',
    rating      INTEGER,
    started_at  TEXT NOT NULL,
    ended_at    TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    -- The person said this one doesn't need a write-up (owner's decision
    -- 2026-10-02: a reflection is never owed). A preference, not content, so it
    -- is left out of `fingerprint`.
    writeup_skipped INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS doses (
    id             INTEGER PRIMARY KEY,
    experience_id  INTEGER NOT NULL REFERENCES experiences(id) ON DELETE CASCADE,
    substance_id   INTEGER REFERENCES substances(id) ON DELETE SET NULL,
    substance_name TEXT NOT NULL,
    amount         REAL,
    unit           TEXT NOT NULL DEFAULT 'mg',
    route          TEXT NOT NULL DEFAULT '',
    taken_at       TEXT NOT NULL,
    note           TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS timeline_events (
    id             INTEGER PRIMARY KEY,
    experience_id  INTEGER NOT NULL REFERENCES experiences(id) ON DELETE CASCADE,
    at             TEXT NOT NULL,
    note           TEXT NOT NULL DEFAULT '',
    mood           TEXT NOT NULL DEFAULT '',
    intensity      INTEGER
);

-- Cached DoseWiki reference data (CC0 public domain). One row per substance;
-- `data` is a serialized pw::PwInfo, loaded from the bundled offline snapshot.
CREATE TABLE IF NOT EXISTS pw_substances (
    name       TEXT PRIMARY KEY,
    data       TEXT NOT NULL,
    fetched_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#;

/// The schema, for tests in sibling modules that build an in-memory DB directly.
#[cfg(test)]
pub(crate) fn schema_for_tests() -> &'static str {
    SCHEMA
}

// ---------- models ----------

#[derive(Debug, Clone, Serialize)]
pub struct Substance {
    pub id: i64,
    pub name: String,
    pub aliases: Vec<String>,
    pub category: String,
    pub classes: Vec<String>,
    pub dose_note: String,
    pub notes: String,
    pub user_added: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Dose {
    pub id: i64,
    pub experience_id: i64,
    pub substance_id: Option<i64>,
    pub substance_name: String,
    pub amount: Option<f64>,
    pub unit: String,
    pub route: String,
    pub taken_at: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineEvent {
    pub id: i64,
    pub experience_id: i64,
    pub at: String,
    pub note: String,
    pub mood: String,
    pub intensity: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Experience {
    pub id: i64,
    /// `'session'` or `'note'`. Set at creation and never changed by edits.
    pub kind: String,
    pub title: String,
    pub intention: String,
    pub setting: String,
    pub notes: String,
    pub rating: Option<i64>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub created_at: String,
    /// Marked as not needing a write-up. It leaves "Waiting for a write-up".
    pub writeup_skipped: bool,
}

/// An experience plus the substances used in it — for list views.
#[derive(Debug, Clone, Serialize)]
pub struct ExperienceSummary {
    #[serde(flatten)]
    pub experience: Experience,
    pub substances: Vec<String>,
    pub dose_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExperienceDetail {
    #[serde(flatten)]
    pub experience: Experience,
    pub doses: Vec<Dose>,
    pub timeline: Vec<TimelineEvent>,
}

// ---------- inputs ----------

#[derive(Debug, Deserialize)]
pub struct SubstanceInput {
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub dose_note: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Deserialize)]
pub struct ExperienceInput {
    /// `'session'` (default) or `'note'` — anything else is treated as `'session'`.
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub intention: String,
    #[serde(default)]
    pub setting: String,
    pub started_at: String,
}

fn default_kind() -> String {
    "session".into()
}

#[derive(Debug, Deserialize)]
pub struct DoseInput {
    pub experience_id: i64,
    pub substance_name: String,
    pub amount: Option<f64>,
    #[serde(default = "default_unit")]
    pub unit: String,
    #[serde(default)]
    pub route: String,
    pub taken_at: String,
    #[serde(default)]
    pub note: String,
}

fn default_unit() -> String {
    "mg".to_string()
}

#[derive(Debug, Deserialize)]
pub struct TimelineInput {
    pub experience_id: i64,
    pub at: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub mood: String,
    pub intensity: Option<i64>,
}

// ---------- substances ----------

fn json_vec(s: &str) -> Vec<String> {
    serde_json::from_str(s).unwrap_or_default()
}

fn row_to_substance(r: &rusqlite::Row) -> rusqlite::Result<Substance> {
    Ok(Substance {
        id: r.get("id")?,
        name: r.get("name")?,
        aliases: json_vec(&r.get::<_, String>("aliases")?),
        category: r.get("category")?,
        classes: json_vec(&r.get::<_, String>("classes")?),
        dose_note: r.get("dose_note")?,
        notes: r.get("notes")?,
        user_added: r.get::<_, i64>("user_added")? != 0,
        created_at: r.get("created_at")?,
    })
}

pub fn list_substances(conn: &Connection) -> rusqlite::Result<Vec<Substance>> {
    let mut stmt = conn.prepare("SELECT * FROM substances ORDER BY name COLLATE NOCASE")?;
    let rows = stmt.query_map([], row_to_substance)?;
    rows.collect()
}

pub fn add_substance(conn: &Connection, input: &SubstanceInput) -> rusqlite::Result<Substance> {
    // If the user left classes empty, fall back to built-in pharmacology so the
    // safety checker still works for well-known substances.
    let classes = if input.classes.is_empty() {
        crate::interactions::builtin_classes(&input.name)
    } else {
        input.classes.clone()
    };
    let aliases = serde_json::to_string(&input.aliases).unwrap_or_else(|_| "[]".into());
    let classes_json = serde_json::to_string(&classes).unwrap_or_else(|_| "[]".into());

    conn.execute(
        "INSERT INTO substances (name, aliases, category, classes, dose_note, notes, user_added)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)
         ON CONFLICT(name) DO UPDATE SET
             aliases=excluded.aliases, category=excluded.category, classes=excluded.classes,
             dose_note=excluded.dose_note, notes=excluded.notes",
        params![input.name, aliases, input.category, classes_json, input.dose_note, input.notes],
    )?;
    let mut stmt = conn.prepare("SELECT * FROM substances WHERE name = ?1")?;
    stmt.query_row([&input.name], row_to_substance)
}

fn classes_for(conn: &Connection, name: &str) -> Vec<String> {
    conn.query_row(
        "SELECT classes FROM substances WHERE name = ?1 COLLATE NOCASE",
        [name],
        |r| r.get::<_, String>(0),
    )
    .map(|s| json_vec(&s))
    .unwrap_or_else(|_| crate::interactions::builtin_classes(name))
}

// ---------- experiences ----------

fn row_to_experience(r: &rusqlite::Row) -> rusqlite::Result<Experience> {
    Ok(Experience {
        id: r.get("id")?,
        kind: r.get("kind")?,
        title: r.get("title")?,
        intention: r.get("intention")?,
        setting: r.get("setting")?,
        notes: r.get("notes")?,
        rating: r.get("rating")?,
        started_at: r.get("started_at")?,
        ended_at: r.get("ended_at")?,
        created_at: r.get("created_at")?,
        writeup_skipped: r.get::<_, i64>("writeup_skipped")? != 0,
    })
}

pub fn create_experience(conn: &Connection, input: &ExperienceInput) -> rusqlite::Result<Experience> {
    // Normalize, don't validate-and-reject: unknown kinds mean an older client,
    // and an older client means a session (that was the only kind there was).
    let kind = if input.kind == "note" { "note" } else { "session" };
    conn.execute(
        "INSERT INTO experiences (kind, title, intention, setting, started_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![kind, input.title, input.intention, input.setting, input.started_at],
    )?;
    let id = conn.last_insert_rowid();
    get_experience_row(conn, id)
}

/// Refuse a session-only operation against a plain note. Notes have no doses, no
/// timeline, and no "end": that invariant is enforced here, not hoped for in the UI.
fn require_session(conn: &Connection, experience_id: i64, what: &str) -> rusqlite::Result<()> {
    let kind: String =
        conn.query_row("SELECT kind FROM experiences WHERE id = ?1", [experience_id], |r| r.get(0))?;
    if kind == "note" {
        // SqliteFailure with a message Displays as just the message, which is what
        // `Db::with`'s to_string() hands the UI.
        return Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
            Some(format!("This entry is a plain note, not a session — it can't have {what}.")),
        ));
    }
    Ok(())
}

fn get_experience_row(conn: &Connection, id: i64) -> rusqlite::Result<Experience> {
    conn.query_row("SELECT * FROM experiences WHERE id = ?1", [id], row_to_experience)
}

pub fn list_experiences(conn: &Connection) -> rusqlite::Result<Vec<ExperienceSummary>> {
    let mut stmt = conn.prepare("SELECT * FROM experiences ORDER BY started_at DESC")?;
    let exps: Vec<Experience> = stmt.query_map([], row_to_experience)?.collect::<Result<_, _>>()?;

    let mut out = Vec::with_capacity(exps.len());
    for e in exps {
        let mut s = conn.prepare(
            "SELECT DISTINCT substance_name FROM doses WHERE experience_id = ?1 ORDER BY substance_name",
        )?;
        let substances: Vec<String> =
            s.query_map([e.id], |r| r.get(0))?.collect::<Result<_, _>>()?;
        let dose_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM doses WHERE experience_id = ?1", [e.id], |r| r.get(0))?;
        out.push(ExperienceSummary { experience: e, substances, dose_count });
    }
    Ok(out)
}

pub fn get_experience(conn: &Connection, id: i64) -> rusqlite::Result<ExperienceDetail> {
    let experience = get_experience_row(conn, id)?;

    let mut ds = conn.prepare("SELECT * FROM doses WHERE experience_id = ?1 ORDER BY taken_at")?;
    let doses: Vec<Dose> = ds
        .query_map([id], |r| {
            Ok(Dose {
                id: r.get("id")?,
                experience_id: r.get("experience_id")?,
                substance_id: r.get("substance_id")?,
                substance_name: r.get("substance_name")?,
                amount: r.get("amount")?,
                unit: r.get("unit")?,
                route: r.get("route")?,
                taken_at: r.get("taken_at")?,
                note: r.get("note")?,
            })
        })?
        .collect::<Result<_, _>>()?;

    let mut ts = conn.prepare("SELECT * FROM timeline_events WHERE experience_id = ?1 ORDER BY at")?;
    let timeline: Vec<TimelineEvent> = ts
        .query_map([id], |r| {
            Ok(TimelineEvent {
                id: r.get("id")?,
                experience_id: r.get("experience_id")?,
                at: r.get("at")?,
                note: r.get("note")?,
                mood: r.get("mood")?,
                intensity: r.get("intensity")?,
            })
        })?
        .collect::<Result<_, _>>()?;

    Ok(ExperienceDetail { experience, doses, timeline })
}

pub fn end_experience(conn: &Connection, id: i64, ended_at: &str, rating: Option<i64>, notes: &str) -> rusqlite::Result<Experience> {
    require_session(conn, id, "an end time or rating")?;
    conn.execute(
        "UPDATE experiences SET ended_at = ?2, rating = ?3, notes = ?4 WHERE id = ?1",
        params![id, ended_at, rating, notes],
    )?;
    get_experience_row(conn, id)
}

// ---------- doses & timeline ----------

/// Insert a dose and return it together with any interaction warnings against the
/// other substances already logged in the same experience.
pub fn log_dose(conn: &Connection, input: &DoseInput) -> rusqlite::Result<(Dose, Vec<crate::interactions::Warning>)> {
    require_session(conn, input.experience_id, "doses")?;
    let substance_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM substances WHERE name = ?1 COLLATE NOCASE",
            [&input.substance_name],
            |r| r.get(0),
        )
        .ok();

    conn.execute(
        "INSERT INTO doses (experience_id, substance_id, substance_name, amount, unit, route, taken_at, note)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            input.experience_id, substance_id, input.substance_name, input.amount,
            input.unit, input.route, input.taken_at, input.note
        ],
    )?;
    let id = conn.last_insert_rowid();
    let dose = conn.query_row("SELECT * FROM doses WHERE id = ?1", [id], |r| {
        Ok(Dose {
            id: r.get("id")?,
            experience_id: r.get("experience_id")?,
            substance_id: r.get("substance_id")?,
            substance_name: r.get("substance_name")?,
            amount: r.get("amount")?,
            unit: r.get("unit")?,
            route: r.get("route")?,
            taken_at: r.get("taken_at")?,
            note: r.get("note")?,
        })
    })?;

    name_after_first_dose(conn, input.experience_id, &input.substance_name)?;

    // Every substance in this experience that overlapped another, checked together.
    Ok((dose, session_warnings(conn, input.experience_id)?))
}

/// Give an untitled session the name of the first substance logged into it.
///
/// Starting a session mid-stride — especially from the phone, one-handed — is a
/// single tap with nowhere to type a title, and the entry would otherwise land in
/// the journal as "Untitled" forever. The first dose is the most useful thing we
/// know about a session, so it becomes the title.
///
/// Deliberately only the *first* dose: after that the session has a name, and a
/// user who clears the title back to blank meant to, so nothing refills it.
/// Renaming (desktop or phone) overrides this at any point.
fn name_after_first_dose(conn: &Connection, experience_id: i64, substance: &str) -> rusqlite::Result<()> {
    let name = substance.trim();
    if name.is_empty() {
        return Ok(());
    }
    let doses: i64 =
        conn.query_row("SELECT COUNT(*) FROM doses WHERE experience_id = ?1", [experience_id], |r| r.get(0))?;
    if doses != 1 {
        return Ok(());
    }
    conn.execute(
        "UPDATE experiences SET title = ?2 WHERE id = ?1 AND TRIM(COALESCE(title, '')) = ''",
        params![experience_id, name],
    )?;
    Ok(())
}

/// Every warning we know about for a set of substances taken together, from both
/// sources: the class-rule backstop in `interactions.rs` and DoseWiki's graded
/// interaction lists. The most severe warning per pair survives.
///
/// This is the *only* way warnings should be produced. Logging a dose and asking
/// the combo checker "is this safe?" must answer identically — the checker is the
/// one people consult *before* taking something, so it can't know less.
/// Interaction warnings for one session, counting only pairs that were in the
/// body at the same time. Each dose is active from when it was taken for the
/// reference's total duration on that route (or its longest, if the route isn't
/// listed; `UNKNOWN_DURATION_MIN` with no reference at all), plus
/// `OVERLAP_MARGIN_MIN` for after-effects and loose timestamps. Logged live, every
/// dose is "now" and this is the same as [`combo_warnings`]; for a past session,
/// a stimulant at 6pm and a benzo at 3am a day later aren't a combination.
pub fn session_warnings(conn: &Connection, experience_id: i64) -> rusqlite::Result<Vec<crate::interactions::Warning>> {
    let mut stmt = conn.prepare("SELECT substance_name, route, taken_at FROM doses WHERE experience_id = ?1")?;
    let doses: Vec<(String, String, String)> = stmt
        .query_map([experience_id], |r| Ok((r.get(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default(), r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let mut names: Vec<String> = doses.iter().map(|d| d.0.clone()).collect();
    names.sort();
    names.dedup();
    let mut warnings = combo_warnings(conn, &names);

    // Each dose's active window, in minutes since the epoch.
    let windows: Vec<(String, f64, f64)> = doses
        .iter()
        .filter_map(|(name, route, at)| {
            let t = chrono::DateTime::parse_from_rfc3339(at).ok()?.timestamp() as f64 / 60.0;
            let info = pw_lookup(conn, name).ok().flatten();
            let dur = info.as_ref().and_then(|i| active_minutes(i, route)).unwrap_or(UNKNOWN_DURATION_MIN);
            Some((name.to_lowercase(), t, t + dur + OVERLAP_MARGIN_MIN))
        })
        .collect();
    let overlap = |a: &str, b: &str| {
        let (a, b) = (a.to_lowercase(), b.to_lowercase());
        let wa: Vec<_> = windows.iter().filter(|w| w.0 == a).collect();
        let wb: Vec<_> = windows.iter().filter(|w| w.0 == b).collect();
        // A dose with no usable time can't be ruled out.
        wa.is_empty() || wb.is_empty() || wa.iter().any(|x| wb.iter().any(|y| x.1 <= y.2 && y.1 <= x.2))
    };
    warnings.retain(|w| overlap(&w.a, &w.b));
    Ok(warnings)
}

/// Unknown substances are assumed to last this long: long enough not to miss a
/// combination with most things, short enough that a whole night isn't one dose.
const UNKNOWN_DURATION_MIN: f64 = 8.0 * 60.0;
const OVERLAP_MARGIN_MIN: f64 = 60.0;

/// How long a dose of this lasts by the reference's "total" for the route, longest
/// end of the range. IM is read as IV, the nearest route DoseWiki lists.
fn active_minutes(info: &PwInfo, route: &str) -> Option<f64> {
    let want = match route.to_lowercase().as_str() {
        "im" | "iv" => "intravenous".to_string(),
        "vaporized" => "vaporized".to_string(),
        r => r.to_string(),
    };
    let parse = |s: &str| -> Option<f64> {
        let max = s.split(|c: char| !(c.is_ascii_digit() || c == '.')).filter_map(|n| n.parse::<f64>().ok()).last()?;
        let unit = s.to_lowercase();
        Some(if unit.contains("min") { max } else if unit.contains("day") { max * 1440.0 } else { max * 60.0 })
    };
    let on_route = info.roas.iter().find(|r| r.name.eq_ignore_ascii_case(&want)).and_then(|r| r.total.as_deref()).and_then(parse);
    on_route.or_else(|| info.roas.iter().filter_map(|r| r.total.as_deref().and_then(parse)).reduce(f64::max))
}

pub fn combo_warnings(conn: &Connection, names: &[String]) -> Vec<crate::interactions::Warning> {
    let with_classes: Vec<(String, Vec<String>)> =
        names.iter().map(|n| (n.clone(), classes_for(conn, n))).collect();
    let mut warnings = crate::interactions::check(&with_classes);
    warnings.extend(pw_interaction_warnings(conn, names));
    crate::interactions::dedup_pairs(warnings)
}

pub fn add_timeline_event(conn: &Connection, input: &TimelineInput) -> rusqlite::Result<TimelineEvent> {
    require_session(conn, input.experience_id, "timeline events")?;
    conn.execute(
        "INSERT INTO timeline_events (experience_id, at, note, mood, intensity)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![input.experience_id, input.at, input.note, input.mood, input.intensity],
    )?;
    let id = conn.last_insert_rowid();
    conn.query_row("SELECT * FROM timeline_events WHERE id = ?1", [id], |r| {
        Ok(TimelineEvent {
            id: r.get("id")?,
            experience_id: r.get("experience_id")?,
            at: r.get("at")?,
            note: r.get("note")?,
            mood: r.get("mood")?,
            intensity: r.get("intensity")?,
        })
    })
}

// ---------- by-substance rollup ----------

#[derive(Debug, Clone, Serialize)]
pub struct SubstanceUsage {
    pub substance_name: String,
    pub times_used: i64,
    pub doses: Vec<Dose>,
}

/// Every dose grouped by substance, most-used first — "organize by substance and
/// dosage."
pub fn usage_by_substance(conn: &Connection) -> rusqlite::Result<Vec<SubstanceUsage>> {
    let mut stmt = conn.prepare(
        "SELECT substance_name, COUNT(*) c FROM doses
         GROUP BY substance_name COLLATE NOCASE ORDER BY c DESC, substance_name",
    )?;
    let heads: Vec<(String, i64)> =
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;

    let mut out = Vec::with_capacity(heads.len());
    for (name, times) in heads {
        let mut ds = conn.prepare(
            "SELECT * FROM doses WHERE substance_name = ?1 COLLATE NOCASE ORDER BY taken_at DESC",
        )?;
        let doses: Vec<Dose> = ds
            .query_map([&name], |r| {
                Ok(Dose {
                    id: r.get("id")?,
                    experience_id: r.get("experience_id")?,
                    substance_id: r.get("substance_id")?,
                    substance_name: r.get("substance_name")?,
                    amount: r.get("amount")?,
                    unit: r.get("unit")?,
                    route: r.get("route")?,
                    taken_at: r.get("taken_at")?,
                    note: r.get("note")?,
                })
            })?
            .collect::<Result<_, _>>()?;
        out.push(SubstanceUsage { substance_name: name, times_used: times, doses });
    }
    Ok(out)
}

// ---------- edit & delete ----------

#[derive(Debug, Deserialize)]
pub struct ExperienceUpdate {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub intention: String,
    #[serde(default)]
    pub setting: String,
    #[serde(default)]
    pub notes: String,
    pub rating: Option<i64>,
    pub started_at: String,
    pub ended_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DoseUpdate {
    pub substance_name: String,
    pub amount: Option<f64>,
    #[serde(default = "default_unit")]
    pub unit: String,
    #[serde(default)]
    pub route: String,
    pub taken_at: String,
    #[serde(default)]
    pub note: String,
}

/// Mark an entry as not needing a write-up, or undo that.
pub fn set_writeup_skipped(conn: &Connection, id: i64, skipped: bool) -> rusqlite::Result<Experience> {
    conn.execute("UPDATE experiences SET writeup_skipped = ?2 WHERE id = ?1", params![id, skipped as i64])?;
    get_experience_row(conn, id)
}

pub fn update_experience(conn: &Connection, id: i64, u: &ExperienceUpdate) -> rusqlite::Result<Experience> {
    conn.execute(
        "UPDATE experiences SET title=?2, intention=?3, setting=?4, notes=?5, rating=?6,
             started_at=?7, ended_at=?8 WHERE id=?1",
        params![id, u.title, u.intention, u.setting, u.notes, u.rating, u.started_at, u.ended_at],
    )?;
    get_experience_row(conn, id)
}

#[derive(Debug, Deserialize)]
pub struct TimelineUpdate {
    pub at: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub mood: String,
    pub intensity: Option<i64>,
}

pub fn update_timeline_event(conn: &Connection, id: i64, u: &TimelineUpdate) -> rusqlite::Result<TimelineEvent> {
    conn.execute(
        "UPDATE timeline_events SET at=?2, note=?3, mood=?4, intensity=?5 WHERE id=?1",
        params![id, u.at, u.note, u.mood, u.intensity],
    )?;
    conn.query_row("SELECT * FROM timeline_events WHERE id = ?1", [id], |r| {
        Ok(TimelineEvent {
            id: r.get("id")?,
            experience_id: r.get("experience_id")?,
            at: r.get("at")?,
            note: r.get("note")?,
            mood: r.get("mood")?,
            intensity: r.get("intensity")?,
        })
    })
}

pub fn update_dose(conn: &Connection, id: i64, u: &DoseUpdate) -> rusqlite::Result<Dose> {
    let substance_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM substances WHERE name = ?1 COLLATE NOCASE",
            [&u.substance_name],
            |r| r.get(0),
        )
        .ok();
    conn.execute(
        "UPDATE doses SET substance_id=?2, substance_name=?3, amount=?4, unit=?5, route=?6,
             taken_at=?7, note=?8 WHERE id=?1",
        params![id, substance_id, u.substance_name, u.amount, u.unit, u.route, u.taken_at, u.note],
    )?;
    conn.query_row("SELECT * FROM doses WHERE id = ?1", [id], |r| {
        Ok(Dose {
            id: r.get("id")?,
            experience_id: r.get("experience_id")?,
            substance_id: r.get("substance_id")?,
            substance_name: r.get("substance_name")?,
            amount: r.get("amount")?,
            unit: r.get("unit")?,
            route: r.get("route")?,
            taken_at: r.get("taken_at")?,
            note: r.get("note")?,
        })
    })
}

pub fn delete_experience(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM experiences WHERE id = ?1", [id])?;
    Ok(())
}

pub fn delete_dose(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM doses WHERE id = ?1", [id])?;
    Ok(())
}

pub fn delete_timeline_event(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM timeline_events WHERE id = ?1", [id])?;
    Ok(())
}

pub fn delete_substance(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM substances WHERE id = ?1", [id])?;
    Ok(())
}

// ---------- duplicates ----------

/// What makes two entries "the same entry": everything the user wrote — kind,
/// title, intention, setting, write-up, rating, times, every dose and timeline
/// note — and nothing the database assigned (IDs, `created_at`). Takes the JSON
/// shape of an [`ExperienceDetail`], so an entry here and one on a server (which
/// arrives as JSON) compare the same way.
///
/// Deliberately strict: a copy that differs in any word is *not* a duplicate. Only
/// the formatting noise that a round trip between computers can introduce is
/// ignored — surrounding whitespace, `\r\n`, `T` vs space and a trailing `Z` or
/// fractional seconds in timestamps, and the order doses and notes come back in.
pub fn fingerprint(d: &serde_json::Value) -> String {
    fn text(v: &serde_json::Value) -> String {
        match v {
            serde_json::Value::Null => String::new(),
            serde_json::Value::String(s) => s.replace("\r\n", "\n").trim().to_string(),
            serde_json::Value::Number(n) => n.as_f64().map(|f| format!("{f}")).unwrap_or_default(),
            other => other.to_string(),
        }
    }
    fn time(v: &serde_json::Value) -> String {
        let t = text(v).replace('T', " ");
        let t = t.trim_end_matches('Z');
        t.split('.').next().unwrap_or(t).to_string()
    }
    let rows = |key: &str, row: &dyn Fn(&serde_json::Value) -> String| -> Vec<String> {
        let mut v: Vec<String> = d[key].as_array().map(|a| a.iter().map(row).collect()).unwrap_or_default();
        v.sort();
        v
    };
    let doses = rows("doses", &|x| {
        [time(&x["taken_at"]), text(&x["substance_name"]).to_lowercase(), text(&x["amount"]),
         text(&x["unit"]), text(&x["route"]), text(&x["note"])].join("\u{1f}")
    });
    let timeline = rows("timeline", &|x| {
        [time(&x["at"]), text(&x["note"]), text(&x["mood"]), text(&x["intensity"])].join("\u{1f}")
    });
    serde_json::json!([
        text(&d["kind"]), text(&d["title"]), text(&d["intention"]), text(&d["setting"]),
        text(&d["notes"]), text(&d["rating"]), time(&d["started_at"]), time(&d["ended_at"]),
        doses, timeline,
    ])
    .to_string()
}

/// Remove entries that are exact copies of another entry (see [`fingerprint`]),
/// keeping the oldest of each set. Returns how many were removed. Safe to run at
/// any time: an entry with anything of its own — one more dose, one changed word —
/// is never touched.
pub fn remove_duplicate_experiences(conn: &Connection) -> rusqlite::Result<usize> {
    let ids: Vec<i64> = conn
        .prepare("SELECT id FROM experiences ORDER BY id")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let mut seen = std::collections::HashSet::new();
    let mut extra = Vec::new();
    for id in ids {
        let detail = get_experience(conn, id)?;
        let v = serde_json::to_value(&detail).unwrap_or_default();
        if !seen.insert(fingerprint(&v)) {
            extra.push(id);
        }
    }
    for id in &extra {
        delete_experience(conn, *id)?;
    }
    Ok(extra.len())
}

// ---------- DoseWiki reference cache ----------

/// Replace the whole cache with a freshly loaded set, in one transaction.
pub fn pw_replace_all(conn: &mut Connection, subs: &[PwInfo]) -> rusqlite::Result<usize> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM pw_substances", [])?;
    {
        let mut stmt = tx.prepare("INSERT OR REPLACE INTO pw_substances (name, data) VALUES (?1, ?2)")?;
        for s in subs {
            let data = serde_json::to_string(s).unwrap_or_default();
            stmt.execute(params![s.name, data])?;
        }
    }
    tx.commit()?;
    Ok(subs.len())
}

/// Look up cached reference data by substance name, falling back to an alias
/// match against the stored common names.
pub fn pw_lookup(conn: &Connection, name: &str) -> rusqlite::Result<Option<PwInfo>> {
    let exact: Option<String> = conn
        .query_row("SELECT data FROM pw_substances WHERE name = ?1 COLLATE NOCASE", [name], |r| r.get(0))
        .optional()?;
    let data = match exact {
        Some(d) => Some(d),
        None => conn
            .query_row(
                "SELECT data FROM pw_substances WHERE data LIKE ?1 COLLATE NOCASE LIMIT 1",
                [format!("%\"{name}\"%")],
                |r| r.get(0),
            )
            .optional()?,
    };
    Ok(data.and_then(|d| serde_json::from_str(&d).ok()))
}

/// Every substance in the dose reference with its street names — so a pasted log
/// that says "mesc" or "molly" can be matched to a real entry. Reference data only.
pub fn pw_names(conn: &Connection) -> rusqlite::Result<Vec<PwName>> {
    let mut stmt = conn.prepare("SELECT data FROM pw_substances ORDER BY name")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for data in rows {
        if let Ok(info) = serde_json::from_str::<PwInfo>(&data?) {
            out.push(PwName { name: info.name, aliases: info.common_names });
        }
    }
    Ok(out)
}

#[derive(Debug, Serialize)]
pub struct PwName {
    pub name: String,
    pub aliases: Vec<String>,
}

/// (number of cached substances, most recent fetch timestamp).
pub fn pw_status(conn: &Connection) -> rusqlite::Result<(i64, Option<String>)> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM pw_substances", [], |r| r.get(0))?;
    let last: Option<String> = conn.query_row("SELECT MAX(fetched_at) FROM pw_substances", [], |r| r.get(0))?;
    Ok((count, last))
}

/// Does a DoseWiki interaction entry (a substance name or a class like
/// "Stimulants"/"MAOIs") refer to `other`? Matches `other`'s name, aliases, and
/// psychoactive/chemical classes, with light singular/substring tolerance.
/// Active metabolites sold or taken in their own right, and prodrugs, with the drug
/// whose interaction warnings apply to them. Whole-word matching can't see that
/// "O-Desmethyltramadol" is tramadol's active form, or that 1,4-butanediol becomes
/// GHB in the body, and neither reference entry lists anything to match on, so
/// without this no warning naming tramadol or GHB (etizolam: "GHB/GBL", dangerous)
/// would ever reach them. (metabolite or prodrug, the drug it acts as), lowercase.
const ACTIVE_METABOLITES: &[(&str, &str)] = &[("o-desmethyltramadol", "tramadol"), ("1,4-butanediol", "ghb")];

/// A DoseWiki family name with wildcards, like "5-MeO-xxT" or "2C-x": an `x`
/// segment stands for any one segment, and `xx` inside one for one to four
/// characters. Does `id` (a name or alias) belong to the family?
fn family_match(pattern: &str, id: &str) -> bool {
    let (ps, is): (Vec<&str>, Vec<&str>) = (pattern.split('-').collect(), id.split('-').collect());
    ps.len() == is.len()
        && ps.iter().zip(&is).all(|(p, i)| {
            if *p == "x" {
                !i.is_empty() && i.chars().all(char::is_alphanumeric)
            } else if let Some((pre, post)) = p.split_once("xx") {
                let mid = i.len() as isize - pre.len() as isize - post.len() as isize;
                i.starts_with(pre) && i.ends_with(post) && (1..=4).contains(&mid)
            } else {
                p == i
            }
        })
}

/// The wildcard family a DoseWiki entry names, if any ("5-MeO-xxT tryptamines" →
/// "5-meo-xxt"). Such an entry means that family, not every member of the class
/// word after it: "5-MeO-xxT tryptamines" is not about DMT.
fn family_of(interaction: &str) -> Option<&str> {
    interaction
        .split_whitespace()
        .find(|t| t.contains('-') && t.split('-').any(|seg| seg == "x" || seg.contains("xx")))
}

fn matches_interaction(interaction: &str, other: &PwInfo) -> bool {
    let i = interaction.to_lowercase();
    if let Some(fam) = family_of(&i) {
        // By name or alias only, or a class that *is* the family ("2C-X").
        let ids = std::iter::once(&other.name).chain(&other.common_names).chain(&other.chemical);
        return ids.map(|s| s.to_lowercase()).any(|id| id == fam || family_match(fam, &id));
    }
    let i_sing = i.trim_end_matches('s');
    let mut ids: Vec<String> = vec![other.name.to_lowercase()];
    ids.extend(
        ACTIVE_METABOLITES
            .iter()
            .filter(|(m, _)| other.name.eq_ignore_ascii_case(m))
            .map(|(_, parent)| parent.to_string()),
    );
    ids.extend(other.common_names.iter().map(|s| s.to_lowercase()));
    ids.extend(other.psychoactive.iter().map(|s| s.to_lowercase()));
    ids.extend(other.chemical.iter().map(|s| s.to_lowercase()));
    ids.iter().any(|id| {
        let id_sing = id.trim_end_matches('s');
        // Partial matches are whole words only, and never on a one- or two-letter
        // street name: LSD is also "L", MDMA "E" and "X", ketamine "K", and a plain
        // substring test made "Lithium", "Tramadol" and "Alcohol" all match LSD.
        id == &i
            || id_sing == i_sing
            || (id.len() >= 3 && has_word(&i, id))
            || (i.len() >= 4 && has_word(id, &i))
            || (i_sing.len() >= 4 && has_word(id, i_sing))
    })
}

/// `needle` appears in `hay` as a whole word (a plural "s" after it is allowed):
/// no letter or digit directly on either side.
fn has_word(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    hay.match_indices(needle).any(|(at, _)| {
        let before = hay[..at].chars().next_back();
        let rest = &hay[at + needle.len()..];
        let rest = rest.strip_prefix('s').filter(|r| !r.starts_with(char::is_alphanumeric)).unwrap_or(rest);
        !before.is_some_and(char::is_alphanumeric) && !rest.starts_with(char::is_alphanumeric)
    })
}

/// Rank a DoseWiki-derived severity for picking the most severe match per pair.
fn sev_rank(sev: &str) -> u8 {
    match sev {
        "danger" => 3,
        "caution" => 2,
        _ => 1,
    }
}

/// Map a stored severity string back to the static set the `Warning` type uses.
fn static_sev(sev: &str) -> &'static str {
    match sev {
        "danger" => "danger",
        "caution" => "caution",
        _ => "note",
    }
}

/// Graded warnings from DoseWiki's interaction lists for every pair of the given
/// substances that has cached reference data. Keeps the most severe match per
/// pair and carries DoseWiki's reason text.
pub fn pw_interaction_warnings(conn: &Connection, names: &[String]) -> Vec<crate::interactions::Warning> {
    use crate::interactions::{dosewiki_message, Warning};
    let infos: Vec<(String, PwInfo)> = names
        .iter()
        .filter_map(|n| pw_lookup(conn, n).ok().flatten().map(|info| (n.clone(), info)))
        .collect();
    let mut out = Vec::new();
    for a in 0..infos.len() {
        for b in (a + 1)..infos.len() {
            let (na, ia) = &infos[a];
            let (nb, ib) = &infos[b];
            // Consider graded matches in both directions; keep the most severe.
            let matches = ia
                .interactions
                .iter()
                .filter(|x| matches_interaction(&x.name, ib))
                .chain(ib.interactions.iter().filter(|x| matches_interaction(&x.name, ia)));
            let mut best: Option<&crate::pw::PwInteraction> = None;
            for x in matches {
                if best.map_or(0, |b| sev_rank(&b.severity)) < sev_rank(&x.severity) {
                    best = Some(x);
                }
            }
            if let Some(x) = best {
                out.push(Warning {
                    severity: static_sev(&x.severity),
                    a: na.clone(),
                    b: nb.clone(),
                    message: dosewiki_message(&x.severity, x.reason.as_deref()),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(name: &str, common: &[&str], psy: &[&str], chem: &[&str]) -> PwInfo {
        let v = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect();
        PwInfo {
            name: name.into(),
            common_names: v(common),
            psychoactive: v(psy),
            chemical: v(chem),
            roas: vec![],
            interactions: vec![],
        }
    }

    #[test]
    fn a_one_letter_street_name_does_not_match_inside_another_word() {
        // LSD is also "L", MDMA "E" and "X", ketamine "K". Each of these was a
        // false "dangerous combination" before matching went whole-word.
        let lsd = info("LSD", &["Acid", "L", "LSD-25"], &["Psychedelic"], &["Lysergamide"]);
        let mdma = info("MDMA", &["Molly", "E", "X", "MD"], &["Entactogen", "Stimulant"], &["Amphetamine"]);
        let ket = info("Ketamine", &["K", "Ket"], &["Dissociative"], &["Arylcyclohexylamine"]);
        for i in ["Lithium", "Tramadol", "Alcohol"] {
            assert!(!matches_interaction(i, &lsd), "{i} must not match LSD");
        }
        assert!(!matches_interaction("Diphenhydramine", &mdma));
        assert!(!matches_interaction("Psychedelics", &info("Heroin", &["H", "Smack"], &["Opioid"], &["Morphinan"])));
        assert!(!matches_interaction("Tricyclic antidepressants", &info("Alprazolam", &[], &["Depressant"], &["Benzodiazepine"])));
        assert!(!matches_interaction("Ketamine", &info("Clonazepam", &["K-pin"], &["Depressant"], &["Benzodiazepine"])));
        assert!(matches_interaction("Ketamine", &ket));
    }

    #[test]
    fn real_matches_still_match() {
        let mdma = info("MDMA", &["Molly", "E"], &["Entactogen", "Stimulant"], &["Amphetamine"]);
        assert!(matches_interaction("MDMA", &mdma));
        assert!(matches_interaction("Molly", &mdma));
        assert!(matches_interaction("Amphetamines", &mdma));
        assert!(matches_interaction("Stimulants", &mdma));
        let ghb = info("GHB", &["G"], &["Depressant"], &[]);
        assert!(matches_interaction("GHB/GBL", &ghb));
        // A class written with a qualifier, which plain substring matching missed.
        let fa = info("2-FA", &[], &["Stimulant"], &["Amphetamine (substituted)"]);
        assert!(matches_interaction("Amphetamines", &fa));
        let mild = info("3-MeO-PCPr", &[], &["Dissociative", "Stimulant (mild)"], &[]);
        assert!(matches_interaction("Stimulants", &mild));
        let dmt = info("DMT", &[], &["Psychedelic"], &["Tryptamine"]);
        assert!(matches_interaction("Tryptamines", &dmt));
    }

    #[test]
    fn a_warning_about_tramadol_reaches_its_active_metabolite() {
        let odsmt = info("O-Desmethyltramadol", &[], &[], &["Phenylpropylamine"]);
        assert!(matches_interaction("Tramadol", &odsmt));
        // One direction only: the metabolite is not tramadol for every other purpose.
        let tramadol = info("Tramadol", &["Ultram"], &["Opioid"], &[]);
        assert!(!matches_interaction("O-Desmethyltramadol", &tramadol));
    }

    #[test]
    fn mdma_and_o_desmethyltramadol_are_flagged_from_the_bundled_reference() {
        let c = mem();
        let mut c = c;
        let all = crate::pw::parse_slim(include_str!("../resources/dosewiki.json")).unwrap();
        pw_replace_all(&mut c, &all).unwrap();
        let w = pw_interaction_warnings(&c, &["MDMA".into(), "O-Desmethyltramadol".into()]);
        assert!(w.iter().any(|x| x.severity == "danger"), "{w:?}");
    }

    fn mem() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        c.execute_batch(SCHEMA).unwrap();
        c
    }

    #[test]
    fn full_round_trip_with_interaction_warning() {
        let c = mem();

        // Catalogue two substances that should flag against each other.
        add_substance(&c, &SubstanceInput {
            name: "MDMA".into(), aliases: vec![], category: "empathogen".into(),
            classes: vec![], dose_note: String::new(), notes: String::new(),
        }).unwrap();
        add_substance(&c, &SubstanceInput {
            name: "sertraline".into(), aliases: vec![], category: "SSRI".into(),
            classes: vec!["ssri".into()], dose_note: String::new(), notes: String::new(),
        }).unwrap();

        let exp = create_experience(&c, &ExperienceInput {
            kind: "session".into(),
            title: "test".into(), intention: String::new(), setting: String::new(),
            started_at: "2026-01-01T20:00:00Z".into(),
        }).unwrap();

        let (_d1, w1) = log_dose(&c, &DoseInput {
            experience_id: exp.id, substance_name: "MDMA".into(), amount: Some(100.0),
            unit: "mg".into(), route: "oral".into(), taken_at: "2026-01-01T20:05:00Z".into(),
            note: String::new(),
        }).unwrap();
        assert!(w1.is_empty(), "single substance should not warn");

        let (_d2, w2) = log_dose(&c, &DoseInput {
            experience_id: exp.id, substance_name: "sertraline".into(), amount: Some(50.0),
            unit: "mg".into(), route: "oral".into(), taken_at: "2026-01-01T21:00:00Z".into(),
            note: String::new(),
        }).unwrap();
        assert!(!w2.is_empty(), "MDMA + SSRI must produce a warning");

        // list + rollup
        let list = list_experiences(&c).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].dose_count, 2);
        assert_eq!(list[0].substances.len(), 2);

        let usage = usage_by_substance(&c).unwrap();
        assert_eq!(usage.len(), 2);
        assert!(usage.iter().all(|u| u.times_used == 1));

        let detail = get_experience(&c, exp.id).unwrap();
        assert_eq!(detail.doses.len(), 2);
    }

    #[test]
    fn an_untitled_session_takes_the_name_of_its_first_dose() {
        let c = mem();
        let blank = |started_at: &str, title: &str| {
            create_experience(&c, &ExperienceInput {
                kind: "session".into(), title: title.into(),
                intention: String::new(), setting: String::new(),
                started_at: started_at.into(),
            }).unwrap()
        };
        let dose = |exp: i64, name: &str, at: &str| {
            log_dose(&c, &DoseInput {
                experience_id: exp, substance_name: name.into(), amount: Some(100.0),
                unit: "mg".into(), route: "oral".into(), taken_at: at.into(),
                note: String::new(),
            }).unwrap()
        };

        // Started from the phone with nowhere to type a title: the first dose names it.
        let auto = blank("2026-08-01T20:00:00Z", "");
        assert_eq!(auto.title, "");
        dose(auto.id, "ketamine", "2026-08-01T20:10:00Z");
        assert_eq!(get_experience_row(&c, auto.id).unwrap().title, "ketamine");

        // The second dose does not rename it — only the first one names a session.
        dose(auto.id, "MDMA", "2026-08-01T21:00:00Z");
        assert_eq!(get_experience_row(&c, auto.id).unwrap().title, "ketamine");

        // …and a title the user typed is never overwritten.
        let named = blank("2026-08-02T20:00:00Z", "Birthday");
        dose(named.id, "LSD", "2026-08-02T20:10:00Z");
        assert_eq!(get_experience_row(&c, named.id).unwrap().title, "Birthday");

        // A cleared title stays cleared: that was a decision, not a gap to fill.
        update_experience(&c, auto.id, &ExperienceUpdate {
            title: String::new(), intention: String::new(), setting: String::new(),
            notes: String::new(), rating: None,
            started_at: "2026-08-01T20:00:00Z".into(), ended_at: None,
        }).unwrap();
        dose(auto.id, "caffeine", "2026-08-01T22:00:00Z");
        assert_eq!(get_experience_row(&c, auto.id).unwrap().title, "");
    }

    #[test]
    fn timeline_events_can_be_edited_in_place() {
        let c = mem();
        let exp = create_experience(&c, &ExperienceInput {
            kind: "session".into(),
            title: "test".into(), intention: String::new(), setting: String::new(),
            started_at: "2026-01-01T20:00:00Z".into(),
        }).unwrap();
        let ev = add_timeline_event(&c, &TimelineInput {
            experience_id: exp.id, at: "2026-01-01T21:00:00Z".into(),
            note: "coming up".into(), mood: "nervous".into(), intensity: Some(3),
        }).unwrap();

        let edited = update_timeline_event(&c, ev.id, &TimelineUpdate {
            at: "2026-01-01T21:10:00Z".into(),
            note: "coming up smoothly".into(), mood: "settled".into(), intensity: Some(4),
        }).unwrap();
        assert_eq!(edited.id, ev.id);
        assert_eq!(edited.experience_id, exp.id, "editing never moves an event to another experience");
        assert_eq!(edited.at, "2026-01-01T21:10:00Z");
        assert_eq!(edited.note, "coming up smoothly");
        assert_eq!(edited.mood, "settled");
        assert_eq!(edited.intensity, Some(4));

        // It's the stored row, not an echo.
        let detail = get_experience(&c, exp.id).unwrap();
        assert_eq!(detail.timeline.len(), 1);
        assert_eq!(detail.timeline[0].note, "coming up smoothly");
    }

    #[test]
    fn plain_notes_are_explicit_and_session_only_ops_refuse_them() {
        let c = mem();

        // Kind is normalized at creation: unknown values mean an older client.
        let weird = create_experience(&c, &ExperienceInput {
            kind: "diary".into(), title: "old client".into(),
            intention: String::new(), setting: String::new(),
            started_at: "2026-07-01T09:00:00Z".into(),
        }).unwrap();
        assert_eq!(weird.kind, "session");

        let note = create_experience(&c, &ExperienceInput {
            kind: "note".into(), title: "just a day".into(),
            intention: String::new(), setting: String::new(),
            started_at: "2026-07-02T09:00:00Z".into(),
        }).unwrap();
        assert_eq!(note.kind, "note");

        // A note is not a session: no doses, no timeline, no "end".
        let dose = log_dose(&c, &DoseInput {
            experience_id: note.id, substance_name: "MDMA".into(), amount: Some(100.0),
            unit: "mg".into(), route: "oral".into(), taken_at: "2026-07-02T10:00:00Z".into(),
            note: String::new(),
        });
        assert!(dose.is_err());
        let ev = add_timeline_event(&c, &TimelineInput {
            experience_id: note.id, at: "2026-07-02T10:00:00Z".into(),
            note: "hm".into(), mood: String::new(), intensity: None,
        });
        assert!(ev.is_err());
        assert!(end_experience(&c, note.id, "2026-07-02T11:00:00Z", Some(5), "").is_err());

        // Editing the body doesn't flip the kind.
        let edited = update_experience(&c, note.id, &ExperienceUpdate {
            title: "just a day".into(), intention: String::new(), setting: String::new(),
            notes: "wrote some words".into(), rating: None,
            started_at: "2026-07-02T09:00:00Z".into(), ended_at: None,
        }).unwrap();
        assert_eq!(edited.kind, "note");
    }

    #[test]
    fn kind_column_is_added_to_pre_v05_journals() {
        let dir = std::env::temp_dir().join(format!("fn-migrate-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("journal.db");

        // A journal created before `kind` existed: same table minus the column.
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch(
                "CREATE TABLE experiences (
                    id INTEGER PRIMARY KEY, title TEXT NOT NULL DEFAULT '',
                    intention TEXT NOT NULL DEFAULT '', setting TEXT NOT NULL DEFAULT '',
                    notes TEXT NOT NULL DEFAULT '', rating INTEGER,
                    started_at TEXT NOT NULL, ended_at TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')));
                 INSERT INTO experiences (title, started_at) VALUES ('old', '2026-01-01T00:00:00Z');",
            ).unwrap();
        }

        // Reopening through the front door migrates it.
        let c = open(&path, None).unwrap();
        let exp = get_experience_row(&c, 1).unwrap();
        assert_eq!(exp.kind, "session", "existing rows keep their meaning");
        assert!(!exp.writeup_skipped, "and still wait for a write-up");
        drop(c);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_write_up_can_be_skipped_and_unskipped_without_changing_the_entry() {
        let c = mem();
        let e = create_experience(&c, &ExperienceInput {
            kind: "session".into(), title: "Quick one".into(), intention: String::new(),
            setting: String::new(), started_at: "2026-10-01T20:00:00Z".into(),
        }).unwrap();
        assert!(!e.writeup_skipped);
        let before = fingerprint(&serde_json::to_value(get_experience(&c, e.id).unwrap()).unwrap());
        assert!(set_writeup_skipped(&c, e.id, true).unwrap().writeup_skipped);
        let after = fingerprint(&serde_json::to_value(get_experience(&c, e.id).unwrap()).unwrap());
        assert_eq!(before, after, "a preference, not content: duplicates still match");
        assert!(!set_writeup_skipped(&c, e.id, false).unwrap().writeup_skipped);
    }

    #[test]
    fn only_exact_copies_are_removed_as_duplicates() {
        let c = mem();
        let entry = |note: &str, at: &str| {
            let e = create_experience(&c, &ExperienceInput {
                kind: "session".into(), title: "Evening".into(), intention: String::new(),
                setting: String::new(), started_at: at.into(),
            }).unwrap();
            log_dose(&c, &DoseInput {
                experience_id: e.id, substance_name: "Caffeine".into(), amount: Some(100.0),
                unit: "mg".into(), route: String::new(), taken_at: at.into(), note: note.into(),
            }).unwrap();
            e.id
        };
        let first = entry("", "2026-08-01 20:00:00");
        entry("", "2026-08-01T20:00:00Z"); // the same, formatted by another computer
        entry("", "2026-08-01 20:00:00");
        let differs = entry("strong cup", "2026-08-01 20:00:00");

        assert_eq!(remove_duplicate_experiences(&c).unwrap(), 2);
        let left: Vec<i64> = list_experiences(&c).unwrap().iter().map(|e| e.experience.id).collect();
        assert!(left.contains(&first), "the oldest copy is kept");
        assert!(left.contains(&differs), "an entry with a word of its own is not a duplicate");
        assert_eq!(left.len(), 2);
        assert_eq!(remove_duplicate_experiences(&c).unwrap(), 0);
    }

    #[test]
    fn duplicates_are_cleared_once_on_the_first_open_after_updating() {
        let dir = std::env::temp_dir().join(format!("fn-dedupe-open-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("journal.db");
        {
            let c = open(&path, None).unwrap();
            c.pragma_update(None, "user_version", 0).unwrap(); // as left by v0.13.2
            for _ in 0..2 {
                c.execute("INSERT INTO experiences (title, started_at) VALUES ('twice', '2026-09-01 10:00:00')", []).unwrap();
            }
        }
        let c = open(&path, None).unwrap();
        assert_eq!(list_experiences(&c).unwrap().len(), 1);
        drop(c);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Uses the bundled DoseWiki snapshot (no network). MDMA + Tramadol is a
    // graded "dangerous" interaction on both sides, so it must surface at log time.
    #[test]
    fn pw_interactions_flag_mdma_tramadol() {
        let mut c = mem();
        let all = crate::pw::parse_slim(include_str!("../resources/dosewiki.json")).expect("parse bundled");
        pw_replace_all(&mut c, &all).unwrap();

        let exp = create_experience(&c, &ExperienceInput {
            kind: "session".into(),
            title: "t".into(), intention: String::new(), setting: String::new(),
            started_at: "2026-01-01T00:00:00Z".into(),
        }).unwrap();
        let mut last = Vec::new();
        for (name, at) in [("MDMA", "2026-01-01T00:00:00Z"), ("Tramadol", "2026-01-01T01:00:00Z")] {
            let (_d, w) = log_dose(&c, &DoseInput {
                experience_id: exp.id, substance_name: name.into(), amount: Some(50.0),
                unit: "mg".into(), route: "oral".into(), taken_at: at.into(), note: String::new(),
            }).unwrap();
            last = w;
        }
        assert!(last.iter().any(|x| x.severity == "danger" && x.message.contains("DoseWiki")),
            "expected a DoseWiki danger warning for MDMA + Tramadol, got {last:?}");
    }

    // The combo checker is what people consult *before* dosing, so it must not know
    // less than the dose log does. MDMA + Tramadol is flagged only by DoseWiki's
    // graded lists — no class rule covers it — so this pair is precisely the one
    // that used to sail through the checker while being flagged at log time.
    #[test]
    fn the_combo_checker_knows_everything_the_dose_log_knows() {
        let mut c = mem();
        let all = crate::pw::parse_slim(include_str!("../resources/dosewiki.json")).expect("parse bundled");
        pw_replace_all(&mut c, &all).unwrap();

        let pair = ["MDMA".to_string(), "Tramadol".to_string()];
        let w = combo_warnings(&c, &pair);
        assert!(w.iter().any(|x| x.severity == "danger" && x.message.contains("DoseWiki")),
            "combo checker missed the DoseWiki danger for MDMA + Tramadol, got {w:?}");

        // ...and the class-rule backstop still fires for pairs DoseWiki may not list.
        let pair = ["Heroin".to_string(), "Xanax".to_string()];
        let w = combo_warnings(&c, &pair);
        assert!(w.iter().any(|x| x.severity == "danger"),
            "expected a danger warning for opioid + benzodiazepine, got {w:?}");

        // One warning per pair, never a duplicate from the two sources.
        let three = ["MDMA".to_string(), "Tramadol".to_string(), "Alcohol".to_string()];
        let w = combo_warnings(&c, &three);
        let mut pairs: Vec<(String, String)> = w
            .iter()
            .map(|x| if x.a <= x.b { (x.a.clone(), x.b.clone()) } else { (x.b.clone(), x.a.clone()) })
            .collect();
        pairs.sort();
        let before = pairs.len();
        pairs.dedup();
        assert_eq!(before, pairs.len(), "a pair was warned about twice: {w:?}");
    }
    fn bundled() -> Connection {
        let mut c = mem();
        let all = crate::pw::parse_slim(include_str!("../resources/dosewiki.json")).unwrap();
        pw_replace_all(&mut c, &all).unwrap();
        c
    }

    fn dose(c: &Connection, exp: i64, name: &str, route: &str, at: &str) -> Vec<crate::interactions::Warning> {
        log_dose(c, &DoseInput {
            experience_id: exp, substance_name: name.into(), amount: None, unit: "mg".into(),
            route: route.into(), taken_at: at.into(), note: String::new(),
        }).unwrap().1
    }

    fn session(c: &Connection) -> i64 {
        create_experience(c, &ExperienceInput {
            kind: "session".into(), title: "t".into(), intention: String::new(), setting: String::new(),
            started_at: "2025-12-31T17:30:00Z".into(),
        }).unwrap().id
    }

    #[test]
    fn wildcard_families_match_their_members_only() {
        assert!(family_match("5-meo-xxt", "5-meo-dmt"));
        assert!(family_match("5-meo-xxt", "5-meo-mipt"));
        assert!(!family_match("5-meo-xxt", "dmt"));
        assert!(family_match("2c-x", "2c-d"));
        assert!(family_match("2c-t-x", "2c-t-2"));
        assert!(!family_match("2c-t-x", "2c-d"));
        let c = bundled();
        let dmt = pw_lookup(&c, "DMT").unwrap().unwrap();
        assert!(!matches_interaction("5-MeO-xxT tryptamines", &dmt), "DMT is not a 5-MeO tryptamine");
        let fivemeo = pw_lookup(&c, "5-MeO-DMT").unwrap().unwrap();
        assert!(matches_interaction("5-MeO-xxT tryptamines", &fivemeo));
        let twocd = pw_lookup(&c, "2C-D").unwrap().unwrap();
        assert!(matches_interaction("2C-x compounds", &twocd));
    }

    #[test]
    fn butanediol_gets_ghb_warnings_and_rilmazafone_is_a_benzo() {
        let c = bundled();
        let w = combo_warnings(&c, &["1,4-Butanediol".into(), "Etizolam".into()]);
        assert!(w.iter().any(|x| x.severity == "danger"), "{w:?}");
        let w = combo_warnings(&c, &["1,4-Butanediol".into(), "Rilmazafone".into()]);
        assert!(!w.is_empty(), "GHB-class + benzo must warn: {w:?}");
    }

    #[test]
    fn only_overlapping_doses_combine() {
        let c = bundled();
        let exp = session(&c);
        // DMT lasts well under an hour; MDMA six. Hours apart, they never meet.
        dose(&c, exp, "DMT", "vaporized", "2025-12-31T18:00:00Z");
        let w = dose(&c, exp, "MDMA", "oral", "2025-12-31T23:00:00Z");
        assert!(w.is_empty(), "{w:?}");
        // A benzo inside the 1,4-B window does.
        dose(&c, exp, "1,4-Butanediol", "oral", "2026-01-01T02:12:00Z");
        let w = dose(&c, exp, "Etizolam", "oral", "2026-01-01T03:00:00Z");
        assert!(w.iter().any(|x| x.severity == "danger" && [&x.a, &x.b].iter().any(|n| n.as_str() == "Etizolam")), "{w:?}");
        // MDMA at 23:00 is still active at 02:12 (6h + margin), so that pair stays too.
        assert!(w.iter().any(|x| [&x.a, &x.b].iter().any(|n| n.as_str() == "MDMA")), "{w:?}");
    }

}
