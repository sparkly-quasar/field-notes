// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Local SQLite store for the harm-reduction journal: substances the user
//! catalogues, experiences, the doses taken during them, and a live timeline.
//! Everything stays on-device. Timestamps are ISO-8601 strings supplied by the
//! frontend; `created_at` columns default to SQLite's clock.

use crate::pw::PwInfo;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// Open (creating if needed) the journal database at `path` and run migrations.
pub fn init(path: &std::path::Path) -> rusqlite::Result<Connection> {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
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
    title       TEXT NOT NULL DEFAULT '',
    intention   TEXT NOT NULL DEFAULT '',
    setting     TEXT NOT NULL DEFAULT '',
    notes       TEXT NOT NULL DEFAULT '',
    rating      INTEGER,
    started_at  TEXT NOT NULL,
    ended_at    TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
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

-- Cached PsychonautWiki reference data (CC-BY-SA 4.0). One row per substance;
-- `data` is a serialized pw::PwInfo. Populated only on explicit user refresh.
CREATE TABLE IF NOT EXISTS pw_substances (
    name       TEXT PRIMARY KEY,
    data       TEXT NOT NULL,
    fetched_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#;

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
    pub title: String,
    pub intention: String,
    pub setting: String,
    pub notes: String,
    pub rating: Option<i64>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub created_at: String,
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
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub intention: String,
    #[serde(default)]
    pub setting: String,
    pub started_at: String,
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
        title: r.get("title")?,
        intention: r.get("intention")?,
        setting: r.get("setting")?,
        notes: r.get("notes")?,
        rating: r.get("rating")?,
        started_at: r.get("started_at")?,
        ended_at: r.get("ended_at")?,
        created_at: r.get("created_at")?,
    })
}

pub fn create_experience(conn: &Connection, input: &ExperienceInput) -> rusqlite::Result<Experience> {
    conn.execute(
        "INSERT INTO experiences (title, intention, setting, started_at) VALUES (?1, ?2, ?3, ?4)",
        params![input.title, input.intention, input.setting, input.started_at],
    )?;
    let id = conn.last_insert_rowid();
    get_experience_row(conn, id)
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

    // Gather every distinct substance in this experience and check interactions.
    let mut stmt =
        conn.prepare("SELECT DISTINCT substance_name FROM doses WHERE experience_id = ?1")?;
    let names: Vec<String> =
        stmt.query_map([input.experience_id], |r| r.get(0))?.collect::<Result<_, _>>()?;
    let with_classes: Vec<(String, Vec<String>)> =
        names.iter().map(|n| (n.clone(), classes_for(conn, n))).collect();
    let mut warnings = crate::interactions::check(&with_classes);
    warnings.extend(pw_interaction_warnings(conn, &names));
    let warnings = crate::interactions::dedup_pairs(warnings);

    Ok((dose, warnings))
}

pub fn add_timeline_event(conn: &Connection, input: &TimelineInput) -> rusqlite::Result<TimelineEvent> {
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

pub fn update_experience(conn: &Connection, id: i64, u: &ExperienceUpdate) -> rusqlite::Result<Experience> {
    conn.execute(
        "UPDATE experiences SET title=?2, intention=?3, setting=?4, notes=?5, rating=?6,
             started_at=?7, ended_at=?8 WHERE id=?1",
        params![id, u.title, u.intention, u.setting, u.notes, u.rating, u.started_at, u.ended_at],
    )?;
    get_experience_row(conn, id)
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

// ---------- PsychonautWiki reference cache ----------

/// Replace the whole cache with a freshly fetched set, in one transaction.
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

/// (number of cached substances, most recent fetch timestamp).
pub fn pw_status(conn: &Connection) -> rusqlite::Result<(i64, Option<String>)> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM pw_substances", [], |r| r.get(0))?;
    let last: Option<String> = conn.query_row("SELECT MAX(fetched_at) FROM pw_substances", [], |r| r.get(0))?;
    Ok((count, last))
}

/// Does a PsychonautWiki interaction entry (a substance name or a class like
/// "Stimulants"/"MAOIs") refer to `other`? Matches `other`'s name, aliases, and
/// psychoactive/chemical classes, with light singular/substring tolerance.
fn matches_interaction(interaction: &str, other: &PwInfo) -> bool {
    let i = interaction.to_lowercase();
    let i_sing = i.trim_end_matches('s');
    let mut ids: Vec<String> = vec![other.name.to_lowercase()];
    ids.extend(other.common_names.iter().map(|s| s.to_lowercase()));
    ids.extend(other.psychoactive.iter().map(|s| s.to_lowercase()));
    ids.extend(other.chemical.iter().map(|s| s.to_lowercase()));
    ids.iter().any(|id| {
        let id_sing = id.trim_end_matches('s');
        id == &i || id_sing == i_sing || (i.len() >= 4 && (id.contains(&i) || i.contains(id.as_str())))
    })
}

/// Warnings from PsychonautWiki's `dangerousInteractions` for every pair of the
/// given substances that has cached reference data.
pub fn pw_interaction_warnings(conn: &Connection, names: &[String]) -> Vec<crate::interactions::Warning> {
    use crate::interactions::{Warning, PW_MESSAGE};
    let infos: Vec<(String, PwInfo)> = names
        .iter()
        .filter_map(|n| pw_lookup(conn, n).ok().flatten().map(|info| (n.clone(), info)))
        .collect();
    let mut out = Vec::new();
    for a in 0..infos.len() {
        for b in (a + 1)..infos.len() {
            let (na, ia) = &infos[a];
            let (nb, ib) = &infos[b];
            let hit = ia.interactions.iter().any(|x| matches_interaction(x, ib))
                || ib.interactions.iter().any(|x| matches_interaction(x, ia));
            if hit {
                out.push(Warning { severity: "danger", a: na.clone(), b: nb.clone(), message: PW_MESSAGE });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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

    // Needs network (fetches live PsychonautWiki data). Run explicitly:
    //   cargo test --lib -- --ignored --nocapture pw_interactions
    #[test]
    #[ignore]
    fn pw_interactions_flag_mdma_tramadol() {
        let mut c = mem();
        let all = crate::pw::fetch_all().expect("fetch PW");
        pw_replace_all(&mut c, &all).unwrap();

        let exp = create_experience(&c, &ExperienceInput {
            title: "t".into(), intention: String::new(), setting: String::new(),
            started_at: "2026-01-01T00:00:00Z".into(),
        }).unwrap();
        for (name, at) in [("MDMA", "2026-01-01T00:00:00Z"), ("Tramadol", "2026-01-01T01:00:00Z")] {
            let (_d, w) = log_dose(&c, &DoseInput {
                experience_id: exp.id, substance_name: name.into(), amount: Some(50.0),
                unit: "mg".into(), route: "oral".into(), taken_at: at.into(), note: String::new(),
            }).unwrap();
            eprintln!("after {name}: {:?}", w.iter().map(|x| (x.severity, x.a.as_str(), x.b.as_str())).collect::<Vec<_>>());
            if name == "Tramadol" {
                assert!(w.iter().any(|x| x.message.contains("PsychonautWiki")),
                    "expected a PsychonautWiki interaction warning for MDMA + Tramadol");
            }
        }
    }
}
