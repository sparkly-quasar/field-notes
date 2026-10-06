// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! How a substance is taken (dose-aware Stats, step 4 in ROADMAP.md): as
//! **experiences** (the default), as part of a **routine** (a daily
//! prescription, the morning coffee), or **as needed** (lorazepam for panic,
//! zolpidem on bad nights, kratom for pain).
//!
//! The one rule: **the kind changes where doses are shown, never whether they're
//! tracked.** Routine and as-needed doses leave the experience views in Stats
//! (how often, the calendar, time between), but the dose chart, time of day,
//! amount trends and the combination checker always count them. A habit filed
//! as routine by mistake must still be visible.
//!
//! The app asks rather than making people tag things, and the asking is
//! careful (owner's decisions, 2026-10-05):
//!
//! - **Routine** is only ever *suggested* from a pattern: the same substance on
//!   5 or more of the last 10 days, at about the same time and amount, logged as
//!   plain doses rather than experiences. Never suggested for alcohol, the GHB
//!   group, benzodiazepines, Z-drugs, gabapentinoids, phenibut, opioids or
//!   kratom, where a daily pattern is the thing worth seeing. Anyone can still
//!   mark any of them routine themselves (prescribed clonazepam, methadone).
//! - **As needed** is asked once, on the second dose of a benzodiazepine, Z-drug,
//!   gabapentin, pregabalin or kratom. Opioids are only ever marked by hand.
//! - A **No** sticks: asked again only if the pattern clearly changes (amount by
//!   more than a quarter, or time of day by more than three hours), and not
//!   within a month. **Not now** waits a week.

use crate::db;
use crate::stats::parse_ts;
use chrono::{DateTime, Duration, Local, Timelike, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::BTreeSet;

pub const ROUTINE: &str = "routine";
pub const AS_NEEDED: &str = "as_needed";

/// The groups that change what's asked. Matched by name where the reference
/// can't be trusted (it calls GHB a gabapentinoid), by chemical class for the
/// benzodiazepines, by family for opioids.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Alcohol,
    Ghb,
    Benzodiazepine,
    ZDrug,
    Gabapentinoid,
    Phenibut,
    Kratom,
    Opioid,
    Other,
}

impl Group {
    /// A daily pattern may be suggested as routine.
    fn suggests_routine(self) -> bool {
        self == Group::Other
    }
    /// Asked once whether it's taken as needed.
    fn asks_as_needed(self) -> bool {
        matches!(self, Group::Benzodiazepine | Group::ZDrug | Group::Gabapentinoid | Group::Kratom)
    }
}

/// Drinks by name, as the built-in matcher and the drink unit know them.
const DRINKS: &[&str] = &[
    "alcohol", "ethanol", "booze", "beer", "wine", "cider", "vodka", "whiskey", "whisky", "rum", "gin", "tequila",
    "mezcal", "liquor", "spirits", "sake", "champagne", "glass of wine", "shot",
];

pub fn group(conn: &Connection, key: &str) -> Group {
    let k = key.trim().to_lowercase();
    if k == "kratom" {
        return Group::Kratom;
    }
    if DRINKS.contains(&k.as_str()) {
        return Group::Alcohol;
    }
    if ["ghb", "gbl", "1,4-butanediol", "1,4-b", "1,4-bd", "14b", "bdo"].contains(&k.as_str()) {
        return Group::Ghb;
    }
    if k == "phenibut" {
        return Group::Phenibut;
    }
    if ["gabapentin", "pregabalin"].contains(&k.as_str()) {
        return Group::Gabapentinoid;
    }
    if ["zolpidem", "zopiclone", "eszopiclone", "zaleplon"].contains(&k.as_str()) {
        return Group::ZDrug;
    }
    let chemical = db::pw_lookup(conn, &k).ok().flatten().map(|i| i.chemical).unwrap_or_default();
    let benzo = chemical.iter().any(|c| {
        let c = c.to_lowercase();
        (c.contains("benzodiazepine") && !c.contains("nonbenzodiazepine")) || c.contains("thienodiazepine")
    }) || crate::interactions::builtin_classes(&k).iter().any(|c| c == "benzodiazepine");
    if benzo {
        return Group::Benzodiazepine;
    }
    if crate::stats::families_of(conn, &k).iter().any(|f| f == "opioids") {
        return Group::Opioid;
    }
    Group::Other
}

#[derive(Debug, Default)]
struct Row {
    kind: String,
    as_needed_asked: bool,
    routine_no_at: Option<String>,
    routine_no_amount: Option<f64>,
    routine_no_minute: Option<i64>,
    snooze_until: Option<String>,
}

fn row(conn: &Connection, key: &str) -> rusqlite::Result<Row> {
    Ok(conn
        .query_row(
            "SELECT kind, as_needed_asked, routine_no_at, routine_no_amount, routine_no_minute, snooze_until
             FROM substance_kinds WHERE substance = ?1",
            [key],
            |r| {
                Ok(Row {
                    kind: r.get(0)?,
                    as_needed_asked: r.get::<_, i64>(1)? != 0,
                    routine_no_at: r.get(2)?,
                    routine_no_amount: r.get(3)?,
                    routine_no_minute: r.get(4)?,
                    snooze_until: r.get(5)?,
                })
            },
        )
        .optional()?
        .unwrap_or_default())
}

fn ensure(conn: &Connection, key: &str) -> rusqlite::Result<()> {
    conn.execute("INSERT OR IGNORE INTO substance_kinds (substance) VALUES (?1)", [key])?;
    Ok(())
}

/// How a substance is taken: "", "routine" or "as_needed". `key` is already the
/// substance's key (as Stats groups it).
pub fn kind_of(conn: &Connection, key: &str) -> rusqlite::Result<String> {
    Ok(row(conn, key)?.kind)
}

/// Mark a substance by hand, from its Stats page: any kind, any substance.
pub fn set_kind(conn: &Connection, substance: &str, kind: &str) -> rusqlite::Result<()> {
    let key = db::substance_key(conn, substance)?;
    ensure(conn, &key)?;
    conn.execute("UPDATE substance_kinds SET kind = ?2 WHERE substance = ?1", params![key, kind])?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Choice {
    pub value: String,
    pub label: String,
}

/// A question to ask about one substance, right after a dose of it is logged.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct KindQuestion {
    /// The name as typed when logging, so the answer goes to the same substance.
    pub substance: String,
    /// "routine" or "as_needed": which question this is.
    pub ask: String,
    pub text: String,
    pub choices: Vec<Choice>,
}

fn choice(value: &str, label: &str) -> Choice {
    Choice { value: value.into(), label: label.into() }
}

/// A routine-looking pattern: the typical amount and time of day of the plain
/// doses in the last 10 days.
#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    pub days: usize,
    pub amount: Option<f64>,
    /// Minutes after local midnight.
    pub minute: i64,
}

/// Minutes between two times of day, the short way round the clock.
fn clock_gap(a: i64, b: i64) -> i64 {
    let d = (a - b).rem_euclid(1440);
    d.min(1440 - d)
}

fn median(mut xs: Vec<f64>) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    xs.sort_by(|a, b| a.total_cmp(b));
    let m = xs.len() / 2;
    Some(if xs.len() % 2 == 1 { xs[m] } else { (xs[m - 1] + xs[m]) / 2.0 })
}

/// Is `key` taken like a routine lately? Doses on 5+ of the last 10 days, at
/// least 4 in 5 of them within about 3 hours of one time of day and (when
/// amounts are written) within a quarter of one amount, logged as plain doses:
/// an entry with an intention or a rating is an experience, and then it isn't a
/// routine. One late or doubled dose doesn't hide a routine; a scattered habit
/// isn't one.
pub fn routine_pattern(conn: &Connection, key: &str, now: DateTime<Utc>) -> rusqlite::Result<Option<Pattern>> {
    let names = db::name_index(conn)?;
    let since = now - Duration::days(10);
    let mut stmt = conn.prepare(
        "SELECT d.substance_name, s.name, d.amount, d.unit, d.taken_at, e.intention, e.rating
         FROM doses d JOIN experiences e ON e.id = d.experience_id
         LEFT JOIN substances s ON s.id = d.substance_id
         WHERE e.kind = 'session'",
    )?;
    let mut doses: Vec<(Option<f64>, String, DateTime<Local>)> = Vec::new();
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<f64>>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, Option<i64>>(6)?,
        ))
    })?;
    for x in rows {
        let (logged, catalogue, amount, unit, taken, intention, rating) = x?;
        let name = catalogue.unwrap_or_else(|| names.canonical(&logged).unwrap_or(logged));
        if name.trim().to_lowercase() != key {
            continue;
        }
        let Some(at) = parse_ts(&taken) else { continue };
        if at < since || at > now {
            continue;
        }
        if !intention.trim().is_empty() || rating.is_some() {
            return Ok(None);
        }
        doses.push((amount, unit, at.with_timezone(&Local)));
    }
    let most = |n: usize, of: usize| n * 5 >= of * 4;
    let days: BTreeSet<_> = doses.iter().map(|d| d.2.date_naive()).collect();
    if days.len() < 5 {
        return Ok(None);
    }
    // Time of day: the time most doses are within 3 hours of.
    let minutes: Vec<i64> = doses.iter().map(|d| (d.2.hour() * 60 + d.2.minute()) as i64).collect();
    let near = |a: i64| minutes.iter().filter(|m| clock_gap(**m, a) <= 180).count();
    let Some(anchor) = minutes.iter().copied().max_by_key(|a| (near(*a), std::cmp::Reverse(*a))) else {
        return Ok(None);
    };
    if !most(near(anchor), minutes.len()) {
        return Ok(None);
    }
    let usual: Vec<usize> = (0..doses.len()).filter(|i| clock_gap(minutes[*i], anchor) <= 180).collect();
    if usual.iter().map(|i| doses[*i].2.date_naive()).collect::<BTreeSet<_>>().len() < 5 {
        return Ok(None);
    }
    let offsets: Vec<f64> = usual.iter().map(|i| ((minutes[*i] - anchor + 720).rem_euclid(1440) - 720) as f64).collect();
    let minute = (anchor + median(offsets).unwrap_or(0.0).round() as i64).rem_euclid(1440);
    // Amount: only when most doses say one; then most within a quarter of it.
    let with: Vec<f64> = doses.iter().filter_map(|d| d.0).collect();
    let amount = if with.len() * 2 >= doses.len() {
        let mid = median(with.clone()).unwrap_or(0.0);
        if mid > 0.0 && !most(with.iter().filter(|a| (*a - mid).abs() <= mid * 0.25).count(), with.len()) {
            return Ok(None);
        }
        Some(mid)
    } else {
        None
    };
    Ok(Some(Pattern { days: days.len(), amount, minute }))
}

/// "8am", "8:30pm": a time of day said roughly, to the half hour.
fn say_time(minute: i64) -> String {
    let m = ((minute + 15) / 30 * 30).rem_euclid(1440);
    let (h, mm) = (m / 60, m % 60);
    let h12 = if h % 12 == 0 { 12 } else { h % 12 };
    let ap = if h < 12 { "am" } else { "pm" };
    if mm == 0 { format!("{h12}{ap}") } else { format!("{h12}:{mm:02}{ap}") }
}

fn snoozed(r: &Row, now: DateTime<Utc>) -> bool {
    r.snooze_until.as_deref().and_then(parse_ts).is_some_and(|t| t > now)
}

/// The question to ask about `substance` now that a dose of it was logged, if any.
pub fn question(conn: &Connection, substance: &str, now: DateTime<Utc>) -> rusqlite::Result<Option<KindQuestion>> {
    let typed = substance.trim();
    if typed.is_empty() {
        return Ok(None);
    }
    let key = db::substance_key(conn, typed)?;
    let r = row(conn, &key)?;
    if !r.kind.is_empty() || snoozed(&r, now) {
        return Ok(None);
    }
    let name = db::name_index(conn)?.canonical(typed).unwrap_or_else(|| typed.to_string());
    let g = group(conn, &key);
    if g.asks_as_needed() {
        if r.as_needed_asked {
            return Ok(None);
        }
        let doses: i64 = conn.query_row(
            "SELECT COUNT(*) FROM doses d JOIN experiences e ON e.id = d.experience_id
             WHERE e.kind = 'session' AND lower(trim(d.substance_name)) IN (?1, ?2)",
            params![key, typed.to_lowercase()],
            |x| x.get(0),
        )?;
        if doses < 2 {
            return Ok(None);
        }
        return Ok(Some(KindQuestion {
            substance: typed.to_string(),
            ask: AS_NEEDED.into(),
            text: format!("How do you take {name}?"),
            choices: vec![
                choice("as_needed", "As needed"),
                choice("routine", "Regularly"),
                choice("neither", "Neither"),
            ],
        }));
    }
    if !g.suggests_routine() {
        return Ok(None);
    }
    let Some(p) = routine_pattern(conn, &key, now)? else { return Ok(None) };
    if let Some(no) = r.routine_no_at.as_deref().and_then(parse_ts) {
        let changed = r.routine_no_amount.zip(p.amount).is_some_and(|(was, is)| (is - was).abs() > was * 0.25)
            || r.routine_no_minute.is_some_and(|was| clock_gap(was, p.minute) > 180);
        if now - no < Duration::days(30) || !changed {
            return Ok(None);
        }
    }
    Ok(Some(KindQuestion {
        substance: typed.to_string(),
        ask: ROUTINE.into(),
        text: format!(
            "You take {name} around {} most days. Should Field Notes treat it as part of your routine? \
             Routine doses stay in your journal and in combination checks, but don't count toward experience stats.",
            say_time(p.minute)
        ),
        choices: vec![choice("yes", "Yes, it's routine"), choice("no", "No"), choice("later", "Not now")],
    }))
}

/// Record an answer to a [`question`].
pub fn answer(conn: &Connection, substance: &str, ask: &str, answer: &str, now: DateTime<Utc>) -> Result<(), String> {
    let e = |x: rusqlite::Error| x.to_string();
    let key = db::substance_key(conn, substance).map_err(e)?;
    ensure(conn, &key).map_err(e)?;
    let set = |sql: &str| conn.execute(sql, [&key]).map(|_| ()).map_err(e);
    match (ask, answer) {
        (ROUTINE, "yes") => set("UPDATE substance_kinds SET kind = 'routine' WHERE substance = ?1"),
        (ROUTINE, "no") => {
            let p = routine_pattern(conn, &key, now).map_err(e)?;
            conn.execute(
                "UPDATE substance_kinds SET routine_no_at = ?2, routine_no_amount = ?3, routine_no_minute = ?4
                 WHERE substance = ?1",
                params![key, now.to_rfc3339(), p.as_ref().and_then(|p| p.amount), p.as_ref().map(|p| p.minute)],
            )
            .map(|_| ())
            .map_err(e)
        }
        (ROUTINE, "later") => conn
            .execute(
                "UPDATE substance_kinds SET snooze_until = ?2 WHERE substance = ?1",
                params![key, (now + Duration::days(7)).to_rfc3339()],
            )
            .map(|_| ())
            .map_err(e),
        (AS_NEEDED, "as_needed" | "routine" | "neither") => {
            let kind = match answer {
                "as_needed" => AS_NEEDED,
                "routine" => ROUTINE,
                _ => "",
            };
            conn.execute(
                "UPDATE substance_kinds SET kind = ?2, as_needed_asked = 1 WHERE substance = ?1",
                params![key, kind],
            )
            .map(|_| ())
            .map_err(e)
        }
        _ => Err(format!("Not an answer to that question: {ask} / {answer}")),
    }
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

    /// Local wall-clock time, so the tests mean the same in any time zone.
    fn local(day: u32, h: u32, m: u32) -> String {
        format!("2026-09-{day:02} {h:02}:{m:02}:00")
    }

    fn now() -> DateTime<Utc> {
        Local.with_ymd_and_hms(2026, 9, 20, 12, 0, 0).unwrap().with_timezone(&Utc)
    }

    /// A plain dose: its own quick-logged entry, no intention, no rating.
    fn plain(c: &Connection, name: &str, amount: Option<f64>, at: &str) {
        c.execute("INSERT INTO experiences (kind, title, started_at) VALUES ('session', '', ?1)", [at]).unwrap();
        let id = c.last_insert_rowid();
        c.execute(
            "INSERT INTO doses (experience_id, substance_name, amount, unit, taken_at) VALUES (?1, ?2, ?3, 'mg', ?4)",
            params![id, name, amount, at],
        )
        .unwrap();
    }

    fn mornings(c: &Connection, name: &str, days: std::ops::RangeInclusive<u32>, amount: f64) {
        for (i, d) in days.enumerate() {
            // A little wobble in time and amount, as real mornings have.
            plain(c, name, Some(amount + (i % 2) as f64), &local(d, 8, (i as u32 * 20) % 60));
        }
    }

    #[test]
    fn a_steady_morning_dose_is_offered_as_routine() {
        let c = journal();
        mornings(&c, "Adderall", 12..=19, 20.0);
        let q = question(&c, "Adderall", now()).unwrap().expect("asked");
        assert_eq!(q.ask, "routine");
        assert!(q.text.starts_with("You take Adderall around 8:30am most days."), "{}", q.text);
        assert_eq!(q.choices.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(), ["yes", "no", "later"]);
    }

    #[test]
    fn one_late_dose_doesnt_hide_a_routine() {
        let c = journal();
        mornings(&c, "Adderall", 12..=18, 20.0);
        plain(&c, "Adderall", Some(20.0), &local(19, 21, 50));
        let q = question(&c, "Adderall", now()).unwrap().expect("still a routine");
        assert!(q.text.contains("around 8:30am"), "{}", q.text);
    }

    #[test]
    fn not_routine_when_scattered_or_too_few_or_an_experience() {
        let c = journal();
        // Four days isn't most days.
        mornings(&c, "Modafinil", 16..=19, 100.0);
        assert_eq!(question(&c, "Modafinil", now()).unwrap(), None);
        // Mornings and evenings.
        for (i, d) in (12..=19).enumerate() {
            plain(&c, "Caffeine", Some(100.0), &local(d, if i % 2 == 0 { 8 } else { 20 }, 0));
        }
        assert_eq!(question(&c, "Caffeine", now()).unwrap(), None);
        // The amount doubles on alternate days.
        for (i, d) in (12..=19).enumerate() {
            plain(&c, "Nicotine", Some(if i % 2 == 0 { 8.0 } else { 4.0 }), &local(d, 9, 0));
        }
        assert_eq!(question(&c, "Nicotine", now()).unwrap(), None);
        // One of them had an intention: these are experiences.
        mornings(&c, "Cannabis", 12..=19, 10.0);
        c.execute("UPDATE experiences SET intention = 'unwind' WHERE id = (SELECT MAX(experience_id) FROM doses WHERE substance_name = 'Cannabis')", []).unwrap();
        assert_eq!(question(&c, "Cannabis", now()).unwrap(), None);
    }

    #[test]
    fn a_daily_drink_or_depressant_is_never_offered_as_routine() {
        let c = journal();
        for name in ["Alcohol", "Beer", "GHB", "1,4-butanediol", "Phenibut", "Oxycodone", "Heroin"] {
            mornings(&c, name, 12..=19, 2.0);
            assert_eq!(question(&c, name, now()).unwrap(), None, "{name}");
        }
    }

    #[test]
    fn a_no_sticks_until_the_pattern_changes_and_a_month_has_passed() {
        let c = journal();
        mornings(&c, "Adderall", 12..=19, 20.0);
        answer(&c, "Adderall", "routine", "no", now()).unwrap();
        assert_eq!(question(&c, "Adderall", now()).unwrap(), None);
        // A month on, the same pattern: still no.
        let later = now() + Duration::days(31);
        for d in 12..=19 {
            plain(&c, "Adderall", Some(20.0), &format!("2026-10-{d:02} 08:10:00"));
        }
        assert_eq!(question(&c, "Adderall", later).unwrap(), None);
        // The dose has doubled: worth asking again.
        for d in 12..=19 {
            plain(&c, "Adderall", Some(40.0), &format!("2026-10-{d:02} 08:20:00"));
        }
        c.execute("DELETE FROM doses WHERE amount = 20 AND taken_at LIKE '2026-10-%'", []).unwrap();
        assert!(question(&c, "Adderall", later).unwrap().is_some());
    }

    #[test]
    fn not_now_waits_a_week_and_yes_is_final() {
        let c = journal();
        // Taken every morning, through the week of waiting too.
        mornings(&c, "Sertraline", 12..=27, 50.0);
        answer(&c, "Sertraline", "routine", "later", now()).unwrap();
        assert_eq!(question(&c, "Sertraline", now() + Duration::days(6)).unwrap(), None);
        assert!(question(&c, "Sertraline", now() + Duration::days(8)).unwrap().is_some());
        answer(&c, "Sertraline", "routine", "yes", now()).unwrap();
        assert_eq!(kind_of(&c, "sertraline").unwrap(), "routine");
        assert_eq!(question(&c, "Sertraline", now() + Duration::days(8)).unwrap(), None);
    }

    #[test]
    fn as_needed_is_asked_once_on_the_second_dose() {
        let c = journal();
        plain(&c, "Lorazepam", Some(1.0), &local(10, 22, 0));
        assert_eq!(question(&c, "Lorazepam", now()).unwrap(), None, "not on the first");
        plain(&c, "Lorazepam", Some(1.0), &local(14, 23, 0));
        let q = question(&c, "Lorazepam", now()).unwrap().expect("asked on the second");
        assert_eq!(q.ask, "as_needed");
        assert_eq!(q.text, "How do you take Lorazepam?");
        answer(&c, "Lorazepam", "as_needed", "as_needed", now()).unwrap();
        assert_eq!(kind_of(&c, "lorazepam").unwrap(), "as_needed");
        assert_eq!(question(&c, "Lorazepam", now()).unwrap(), None);
        // "Neither" is an answer too, and isn't asked again.
        for d in [3, 5] {
            plain(&c, "Kratom", Some(3.0), &local(d, 9, 0));
        }
        answer(&c, "Kratom", "as_needed", "neither", now()).unwrap();
        assert_eq!(kind_of(&c, "kratom").unwrap(), "");
        assert_eq!(question(&c, "Kratom", now()).unwrap(), None);
    }

    #[test]
    fn opioids_are_never_asked_only_marked() {
        let c = journal();
        for d in [3, 5, 7] {
            plain(&c, "Oxycodone", Some(5.0), &local(d, 9, 0));
        }
        assert_eq!(question(&c, "Oxycodone", now()).unwrap(), None);
        set_kind(&c, "Oxycodone", "as_needed").unwrap();
        assert_eq!(kind_of(&c, "oxycodone").unwrap(), "as_needed");
    }

    #[test]
    fn the_groups_come_from_names_classes_and_families() {
        let c = journal();
        assert_eq!(group(&c, "beer"), Group::Alcohol);
        assert_eq!(group(&c, "ghb"), Group::Ghb);
        assert_eq!(group(&c, "alprazolam"), Group::Benzodiazepine);
        assert_eq!(group(&c, "zolpidem"), Group::ZDrug);
        assert_eq!(group(&c, "pregabalin"), Group::Gabapentinoid);
        assert_eq!(group(&c, "kratom"), Group::Kratom);
        assert_eq!(group(&c, "fentanyl"), Group::Opioid);
        assert_eq!(group(&c, "caffeine"), Group::Other);
    }

    #[test]
    fn times_are_said_roughly() {
        assert_eq!(say_time(8 * 60 + 10), "8am");
        assert_eq!(say_time(8 * 60 + 20), "8:30am");
        assert_eq!(say_time(23 * 60 + 50), "12am");
        assert_eq!(say_time(13 * 60), "1pm");
    }
}
