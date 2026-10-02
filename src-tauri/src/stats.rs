// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Usage stats (roadmap #2): read-only patterns from the user's own doses.
//!
//! This is the one place the grouping rules live. The desktop and the phone both
//! render what [`usage_stats`] returns, so neither UI re-derives them:
//!
//! - **Never merge units.** A substance logged in `µg` and in `tab` becomes two
//!   series. Amounts are never converted: a tab has no µg value, and guessing one
//!   would be inventing a number.
//! - **A dose with no amount still happened.** It counts toward sessions, spacing
//!   and combinations, and is counted in `without_amount`, but it has no place on a
//!   dose axis.
//! - **One substance is one key**, whether a dose was matched to the catalogue or
//!   not: the catalogue's current name when `substance_id` is set (so a rename
//!   doesn't split a series), otherwise the logged name, case-folded.
//! - **Plain journal notes are not use.** Only `kind = 'session'` entries count.
//! - **Frequency counts sessions, not dose rows.** Three redoses in one night are
//!   one session.
//!
//! Day, week and time-of-day bucketing is left to the viewer, because it depends on
//! the viewer's time zone (the phone and the server need not agree). The frontend
//! does that in one shared helper, `src/lib/stats.ts`.
//!
//! Dose-range bands come from the bundled dose reference (`pw.rs`), never from the
//! knowledge corpus: the same containment rule the Companion follows. They're only
//! attached when the reference's unit for that route matches the logged unit.

use crate::db;
use crate::pw::Range;
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Parse a stored or imported timestamp: RFC 3339 first, then the zone-less forms
/// models emit (read as local time, as the browser's `Date.parse` reads them).
pub fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    if let Ok(t) = DateTime::parse_from_rfc3339(s) {
        return Some(t.with_timezone(&Utc));
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%d %H:%M"] {
        if let Ok(n) = NaiveDateTime::parse_from_str(s, fmt) {
            return Local.from_local_datetime(&n).earliest().map(|t| t.with_timezone(&Utc));
        }
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|n| Utc.from_utc_datetime(&n))
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageStats {
    /// Sessions with at least one dose in range, oldest first.
    pub sessions: Vec<SessionPoint>,
    /// One entry per substance, most sessions first.
    pub substances: Vec<SubstanceStats>,
    /// Substances taken in the same session, most common first.
    pub pairs: Vec<PairCount>,
    pub total_sessions: usize,
    pub total_doses: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionPoint {
    pub experience_id: i64,
    pub title: String,
    pub started_at: String,
    pub rating: Option<i64>,
    /// Substance keys taken in this session.
    pub substances: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubstanceStats {
    /// Stable identity, used to link series, sessions and pairs.
    pub key: String,
    /// What to show: the catalogue name, else the most recently logged spelling.
    pub name: String,
    pub sessions: usize,
    pub doses: usize,
    /// Time of the most recent dose, as stored.
    pub last_used: String,
    /// Days between consecutive sessions with this substance, oldest gap first.
    pub gaps_days: Vec<f64>,
    /// One series per unit, never merged.
    pub series: Vec<UnitSeries>,
    /// Route → dose count, most used first. An empty route is reported as "".
    pub routes: Vec<(String, usize)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnitSeries {
    pub unit: String,
    /// Every dose in this unit, oldest first, amount or not.
    pub points: Vec<DosePoint>,
    pub without_amount: usize,
    /// Reference dose ranges for this unit, when the reference has them.
    pub bands: Option<Bands>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DosePoint {
    pub dose_id: i64,
    pub experience_id: i64,
    pub taken_at: String,
    pub amount: Option<f64>,
    pub route: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Bands {
    /// The reference route the bands are for.
    pub route: String,
    pub threshold: Option<f64>,
    pub light: Range,
    pub common: Range,
    pub strong: Range,
    pub heavy: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PairCount {
    pub a: String,
    pub b: String,
    pub sessions: usize,
}

struct Row {
    dose_id: i64,
    experience_id: i64,
    title: String,
    started_at: String,
    rating: Option<i64>,
    key: String,
    name: String,
    amount: Option<f64>,
    unit: String,
    route: String,
    taken_at: String,
    at: Option<DateTime<Utc>>,
}

/// Units that are the same unit spelled differently. Used only to decide whether a
/// reference band applies; logged amounts and units are never rewritten.
fn unit_eq(a: &str, b: &str) -> bool {
    fn canon(u: &str) -> String {
        let u = u.trim().to_lowercase();
        match u.as_str() {
            "ug" | "mcg" | "μg" | "µg" => "µg".into(),
            _ => u,
        }
    }
    canon(a) == canon(b)
}

/// The reference bands for a substance logged in `unit`, preferring the route the
/// doses were most often taken by. `None` unless the reference's unit matches.
fn bands_for(conn: &Connection, name: &str, unit: &str, route: &str) -> Option<Bands> {
    let info = db::pw_lookup(conn, name).ok().flatten()?;
    let fits = |r: &&crate::pw::PwRoa| r.units.as_deref().is_some_and(|u| unit_eq(u, unit));
    let candidates: Vec<_> = info.roas.iter().filter(fits).collect();
    let roa = candidates
        .iter()
        .find(|r| !route.is_empty() && r.name.eq_ignore_ascii_case(route))
        .or(if candidates.len() == 1 { candidates.first() } else { None })?;
    Some(Bands {
        route: roa.name.clone(),
        threshold: roa.threshold,
        light: roa.light.clone(),
        common: roa.common.clone(),
        strong: roa.strong.clone(),
        heavy: roa.heavy,
    })
}

/// Patterns across every dose taken at or after `since` (all time when `None`).
pub fn usage_stats(conn: &Connection, since: Option<&str>) -> rusqlite::Result<UsageStats> {
    let since = since.and_then(parse_ts);
    let mut stmt = conn.prepare(
        "SELECT d.id, d.experience_id, e.title, e.started_at, e.rating,
                s.name, d.substance_name, d.amount, d.unit, d.route, d.taken_at
         FROM doses d
         JOIN experiences e ON e.id = d.experience_id
         LEFT JOIN substances s ON s.id = d.substance_id
         WHERE e.kind = 'session'",
    )?;
    let mut rows: Vec<Row> = stmt
        .query_map([], |r| {
            let catalogue: Option<String> = r.get(5)?;
            let logged: String = r.get(6)?;
            let name = catalogue.unwrap_or(logged).trim().to_string();
            let taken_at: String = r.get(10)?;
            Ok(Row {
                dose_id: r.get(0)?,
                experience_id: r.get(1)?,
                title: r.get(2)?,
                started_at: r.get(3)?,
                rating: r.get(4)?,
                key: name.to_lowercase(),
                name,
                amount: r.get(7)?,
                unit: r.get::<_, String>(8)?.trim().to_string(),
                route: r.get::<_, String>(9)?.trim().to_string(),
                at: parse_ts(&taken_at),
                taken_at,
            })
        })?
        .collect::<Result<_, _>>()?;
    rows.retain(|r| !r.key.is_empty());
    if let Some(since) = since {
        // A dose whose time can't be read can't be placed in a range; keep it only
        // for all-time views.
        rows.retain(|r| r.at.is_some_and(|t| t >= since));
    }
    // Oldest first, unparseable times last, ties by id so output is stable.
    rows.sort_by(|a, b| match (a.at, b.at) {
        (Some(x), Some(y)) => x.cmp(&y).then(a.dose_id.cmp(&b.dose_id)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.dose_id.cmp(&b.dose_id),
    });

    // Sessions, and which substances each one had.
    let mut session_keys: BTreeMap<i64, BTreeSet<String>> = BTreeMap::new();
    let mut session_head: HashMap<i64, (String, String, Option<i64>)> = HashMap::new();
    for r in &rows {
        session_keys.entry(r.experience_id).or_default().insert(r.key.clone());
        session_head
            .entry(r.experience_id)
            .or_insert_with(|| (r.title.clone(), r.started_at.clone(), r.rating));
    }
    let session_time = |id: i64| session_head.get(&id).and_then(|h| parse_ts(&h.1));
    let mut sessions: Vec<SessionPoint> = session_keys
        .iter()
        .map(|(id, keys)| {
            let (title, started_at, rating) = session_head[id].clone();
            SessionPoint { experience_id: *id, title, started_at, rating, substances: keys.iter().cloned().collect() }
        })
        .collect();
    sessions.sort_by_key(|s| (parse_ts(&s.started_at), s.experience_id));

    // Per substance: order of first appearance is irrelevant; sort at the end.
    let mut by_key: BTreeMap<String, Vec<&Row>> = BTreeMap::new();
    for r in &rows {
        by_key.entry(r.key.clone()).or_default().push(r);
    }
    let mut substances = Vec::with_capacity(by_key.len());
    for (key, doses) in by_key {
        let name = doses.last().map(|r| r.name.clone()).unwrap_or_default();
        let last_used = doses
            .iter()
            .rfind(|r| r.at.is_some())
            .or(doses.last())
            .map(|r| r.taken_at.clone())
            .unwrap_or_default();

        let mut session_ids: Vec<i64> = doses.iter().map(|r| r.experience_id).collect();
        session_ids.sort();
        session_ids.dedup();
        let mut times: Vec<DateTime<Utc>> = session_ids.iter().filter_map(|id| session_time(*id)).collect();
        times.sort();
        let gaps_days = times
            .windows(2)
            .map(|w| ((w[1] - w[0]).num_minutes() as f64 / 1440.0 * 10.0).round() / 10.0)
            .collect();

        let mut routes: HashMap<String, usize> = HashMap::new();
        for r in &doses {
            *routes.entry(r.route.to_lowercase()).or_default() += 1;
        }
        let mut routes: Vec<(String, usize)> = routes.into_iter().collect();
        routes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

        // Units: one series per unit, however it was spelled ("ug", "UG", "mcg"
        // and "µg" are one unit). Micrograms are labelled "µg"; any other unit
        // keeps the first spelling seen. Amounts are never converted.
        let mut units: Vec<(String, Vec<&Row>)> = Vec::new();
        for r in &doses {
            match units.iter_mut().find(|(u, _)| unit_eq(u, &r.unit)) {
                Some((_, v)) => v.push(r),
                None => {
                    let label = if unit_eq(&r.unit, "µg") { "µg".to_string() } else { r.unit.clone() };
                    units.push((label, vec![r]))
                }
            }
        }
        units.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
        let series = units
            .into_iter()
            .map(|(unit, rs)| {
                let mut rc: HashMap<&str, usize> = HashMap::new();
                for r in &rs {
                    *rc.entry(r.route.as_str()).or_default() += 1;
                }
                let top_route = rc.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(a.0))).map(|x| x.0).unwrap_or("");
                UnitSeries {
                    bands: bands_for(conn, &name, &unit, top_route),
                    without_amount: rs.iter().filter(|r| r.amount.is_none()).count(),
                    points: rs
                        .iter()
                        .map(|r| DosePoint {
                            dose_id: r.dose_id,
                            experience_id: r.experience_id,
                            taken_at: r.taken_at.clone(),
                            amount: r.amount,
                            route: r.route.clone(),
                        })
                        .collect(),
                    unit,
                }
            })
            .collect();

        substances.push(SubstanceStats {
            key,
            name,
            sessions: session_ids.len(),
            doses: doses.len(),
            last_used,
            gaps_days,
            series,
            routes,
        });
    }
    substances.sort_by(|a, b| b.sessions.cmp(&a.sessions).then(b.doses.cmp(&a.doses)).then(a.name.cmp(&b.name)));

    let mut pair_counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    for keys in session_keys.values() {
        let keys: Vec<&String> = keys.iter().collect();
        for i in 0..keys.len() {
            for j in i + 1..keys.len() {
                *pair_counts.entry((keys[i].clone(), keys[j].clone())).or_default() += 1;
            }
        }
    }
    let mut pairs: Vec<PairCount> =
        pair_counts.into_iter().map(|((a, b), sessions)| PairCount { a, b, sessions }).collect();
    pairs.sort_by(|x, y| y.sessions.cmp(&x.sessions).then(x.a.cmp(&y.a)).then(x.b.cmp(&y.b)));

    Ok(UsageStats { total_sessions: sessions.len(), total_doses: rows.len(), sessions, substances, pairs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    fn journal() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(db::schema_for_tests()).unwrap();
        c
    }

    fn session(c: &Connection, kind: &str, started: &str) -> i64 {
        c.execute(
            "INSERT INTO experiences (kind, title, started_at) VALUES (?1, 'x', ?2)",
            params![kind, started],
        )
        .unwrap();
        c.last_insert_rowid()
    }

    fn dose(c: &Connection, exp: i64, name: &str, amount: Option<f64>, unit: &str, at: &str) {
        c.execute(
            "INSERT INTO doses (experience_id, substance_name, amount, unit, route, taken_at)
             VALUES (?1, ?2, ?3, ?4, 'oral', ?5)",
            params![exp, name, amount, unit, at],
        )
        .unwrap();
    }

    #[test]
    fn different_units_are_never_merged() {
        let c = journal();
        let e = session(&c, "session", "2026-09-01T20:00:00Z");
        dose(&c, e, "LSD", Some(100.0), "µg", "2026-09-01T20:00:00Z");
        dose(&c, e, "LSD", Some(1.0), "tab", "2026-09-01T22:00:00Z");
        let s = usage_stats(&c, None).unwrap();
        let lsd = &s.substances[0];
        assert_eq!(lsd.series.len(), 2);
        assert!(lsd.series.iter().all(|u| u.points.len() == 1));
    }

    #[test]
    fn a_dose_without_an_amount_counts_but_is_flagged() {
        let c = journal();
        let e = session(&c, "session", "2026-09-01T20:00:00Z");
        dose(&c, e, "MDMA", None, "mg", "2026-09-01T20:00:00Z");
        let s = usage_stats(&c, None).unwrap();
        assert_eq!(s.total_sessions, 1);
        assert_eq!(s.substances[0].sessions, 1);
        assert_eq!(s.substances[0].series[0].without_amount, 1);
    }

    #[test]
    fn redoses_in_one_session_count_as_one_session() {
        let c = journal();
        let e = session(&c, "session", "2026-09-01T20:00:00Z");
        for at in ["2026-09-01T20:00:00Z", "2026-09-01T21:30:00Z", "2026-09-01T23:00:00Z"] {
            dose(&c, e, "ketamine", Some(30.0), "mg", at);
        }
        let s = usage_stats(&c, None).unwrap();
        assert_eq!(s.total_sessions, 1);
        assert_eq!(s.substances[0].sessions, 1);
        assert_eq!(s.substances[0].doses, 3);
        assert!(s.substances[0].gaps_days.is_empty());
    }

    #[test]
    fn plain_journal_notes_are_not_use() {
        let c = journal();
        // Notes can't hold doses through the app; insert directly to prove the
        // filter, not the write path, keeps them out.
        let n = session(&c, "note", "2026-09-01T20:00:00Z");
        dose(&c, n, "LSD", Some(100.0), "µg", "2026-09-01T20:00:00Z");
        let s = usage_stats(&c, None).unwrap();
        assert_eq!(s.total_sessions, 0);
        assert!(s.substances.is_empty());
    }

    #[test]
    fn spacing_is_measured_between_sessions_in_days() {
        let c = journal();
        for day in ["2026-09-01", "2026-09-15", "2026-09-16"] {
            let e = session(&c, "session", &format!("{day}T20:00:00Z"));
            dose(&c, e, "psilocybin mushrooms", Some(2.0), "g", &format!("{day}T20:00:00Z"));
        }
        let s = usage_stats(&c, None).unwrap();
        assert_eq!(s.substances[0].gaps_days, vec![14.0, 1.0]);
        assert_eq!(s.substances[0].last_used, "2026-09-16T20:00:00Z");
    }

    #[test]
    fn spelling_differences_are_one_substance() {
        let c = journal();
        let e = session(&c, "session", "2026-09-01T20:00:00Z");
        dose(&c, e, "LSD", Some(100.0), "ug", "2026-09-01T20:00:00Z");
        let e2 = session(&c, "session", "2026-09-10T20:00:00Z");
        dose(&c, e2, " lsd ", Some(80.0), "UG", "2026-09-10T20:00:00Z");
        let s = usage_stats(&c, None).unwrap();
        assert_eq!(s.substances.len(), 1);
        assert_eq!(s.substances[0].series.len(), 1, "ug and UG are one unit");
        assert_eq!(s.substances[0].series[0].unit, "µg");
        let e3 = session(&c, "session", "2026-09-20T20:00:00Z");
        dose(&c, e3, "LSD", Some(90.0), "µg", "2026-09-20T20:00:00Z");
        let s = usage_stats(&c, None).unwrap();
        assert_eq!(s.substances[0].series.len(), 1, "ug and µg are one unit");
        assert_eq!(s.substances[0].series[0].points.len(), 3);
    }

    #[test]
    fn combinations_count_sessions_not_doses() {
        let c = journal();
        let e = session(&c, "session", "2026-09-01T20:00:00Z");
        dose(&c, e, "MDMA", Some(100.0), "mg", "2026-09-01T20:00:00Z");
        dose(&c, e, "MDMA", Some(50.0), "mg", "2026-09-01T22:00:00Z");
        dose(&c, e, "LSD", Some(100.0), "µg", "2026-09-01T20:00:00Z");
        let s = usage_stats(&c, None).unwrap();
        assert_eq!(s.pairs.len(), 1);
        assert_eq!(s.pairs[0].sessions, 1);
    }

    #[test]
    fn since_drops_older_doses() {
        let c = journal();
        let old = session(&c, "session", "2025-01-01T20:00:00Z");
        dose(&c, old, "LSD", Some(100.0), "µg", "2025-01-01T20:00:00Z");
        let new = session(&c, "session", "2026-09-01T20:00:00Z");
        dose(&c, new, "LSD", Some(100.0), "µg", "2026-09-01T20:00:00Z");
        let s = usage_stats(&c, Some("2026-01-01T00:00:00Z")).unwrap();
        assert_eq!(s.total_sessions, 1);
        assert_eq!(s.total_doses, 1);
    }

    #[test]
    fn micrograms_spelled_any_way_match_the_reference_unit() {
        assert!(unit_eq("ug", "µg"));
        assert!(unit_eq("mcg", "µg"));
        assert!(unit_eq("MG", "mg"));
        assert!(!unit_eq("mg", "µg"));
        assert!(!unit_eq("tab", "µg"));
    }
}
