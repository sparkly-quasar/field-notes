// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! The Field Notes dose reference. Reads a bundled snapshot of the DoseWiki
//! substance encyclopedia (dose ranges, durations, graded interactions), corrects
//! it on load (units, band order, route names, impossible durations), replaces
//! the routes it gets badly wrong ([`ROUTE_OVERRIDES`]), and maps the result into
//! the local reference cache. Based on DoseWiki, with substantial revisions. The data ships *with* the app as an offline
//! resource, so there is no network call and every lookup is private.
//!
//! DoseWiki content is dedicated to the public domain under CC0 (the site code is
//! MIT); no attribution is legally required. We credit DoseWiki in-app as a
//! courtesy. See `data/dosewiki/README.md` for the snapshot + slimming pipeline.

use serde::{Deserialize, Serialize};

/// Date of the bundled DoseWiki snapshot (see `data/dosewiki/slim.py`). Bump this
/// whenever `resources/dosewiki.json` is regenerated from a fresh download.
pub const DOSEWIKI_SNAPSHOT: &str = "2026-07-08";

/// Names people write that DoseWiki doesn't list, added to its own on load so they
/// survive a fresh snapshot. They let a pasted log or a quick log find the real
/// entry, and with it the dose ranges and interaction warnings. (name, extra aliases)
const EXTRA_ALIASES: &[(&str, &[&str])] = &[
    ("Mephedrone", &["4mmc", "4-MMC", "meph", "m-cat"]),
    ("1,4-Butanediol", &["14b", "1,4b"]),
    ("Dextroamphetamine", &["dexamp"]),
];

// ---- slimmed DoseWiki JSON shapes (see data/dosewiki/slim.py) ----

#[derive(Deserialize)]
struct DwSub {
    title: String,
    #[serde(default)]
    alternative_names: Vec<String>,
    #[serde(default)]
    chemical_class: Vec<String>,
    #[serde(default)]
    psychoactive_class: Vec<String>,
    #[serde(default)]
    routes: Vec<DwRoute>,
    #[serde(default)]
    interactions: DwInteractions,
}
#[derive(Deserialize)]
struct DwRoute {
    #[serde(default)]
    route: String,
    #[serde(default)]
    dose_ranges: DwDoseRanges,
    #[serde(default)]
    stages: DwStages,
    #[serde(default)]
    half_life: Option<String>,
}
#[derive(Deserialize, Default)]
struct DwDoseRanges {
    threshold: Option<DwRange>,
    light: Option<DwRange>,
    moderate: Option<DwRange>,
    strong: Option<DwRange>,
    heavy: Option<DwRange>,
}
#[derive(Deserialize)]
struct DwRange {
    min: Option<f64>,
    max: Option<f64>,
    #[serde(default)]
    unit: Option<String>,
}
#[derive(Deserialize, Default)]
struct DwStages {
    onset: Option<DwStage>,
    come_up: Option<DwStage>,
    peak: Option<DwStage>,
    offset: Option<DwStage>,
    after_effects: Option<DwStage>,
    total_duration: Option<DwStage>,
}
#[derive(Deserialize)]
struct DwStage {
    min: Option<f64>,
    max: Option<f64>,
    #[serde(default)]
    unit: Option<String>,
}
#[derive(Deserialize, Default)]
struct DwInteractions {
    #[serde(default)]
    dangerous: Vec<String>,
    // `unsafe` is a Rust keyword, so store it under a safe field name.
    #[serde(default, rename = "unsafe")]
    unsafe_: Vec<String>,
    #[serde(default)]
    caution: Vec<String>,
}

// ---- stored / exposed shape ----

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Range {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

/// A single graded interaction: the substance/class this one is risky with, an
/// optional human reason, and a severity mapped onto our danger/caution/note scale.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PwInteraction {
    pub name: String,
    pub reason: Option<String>,
    /// "danger" | "caution" | "note"
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PwRoa {
    pub name: String,
    pub units: Option<String>,
    pub threshold: Option<f64>,
    pub light: Range,
    pub common: Range,
    pub strong: Range,
    pub heavy: Option<f64>,
    /// The top of the heavy band, when a source gives one (most give only "N+").
    #[serde(default)]
    pub heavy_max: Option<f64>,
    pub onset: Option<String>,
    pub come_up: Option<String>,
    pub peak: Option<String>,
    pub offset: Option<String>,
    pub after_effects: Option<String>,
    pub total: Option<String>,
    pub half_life: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PwInfo {
    pub name: String,
    pub common_names: Vec<String>,
    pub psychoactive: Vec<String>,
    pub chemical: Vec<String>,
    pub roas: Vec<PwRoa>,
    pub interactions: Vec<PwInteraction>,
    /// Where the dose figures come from, when it isn't DoseWiki (see [`ROUTE_OVERRIDES`]).
    #[serde(default)]
    pub dose_note: Option<String>,
}

fn range(g: &Option<DwRange>) -> Range {
    match g {
        Some(r) => Range { min: r.min, max: r.max },
        None => Range::default(),
    }
}

/// First unit found across a route's dose ranges (they're generally consistent).
fn route_units(d: &DwDoseRanges) -> Option<String> {
    [&d.moderate, &d.light, &d.threshold, &d.strong, &d.heavy]
        .into_iter()
        .flatten()
        .find_map(|r| r.unit.as_deref().map(canonical_unit).filter(|u| !u.is_empty()))
}

fn fmt_stage(g: &Option<DwStage>) -> Option<String> {
    let d = g.as_ref()?;
    let units = d.unit.clone().unwrap_or_default();
    match (d.min, d.max) {
        (Some(a), Some(b)) if (a - b).abs() > f64::EPSILON => Some(format!("{a}–{b} {units}").trim().to_string()),
        (Some(a), _) | (_, Some(a)) => Some(format!("{a} {units}").trim().to_string()),
        _ => None,
    }
}

/// Split a DoseWiki interaction entry `Name (reason)` into (name, reason). The
/// head before the first `(` is the substance/class we match on; the parenthetical
/// (if any) is a human-readable reason.
fn split_interaction(entry: &str) -> Option<(String, Option<String>)> {
    let entry = entry.trim();
    if entry.is_empty() {
        return None;
    }
    match entry.find('(') {
        Some(i) => {
            let name = entry[..i].trim().to_string();
            let reason = entry[i + 1..].trim_end_matches(')').trim().to_string();
            let reason = if reason.is_empty() { None } else { Some(reason) };
            if name.is_empty() { None } else { Some((name, reason)) }
        }
        None => Some((entry.to_string(), None)),
    }
}

fn interactions_of(list: &[String], severity: &str) -> Vec<PwInteraction> {
    list.iter()
        .filter_map(|e| split_interaction(e))
        .map(|(name, reason)| PwInteraction { name, reason, severity: severity.to_string() })
        .collect()
}

// ---- revisions applied to every DoseWiki entry on load ----
//
// DoseWiki's dose data has structural errors that the app would otherwise read
// wrongly without saying so (see ROADMAP, "Dose reference review"). They are
// fixed here, on load, so a fresh snapshot gets the same treatment. Each change
// is written to the revision log (`parse_slim_logged`), which is committed as
// `data/dosewiki/REVISIONS.txt`.

/// The dose form's route names: oral, insufflated, sublingual, vaporized, rectal,
/// IM, IV. DoseWiki spells routes many ways ("I.M.", "Intravenous", "Smoked",
/// "Inhaled"); smoked and inhaled are both the form's "vaporized".
pub fn canonical_route(route: &str) -> String {
    let r = route.trim().to_lowercase();
    match r.as_str() {
        "smoked" | "inhaled" | "inhalation" | "vaped" | "vaporised" | "vaporized" => "vaporized".into(),
        "im" | "i.m." | "intramuscular" => "IM".into(),
        "iv" | "i.v." | "intravenous" => "IV".into(),
        "intranasal" | "nasal" | "snorted" => "insufflated".into(),
        _ => r,
    }
}

/// One spelling per unit: "ug", "mcg" and "μg" are all "µg".
fn canonical_unit(u: &str) -> String {
    let u = u.trim().to_lowercase();
    match u.as_str() {
        "ug" | "mcg" | "μg" | "µg" => "µg".into(),
        _ => u,
    }
}

/// How many micrograms one of `unit` is, for the units that convert.
fn mass_in_ug(unit: &str) -> Option<f64> {
    match unit {
        "µg" => Some(1.0),
        "mg" => Some(1_000.0),
        "g" => Some(1_000_000.0),
        _ => None,
    }
}

const TIERS: [&str; 5] = ["threshold", "light", "moderate", "strong", "heavy"];

fn tiers_mut(d: &mut DwDoseRanges) -> [&mut Option<DwRange>; 5] {
    [&mut d.threshold, &mut d.light, &mut d.moderate, &mut d.strong, &mut d.heavy]
}

fn has_amount(g: &Option<DwRange>) -> bool {
    g.as_ref().is_some_and(|r| r.min.is_some() || r.max.is_some())
}

/// Put every band of a route in one unit. DoseWiki sometimes gives some bands in
/// g and others in mg (or µg and mg) within one route, and the app reads all of
/// them in one unit, so a psilocin "heavy 5 g" read as 5 mg. The route's unit is
/// the one most bands use (ties go to the moderate band's). A band in a unit that
/// can't be converted (UK units against US drinks, mg/kg) is dropped. A band that
/// only fits after converting but then sits more than 10× away from the bands
/// that were already in the route's unit was copied from somewhere else (psilocin
/// carried psilocybin mushroom figures in grams) and is dropped too.
fn unify_units(title: &str, route: &str, d: &mut DwDoseRanges, log: &mut Vec<String>) -> Option<String> {
    let units: Vec<Option<String>> = tiers_mut(d)
        .iter()
        .map(|g| g.as_ref().filter(|_| has_amount(g)).and_then(|r| r.unit.as_deref()).map(canonical_unit).filter(|u| !u.is_empty()))
        .collect();
    let mut counts: Vec<(String, usize)> = Vec::new();
    for u in units.iter().flatten() {
        match counts.iter_mut().find(|(c, _)| c == u) {
            Some((_, n)) => *n += 1,
            None => counts.push((u.clone(), 1)),
        }
    }
    let top = counts.iter().map(|c| c.1).max()?;
    let tied: Vec<&String> = counts.iter().filter(|c| c.1 == top).map(|c| &c.0).collect();
    let unit = match &units[2] {
        Some(m) if tied.contains(&m) => m.clone(),
        _ => tied[0].clone(),
    };

    let mut converted = [false; 5];
    for (i, g) in tiers_mut(d).into_iter().enumerate() {
        let Some(from) = units[i].as_ref().filter(|u| **u != unit) else { continue };
        match (mass_in_ug(from), mass_in_ug(&unit)) {
            (Some(a), Some(b)) => {
                let r = g.as_mut().unwrap();
                r.min = r.min.map(|x| x * a / b);
                r.max = r.max.map(|x| x * a / b);
                r.unit = Some(unit.clone());
                converted[i] = true;
            }
            _ => {
                log.push(format!("{title} [{route}]: dropped {} band, given in {from} where the rest is in {unit}", TIERS[i]));
                *g = None;
            }
        }
    }
    // A converted band far out of line with the native ones came from elsewhere.
    let mins: Vec<Option<f64>> = tiers_mut(d).iter().map(|g| g.as_ref().and_then(|r| r.min.or(r.max))).collect();
    for i in 0..5 {
        let (true, Some(v)) = (converted[i], mins[i]) else { continue };
        let near = (1..5)
            .flat_map(|k| [i.checked_sub(k), Some(i + k)])
            .flatten()
            .filter(|&j| j < 5 && !converted[j])
            .find_map(|j| mins[j]);
        if let Some(n) = near.filter(|&n| n > 0.0 && v > 0.0) {
            if v / n > 10.0 || n / v > 10.0 {
                log.push(format!(
                    "{title} [{route}]: dropped {} band ({v} {unit} once converted), more than 10x off the other bands",
                    TIERS[i]
                ));
                *tiers_mut(d)[i] = None;
                continue;
            }
        }
        log.push(format!("{title} [{route}]: {} band converted to {unit}", TIERS[i]));
    }
    Some(unit)
}

/// Bands must rise: each tier's lower bound at or above the one before it, since
/// the classifier reads them from heavy down. Keep the longest rising run of
/// tiers and drop the rest (ties keep the lower tiers), and drop a band whose
/// min is above its own max.
fn order_bands(title: &str, route: &str, d: &mut DwDoseRanges, log: &mut Vec<String>) {
    for (i, g) in tiers_mut(d).into_iter().enumerate() {
        if let Some(r) = g.as_ref() {
            if let (Some(a), Some(b)) = (r.min, r.max) {
                if a > b {
                    log.push(format!("{title} [{route}]: dropped {} band, min {a} above max {b}", TIERS[i]));
                    *g = None;
                }
            }
        }
    }
    let mins: Vec<Option<f64>> = tiers_mut(d).iter().map(|g| g.as_ref().and_then(|r| r.min)).collect();
    let idx: Vec<usize> = (0..5).filter(|&i| mins[i].is_some()).collect();
    // Longest non-decreasing subsequence over at most five values.
    let mut best: Vec<usize> = Vec::new();
    for mask in 1u32..(1 << idx.len()) {
        let pick: Vec<usize> = (0..idx.len()).filter(|b| mask & (1 << b) != 0).map(|b| idx[b]).collect();
        let rising = pick.windows(2).all(|w| mins[w[0]] <= mins[w[1]]);
        let better = pick.len() > best.len() || (pick.len() == best.len() && pick < best);
        if rising && better {
            best = pick;
        }
    }
    for &i in idx.iter().filter(|i| !best.contains(i)) {
        log.push(format!(
            "{title} [{route}]: dropped {} band (from {}), out of order with the bands around it",
            TIERS[i],
            mins[i].unwrap()
        ));
        *tiers_mut(d)[i] = None;
    }
}

/// A stage that ends after the whole experience does is wrong (buspirone's peak
/// was "40–90 hours" against a total of 2.5); drop it rather than time warnings
/// by it.
fn check_stages(title: &str, route: &str, st: &mut DwStages, log: &mut Vec<String>) {
    let minutes = |g: &Option<DwStage>| {
        let g = g.as_ref()?;
        let v = g.max.or(g.min)?;
        let u = g.unit.as_deref().unwrap_or("").to_lowercase();
        Some(if u.starts_with("sec") { v / 60.0 } else if u.starts_with("min") { v } else if u.starts_with("day") { v * 1440.0 } else { v * 60.0 })
    };
    let Some(total) = minutes(&st.total_duration) else { return };
    for (name, g) in [("onset", &mut st.onset), ("come-up", &mut st.come_up), ("peak", &mut st.peak), ("offset", &mut st.offset)] {
        if minutes(g).is_some_and(|m| m > total) {
            log.push(format!("{title} [{route}]: dropped {name}, longer than the total duration"));
            *g = None;
        }
    }
}

fn map_sub(s: DwSub, log: &mut Vec<String>) -> PwInfo {
    let title = s.title.clone();
    let mut seen: Vec<String> = Vec::new();
    let roas = s
        .routes
        .into_iter()
        .filter_map(|mut r| {
            let name = canonical_route(&r.route);
            if seen.contains(&name) {
                log.push(format!("{title}: dropped duplicate route \"{}\" (already have {name})", r.route));
                return None;
            }
            if !name.eq_ignore_ascii_case(&r.route) {
                log.push(format!("{title}: route \"{}\" renamed {name}", r.route));
            }
            seen.push(name.clone());
            let units = unify_units(&title, &name, &mut r.dose_ranges, log);
            order_bands(&title, &name, &mut r.dose_ranges, log);
            check_stages(&title, &name, &mut r.stages, log);
            Some((name, units, r))
        })
        .map(|(name, units, r)| {
            let d = &r.dose_ranges;
            PwRoa {
                name,
                units: units.or_else(|| route_units(d)),
                threshold: d.threshold.as_ref().and_then(|t| t.min),
                light: range(&d.light),
                common: range(&d.moderate), // DoseWiki calls our "common" tier "moderate"
                strong: range(&d.strong),
                heavy: d.heavy.as_ref().and_then(|h| h.min),
                heavy_max: None,
                onset: fmt_stage(&r.stages.onset),
                come_up: fmt_stage(&r.stages.come_up),
                peak: fmt_stage(&r.stages.peak),
                offset: fmt_stage(&r.stages.offset),
                after_effects: fmt_stage(&r.stages.after_effects),
                total: fmt_stage(&r.stages.total_duration),
                half_life: r.half_life.filter(|h| !h.is_empty()),
            }
        })
        .collect();

    // dangerous -> danger, unsafe -> caution, caution -> note (per ROADMAP #1).
    let mut interactions = interactions_of(&s.interactions.dangerous, "danger");
    interactions.extend(interactions_of(&s.interactions.unsafe_, "caution"));
    interactions.extend(interactions_of(&s.interactions.caution, "note"));

    let mut common_names = s.alternative_names;
    for (_, extra) in EXTRA_ALIASES.iter().filter(|(n, _)| n.eq_ignore_ascii_case(&s.title)) {
        for a in extra.iter() {
            if !common_names.iter().any(|c| c.eq_ignore_ascii_case(a)) {
                common_names.push(a.to_string());
            }
        }
    }

    let mut info = PwInfo {
        name: s.title,
        common_names,
        psychoactive: s.psychoactive_class,
        chemical: s.chemical_class,
        roas,
        interactions,
        dose_note: None,
    };
    if let Some((_, note, routes)) = ROUTE_OVERRIDES.iter().find(|(n, ..)| n.eq_ignore_ascii_case(&info.name)) {
        info.roas = routes.iter().map(RouteSpec::roa).collect();
        info.dose_note = Some(note.to_string());
    }
    info
}

/// One route's figures, written out by hand (see [`ROUTE_OVERRIDES`]). Amounts in
/// `units`; durations as display text, like DoseWiki's.
struct RouteSpec {
    name: &'static str,
    units: &'static str,
    threshold: Option<f64>,
    light: (f64, f64),
    common: (f64, f64),
    strong: (f64, f64),
    heavy: Option<(f64, Option<f64>)>,
    onset: Option<&'static str>,
    peak: Option<&'static str>,
    after_effects: Option<&'static str>,
    total: Option<&'static str>,
}

impl RouteSpec {
    fn roa(&self) -> PwRoa {
        let r = |(a, b): (f64, f64)| Range { min: Some(a), max: Some(b) };
        PwRoa {
            name: self.name.to_string(),
            units: Some(self.units.to_string()),
            threshold: self.threshold,
            light: r(self.light),
            common: r(self.common),
            strong: r(self.strong),
            heavy: self.heavy.map(|h| h.0),
            heavy_max: self.heavy.and_then(|h| h.1),
            onset: self.onset.map(String::from),
            come_up: None,
            peak: self.peak.map(String::from),
            offset: None,
            after_effects: self.after_effects.map(String::from),
            total: self.total.map(String::from),
            half_life: None,
        }
    }
}

/// Substances whose DoseWiki routes are wrong enough to replace outright, kept here
/// so a fresh snapshot doesn't bring the error back. Each fix is also worth sending
/// upstream (contribute.rs). (name, where the figures come from, routes)
///
/// 5-MeO-DMT: DoseWiki listed an oral route (it isn't orally active) with "heavy"
/// below "strong", and "inhaled" duplicated "smoked". Smoked and insufflated are
/// Erowid's figures; intramuscular is the owner's, from practice. Sublingual is
/// left out: Erowid gives only a single light figure for it.
const ROUTE_OVERRIDES: &[(&str, &str, &[RouteSpec])] = &[(
    "5-MeO-DMT",
    "Smoked and insufflated ranges from Erowid. Intramuscular ranges from the Field Notes maintainer. Not orally active.",
    &[
        RouteSpec {
            name: "vaporized", units: "mg", threshold: Some(1.0),
            light: (2.0, 5.0), common: (5.0, 10.0), strong: (10.0, 20.0), heavy: None,
            onset: Some("0–30 seconds"), peak: Some("1–15 minutes"), after_effects: Some("1 hour"), total: Some("30 minutes"),
        },
        RouteSpec {
            name: "insufflated", units: "mg", threshold: Some(3.0),
            light: (5.0, 10.0), common: (8.0, 15.0), strong: (10.0, 25.0), heavy: None,
            onset: Some("5 minutes"), peak: Some("10–30 minutes"), after_effects: Some("1–3 hours"), total: Some("30–45 minutes"),
        },
        RouteSpec {
            name: "IM", units: "mg", threshold: None,
            light: (0.5, 1.0), common: (1.5, 3.0), strong: (5.0, 7.0), heavy: Some((8.0, Some(12.0))),
            onset: None, peak: None, after_effects: None, total: None,
        },
    ],
)];

/// Parse the slimmed DoseWiki JSON (the bundled `dosewiki.json`) into our shape.
pub fn parse_slim(json: &str) -> Result<Vec<PwInfo>, String> {
    parse_slim_logged(json).map(|(subs, _)| subs)
}

/// [`parse_slim`], plus one line for every change made to DoseWiki's figures.
pub fn parse_slim_logged(json: &str) -> Result<(Vec<PwInfo>, Vec<String>), String> {
    let subs: Vec<DwSub> =
        serde_json::from_str(json).map_err(|e| format!("Couldn't parse the bundled dose reference: {e}"))?;
    let mut log = Vec::new();
    let infos = subs.into_iter().map(|s| map_sub(s, &mut log)).collect();
    Ok((infos, log))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_interaction_name_and_reason() {
        let (n, r) = split_interaction("Tramadol (serotonin syndrome risk)").unwrap();
        assert_eq!(n, "Tramadol");
        assert_eq!(r.as_deref(), Some("serotonin syndrome risk"));

        let (n, r) = split_interaction("Lithium").unwrap();
        assert_eq!(n, "Lithium");
        assert!(r.is_none());

        assert!(split_interaction("   ").is_none());
    }

    #[test]
    fn five_meo_dmt_routes_are_replaced() {
        let subs = parse_slim(include_str!("../../resources/dosewiki.json")).unwrap();
        let s = subs.iter().find(|s| s.name == "5-MeO-DMT").unwrap();
        let names: Vec<&str> = s.roas.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["vaporized", "insufflated", "IM"]);
        let im = &s.roas[2];
        assert_eq!((im.light.min, im.light.max), (Some(0.5), Some(1.0)));
        assert_eq!((im.heavy, im.heavy_max), (Some(8.0), Some(12.0)));
        assert!(s.dose_note.as_deref().unwrap().contains("Erowid"));
    }

    fn bundled() -> Vec<PwInfo> {
        parse_slim(include_str!("../../resources/dosewiki.json")).unwrap()
    }

    fn route<'a>(subs: &'a [PwInfo], name: &str, route: &str) -> &'a PwRoa {
        let s = subs.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("no {name}"));
        s.roas.iter().find(|r| r.name == route).unwrap_or_else(|| panic!("{name} has no {route} route"))
    }

    #[test]
    fn every_route_is_clean_after_load() {
        let form = ["oral", "insufflated", "sublingual", "vaporized", "rectal", "IM", "IV"];
        for s in bundled() {
            let mut names: Vec<&str> = s.roas.iter().map(|r| r.name.as_str()).collect();
            names.sort();
            names.dedup();
            assert_eq!(names.len(), s.roas.len(), "{} has a duplicate route", s.name);
            for r in &s.roas {
                let n = r.name.as_str();
                assert!(!["smoked", "inhaled", "intravenous", "intramuscular", "I.M."].contains(&n), "{}: {n}", s.name);
                assert!(form.contains(&n) || n == n.to_lowercase(), "{}: route {n} not canonical", s.name);
                assert!(r.units.as_deref() != Some("ug"), "{}: ug not normalised", s.name);
                let mins = [r.threshold, r.light.min, r.common.min, r.strong.min, r.heavy];
                let set: Vec<f64> = mins.into_iter().flatten().collect();
                assert!(set.windows(2).all(|w| w[0] <= w[1]), "{} [{n}]: bands out of order {mins:?}", s.name);
            }
        }
    }

    #[test]
    fn mixed_units_are_converted_or_dropped() {
        let subs = bundled();
        // Psilocin's gram figures were psilocybin mushroom figures: gone, mg bands kept.
        let p = route(&subs, "Psilocin", "oral");
        assert_eq!((p.units.as_deref(), p.threshold, p.heavy), (Some("mg"), None, None));
        assert_eq!((p.light.min, p.strong.max), (Some(5.0), Some(25.0)));
        // Butyrfentanyl: strong and heavy were in mg, the rest in µg.
        let b = route(&subs, "Butyrfentanyl", "oral");
        assert_eq!((b.units.as_deref(), b.strong.min, b.heavy), (Some("µg"), Some(1500.0), Some(3000.0)));
        // Piracetam is mostly in g; its one mg band becomes g.
        let pi = route(&subs, "Piracetam", "oral");
        assert_eq!((pi.units.as_deref(), pi.common.min, pi.heavy), (Some("g"), Some(1.2), Some(5.0)));
    }

    #[test]
    fn routes_take_the_dose_forms_names() {
        let subs = bundled();
        assert_eq!(route(&subs, "Ketamine", "IM").light.min, Some(15.0));
        assert!(route(&subs, "Heroin", "IV").common.min.is_some());
        // DMT listed Vaporized and Inhaled; the first is kept.
        let dmt = subs.iter().find(|s| s.name == "DMT").unwrap();
        assert_eq!(dmt.roas.iter().filter(|r| r.name == "vaporized").count(), 1);
    }

    #[test]
    fn stages_longer_than_the_whole_are_dropped() {
        let subs = bundled();
        assert!(route(&subs, "Buspirone", "oral").peak.is_none());
    }

    /// Writes the revision log. Run after a refresh and commit the result:
    /// `cargo test -p field_notes_core write_revision_log -- --ignored`
    #[test]
    #[ignore]
    fn write_revision_log() {
        let (_, log) = parse_slim_logged(include_str!("../../resources/dosewiki.json")).unwrap();
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/dosewiki/REVISIONS.txt");
        let head = format!(
            "Changes Field Notes makes to the DoseWiki snapshot ({DOSEWIKI_SNAPSHOT}) on load.\n\
             Generated by `cargo test -p field_notes_core write_revision_log -- --ignored`;\n\
             see src-tauri/core/src/pw.rs. Hand-written replacements (ROUTE_OVERRIDES) are listed there.\n\n"
        );
        std::fs::write(path, head + &log.join("\n") + "\n").unwrap();
    }

    #[test]
    fn adds_extra_aliases() {
        let subs = parse_slim(include_str!("../../resources/dosewiki.json")).unwrap();
        for (name, extra) in EXTRA_ALIASES {
            let s = subs.iter().find(|s| s.name == *name).unwrap_or_else(|| panic!("{name} is no longer in DoseWiki"));
            for a in extra.iter() {
                assert!(s.common_names.iter().any(|c| c == a), "{name} is missing {a}");
            }
        }
    }

    #[test]
    fn parses_bundled_snapshot() {
        // The committed resource must parse and carry dose + interaction data.
        let json = include_str!("../../resources/dosewiki.json");
        let subs = parse_slim(json).expect("parse bundled dosewiki.json");
        assert!(subs.len() > 500, "expected hundreds of substances, got {}", subs.len());
        assert!(subs.iter().any(|s| !s.roas.is_empty()), "expected some dose data");
        let graded: Vec<&PwInteraction> = subs.iter().flat_map(|s| &s.interactions).collect();
        assert!(graded.iter().any(|i| i.severity == "danger" || i.severity == "note"),
            "expected graded interactions");
        assert!(graded.iter().any(|i| i.reason.is_some()), "expected interaction reasons");
    }
}
