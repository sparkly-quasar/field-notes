// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Dependence and withdrawal notes (ROADMAP "Patterns to watch, by group").
//! Wording and triggers reviewed by the owner on 2026-10-06 (the "wording for
//! clinical review" packet). Each note is a fact that matters if someone later
//! decides to cut back, usually that stopping suddenly is the dangerous part.
//! None marks use as a concern.
//!
//! A note appears once after logging, when its pattern first shows up
//! ([`take_new`]), then stays on the substance's Stats page while the pattern
//! holds ([`notes_for`]).
//!
//! | Group | Appears when |
//! |---|---|
//! | GHB, GBL, 1,4-B | doses 4 hours apart or less for more than a day, or doses on 21+ of the last 28 days |
//! | Alcohol | 5+ standard drinks on 4 of the last 7 days, 3 heavy days in a row, or drinks between 4 and 10am on 3 of the last 14 days |
//! | Benzodiazepines, Z-drugs | doses on 20+ of the last 28 days, or the daily amount up by half |
//! | Gabapentin, pregabalin, phenibut | doses on 20+ of the last 28 days |
//! | Kratom | most days (20+ of 28) with the daily amount up by a quarter, or extract or 7-OH most days |
//! | Anything marked as needed | doses on 20+ of the last 28 days |
//! | Benzodiazepines, Z-drugs | a week or more of nightly use, then a night or more without: the rebound note |

use crate::db;
use crate::kinds::{group, kind_of, Group, AS_NEEDED};
use crate::stats::parse_ts;
use chrono::{DateTime, Duration, Local, NaiveDate, Timelike, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PatternNote {
    /// Stable: "ghb_round_clock", "ghb_daily", "alcohol", "benzo", "gabapentinoid",
    /// "kratom", "kratom_strong", "as_needed", "rebound".
    pub key: String,
    pub text: String,
}

struct D {
    at: DateTime<Local>,
    amount: Option<f64>,
    unit: String,
    form: String,
}

/// The doses of `key` in the 56 days before `now`, oldest first.
fn doses(conn: &Connection, key: &str, now: DateTime<Utc>) -> rusqlite::Result<Vec<D>> {
    let names = db::name_index(conn)?;
    let since = now - Duration::days(56);
    let mut stmt = conn.prepare(
        "SELECT d.substance_name, s.name, d.amount, d.unit, d.taken_at, d.form
         FROM doses d JOIN experiences e ON e.id = d.experience_id
         LEFT JOIN substances s ON s.id = d.substance_id
         WHERE e.kind = 'session'",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<f64>>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, String>(5)?,
        ))
    })?;
    let mut out = Vec::new();
    for x in rows {
        let (logged, catalogue, amount, unit, taken, form) = x?;
        let name = catalogue.unwrap_or_else(|| names.canonical(&logged).unwrap_or(logged));
        if name.trim().to_lowercase() != key {
            continue;
        }
        let Some(at) = parse_ts(&taken) else { continue };
        if at < since || at > now {
            continue;
        }
        out.push(D { at: at.with_timezone(&Local), amount, unit, form: form.trim().to_lowercase() });
    }
    out.sort_by_key(|d| d.at);
    Ok(out)
}

/// Distinct local days with a dose in the `days` days up to `now`.
fn days_in(ds: &[D], now: DateTime<Local>, from_days: i64, to_days: i64) -> BTreeSet<NaiveDate> {
    let from = now - Duration::days(from_days);
    let to = now - Duration::days(to_days);
    ds.iter().filter(|d| d.at > from && d.at <= to).map(|d| d.at.date_naive()).collect()
}

/// Total amount per local day, in grams for units of mass, else as logged, for
/// the doses in the window.
fn daily_totals(ds: &[D], now: DateTime<Local>, from_days: i64, to_days: i64) -> Vec<f64> {
    let from = now - Duration::days(from_days);
    let to = now - Duration::days(to_days);
    let mut by: BTreeMap<NaiveDate, f64> = BTreeMap::new();
    for d in ds.iter().filter(|d| d.at > from && d.at <= to) {
        let Some(a) = d.amount else { continue };
        let g = crate::stats::in_unit(a, &d.unit, "g").unwrap_or(a);
        *by.entry(d.at.date_naive()).or_default() += g;
    }
    by.into_values().collect()
}

fn median(mut xs: Vec<f64>) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    xs.sort_by(|a, b| a.total_cmp(b));
    let m = xs.len() / 2;
    Some(if xs.len() % 2 == 1 { xs[m] } else { (xs[m - 1] + xs[m]) / 2.0 })
}

/// The recent daily amount against the one before it: `by` or more is rising.
/// Needs 3 days in each.
fn rising(ds: &[D], now: DateTime<Local>, recent: i64, before: i64, by: f64) -> bool {
    let a = daily_totals(ds, now, recent, 0);
    let b = daily_totals(ds, now, before, recent);
    if a.len() < 3 || b.len() < 3 {
        return false;
    }
    matches!((median(a), median(b)), (Some(x), Some(y)) if y > 0.0 && x >= y * by)
}

/// Doses no more than 4 hours apart, running for more than a day, in the last
/// two weeks: "around the clock".
fn round_the_clock(ds: &[D], now: DateTime<Local>) -> bool {
    let recent: Vec<&D> = ds.iter().filter(|d| d.at > now - Duration::days(14)).collect();
    let mut start = 0;
    for i in 1..recent.len() {
        if recent[i].at - recent[i - 1].at > Duration::hours(4) {
            start = i;
        } else if recent[i].at - recent[start].at > Duration::hours(24) {
            return true;
        }
    }
    false
}

/// The reference half-life's low end, in hours.
fn half_life_low(conn: &Connection, key: &str) -> Option<f64> {
    let info = db::pw_lookup(conn, key).ok().flatten()?;
    info.roas.iter().filter_map(|r| r.half_life.as_deref().and_then(crate::stats::half_life_hours)).map(|h| h.0).next()
}

fn note(key: &str, text: String) -> PatternNote {
    PatternNote { key: key.into(), text }
}

/// Every note whose pattern holds for `substance` now.
pub fn notes_for(conn: &Connection, substance: &str, now: DateTime<Utc>) -> rusqlite::Result<Vec<PatternNote>> {
    let key = db::substance_key(conn, substance)?;
    let name = db::name_index(conn)?.canonical(substance.trim()).unwrap_or_else(|| substance.trim().to_string());
    let ds = doses(conn, &key, now)?;
    if ds.is_empty() {
        return Ok(vec![]);
    }
    let local = now.with_timezone(&Local);
    let last28 = days_in(&ds, local, 28, 0).len();
    let mut out = Vec::new();
    let g = group(conn, &key);

    match g {
        Group::Ghb => {
            if round_the_clock(&ds, local) {
                out.push(note("ghb_round_clock", format!(
                    "You've been taking {name} every few hours, around the clock. Dosing like that can build physical dependence within weeks, and then stopping suddenly can be dangerous. If you ever want to cut back, tapering with medical support is the safer route."
                )));
            } else if last28 >= 21 {
                out.push(note("ghb_daily", format!(
                    "You've taken {name} on most days for the last few weeks. Daily use can build physical dependence, and then stopping suddenly can be dangerous. If you ever want to cut back, tapering with medical support is the safer route."
                )));
            }
        }
        Group::Alcohol => {
            // Standard drinks per local day; amounts in other units can't be read as drinks.
            let mut per_day: BTreeMap<NaiveDate, f64> = BTreeMap::new();
            for d in ds.iter().filter(|d| d.at > local - Duration::days(7)) {
                if matches!(d.unit.trim().to_lowercase().as_str(), "drink" | "drinks") {
                    *per_day.entry(d.at.date_naive()).or_default() += d.amount.unwrap_or(0.0);
                }
            }
            let heavy: Vec<NaiveDate> = per_day.iter().filter(|(_, n)| **n >= 5.0).map(|(d, _)| *d).collect();
            let in_a_row = heavy.windows(3).any(|w| (w[2] - w[0]).num_days() == 2);
            let mornings: BTreeSet<NaiveDate> = ds
                .iter()
                .filter(|d| d.at > local - Duration::days(14) && (4..10).contains(&d.at.hour()))
                .map(|d| d.at.date_naive())
                .collect();
            let lead = if heavy.len() >= 4 {
                Some("You've had 5 or more drinks on most days lately.")
            } else if in_a_row {
                Some("You've had 5 or more drinks on several days in a row lately.")
            } else if mornings.len() >= 3 {
                Some("You've had drinks soon after waking on several mornings lately.")
            } else {
                None
            };
            if let Some(lead) = lead {
                out.push(note("alcohol", format!(
                    "{lead} After regular heavy drinking, withdrawal can start 6 to 24 hours after the last drink and can include seizures, so a gap between drinking doesn't protect you. If you decide to cut back, tapering or medical support is safer than stopping suddenly."
                )));
            }
        }
        Group::Benzodiazepine | Group::ZDrug => {
            let lead = if last28 >= 20 {
                Some(format!("You've taken {name} on most days for about a month."))
            } else if rising(&ds, local, 14, 42, 1.5) {
                Some(format!("Your daily amount of {name} has gone up lately."))
            } else {
                None
            };
            if let Some(lead) = lead {
                let by_half_life = match half_life_low(conn, &key) {
                    Some(h) if h >= 20.0 => " It builds up over days, so stopping is gentler but slower.",
                    Some(h) if h <= 12.0 => " It leaves the body quickly, so rebound after stopping can be sharper.",
                    _ => "",
                };
                out.push(note("benzo", format!(
                    "{lead} Taken daily, the body can come to rely on it. After longer use, a gradual taper is safer than stopping suddenly.{by_half_life}"
                )));
            }
        }
        Group::Gabapentinoid | Group::Phenibut if last28 >= 20 => {
            out.push(note("gabapentinoid", format!(
                "You've taken {name} on most days for weeks. Daily use can cause dependence, and stopping suddenly can bring on withdrawal, including seizures. Tapering is safer."
            )));
        }
        Group::Kratom if last28 >= 20 => {
            let strong = ds.iter().filter(|d| d.at > local - Duration::days(28)).find(|d| d.form == "extract" || d.form == "7-oh");
            if let Some(d) = strong {
                let what = if d.form == "7-oh" { "7-OH products" } else { "kratom extract" };
                out.push(note("kratom_strong", format!(
                    "You've taken {what} on most days lately. Daily use can cause dependence, and withdrawal from extracts, 7-OH or high daily amounts can be more serious than from leaf, and can be dangerous. Tapering with support is safer than stopping suddenly."
                )));
            } else if rising(&ds, local, 14, 28, 1.25) {
                out.push(note("kratom", "You've taken kratom most days lately, and your daily amount has gone up. Daily kratom can cause dependence. Withdrawal from leaf is mostly like a mild opioid withdrawal (aches, restlessness, low mood) and rarely dangerous, and tapering makes it easier.".into()));
            }
        }
        _ => {}
    }

    if kind_of(conn, &key)? == AS_NEEDED && last28 >= 20 && !out.iter().any(|n| n.key == "benzo" || n.key == "gabapentinoid") {
        out.push(note("as_needed", format!(
            "You've taken {name} on {last28} of the last 28 days. Taken most days for weeks, the body can come to rely on it. If you change how you take it, a gradual taper is safer than stopping."
        )));
    }

    // Rebound: a week or more of nightly use in the two weeks before the last
    // dose, and at least a night since it, within the last two weeks.
    if matches!(g, Group::Benzodiazepine | Group::ZDrug) {
        if let Some(last) = ds.last() {
            let since_last = local - last.at;
            let nights = ds.iter().filter(|d| d.at > last.at - Duration::days(14)).map(|d| d.at.date_naive()).collect::<BTreeSet<_>>().len();
            if since_last >= Duration::hours(24) && since_last <= Duration::days(14) && nights >= 7 {
                out.push(note("rebound", "After a week or two of a sleeping pill, a few rough nights once you stop are common. That's rebound, and it usually passes within a few days, or a week or two after longer use; natural sleep does come back. It isn't a sign you need it. If you want help with sleep while it settles, CBT-I (a structured approach you can do as self-help) has the best evidence. Melatonin helps some people.".into()));
            }
        }
    }
    Ok(out)
}

/// The first note for `substance` not shown before, marked shown: each note
/// appears once after logging. It stays on the Stats page while it holds.
pub fn take_new(conn: &Connection, substance: &str, now: DateTime<Utc>) -> rusqlite::Result<Option<PatternNote>> {
    let key = db::substance_key(conn, substance)?;
    for n in notes_for(conn, substance, now)? {
        let seen: Option<String> = conn
            .query_row("SELECT at FROM pattern_notes_seen WHERE substance = ?1 AND key = ?2", params![key, n.key], |r| r.get(0))
            .optional()?;
        if seen.is_none() {
            conn.execute(
                "INSERT INTO pattern_notes_seen (substance, key, at) VALUES (?1, ?2, ?3)",
                params![key, n.key, now.to_rfc3339()],
            )?;
            return Ok(Some(n));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn journal() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(db::schema_for_tests()).unwrap();
        c
    }

    fn now() -> DateTime<Utc> {
        Local.with_ymd_and_hms(2026, 9, 30, 12, 0, 0).unwrap().with_timezone(&Utc)
    }

    /// A dose `days` days and `hours` hours before now, local time.
    fn take(c: &Connection, name: &str, amount: f64, unit: &str, days: i64, hours: i64, form: &str) {
        let at = (now().with_timezone(&Local) - Duration::days(days) - Duration::hours(hours)).format("%Y-%m-%d %H:%M:%S").to_string();
        c.execute("INSERT INTO experiences (kind, title, started_at) VALUES ('session', '', ?1)", [&at]).unwrap();
        let id = c.last_insert_rowid();
        c.execute(
            "INSERT INTO doses (experience_id, substance_name, amount, unit, taken_at, form) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, name, amount, unit, at, form],
        )
        .unwrap();
    }

    fn keys(c: &Connection, name: &str) -> Vec<String> {
        notes_for(c, name, now()).unwrap().into_iter().map(|n| n.key).collect()
    }

    #[test]
    fn ghb_around_the_clock_or_daily_but_not_once_a_day_briefly() {
        let c = journal();
        // Once a night for a week: nothing.
        for d in 1..=7 {
            take(&c, "GHB", 2.0, "ml", d, 0, "");
        }
        assert!(keys(&c, "GHB").is_empty());
        // Every 3 hours for 30 hours.
        for h in (0..=30).step_by(3) {
            take(&c, "GHB", 2.0, "ml", 9, h, "");
        }
        assert_eq!(keys(&c, "GHB"), ["ghb_round_clock"]);
        let c = journal();
        for d in 1..=22 {
            take(&c, "GBL", 1.0, "ml", d, 0, "");
        }
        assert_eq!(keys(&c, "GBL"), ["ghb_daily"]);
    }

    #[test]
    fn alcohol_heavy_days_in_a_row_or_mornings() {
        let c = journal();
        for d in 1..=3 {
            take(&c, "Alcohol", 6.0, "drinks", d, 0, "");
        }
        let n = notes_for(&c, "Alcohol", now()).unwrap();
        assert_eq!(n.len(), 1);
        assert!(n[0].text.starts_with("You've had 5 or more drinks on several days in a row lately."));
        // Two drinks most evenings: nothing.
        let c = journal();
        for d in 1..=7 {
            take(&c, "Alcohol", 2.0, "drinks", d, 0, "");
        }
        assert!(keys(&c, "Alcohol").is_empty());
        // Drinks at 8am on three mornings.
        let c = journal();
        for d in 1..=3 {
            take(&c, "Alcohol", 1.0, "drink", d, 4, ""); // noon minus 4h = 8am
        }
        assert!(notes_for(&c, "Alcohol", now()).unwrap()[0].text.contains("soon after waking"));
    }

    #[test]
    fn benzos_after_about_a_month_and_the_rebound_note_after_stopping() {
        let c = journal();
        for d in 1..=21 {
            take(&c, "Lorazepam", 1.0, "mg", d, 0, "");
        }
        assert_eq!(keys(&c, "Lorazepam"), ["benzo", "rebound"]);
        // Ten nights of zolpidem, stopped three days ago.
        let c = journal();
        for d in 3..=12 {
            take(&c, "Zolpidem", 10.0, "mg", d, 0, "");
        }
        assert_eq!(keys(&c, "Zolpidem"), ["rebound"]);
        // Still taking it: no rebound yet.
        take(&c, "Zolpidem", 10.0, "mg", 0, 1, "");
        assert!(keys(&c, "Zolpidem").is_empty());
    }

    #[test]
    fn kratom_rising_leaf_or_strong_forms() {
        let c = journal();
        for d in 1..=24 {
            take(&c, "Kratom", if d <= 13 { 6.0 } else { 3.0 }, "g", d, 0, "leaf");
        }
        let n = notes_for(&c, "Kratom", now()).unwrap();
        assert_eq!(n[0].key, "kratom");
        assert!(n[0].text.contains("rarely dangerous"));
        let c = journal();
        for d in 1..=22 {
            take(&c, "Kratom", 15.0, "mg", d, 0, "7-oh");
        }
        let n = notes_for(&c, "Kratom", now()).unwrap();
        assert_eq!(n[0].key, "kratom_strong");
        assert!(n[0].text.contains("can be dangerous"));
    }

    #[test]
    fn as_needed_drift_and_each_note_once() {
        let c = journal();
        for d in 1..=22 {
            take(&c, "Kratom", 2.0, "g", d, 0, "leaf");
        }
        crate::kinds::set_kind(&c, "Kratom", "as_needed").unwrap();
        let n = notes_for(&c, "Kratom", now()).unwrap();
        assert_eq!(n.iter().map(|n| n.key.as_str()).collect::<Vec<_>>(), ["as_needed"]);
        assert!(n[0].text.contains("on 22 of the last 28 days"));
        assert_eq!(take_new(&c, "Kratom", now()).unwrap().map(|n| n.key).as_deref(), Some("as_needed"));
        assert_eq!(take_new(&c, "Kratom", now()).unwrap(), None, "shown once");
        assert_eq!(keys(&c, "Kratom"), ["as_needed"], "still on the Stats page");
    }

    #[test]
    fn cannabis_and_stimulants_get_no_note() {
        let c = journal();
        for d in 1..=27 {
            take(&c, "Cannabis", 3.0, "hit", d, 0, "");
            take(&c, "Amphetamine", 20.0, "mg", d, 0, "");
        }
        assert!(keys(&c, "Cannabis").is_empty());
        assert!(keys(&c, "Amphetamine").is_empty());
    }
}
