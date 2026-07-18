// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Bidirectional, fully-offline sync between the journal and an Obsidian vault.
//!
//! Export writes one Markdown note per experience — YAML frontmatter plus a
//! human-readable body (Obsidian renders it nicely) — and appends a fenced
//! ```` ```fieldnotes ```` block holding the canonical experience data so the
//! round-trip is lossless. Import reads that block back; notes without one are
//! left untouched, so hand-written vault notes are never mangled.
//!
//! Conflict model (kept deliberately simple and predictable): export is
//! app → vault, import is vault → app and the vault wins for any experience it
//! contains (matched on start time + title).

use crate::db::{self, DoseInput, ExperienceInput, TimelineInput};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use std::path::Path;

const BLOCK_OPEN: &str = "```fieldnotes";
const BLOCK_CLOSE: &str = "```";

/// Canonical experience data as parsed back from a note's `fieldnotes` block.
#[derive(Deserialize)]
struct NoteData {
    /// `'session'` or `'note'`. Exports predating plain entries carry no kind —
    /// they were all sessions.
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    intention: String,
    #[serde(default)]
    setting: String,
    #[serde(default)]
    notes: String,
    rating: Option<i64>,
    started_at: String,
    ended_at: Option<String>,
    #[serde(default)]
    doses: Vec<NoteDose>,
    #[serde(default)]
    timeline: Vec<NoteEvent>,
}

fn default_kind() -> String {
    "session".into()
}

#[derive(Deserialize)]
struct NoteDose {
    substance_name: String,
    amount: Option<f64>,
    #[serde(default)]
    unit: String,
    #[serde(default)]
    route: String,
    taken_at: String,
    #[serde(default)]
    note: String,
}

#[derive(Deserialize)]
struct NoteEvent {
    at: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    mood: String,
    intensity: Option<i64>,
}

#[derive(serde::Serialize)]
pub struct ExportResult {
    pub written: usize,
}

#[derive(serde::Serialize)]
pub struct ImportResult {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// A filesystem-safe slug for a note filename.
fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("experience");
    }
    out
}

fn yaml_escape(s: &str) -> String {
    // Quote and escape for a double-quoted YAML scalar.
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

/// The canonical filename for an experience's Markdown note: date + title slug +
/// id, so exports are stable and never collide. Used by the vault sync and by the
/// single-entry export, so a note exported either way lands under the same name.
pub(crate) fn note_filename(exp: &db::ExperienceDetail) -> String {
    let e = &exp.experience;
    let date = e.started_at.get(0..10).unwrap_or("");
    format!("{date}-{}-{}.md", slugify(&e.title), e.id)
}

/// Render one experience as a full Markdown note.
pub(crate) fn render_note(exp: &db::ExperienceDetail) -> Result<String, String> {
    let e = &exp.experience;
    let date = e.started_at.get(0..10).unwrap_or(&e.started_at);
    let substances: Vec<String> = {
        let mut seen = std::collections::BTreeSet::new();
        for d in &exp.doses {
            seen.insert(d.substance_name.clone());
        }
        seen.into_iter().collect()
    };

    let plain_note = e.kind == "note";

    let mut s = String::new();
    // ----- frontmatter -----
    s.push_str("---\n");
    s.push_str(&format!("title: {}\n", yaml_escape(&e.title)));
    s.push_str(&format!("date: {}\n", yaml_escape(date)));
    if plain_note {
        // A plain entry is just a dated piece of writing — no rating, no
        // substances, and its frontmatter shouldn't pretend otherwise.
        s.push_str("kind: note\n");
    } else {
        if let Some(r) = e.rating {
            s.push_str(&format!("rating: {r}\n"));
        }
        s.push_str("substances:\n");
        for name in &substances {
            s.push_str(&format!("  - {}\n", yaml_escape(name)));
        }
    }
    s.push_str("tags:\n  - field-notes\n");
    s.push_str("---\n\n");

    // ----- human-readable body -----
    let fallback = if plain_note { "Untitled note" } else { "Untitled experience" };
    s.push_str(&format!("# {}\n\n", if e.title.is_empty() { fallback } else { &e.title }));
    if plain_note {
        s.push_str(&format!("*{date}*\n\n"));
    } else {
        s.push_str(&format!("*Started {}*", e.started_at));
        if let Some(end) = &e.ended_at {
            s.push_str(&format!(" · *ended {end}*"));
        }
        if let Some(r) = e.rating {
            s.push_str(&format!(" · rating {r}/5"));
        }
        s.push_str("\n\n");
    }

    if !e.intention.is_empty() {
        s.push_str(&format!("**Intention:** {}\n\n", e.intention));
    }
    if !e.setting.is_empty() {
        s.push_str(&format!("**Setting:** {}\n\n", e.setting));
    }

    if !exp.doses.is_empty() {
        s.push_str("## Doses\n\n| Time | Substance | Amount | Route | Note |\n| --- | --- | --- | --- | --- |\n");
        for d in &exp.doses {
            let amt = d.amount.map(|a| format!("{a} {}", d.unit)).unwrap_or_else(|| d.unit.clone());
            s.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                md_cell(&d.taken_at),
                md_cell(&d.substance_name),
                md_cell(&amt),
                md_cell(&d.route),
                md_cell(&d.note),
            ));
        }
        s.push('\n');
    }

    if !exp.timeline.is_empty() {
        s.push_str("## Timeline\n\n");
        for t in &exp.timeline {
            let mut line = format!("- **{}**", t.at);
            if !t.mood.is_empty() {
                line.push_str(&format!(" _{}_", t.mood));
            }
            if let Some(i) = t.intensity {
                line.push_str(&format!(" (intensity {i})"));
            }
            if !t.note.is_empty() {
                line.push_str(&format!(" — {}", md_cell(&t.note)));
            }
            s.push_str(&line);
            s.push('\n');
        }
        s.push('\n');
    }

    if !e.notes.is_empty() {
        // For a plain note the writing *is* the note — no section header over it.
        if !plain_note {
            s.push_str("## Notes\n\n");
        }
        s.push_str(&e.notes);
        s.push_str("\n\n");
    }

    // ----- canonical data block (source of truth for import) -----
    let json = serde_json::to_string_pretty(exp).map_err(err)?;
    s.push_str("## Field Notes data\n\n");
    s.push_str("<small>Machine-readable copy — edit the sections above and re-import to sync.</small>\n\n");
    s.push_str(BLOCK_OPEN);
    s.push('\n');
    s.push_str(&json);
    s.push('\n');
    s.push_str(BLOCK_CLOSE);
    s.push('\n');

    Ok(s)
}

/// Export every experience to `vault` as a Markdown note, overwriting our own
/// prior exports. Returns the number of notes written.
pub fn export_all(conn: &Connection, vault: &Path) -> Result<ExportResult, String> {
    if !vault.is_dir() {
        return Err("Choose a folder inside your Obsidian vault.".to_string());
    }
    let summaries = db::list_experiences(conn).map_err(err)?;
    let mut written = 0;
    for summ in &summaries {
        let detail = db::get_experience(conn, summ.experience.id).map_err(err)?;
        let name = note_filename(&detail);
        let note = render_note(&detail)?;
        std::fs::write(vault.join(name), note).map_err(err)?;
        written += 1;
    }
    Ok(ExportResult { written })
}

/// Pull the `fieldnotes` block out of a note's text, if present.
fn extract_block(text: &str) -> Option<&str> {
    let start = text.find(BLOCK_OPEN)? + BLOCK_OPEN.len();
    let rest = &text[start..];
    let rest = rest.strip_prefix('\n').unwrap_or(rest);
    let end = rest.find("\n```")?;
    Some(&rest[..end])
}

/// Find an existing experience with the same start time + title.
fn find_match(conn: &Connection, started_at: &str, title: &str) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT id FROM experiences WHERE started_at = ?1 AND title = ?2 LIMIT 1",
        params![started_at, title],
        |r| r.get::<_, i64>(0),
    )
    .optional()
}

/// Write the doses + timeline of `note` under experience `id`, replacing any
/// existing ones (used for both create and update so the vault stays authoritative).
fn write_children(conn: &Connection, id: i64, note: &NoteData) -> Result<(), String> {
    conn.execute("DELETE FROM doses WHERE experience_id = ?1", params![id]).map_err(err)?;
    conn.execute("DELETE FROM timeline_events WHERE experience_id = ?1", params![id]).map_err(err)?;
    for d in &note.doses {
        if d.substance_name.trim().is_empty() {
            continue;
        }
        let unit = if d.unit.is_empty() { "mg".to_string() } else { d.unit.clone() };
        db::log_dose(
            conn,
            &DoseInput {
                experience_id: id,
                substance_name: d.substance_name.clone(),
                amount: d.amount,
                unit,
                route: d.route.clone(),
                taken_at: if d.taken_at.is_empty() { note.started_at.clone() } else { d.taken_at.clone() },
                note: d.note.clone(),
            },
        )
        .map_err(err)?;
    }
    for t in &note.timeline {
        db::add_timeline_event(
            conn,
            &TimelineInput {
                experience_id: id,
                at: if t.at.is_empty() { note.started_at.clone() } else { t.at.clone() },
                note: t.note.clone(),
                mood: t.mood.clone(),
                intensity: t.intensity,
            },
        )
        .map_err(err)?;
    }
    Ok(())
}

/// Import every Field Notes note found in `vault` (non-recursive). Experiences
/// already present (same start time + title) are updated from the vault; new
/// ones are created. Notes without a `fieldnotes` block are skipped.
pub fn import_all(conn: &Connection, vault: &Path) -> Result<ImportResult, String> {
    if !vault.is_dir() {
        return Err("Choose a folder inside your Obsidian vault.".to_string());
    }
    let mut res = ImportResult { created: 0, updated: 0, skipped: 0 };
    for entry in std::fs::read_dir(vault).map_err(err)? {
        let entry = entry.map_err(err)?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(err)?;
        let Some(block) = extract_block(&text) else {
            res.skipped += 1;
            continue;
        };
        let note: NoteData = match serde_json::from_str(block) {
            Ok(n) => n,
            Err(_) => {
                res.skipped += 1;
                continue;
            }
        };

        match find_match(conn, &note.started_at, &note.title).map_err(err)? {
            Some(id) => {
                db::update_experience(
                    conn,
                    id,
                    &db::ExperienceUpdate {
                        title: note.title.clone(),
                        intention: note.intention.clone(),
                        setting: note.setting.clone(),
                        notes: note.notes.clone(),
                        rating: note.rating,
                        started_at: note.started_at.clone(),
                        ended_at: note.ended_at.clone(),
                    },
                )
                .map_err(err)?;
                write_children(conn, id, &note)?;
                res.updated += 1;
            }
            None => {
                let exp = db::create_experience(
                    conn,
                    &ExperienceInput {
                        kind: note.kind.clone(),
                        title: note.title.clone(),
                        intention: note.intention.clone(),
                        setting: note.setting.clone(),
                        started_at: note.started_at.clone(),
                    },
                )
                .map_err(err)?;
                // create_experience doesn't take notes/rating/ended — set them now.
                db::update_experience(
                    conn,
                    exp.id,
                    &db::ExperienceUpdate {
                        title: note.title.clone(),
                        intention: note.intention.clone(),
                        setting: note.setting.clone(),
                        notes: note.notes.clone(),
                        rating: note.rating,
                        started_at: note.started_at.clone(),
                        ended_at: note.ended_at.clone(),
                    },
                )
                .map_err(err)?;
                write_children(conn, exp.id, &note)?;
                res.created += 1;
            }
        }
    }
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        c.execute_batch(db::schema_for_tests()).unwrap();
        c
    }

    #[test]
    fn round_trips_an_experience_through_the_vault() {
        let dir = std::env::temp_dir().join(format!("fn-obsidian-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let src = mem();
        let exp = db::create_experience(
            &src,
            &ExperienceInput {
                kind: "session".into(),
                title: "Test session".into(),
                intention: "learn".into(),
                setting: "home".into(),
                started_at: "2026-07-01T20:00:00Z".into(),
            },
        )
        .unwrap();
        db::log_dose(
            &src,
            &DoseInput {
                experience_id: exp.id,
                substance_name: "Caffeine".into(),
                amount: Some(100.0),
                unit: "mg".into(),
                route: "oral".into(),
                taken_at: "2026-07-01T20:05:00Z".into(),
                note: "coffee".into(),
            },
        )
        .unwrap();
        let written = export_all(&src, &dir).unwrap();
        assert_eq!(written.written, 1);

        // Import into a fresh DB — the experience and its dose should reappear.
        let dst = mem();
        let r = import_all(&dst, &dir).unwrap();
        assert_eq!(r.created, 1);
        let got = db::list_experiences(&dst).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].experience.title, "Test session");
        let detail = db::get_experience(&dst, got[0].experience.id).unwrap();
        assert_eq!(detail.doses.len(), 1);
        assert_eq!(detail.doses[0].substance_name, "Caffeine");

        // Re-importing is idempotent (updates in place, no duplicate).
        let r2 = import_all(&dst, &dir).unwrap();
        assert_eq!(r2.updated, 1);
        assert_eq!(db::list_experiences(&dst).unwrap().len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn round_trips_a_plain_note_and_keeps_it_quiet() {
        let dir = std::env::temp_dir().join(format!("fn-obsidian-note-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let src = mem();
        let exp = db::create_experience(
            &src,
            &ExperienceInput {
                kind: "note".into(),
                title: "A quiet day".into(),
                intention: String::new(),
                setting: String::new(),
                started_at: "2026-07-03T09:00:00Z".into(),
            },
        )
        .unwrap();
        db::update_experience(
            &src,
            exp.id,
            &db::ExperienceUpdate {
                title: "A quiet day".into(),
                intention: String::new(),
                setting: String::new(),
                notes: "Slept in. Walked by the river.".into(),
                rating: None,
                started_at: "2026-07-03T09:00:00Z".into(),
                ended_at: None,
            },
        )
        .unwrap();
        export_all(&src, &dir).unwrap();

        // The rendered Markdown is a note, not a session with the drug parts hidden.
        let file = std::fs::read_dir(&dir).unwrap().next().unwrap().unwrap().path();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("kind: note"));
        assert!(!text.contains("substances:"), "a plain note lists no substances");
        assert!(!text.contains("## Doses"));
        assert!(text.contains("Walked by the river."));

        // And it comes back as a note.
        let dst = mem();
        let r = import_all(&dst, &dir).unwrap();
        assert_eq!(r.created, 1);
        let got = db::list_experiences(&dst).unwrap();
        assert_eq!(got[0].experience.kind, "note");
        assert_eq!(got[0].experience.notes, "Slept in. Walked by the river.");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn slugify_is_filesystem_safe() {
        assert_eq!(slugify("A Wild/Trip: 2026!"), "a-wild-trip-2026");
        assert_eq!(slugify(""), "experience");
    }
}
