// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Consent-gated upstream contribution drafts.
//!
//! When you catalogue a substance that DoseWiki doesn't cover, that gap is worth
//! closing upstream — DoseWiki is CC0 and open-source, and the obscure compounds
//! missing from it are exactly the ones where the next person has nowhere to look.
//!
//! So this module turns a locally-added substance into a DoseWiki-shaped JSON
//! draft that the user reviews and submits **by hand**.
//!
//! ## What this module does not do
//!
//! **It never touches the network.** There is no HTTP client here and there must
//! never be one. `draft()` returns a string; saving it is a file dialog, and
//! submitting it is the user opening DoseWiki themselves. An auto-upload path —
//! even an opt-in one, even a "just this once" one — is out of scope for this
//! module and out of character for this app. The data is legally fraught and it
//! is not ours to send.
//!
//! **It never includes journal data.** A draft is built from the *catalogue*
//! entry — the name, classes, and notes the user typed about the substance — and
//! nothing else. No doses, no experiences, no timestamps, not even the row's
//! `created_at` (when you first catalogued a compound is itself a disclosure).
//! A contribution says "this substance exists and here is what is known about
//! it", never "here is what I took". `draft_carries_no_journal_data` enforces it.
//!
//! **It never fabricates numbers.** The local catalogue stores dose notes as free
//! text; DoseWiki stores structured ranges. Parsing one into the other would mean
//! guessing at dose figures and shipping the guess upstream with a confidence it
//! hasn't earned. The dose and duration blocks therefore go out **empty**, with
//! the user's own note carried alongside as prose for them to formalise.

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use serde_json::json;

/// Where a finished draft goes. Shown to the user; never fetched by us.
pub const UPSTREAM_URL: &str = "https://github.com/dose-wiki/dose.wiki";

/// A locally-added substance, and whether it's worth contributing.
#[derive(Serialize)]
pub struct Candidate {
    pub id: i64,
    pub name: String,
    /// DoseWiki already covers this one, so there's nothing to contribute.
    pub in_dosewiki: bool,
    /// The user has already exported a draft for it.
    pub contributed: bool,
}

/// A DoseWiki-shaped draft, ready for the user to read, edit, and submit.
#[derive(Serialize)]
pub struct Draft {
    pub name: String,
    pub slug: String,
    /// Pretty-printed JSON — this is meant to be read and edited by a human.
    pub json: String,
    pub upstream_url: String,
}

/// DoseWiki slugs are lowercase, hyphen-separated: "2C-B (pink)" -> "2c-b-pink".
fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut pending_sep = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_sep && !out.is_empty() {
                out.push('-');
            }
            pending_sep = false;
            out.extend(c.to_lowercase());
        } else {
            pending_sep = true;
        }
    }
    out
}

/// Every user-added substance, flagged with whether it's actually a gap upstream.
///
/// Substances that shipped with the app are not candidates — they came from
/// DoseWiki in the first place.
pub fn candidates(conn: &Connection) -> rusqlite::Result<Vec<Candidate>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, contributed FROM substances WHERE user_added = 1 ORDER BY name COLLATE NOCASE",
    )?;
    let rows: Vec<(i64, String, bool)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? != 0)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut out = Vec::with_capacity(rows.len());
    for (id, name, contributed) in rows {
        let known: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM pw_substances WHERE name = ?1 COLLATE NOCASE",
                [&name],
                |r| r.get(0),
            )
            .optional()?;
        out.push(Candidate { id, name, in_dosewiki: known.is_some(), contributed });
    }
    Ok(out)
}

/// Build the draft for one user-added substance.
///
/// Reads exactly one row of `substances` — see the module docs on why nothing
/// else may be read here.
pub fn draft(conn: &Connection, id: i64) -> Result<Draft, String> {
    let (name, aliases, category, classes, dose_note, notes): (
        String,
        String,
        String,
        String,
        String,
        String,
    ) = conn
        .query_row(
            "SELECT name, aliases, category, classes, dose_note, notes
               FROM substances WHERE id = ?1 AND user_added = 1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or("No user-added substance with that id.")?;

    let aliases: Vec<String> = serde_json::from_str(&aliases).unwrap_or_default();
    let classes: Vec<String> = serde_json::from_str(&classes).unwrap_or_default();

    // The user's free-text dose note is prose, not data. It rides along as prose so
    // the user can turn it into structured ranges upstream, where it will be
    // reviewed — rather than us guessing at numbers here. See module docs.
    let summary = if notes.trim().is_empty() {
        format!("TODO — describe {name}. (Drafted in Field Notes; written up by the contributor.)")
    } else {
        notes.trim().to_string()
    };

    let record = json!({
        "slug": slugify(&name),
        "title": name,
        "summary": summary,
        "classification": {
            "chemical_class": if category.trim().is_empty() { vec![] } else { vec![category.trim()] },
            "psychoactive_class": classes,
        },
        "common_names": aliases,
        // Left empty on purpose: the app has no structured dose data for a
        // user-added substance, and inventing it would be worse than a blank.
        "dosage": { "routes": [], "plateau_dosing": null },
        "duration": { "routes": [] },
        "interactions": { "dangerous": [], "unsafe": [], "caution": [] },
        "pharmacology": {},
        "harm_potential": {},
        "tolerance": {},
        "legality": { "countries": {} },
        "editorial_review": { "status": "needed", "notes": "Drafted in Field Notes — unverified." },
        "_field_notes_draft": {
            "dose_note": dose_note,
            "instructions": [
                "This is a DRAFT. Nothing has been sent anywhere — Field Notes never uploads.",
                "Read it, fix it, fill in the empty blocks, and delete this _field_notes_draft key.",
                "Dose and duration are blank because the app had no verified figures. Add them only from sources you trust.",
                "Submit it yourself at the URL below, and only if you want to.",
            ],
            "upstream": UPSTREAM_URL,
        },
    });

    Ok(Draft {
        name: name.clone(),
        slug: slugify(&name),
        json: serde_json::to_string_pretty(&record).map_err(|e| e.to_string())?,
        upstream_url: UPSTREAM_URL.to_string(),
    })
}

/// Remember that the user exported a draft, so the UI can stop nudging them.
/// Purely a local bookkeeping flag — it does not mean anything was sent.
pub fn mark_contributed(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("UPDATE substances SET contributed = 1 WHERE id = ?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(db::schema_for_tests()).unwrap();
        conn.execute(
            "INSERT INTO substances (name, aliases, category, classes, dose_note, notes, user_added)
             VALUES ('Ethylphenidate', '[\"EPH\"]', 'Phenidate', '[\"stimulant\"]',
                     'started around 20mg oral', 'A short-acting stimulant.', 1)",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn a_substance_dosewiki_already_covers_is_not_a_candidate() {
        let conn = setup();
        conn.execute(
            "INSERT INTO pw_substances (name, data) VALUES ('ethylphenidate', '{}')",
            [],
        )
        .unwrap();
        let c = candidates(&conn).unwrap();
        assert_eq!(c.len(), 1);
        assert!(c[0].in_dosewiki, "upstream has it — nothing to contribute");

        // With no upstream entry, it IS a gap worth filling.
        conn.execute("DELETE FROM pw_substances", []).unwrap();
        assert!(!candidates(&conn).unwrap()[0].in_dosewiki);
    }

    #[test]
    fn draft_is_shaped_like_a_dosewiki_record() {
        let conn = setup();
        let d = draft(&conn, 1).unwrap();
        assert_eq!(d.slug, "ethylphenidate");
        let v: serde_json::Value = serde_json::from_str(&d.json).unwrap();
        for key in ["slug", "title", "summary", "classification", "dosage", "duration", "interactions", "editorial_review"] {
            assert!(v.get(key).is_some(), "draft is missing DoseWiki's `{key}`");
        }
        assert_eq!(v["editorial_review"]["status"], "needed");
        assert_eq!(v["classification"]["psychoactive_class"][0], "stimulant");
    }

    #[test]
    fn draft_invents_no_dose_figures() {
        // The user wrote "started around 20mg oral" in a free-text note. That must
        // not become a structured dose range with upstream's authority behind it.
        let conn = setup();
        let v: serde_json::Value = serde_json::from_str(&draft(&conn, 1).unwrap().json).unwrap();
        assert_eq!(v["dosage"]["routes"].as_array().unwrap().len(), 0);
        assert_eq!(v["duration"]["routes"].as_array().unwrap().len(), 0);
        // It survives as prose, for the user to formalise themselves.
        assert_eq!(v["_field_notes_draft"]["dose_note"], "started around 20mg oral");
    }

    #[test]
    fn draft_carries_no_journal_data() {
        // A contribution says "this substance exists", never "here is what I took".
        // If someone widens the query in `draft()` to join the journal, this fails.
        let conn = setup();
        conn.execute(
            "INSERT INTO experiences (id, title, started_at) VALUES (1, 'Tuesday night', '2026-03-04T21:00:00')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO doses (experience_id, substance_name, amount, unit, route, taken_at)
             VALUES (1, 'Ethylphenidate', 40, 'mg', 'oral', '2026-03-04T21:30:00')",
            [],
        )
        .unwrap();

        let json = draft(&conn, 1).unwrap().json;
        for leak in ["Tuesday night", "2026-03-04", "21:30", "40"] {
            assert!(!json.contains(leak), "journal data leaked into the draft: {leak}");
        }
    }

    #[test]
    fn exporting_marks_it_locally_without_sending_anything() {
        let conn = setup();
        assert!(!candidates(&conn).unwrap()[0].contributed);
        mark_contributed(&conn, 1).unwrap();
        assert!(candidates(&conn).unwrap()[0].contributed);
    }

    #[test]
    fn slugs_match_upstream_style() {
        assert_eq!(slugify("2C-B"), "2c-b");
        assert_eq!(slugify("alpha-PCYP"), "alpha-pcyp");
        assert_eq!(slugify("N,N-DMT"), "n-n-dmt");
        assert_eq!(slugify("  Spaced  Out  "), "spaced-out");
    }
}
