// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Right after a dose of something Field Notes knows little about: what kind of
//! drug is it, and how long does it last? (owner's request, 2026-10-16)
//!
//! Most new substances arrive by being logged, not through the catalogue form,
//! so this is asked on the dose receipt, where it can still be answered:
//!
//! - **Its class** when Stats can't place it in a drug class (it would sit under
//!   "Other", and the combination checker would have nothing to go on). The
//!   answer is saved as the catalogue entry's classes, the same ones the catalogue
//!   form writes, so Stats and the checker both use it.
//! - **How long it lasts** when nothing gives timings for it: not the dose
//!   reference, not the person's own. The answer is saved as their own timings
//!   (`my_refs`), which draw its curve on Today.
//!
//! Asked once per substance, ever: any answer, or skipping, ends it.

use crate::db;
use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Serialize, Debug, PartialEq)]
pub struct UnknownQuestion {
    /// The name as logged.
    pub substance: String,
    /// Stats can't place it in a drug class.
    pub ask_class: bool,
    /// Nothing gives timings for it.
    pub ask_timings: bool,
}

fn asked(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    conn.query_row("SELECT COUNT(*) FROM substance_asked WHERE name = ?1", [name], |r| r.get::<_, i64>(0))
        .map(|n| n > 0)
}

/// What to ask about a substance just logged, or nothing.
pub fn question(conn: &Connection, substance: &str) -> rusqlite::Result<Option<UnknownQuestion>> {
    let name = substance.trim();
    if name.is_empty() || asked(conn, name)? {
        return Ok(None);
    }
    let ask_class = crate::stats::families_of(conn, name) == ["other"];
    let ask_timings = !db::pw_lookup(conn, name)?.is_some_and(|i| {
        i.roas.iter().any(|r| r.onset.is_some() || r.peak.is_some() || r.offset.is_some() || r.total.is_some())
    });
    Ok((ask_class || ask_timings).then(|| UnknownQuestion { substance: name.to_string(), ask_class, ask_timings }))
}

/// Answered or skipped: don't ask about it again.
pub fn done(conn: &Connection, substance: &str) -> rusqlite::Result<()> {
    conn.execute("INSERT OR IGNORE INTO substance_asked (name) VALUES (?1)", [substance.trim()])?;
    Ok(())
}

/// Save what kind of drug it is as its catalogue classes, adding it to the
/// catalogue if it isn't there. Nothing else about an existing entry changes.
pub fn set_classes(conn: &Connection, substance: &str, classes: &[String]) -> rusqlite::Result<()> {
    let name = substance.trim();
    let known: Vec<&str> = classes.iter().map(String::as_str).filter(|c| crate::interactions::CLASSES.contains(c)).collect();
    let json = serde_json::to_string(&known).unwrap_or_else(|_| "[]".into());
    let updated = conn.execute("UPDATE substances SET classes = ?2 WHERE name = ?1 COLLATE NOCASE", params![name, json])?;
    if updated == 0 {
        conn.execute("INSERT INTO substances (name, classes, user_added) VALUES (?1, ?2, 1)", params![name, json])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(db::schema_for_tests()).unwrap();
        c
    }

    #[test]
    fn something_unknown_is_asked_both() {
        let c = conn();
        let q = question(&c, " Nurtec ").unwrap().unwrap();
        assert_eq!(q, UnknownQuestion { substance: "Nurtec".into(), ask_class: true, ask_timings: true });
    }

    #[test]
    fn a_class_saved_moves_it_out_of_other_and_keeps_the_entry() {
        let c = conn();
        c.execute("INSERT INTO substances (name, notes) VALUES ('Mystery', 'from a friend')", []).unwrap();
        set_classes(&c, "mystery", &["opioid".into(), "not_a_class".into()]).unwrap();
        assert_eq!(crate::stats::families_of(&c, "Mystery"), ["opioids"]);
        let (classes, notes): (String, String) =
            c.query_row("SELECT classes, notes FROM substances WHERE name = 'Mystery'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((classes.as_str(), notes.as_str()), (r#"["opioid"]"#, "from a friend"));
        // Still no timings, so that's still worth asking.
        let q = question(&c, "Mystery").unwrap().unwrap();
        assert!(!q.ask_class && q.ask_timings);
    }

    #[test]
    fn a_new_name_joins_the_catalogue() {
        let c = conn();
        set_classes(&c, "Glimmer", &["stimulant".into()]).unwrap();
        assert_eq!(crate::stats::families_of(&c, "Glimmer"), ["stimulants"]);
    }

    #[test]
    fn known_class_and_own_timings_ask_nothing() {
        let c = conn();
        // Classed by the built-in matcher; timings of the person's own.
        let t = db::MyTimings {
            route: "oral".into(), onset: Some("1 hour".into()), come_up: None, peak: None, offset: None,
            after_effects: None, total: Some("6 hours".into()),
        };
        db::set_my_ref(&c, "Kratom", &t).unwrap();
        assert_eq!(question(&c, "Kratom").unwrap(), None);
    }

    #[test]
    fn asked_once_ever() {
        let c = conn();
        done(&c, "Nurtec").unwrap();
        done(&c, "Nurtec").unwrap();
        assert_eq!(question(&c, "Nurtec").unwrap(), None);
        assert_eq!(question(&c, "nurtec").unwrap(), None, "any capitalisation");
    }
}
