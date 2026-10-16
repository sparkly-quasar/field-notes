// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Suggesting reference data back to Field Notes.
//!
//! When someone types their own timings for a substance the dose reference has
//! none for (`my_refs`), or catalogues a substance DoseWiki doesn't cover, the
//! next person usually needs the same thing. This module gathers those, lets the
//! person read exactly what would be shared, and turns their choice into a
//! **prefilled GitHub issue** on the Field Notes repo, which they open in their
//! own browser and submit there, or don't. Reports are reviewed by hand before
//! anything reaches the bundled reference.
//!
//! The same three rules as `contribute.rs`, each with a test:
//!
//! - **No network. Ever.** There is no HTTP client here and there must never be
//!   one. [`issues`] returns URLs; opening one is the person's browser, and
//!   submitting it is them pressing the button on GitHub. Nothing is sent for
//!   them, not even opt-in, not even "anonymously": an app that uploads drug
//!   data on someone's behalf is not this app.
//! - **No journal data.** Built from `my_refs` and the *catalogue* only, never a
//!   dose, an experience, or a timestamp (including `updated_at` and
//!   `created_at`: when you first looked something up is itself a disclosure).
//!   `issues_carry_no_journal_data` enforces it.
//! - **No invented numbers.** Timings go out as the person typed them
//!   ("30–60 minutes"), and a dose note as prose. Nothing is parsed into figures
//!   that would read as more certain than a single person's report.
//!
//! Every item is the person's to share or keep: one marked private is never
//! offered again, and one shared is offered again only if they change it. The
//! monthly reminder ([`due`]) is only ever a question, asked at most once every
//! [`ASK_EVERY_DAYS`] days and only when there's something new; it can be turned
//! off, and answering "not now" counts as asked.

use crate::db::{self, MyTimings};
use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};

/// Where a suggestion goes. Shown to the person; never fetched by us.
pub const REPO: &str = "https://github.com/sparkly-quasar/field-notes";
/// The issue template that labels suggestions `reference-data`, so they collect
/// in one place to review. (A `labels=` parameter would be dropped for anyone
/// without triage access to the repo; a template's own labels are not.)
pub const TEMPLATE: &str = "reference-data.md";
/// How often the reminder may ask, at most.
pub const ASK_EVERY_DAYS: i64 = 30;
/// GitHub stops prefilling a new issue somewhere past 8,000 characters of URL.
/// Stay well under, and split a long list across more than one issue.
const MAX_URL: usize = 7000;

const ASKED: &str = "suggest_asked";
const PROMPT: &str = "suggest_prompt";

/// One thing that could be suggested, and exactly what would be shared.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Item {
    /// Stable id: `timings:<name>` or `substance:<name>`, name lowercased.
    pub key: String,
    /// "timings" (how long it lasted) or "substance" (missing from the reference).
    pub kind: String,
    pub name: String,
    /// The reference has an entry under this name (so the gap is in its timings).
    pub in_reference: bool,
    /// What would be shared, as label and value, in order, for the preview.
    pub fields: Vec<(String, String)>,
    /// Shared before, and changed since.
    pub changed: bool,
}

/// The pending list, and whether now is the time to ask about it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Suggestions {
    pub items: Vec<Item>,
    /// Ask now: the reminder is on, there's something pending, and it hasn't
    /// asked in the last [`ASK_EVERY_DAYS`] days.
    pub due: bool,
    /// The monthly reminder is on (the default).
    pub reminder: bool,
}

/// A prefilled new-issue page, holding some of the chosen items.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Issue {
    pub title: String,
    pub url: String,
    /// The items in this issue, to mark shared once it's opened.
    pub keys: Vec<String>,
}

fn key(kind: &str, name: &str) -> String {
    format!("{kind}:{}", name.trim().to_lowercase())
}

fn in_reference(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    Ok(conn
        .query_row("SELECT 1 FROM pw_substances WHERE name = ?1 COLLATE NOCASE", [name.trim()], |_| Ok(()))
        .optional()?
        .is_some())
}

/// Everything that could be suggested, before checking what's been shared. Each
/// item's `Value` is exactly what goes in the issue, and what a later edit is
/// compared against.
fn all(conn: &Connection) -> rusqlite::Result<Vec<(Item, Value)>> {
    let mut out = Vec::new();

    // Timings the person typed in themselves. Only `name` and `data`: never
    // `updated_at`, see the module docs.
    let mut stmt = conn.prepare("SELECT name, data FROM my_refs ORDER BY name COLLATE NOCASE")?;
    let refs: Vec<(String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    for (name, data) in refs {
        let Ok(t) = serde_json::from_str::<MyTimings>(&data) else { continue };
        let stages = [
            ("Starts working within", &t.onset),
            ("Builds up over", &t.come_up),
            ("Strongest for", &t.peak),
            ("Wears off over", &t.offset),
            ("Lasts in total", &t.total),
            ("After-effects for", &t.after_effects),
        ];
        let mut fields = vec![("Taken".to_string(), t.route.clone())];
        fields.extend(stages.iter().filter_map(|(l, v)| v.as_ref().map(|v| (l.to_string(), v.clone()))));
        if fields.len() == 1 {
            continue; // a route and nothing else says nothing
        }
        let in_ref = in_reference(conn, &name)?;
        let value = json!({
            "kind": "timings",
            "substance": name,
            "in_reference": in_ref,
            "route": t.route,
            "onset": t.onset,
            "come_up": t.come_up,
            "peak": t.peak,
            "offset": t.offset,
            "total": t.total,
            "after_effects": t.after_effects,
        });
        out.push((
            Item { key: key("timings", &name), kind: "timings".into(), name, in_reference: in_ref, fields, changed: false },
            value,
        ));
    }

    // Substances the person catalogued that the reference doesn't cover. The
    // catalogue row only: never `created_at`, see the module docs.
    let mut stmt = conn.prepare(
        "SELECT name, aliases, category, classes, dose_note, notes FROM substances
          WHERE user_added = 1 ORDER BY name COLLATE NOCASE",
    )?;
    let subs: Vec<(String, String, String, String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (name, aliases, category, classes, dose_note, notes) in subs {
        if in_reference(conn, &name)? {
            continue;
        }
        let aliases: Vec<String> = serde_json::from_str(&aliases).unwrap_or_default();
        let classes: Vec<String> = serde_json::from_str(&classes).unwrap_or_default();
        let (category, dose_note, notes) = (category.trim().to_string(), dose_note.trim().to_string(), notes.trim().to_string());
        let mut fields = Vec::new();
        if !aliases.is_empty() {
            fields.push(("Also called".to_string(), aliases.join(", ")));
        }
        if !category.is_empty() {
            fields.push(("Chemical class".to_string(), category.clone()));
        }
        if !classes.is_empty() {
            fields.push(("Interaction classes".to_string(), classes.join(", ")));
        }
        if !dose_note.is_empty() {
            fields.push(("Dose note".to_string(), dose_note.clone()));
        }
        if !notes.is_empty() {
            fields.push(("Notes".to_string(), notes.clone()));
        }
        let value = json!({
            "kind": "substance",
            "substance": name,
            "aliases": aliases,
            "chemical_class": category,
            "classes": classes,
            "dose_note": dose_note,
            "notes": notes,
        });
        out.push((
            Item { key: key("substance", &name), kind: "substance".into(), name, in_reference: false, fields, changed: false },
            value,
        ));
    }
    Ok(out)
}

/// What hasn't been shared yet, or has changed since it was, and isn't private.
pub fn pending(conn: &Connection) -> rusqlite::Result<Vec<Item>> {
    let mut out = Vec::new();
    for (mut item, value) in all(conn)? {
        let seen: Option<(String, String)> = conn
            .query_row("SELECT content, state FROM ref_suggestions WHERE key = ?1", [&item.key], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?;
        match seen {
            None => out.push(item),
            Some((_, state)) if state == "private" => {}
            Some((content, _)) if content != value.to_string() => {
                item.changed = true;
                out.push(item);
            }
            Some(_) => {}
        }
    }
    Ok(out)
}

/// The pending list, and whether to ask about it on `today` (the local date).
pub fn suggestions(conn: &Connection, today: Option<NaiveDate>) -> rusqlite::Result<Suggestions> {
    let items = pending(conn)?;
    let reminder = db::get_setting(conn, PROMPT)?.as_deref() != Some("off");
    let due = match today {
        Some(today) if reminder && !items.is_empty() => due(conn, today)?,
        _ => false,
    };
    Ok(Suggestions { items, due, reminder })
}

/// It's been at least [`ASK_EVERY_DAYS`] since the reminder last asked (or it never has).
fn due(conn: &Connection, today: NaiveDate) -> rusqlite::Result<bool> {
    let asked = db::get_setting(conn, ASKED)?.and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok());
    Ok(asked.is_none_or(|a| (today - a).num_days() >= ASK_EVERY_DAYS))
}

/// The reminder asked on `today` and the person answered, whatever the answer.
pub fn asked(conn: &Connection, today: NaiveDate) -> rusqlite::Result<()> {
    db::set_setting(conn, ASKED, Some(&today.format("%Y-%m-%d").to_string()))
}

/// Turn the monthly reminder on or off. The list in Settings stays either way.
pub fn set_reminder(conn: &Connection, on: bool) -> rusqlite::Result<()> {
    db::set_setting(conn, PROMPT, (!on).then_some("off"))
}

/// Remember the person's choice for `keys`: shared (offered again only if they
/// change it) or private (never offered again). Purely local bookkeeping:
/// "shared" means they opened the issue, not that GitHub received anything.
pub fn mark(conn: &Connection, keys: &[String], private: bool) -> rusqlite::Result<()> {
    let state = if private { "private" } else { "shared" };
    for (item, value) in all(conn)? {
        if keys.contains(&item.key) {
            conn.execute(
                "INSERT INTO ref_suggestions (key, content, state) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET content = excluded.content, state = excluded.state",
                params![item.key, value.to_string(), state],
            )?;
        }
    }
    Ok(())
}

/// Prefilled new-issue pages for the chosen pending items, as few as fit.
pub fn issues(conn: &Connection, keys: &[String]) -> rusqlite::Result<Vec<Issue>> {
    let chosen: Vec<(Item, Value)> = all(conn)?.into_iter().filter(|(i, _)| keys.contains(&i.key)).collect();
    let mut out: Vec<Issue> = Vec::new();
    let mut batch: Vec<&(Item, Value)> = Vec::new();
    for entry in &chosen {
        batch.push(entry);
        if batch.len() > 1 && issue(&batch).url.len() > MAX_URL {
            let last = batch.pop().unwrap();
            out.push(issue(&batch));
            batch = vec![last];
        }
    }
    if !batch.is_empty() {
        out.push(issue(&batch));
    }
    Ok(out)
}

fn issue(batch: &[&(Item, Value)]) -> Issue {
    let title = match batch {
        [(i, _)] if i.kind == "timings" => format!("[Reference] Timings for {}", i.name),
        [(i, _)] => format!("[Reference] Missing substance: {}", i.name),
        _ => format!("[Reference] {} suggestions", batch.len()),
    };
    let mut body = String::from(
        "Suggested from Field Notes: things this person added because the reference didn't have them. \
         One person's report, not a source. Review before adding anything to the reference.\n",
    );
    for (item, _) in batch {
        body.push_str(&format!("\n### {}", item.name));
        if item.kind == "timings" {
            body.push_str(if item.in_reference { " (timings)" } else { " (timings, not in the reference)" });
        } else {
            body.push_str(" (not in the reference)");
        }
        body.push('\n');
        for (label, value) in &item.fields {
            body.push_str(&format!("- **{label}:** {}\n", value.replace('\n', " ")));
        }
    }
    let data: Vec<&Value> = batch.iter().map(|(_, v)| v).collect();
    body.push_str(&format!(
        "\n<details><summary>For import</summary>\n\n```json\n{}\n```\n</details>\n\n---\n_Field Notes v{}_\n",
        serde_json::to_string_pretty(&data).expect("values serialise"),
        env!("CARGO_PKG_VERSION"),
    ));
    let url = format!(
        "{REPO}/issues/new?template={TEMPLATE}&title={}&body={}",
        encode(&title),
        encode(&body)
    );
    Issue { title, url, keys: batch.iter().map(|(i, _)| i.key.clone()).collect() }
}

/// Percent-encode for a URL query value, like JavaScript's `encodeURIComponent`.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(db::schema_for_tests()).unwrap();
        db::set_my_ref(
            &conn,
            "Nurtec",
            &MyTimings {
                route: "oral".into(),
                onset: Some("30–60 minutes".into()),
                come_up: None,
                peak: Some("2 hours".into()),
                offset: None,
                after_effects: None,
                total: Some("6–8 hours".into()),
            },
        )
        .unwrap();
        conn.execute(
            "INSERT INTO substances (name, aliases, category, classes, dose_note, notes, user_added)
             VALUES ('Ethylphenidate', '[\"EPH\"]', 'Phenidate', '[\"stimulant\"]', 'started around 20mg oral', '', 1)",
            [],
        )
        .unwrap();
        conn
    }

    fn keys(items: &[Item]) -> Vec<String> {
        items.iter().map(|i| i.key.clone()).collect()
    }

    fn day(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn decode(s: &str) -> String {
        let b = s.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'%' {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
                i += 3;
            } else {
                out.push(b[i]);
                i += 1;
            }
        }
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn your_timings_and_missing_substances_are_pending() {
        let conn = setup();
        let p = pending(&conn).unwrap();
        assert_eq!(keys(&p), ["timings:nurtec", "substance:ethylphenidate"]);
        assert_eq!(p[0].fields[0], ("Taken".into(), "oral".into()));
        assert!(p[0].fields.contains(&("Lasts in total".into(), "6–8 hours".into())));
    }

    #[test]
    fn a_substance_the_reference_covers_is_not_suggested() {
        let conn = setup();
        conn.execute("INSERT INTO pw_substances (name, data) VALUES ('ethylphenidate', '{}')", []).unwrap();
        assert_eq!(keys(&pending(&conn).unwrap()), ["timings:nurtec"]);
    }

    #[test]
    fn shared_is_offered_again_only_once_changed_and_private_never() {
        let conn = setup();
        mark(&conn, &["timings:nurtec".into()], false).unwrap();
        mark(&conn, &["substance:ethylphenidate".into()], true).unwrap();
        assert!(pending(&conn).unwrap().is_empty());

        // Edited after sharing: offered again, flagged as a change.
        let mut t = db::my_ref(&conn, "Nurtec").unwrap().unwrap();
        t.total = Some("8 hours".into());
        db::set_my_ref(&conn, "Nurtec", &t).unwrap();
        conn.execute("UPDATE substances SET notes = 'edited' WHERE name = 'Ethylphenidate'", []).unwrap();
        let p = pending(&conn).unwrap();
        assert_eq!(keys(&p), ["timings:nurtec"], "private stays private, even edited");
        assert!(p[0].changed);
    }

    #[test]
    fn the_reminder_asks_at_most_monthly_and_only_with_something_new() {
        let conn = setup();
        assert!(suggestions(&conn, Some(day("2026-10-16"))).unwrap().due, "never asked: ask");
        asked(&conn, day("2026-10-16")).unwrap();
        assert!(!suggestions(&conn, Some(day("2026-11-14"))).unwrap().due, "29 days: too soon");
        assert!(suggestions(&conn, Some(day("2026-11-15"))).unwrap().due, "30 days: ask");

        set_reminder(&conn, false).unwrap();
        let s = suggestions(&conn, Some(day("2027-06-01"))).unwrap();
        assert!(!s.due && !s.reminder);
        assert_eq!(s.items.len(), 2, "the list is still there in Settings");

        set_reminder(&conn, true).unwrap();
        mark(&conn, &keys(&s.items), false).unwrap();
        assert!(!suggestions(&conn, Some(day("2027-06-01"))).unwrap().due, "nothing pending: don't ask");
    }

    #[test]
    fn an_issue_holds_what_the_preview_showed_and_nothing_invented() {
        let conn = setup();
        let is = issues(&conn, &["timings:nurtec".into()]).unwrap();
        assert_eq!(is.len(), 1);
        assert_eq!(is[0].title, "[Reference] Timings for Nurtec");
        assert!(is[0].url.starts_with(&format!("{REPO}/issues/new?template={TEMPLATE}&title=")));
        let body = decode(&is[0].url);
        assert!(body.contains("**Lasts in total:** 6–8 hours"));
        assert!(body.contains("\"total\": \"6–8 hours\""), "the import block keeps the person's words");
        assert!(!body.contains("Ethylphenidate"), "only what was chosen");

        // A dose note rides along as the person's prose, never as a figure.
        let body = decode(&issues(&conn, &["substance:ethylphenidate".into()]).unwrap()[0].url);
        assert!(body.contains("\"dose_note\": \"started around 20mg oral\""));
        assert!(!body.contains("\"dose\":"));
    }

    #[test]
    fn issues_carry_no_journal_data() {
        // A suggestion says "this is how long it lasted", never "here is what I
        // took". If someone widens `all()` to join the journal, this fails.
        let conn = setup();
        conn.execute("INSERT INTO experiences (id, title, started_at) VALUES (1, 'Tuesday night', '2026-03-04T21:00:00')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO doses (experience_id, substance_name, amount, unit, route, taken_at)
             VALUES (1, 'Nurtec', 75, 'mg', 'oral', '2026-03-04T21:30:00')",
            [],
        )
        .unwrap();
        let all_keys = keys(&pending(&conn).unwrap());
        let created: String = conn.query_row("SELECT created_at FROM substances", [], |r| r.get(0)).unwrap();
        let updated: String = conn.query_row("SELECT updated_at FROM my_refs", [], |r| r.get(0)).unwrap();
        for i in issues(&conn, &all_keys).unwrap() {
            let body = decode(&i.url);
            for leak in ["Tuesday night", "2026-03-04", "21:30", "75", &created[..10], &updated[..10]] {
                assert!(!body.contains(leak), "journal data leaked into a suggestion: {leak}");
            }
        }
    }

    #[test]
    fn a_long_list_splits_across_issues_that_github_will_prefill() {
        let conn = setup();
        for n in 0..40 {
            conn.execute(
                "INSERT INTO substances (name, notes, user_added) VALUES (?1, ?2, 1)",
                params![format!("Compound {n}"), "A long note about this compound. ".repeat(4)],
            )
            .unwrap();
        }
        let all_keys = keys(&pending(&conn).unwrap());
        let is = issues(&conn, &all_keys).unwrap();
        assert!(is.len() > 1);
        assert!(is.iter().all(|i| i.url.len() <= MAX_URL));
        let covered: Vec<String> = is.iter().flat_map(|i| i.keys.clone()).collect();
        assert_eq!(covered, all_keys, "every chosen item, once, in order");
    }

    #[test]
    fn encodes_like_encode_uri_component() {
        assert_eq!(encode("a b&c=d/é"), "a%20b%26c%3Dd%2F%C3%A9");
        assert_eq!(encode("it's (fine)!"), "it's%20(fine)!");
    }
}
