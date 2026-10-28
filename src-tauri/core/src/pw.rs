// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! The Field Notes dose reference. Reads a bundled snapshot of the DoseWiki
//! substance encyclopedia (dose ranges, durations, graded interactions), corrects
//! it on load (units, band order, route names, impossible durations), replaces
//! the routes it gets badly wrong ([`ROUTE_OVERRIDES`], [`ROUTE_PATCHES`]), and maps the result into
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
    // Mescaline cacti by the names people log them under (issue #32).
    ("Mescaline", &["San Pedro powder", "huachuma", "Peruvian torch"]),
];

/// A route's name as one spelling, for matching a logged route against the
/// reference's: the form's "IM" and DoseWiki's "I.M." and "Intramuscular" are the
/// same route, and so are "smoked" and the reference's "vaporized" (see
/// [`canonical_route`], which names the reference's routes on load).
pub fn route_key(route: &str) -> String {
    let r: String = route.chars().filter(|c| !c.is_whitespace() && *c != '.').collect();
    canonical_route(&r).to_lowercase()
}

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
    /// Some timings here are the person's own, entered in Field Notes.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mine: bool,
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
        mine: false,
    };
    apply_patches(&mut info, log);
    if let Some((_, note, routes)) = ROUTE_OVERRIDES.iter().find(|(n, ..)| n.eq_ignore_ascii_case(&info.name)) {
        info.roas = routes.iter().map(RouteSpec::roa).collect();
        info.dose_note = Some(note.to_string());
    }
    info
}

/// One route's figures, written out by hand (see [`ROUTE_OVERRIDES`]). Amounts in
/// `units`; durations as display text, like DoseWiki's. Start from [`BLANK`] and
/// fill in what the source gives.
struct RouteSpec {
    name: &'static str,
    units: &'static str,
    threshold: Option<f64>,
    light: Option<(f64, f64)>,
    common: Option<(f64, f64)>,
    strong: Option<(f64, f64)>,
    heavy: Option<(f64, Option<f64>)>,
    stages: Stages,
}

/// Timings, as display text. Start from [`NO_STAGES`].
#[derive(Clone, Copy)]
struct Stages {
    onset: Option<&'static str>,
    come_up: Option<&'static str>,
    peak: Option<&'static str>,
    offset: Option<&'static str>,
    after_effects: Option<&'static str>,
    total: Option<&'static str>,
}

const NO_STAGES: Stages = Stages { onset: None, come_up: None, peak: None, offset: None, after_effects: None, total: None };
const BLANK: RouteSpec = RouteSpec {
    name: "", units: "mg", threshold: None, light: None, common: None, strong: None, heavy: None, stages: NO_STAGES,
};

fn range_of(r: Option<(f64, f64)>) -> Range {
    r.map(|(a, b)| Range { min: Some(a), max: Some(b) }).unwrap_or_default()
}

impl RouteSpec {
    fn roa(&self) -> PwRoa {
        let mut roa = PwRoa {
            name: self.name.to_string(),
            units: None,
            threshold: None,
            light: Range::default(),
            common: Range::default(),
            strong: Range::default(),
            heavy: None,
            heavy_max: None,
            onset: None,
            come_up: None,
            peak: None,
            offset: None,
            after_effects: None,
            total: None,
            half_life: None,
        };
        // Timings only: no figures, so no unit to show beside them.
        if self.threshold.is_some() || self.light.is_some() || self.common.is_some() || self.strong.is_some() || self.heavy.is_some() {
            self.set_bands(&mut roa);
        }
        self.stages.set(&mut roa);
        roa
    }

    fn set_bands(&self, roa: &mut PwRoa) {
        roa.units = Some(self.units.to_string());
        roa.threshold = self.threshold;
        roa.light = range_of(self.light);
        roa.common = range_of(self.common);
        roa.strong = range_of(self.strong);
        roa.heavy = self.heavy.map(|h| h.0);
        roa.heavy_max = self.heavy.and_then(|h| h.1);
    }
}

impl Stages {
    /// Replaces every timing, so one the source doesn't give is cleared rather than
    /// left as the wrong figure it replaces.
    fn set(&self, roa: &mut PwRoa) {
        let s = |x: Option<&str>| x.map(String::from);
        roa.onset = s(self.onset);
        roa.come_up = s(self.come_up);
        roa.peak = s(self.peak);
        roa.offset = s(self.offset);
        roa.after_effects = s(self.after_effects);
        roa.total = s(self.total);
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
            name: "vaporized", threshold: Some(1.0),
            light: Some((2.0, 5.0)), common: Some((5.0, 10.0)), strong: Some((10.0, 20.0)),
            stages: Stages { onset: Some("0–30 seconds"), peak: Some("1–15 minutes"), after_effects: Some("1 hour"), total: Some("30 minutes"), ..NO_STAGES },
            ..BLANK
        },
        RouteSpec {
            name: "insufflated", threshold: Some(3.0),
            light: Some((5.0, 10.0)), common: Some((8.0, 15.0)), strong: Some((10.0, 25.0)),
            stages: Stages { onset: Some("5 minutes"), peak: Some("10–30 minutes"), after_effects: Some("1–3 hours"), total: Some("30–45 minutes"), ..NO_STAGES },
            ..BLANK
        },
        RouteSpec {
            name: "IM",
            light: Some((0.5, 1.0)), common: Some((1.5, 3.0)), strong: Some((5.0, 7.0)), heavy: Some((8.0, Some(12.0))),
            ..BLANK
        },
    ],
)];

/// A change to one route, keeping the rest of the substance's entry.
enum Patch {
    /// Put this route in place of the reference's, or add it.
    Route(RouteSpec),
    /// Replace the route's dose bands, keeping its timings.
    Bands(&'static str, RouteSpec),
    /// Replace every timing on the route, keeping its dose bands.
    Stages(&'static str, Stages),
    /// Remove a route the substance isn't taken by.
    Drop(&'static str),
}

/// Erowid's figures where they disagree with DoseWiki, by the owner's rule:
/// Erowid wins, but each disagreement is decided by the owner first (ROADMAP,
/// "Dose reference review"). Checked against Erowid's pages as saved on
/// 2026-10-07 (`data/sources/erowid/`). Erowid gives a threshold as a range; the
/// low end is used, as for 5-MeO-DMT. Opioid figures assume no tolerance.
/// (name, where the figures come from, patches)
const ROUTE_PATCHES: &[(&str, &str, &[Patch])] = &[
    (
        "DMT",
        "Smoked/vaporized figures from Erowid (checked 2026-10-07). Not orally active without an MAOI. Insufflated and IV figures are DoseWiki's.",
        &[
            Patch::Route(RouteSpec {
                name: "vaporized", threshold: Some(2.0),
                light: Some((10.0, 20.0)), common: Some((20.0, 40.0)), strong: Some((40.0, 60.0)),
                stages: Stages {
                    onset: Some("0–1 minutes"), come_up: Some("0–30 seconds"), peak: Some("3–15 minutes"),
                    offset: Some("3–5 minutes"), after_effects: Some("15–60 minutes"), total: Some("6–20 minutes"),
                },
                ..BLANK
            }),
            Patch::Drop("oral"),
        ],
    ),
    (
        "Ketamine",
        "Oral doses and all timings from Erowid (checked 2026-10-07); its other doses agree with Erowid. Heavy is Erowid's “K-hole” range.",
        &[
            Patch::Route(RouteSpec {
                name: "oral", threshold: Some(40.0),
                light: Some((50.0, 100.0)), common: Some((75.0, 300.0)), strong: Some((200.0, 450.0)), heavy: Some((500.0, None)),
                stages: Stages { onset: Some("5–20 minutes"), after_effects: Some("4–8 hours"), total: Some("90 minutes"), ..NO_STAGES },
                ..BLANK
            }),
            Patch::Stages("insufflated", Stages {
                onset: Some("5–15 minutes"), come_up: Some("10–30 minutes"), peak: Some("20–40 minutes"),
                offset: Some("30–60 minutes"), after_effects: Some("1–3 hours"), total: Some("45–60 minutes"),
            }),
            // DoseWiki's IM and rectal timings were each other's.
            Patch::Stages("IM", Stages {
                onset: Some("1–5 minutes"), come_up: Some("5–20 minutes"), peak: Some("20–45 minutes"),
                offset: Some("20–60 minutes"), after_effects: Some("2–4 hours"), total: Some("30–60 minutes"),
            }),
            Patch::Stages("rectal", Stages {
                onset: Some("5–20 minutes"), peak: Some("30–60 minutes"), after_effects: Some("1–4 hours"), total: Some("1–2 hours"), ..NO_STAGES
            }),
        ],
    ),
    (
        "Heroin",
        "Doses assume no opioid tolerance. Oral range and all timings from Erowid (checked 2026-10-07); other dose bands are DoseWiki's.",
        &[
            Patch::Bands("oral", RouteSpec { common: Some((50.0, 70.0)), ..BLANK }),
            // Erowid's "10 to 30 minutes to peak" is the time to reach it, so it's
            // the come-up; no peak length is given, so none is guessed.
            Patch::Stages("insufflated", Stages { onset: Some("30–120 seconds"), come_up: Some("10–30 minutes"), total: Some("3–5 hours"), ..NO_STAGES }),
            Patch::Stages("IV", Stages { onset: Some("10–20 seconds"), total: Some("4–5 hours"), ..NO_STAGES }),
            Patch::Stages("vaporized", Stages {
                onset: Some("5–10 seconds"), come_up: Some("10 minutes"), peak: Some("5–6 minutes"), total: Some("3–5 hours"), ..NO_STAGES
            }),
        ],
    ),
    (
        "Oxycodone",
        "Oral and insufflated figures from Erowid (checked 2026-10-16), assuming no opioid tolerance; Erowid gives no heavy band. IV and smoked figures are DoseWiki's.",
        &[
            // DoseWiki called 20 mg oral or 12 mg snorted "common"; for someone
            // without tolerance Erowid has both as strong.
            Patch::Route(RouteSpec {
                name: "oral",
                light: Some((2.5, 5.0)), common: Some((5.0, 10.0)), strong: Some((10.0, 30.0)),
                // Erowid's plateau (2–4 h) is the app's peak length.
                stages: Stages { onset: Some("10–30 minutes"), come_up: Some("30–60 minutes"), peak: Some("2–4 hours"), total: Some("3–6 hours"), ..NO_STAGES },
                ..BLANK
            }),
            // "Peak: 10–30 minutes" is the time to reach it, as for heroin.
            Patch::Route(RouteSpec {
                name: "insufflated",
                common: Some((2.5, 5.0)), strong: Some((5.0, 10.0)),
                stages: Stages { onset: Some("1–15 minutes"), come_up: Some("10–30 minutes"), total: Some("3–5 hours"), ..NO_STAGES },
                ..BLANK
            }),
        ],
    ),
    (
        "Psilocybin Mushrooms",
        "Timings from Erowid (checked 2026-10-07); dose ranges agree with Erowid's for dried P. cubensis.",
        &[Patch::Stages("oral", Stages {
            onset: Some("15–60 minutes"), come_up: Some("15–30 minutes"), peak: Some("2–4 hours"),
            offset: Some("1–3 hours"), after_effects: Some("0–6 hours"), total: Some("4–7 hours"),
        })],
    ),
    (
        "Dextromethorphan",
        "Dose ranges and duration from Erowid (checked 2026-10-07), which lists 2,500–20,000 mg as a risk-of-death range. The duration is for DXM HBr; extended-release (polistirex) lasts 6–12 hours.",
        &[
            Patch::Bands("oral", RouteSpec {
                light: Some((100.0, 200.0)), common: Some((200.0, 400.0)), strong: Some((300.0, 600.0)), heavy: Some((600.0, Some(1500.0))),
                ..BLANK
            }),
            // Erowid gives only the total; DoseWiki's onset and peak (up to 2 h and
            // 6 h) don't fit inside it, so they go too.
            Patch::Stages("oral", Stages { total: Some("4–8 hours"), ..NO_STAGES }),
        ],
    ),
    // Reference suggestions from issue #23 (2026-10-16), decided by the owner
    // 2026-10-23. Erowid's pages couldn't be checked for these; the drug label and
    // a human PK study were.
    (
        "Zolpidem",
        "Oral timings from the Field Notes maintainer's own use, not a study. They fit the label's pharmacokinetics (blood levels peak 1.4–1.6 hours after a tablet, half-life about 2.5 hours), but the label also warns that driving and other tasks can be impaired the next morning, especially after a full dose or with less than 7–8 hours in bed. Dose ranges are DoseWiki's.",
        &[Patch::Stages("oral", Stages {
            onset: Some("15 minutes"), come_up: Some("10 minutes"), peak: Some("1–2 hours"),
            offset: Some("2 hours"), after_effects: Some("1–2 hours"), total: Some("4–6 hours"),
        })],
    ),
    (
        "Kratom",
        "Mitragynine, kratom's main alkaloid, peaks in the blood about 1–1.3 hours after a dose of leaf powder (Huestis et al. 2024, a placebo-controlled human study), so the come-up is set to reach that. Onset, total duration and after-effects are DoseWiki's; how long the peak lasts isn't known well enough to give. Products vary a lot in strength. Dose ranges are DoseWiki's.",
        // Come-up = time to peak plasma (60–80 min) less DoseWiki's onset
        // (10–40 min), so onset and come-up together reach the measured peak.
        &[Patch::Stages("oral", Stages {
            onset: Some("10–40 minutes"), come_up: Some("20–70 minutes"),
            after_effects: Some("1–12 hours"), total: Some("2–5 hours"), ..NO_STAGES
        })],
    ),
    (
        "1,4-Butanediol",
        "How it builds, peaks and wears off is from the Field Notes maintainer's own use, not a study. The total and after-effects are DoseWiki's longer figures, kept so the line doesn't end early: the body turns 1,4-butanediol into GHB, more slowly with alcohol, and redosing too soon is how GHB overdoses usually happen. Dose ranges are DoseWiki's.",
        &[Patch::Stages("oral", Stages {
            onset: Some("10 minutes"), come_up: Some("15 minutes"), peak: Some("1 hour"),
            offset: Some("1 hour"), after_effects: Some("2–4 hours"), total: Some("3–5 hours"),
        })],
    ),
    // Reference suggestions from issues #32 and #33 (2026-10-28), decided by the
    // owner the same day. The rest of those two issues repeated #23, already done.
    (
        "Mescaline",
        "How long it lasts in total is from Klaiber et al. 2024, a placebo-controlled study of 100–800 mg mescaline hydrochloride in healthy people: effects lasted 6.4 to 14 hours, longer at higher doses, and blood levels peaked about 2 hours after a dose. How it builds up and wears off is from the Field Notes maintainer's own use of San Pedro powder, not a study. Cactus varies a lot in how much mescaline it holds, so a weight of cactus can't be read against these mg figures. Onset, peak, after-effects and dose ranges are DoseWiki's.",
        // Onset, peak and after-effects as DoseWiki has them: `Stages::set` replaces all six.
        &[Patch::Stages("oral", Stages {
            onset: Some("60–180 minutes"), come_up: Some("60 minutes"), peak: Some("1.9–4 hours"),
            offset: Some("3–6 hours"), after_effects: Some("3–5 hours"), total: Some("6–14 hours"),
        })],
    ),
];

/// A substance DoseWiki has no entry for, added on load (see [`EXTRA_SUBSTANCES`]).
struct ExtraSubstance {
    name: &'static str,
    aliases: &'static [&'static str],
    /// The entry whose classes and interactions it shares, if there is one.
    like: Option<&'static str>,
    /// Its classes, when there's no entry to share them with.
    psychoactive: &'static [&'static str],
    chemical: &'static [&'static str],
    note: &'static str,
    routes: &'static [RouteSpec],
}

/// Substances DoseWiki has no entry for, added on load.
///
/// Pure psilocybin: DoseWiki has psilocin and mushrooms (in grams) but nothing for
/// psilocybin itself in mg, the way trials and pharmaceutical products dose it.
/// It's the same drug as in the mushrooms, so it shares their interactions.
///
/// SR-17018 (owner's request, 2026-10-16): a mu-opioid agonist sold as a research
/// chemical, often to come off other opioids. No human studies, so no dose
/// figures; the oral timings are the owner's, from three doses of their own, and
/// three doses from one person can't support a range for a novel opioid. Sources
/// for the note: Schmid et al. 2017, Cell 171:1165 (in mice, less respiratory
/// depression than morphine at pain-relieving doses); Gillis et al. 2020, Sci.
/// Signal. 13:eaaz3140 (explained by low intrinsic efficacy rather than
/// signalling bias); Grim et al. 2020 (blocked morphine withdrawal in mice); NDEWS
/// web monitoring, 2025-11-14 (people's reports: slow onset, very different doses
/// from person to person, redosing, and overdose on going back to other opioids
/// after tolerance fell).
///
/// Syrian rue (issue #33, owner's decision 2026-10-28): Peganum harmala seed, an
/// MAOI-A from its harmala alkaloids (harmine, harmaline), taken as a tea or with
/// DMT as an ayahuasca analogue. No human timing or dose studies were found, so
/// no dose figures; the oral timings are the owner's. It shares Ayahuasca's
/// interactions, which come from the same harmala alkaloids.
const EXTRA_SUBSTANCES: &[ExtraSubstance] = &[ExtraSubstance {
    name: "Psilocybin",
    aliases: &["pure psilocybin", "synthetic psilocybin", "COMP360"],
    like: Some("Psilocybin Mushrooms"),
    psychoactive: &[],
    chemical: &[],
    note: "Doses and timings from Erowid's psilocybin page (checked 2026-10-07), for pure psilocybin in mg, not mushrooms. Trial doses for reference: 22 or 30 mg/70 kg (Griffiths 2016), 0.3 mg/kg (Ross 2016).",
    routes: &[RouteSpec {
        name: "oral", threshold: Some(3.0),
        light: Some((4.0, 8.0)), common: Some((6.0, 20.0)), strong: Some((20.0, 40.0)), heavy: Some((35.0, None)),
        stages: Stages {
            onset: Some("30–60 minutes"), peak: Some("90–180 minutes"), after_effects: Some("up to 8 hours"), total: Some("2–6 hours"),
            ..NO_STAGES
        },
        ..BLANK
    }],
}, ExtraSubstance {
    name: "SR-17018",
    aliases: &["SR17018", "SR 17018"],
    like: None,
    psychoactive: &["Opioid"],
    chemical: &["Benzimidazolone"],
    note: "No dose figures: SR-17018 hasn't been studied in people. Its oral timings are from the Field Notes maintainer's own use, not a study, and others report a slower onset, which can lead to redosing. It's a mu-opioid agonist. In mice it slowed breathing less than morphine, likely because it activates the receptor only weakly, and it blocked morphine withdrawal. People who use it to come off other opioids report very different doses from one person to the next. Treat it as an opioid in combinations. If it lowers your tolerance, a dose of another opioid you used to take can become an overdose.",
    routes: &[RouteSpec {
        name: "oral",
        stages: Stages {
            onset: Some("20 minutes"), come_up: Some("30 minutes"), peak: Some("2–3 hours"), offset: Some("2–3 hours"), total: Some("6 hours"),
            ..NO_STAGES
        },
        ..BLANK
    }],
}, ExtraSubstance {
    name: "Syrian Rue",
    aliases: &["Syrian Rue Tea", "Peganum harmala", "harmal", "harmala", "esfand", "espand"],
    like: Some("Ayahuasca"),
    psychoactive: &[],
    chemical: &[],
    note: "No dose figures: Syrian rue seed varies in strength and hasn't been dosed in human studies. Its oral timings are from the Field Notes maintainer's own use, not a study. Its harmala alkaloids are an MAOI, so it shares Ayahuasca's interactions, and the MAOI effect lasts longer than what you feel: allow a day or more before anything that interacts with an MAOI.",
    routes: &[RouteSpec {
        name: "oral",
        stages: Stages {
            onset: Some("15 minutes"), come_up: Some("30 minutes"), peak: Some("1–2 hours"), offset: Some("3 hours"), total: Some("6–8 hours"),
            ..NO_STAGES
        },
        ..BLANK
    }],
}];

fn add_extra_substances(infos: &mut Vec<PwInfo>, log: &mut Vec<String>) {
    for ExtraSubstance { name, aliases, like, psychoactive, chemical, note, routes } in EXTRA_SUBSTANCES {
        if infos.iter().any(|i| i.name.eq_ignore_ascii_case(name)) {
            log.push(format!("{name}: not added, the reference now has its own entry; check it against EXTRA_SUBSTANCES"));
            continue;
        }
        let base = like.and_then(|l| infos.iter().find(|i| i.name.eq_ignore_ascii_case(l)));
        let own = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        infos.push(PwInfo {
            name: name.to_string(),
            common_names: aliases.iter().map(|a| a.to_string()).collect(),
            psychoactive: base.map(|b| b.psychoactive.clone()).unwrap_or_else(|| own(psychoactive)),
            chemical: base.map(|b| b.chemical.clone()).unwrap_or_else(|| own(chemical)),
            roas: routes.iter().map(RouteSpec::roa).collect(),
            interactions: base.map(|b| b.interactions.clone()).unwrap_or_default(),
            dose_note: Some(note.to_string()),
            mine: false,
        });
        log.push(match like {
            Some(like) => format!("{name}: added (see EXTRA_SUBSTANCES), sharing {like}'s interactions"),
            None => format!("{name}: added (see EXTRA_SUBSTANCES)"),
        });
    }
}

fn apply_patches(info: &mut PwInfo, log: &mut Vec<String>) {
    let Some((_, note, patches)) = ROUTE_PATCHES.iter().find(|(n, ..)| n.eq_ignore_ascii_case(&info.name)) else { return };
    let name = info.name.clone();
    for p in patches.iter() {
        let route = match p {
            Patch::Route(spec) => spec.name,
            Patch::Bands(r, _) | Patch::Stages(r, _) | Patch::Drop(r) => r,
        };
        let at = info.roas.iter().position(|x| x.name == route);
        match (p, at) {
            (Patch::Route(spec), Some(i)) => info.roas[i] = spec.roa(),
            (Patch::Route(spec), None) => info.roas.push(spec.roa()),
            (Patch::Bands(_, spec), Some(i)) => spec.set_bands(&mut info.roas[i]),
            (Patch::Stages(_, st), Some(i)) => st.set(&mut info.roas[i]),
            (Patch::Drop(_), Some(i)) => {
                info.roas.remove(i);
            }
            (_, None) => {
                log.push(format!("{name} [{route}]: patch not applied, the reference has no such route"));
                continue;
            }
        }
        let what = match p {
            Patch::Route(_) => "replaced",
            Patch::Bands(..) => "dose bands replaced",
            Patch::Stages(..) => "timings replaced",
            Patch::Drop(_) => "removed",
        };
        log.push(format!("{name} [{route}]: {what} (see ROUTE_PATCHES)"));
    }
    info.dose_note = Some(note.to_string());
}

/// Parse the slimmed DoseWiki JSON (the bundled `dosewiki.json`) into our shape.
pub fn parse_slim(json: &str) -> Result<Vec<PwInfo>, String> {
    parse_slim_logged(json).map(|(subs, _)| subs)
}

/// [`parse_slim`], plus one line for every change made to DoseWiki's figures.
pub fn parse_slim_logged(json: &str) -> Result<(Vec<PwInfo>, Vec<String>), String> {
    let subs: Vec<DwSub> =
        serde_json::from_str(json).map_err(|e| format!("Couldn't parse the bundled dose reference: {e}"))?;
    let mut log = Vec::new();
    let mut infos: Vec<PwInfo> = subs.into_iter().map(|s| map_sub(s, &mut log)).collect();
    add_extra_substances(&mut infos, &mut log);
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
    fn every_patch_finds_its_route() {
        let (_, log) = parse_slim_logged(include_str!("../../resources/dosewiki.json")).unwrap();
        let missed: Vec<&String> = log.iter().filter(|l| l.contains("patch not applied")).collect();
        assert!(missed.is_empty(), "{missed:?}");
    }

    #[test]
    fn erowid_patches_are_applied() {
        let subs = bundled();
        let dmt = subs.iter().find(|s| s.name == "DMT").unwrap();
        assert!(dmt.roas.iter().all(|r| r.name != "oral"), "DMT isn't orally active alone");
        let v = route(&subs, "DMT", "vaporized");
        assert_eq!((v.threshold, v.common.min, v.strong.max, v.heavy), (Some(2.0), Some(20.0), Some(60.0), None));
        assert!(dmt.dose_note.as_deref().unwrap().contains("Erowid"));

        let k = route(&subs, "Ketamine", "oral");
        assert_eq!((k.threshold, k.heavy, k.onset.as_deref()), (Some(40.0), Some(500.0), Some("5–20 minutes")));
        let ki = route(&subs, "Ketamine", "insufflated");
        assert_eq!((ki.peak.as_deref(), ki.total.as_deref()), (Some("20–40 minutes"), Some("45–60 minutes")));
        assert_eq!(ki.heavy, Some(100.0), "DoseWiki's insufflated bands are kept");

        let h = route(&subs, "Heroin", "oral");
        assert_eq!((h.common.min, h.strong.min, h.heavy), (Some(50.0), None, None));
        assert!(h.onset.is_some(), "a bands patch keeps the timings");
        let iv = route(&subs, "Heroin", "IV");
        assert_eq!((iv.peak.as_deref(), iv.onset.as_deref()), (None, Some("10–20 seconds")));
        assert_eq!(iv.common.min, Some(5.0), "a timings patch keeps the bands");

        let d = route(&subs, "Dextromethorphan", "oral");
        assert_eq!((d.threshold, d.heavy, d.heavy_max), (None, Some(600.0), Some(1500.0)));
        assert_eq!(route(&subs, "Psilocybin Mushrooms", "oral").come_up.as_deref(), Some("15–30 minutes"));

        let o = route(&subs, "Oxycodone", "oral");
        assert_eq!((o.threshold, o.strong.min, o.strong.max, o.heavy), (None, Some(10.0), Some(30.0), None));
        assert_eq!(route(&subs, "Oxycodone", "insufflated").strong.min, Some(5.0));
        assert!(route(&subs, "Oxycodone", "IV").common.min.is_some(), "routes Erowid lacks are kept");

        let im = route(&subs, "Ketamine", "IM");
        assert_eq!((im.onset.as_deref(), im.total.as_deref()), (Some("1–5 minutes"), Some("30–60 minutes")));
        let rec = route(&subs, "Ketamine", "rectal");
        assert_eq!((rec.onset.as_deref(), rec.total.as_deref()), (Some("5–20 minutes"), Some("1–2 hours")));
        let d = route(&subs, "Dextromethorphan", "oral");
        assert_eq!((d.total.as_deref(), d.peak.as_deref()), (Some("4–8 hours"), None));
    }

    /// Issues #32 and #33's new suggestions, as the owner decided them.
    #[test]
    fn second_batch_of_suggestions_is_applied() {
        let subs = bundled();
        let reference: Vec<(String, Vec<String>)> = subs.iter().map(|s| (s.name.clone(), s.common_names.clone())).collect();
        let names = crate::names::NameIndex::new(&[], &reference);
        let note = |n: &str| subs.iter().find(|s| s.name == n).unwrap().dose_note.clone().unwrap();

        let m = route(&subs, "Mescaline", "oral");
        assert_eq!((m.come_up.as_deref(), m.offset.as_deref()), (Some("60 minutes"), Some("3–6 hours")));
        assert_eq!(m.total.as_deref(), Some("6–14 hours"), "Klaiber 2024's range");
        assert_eq!(m.onset.as_deref(), Some("60–180 minutes"), "DoseWiki's onset is kept");
        assert!(note("Mescaline").contains("Klaiber") && note("Mescaline").contains("maintainer's own use"));
        assert_eq!(names.canonical("San Pedro Powder").as_deref(), Some("Mescaline"));

        let r = route(&subs, "Syrian Rue", "oral");
        assert_eq!((r.peak.as_deref(), r.total.as_deref()), (Some("1–2 hours"), Some("6–8 hours")));
        assert_eq!((r.threshold, r.common.min, r.heavy), (None, None, None), "no dose figures without a study");
        let rue = subs.iter().find(|s| s.name == "Syrian Rue").unwrap();
        let aya = subs.iter().find(|s| s.name == "Ayahuasca").unwrap();
        let named = |s: &PwInfo| s.interactions.iter().map(|i| (i.name.clone(), i.severity.clone())).collect::<Vec<_>>();
        assert!(!rue.interactions.is_empty());
        assert_eq!(named(rue), named(aya), "shares Ayahuasca's MAOI interactions");
        assert_eq!(names.canonical("Syrian Rue Tea").as_deref(), Some("Syrian Rue"));
        assert!(note("Syrian Rue").contains("MAOI"));
    }

    /// Issue #23's suggestions, as the owner decided them.
    #[test]
    fn suggested_timings_are_applied() {
        let subs = bundled();
        let note = |n: &str| subs.iter().find(|s| s.name == n).unwrap().dose_note.clone().unwrap();

        let z = route(&subs, "Zolpidem", "oral");
        assert_eq!((z.come_up.as_deref(), z.total.as_deref()), (Some("10 minutes"), Some("4–6 hours")));
        assert_eq!(z.common.max, Some(10.0), "DoseWiki's bands are kept");
        assert!(note("Zolpidem").contains("maintainer's own use") && note("Zolpidem").contains("next morning"));

        let k = route(&subs, "Kratom", "oral");
        assert_eq!((k.onset.as_deref(), k.come_up.as_deref()), (Some("10–40 minutes"), Some("20–70 minutes")));
        assert_eq!((k.peak.as_deref(), k.offset.as_deref()), (None, None), "no source for how long it peaks");
        assert!(note("Kratom").contains("Huestis"));

        let b = route(&subs, "1,4-Butanediol", "oral");
        assert_eq!((b.peak.as_deref(), b.total.as_deref()), (Some("1 hour"), Some("3–5 hours")), "DoseWiki's longer total is kept");
        assert!(note("1,4-Butanediol").contains("redosing"));
    }

    #[test]
    fn pure_psilocybin_is_its_own_entry_in_mg() {
        let subs = bundled();
        assert_eq!(subs.iter().filter(|s| s.name == "Psilocybin").count(), 1);
        let p = route(&subs, "Psilocybin", "oral");
        assert_eq!((p.units.as_deref(), p.threshold, p.common.max, p.heavy), (Some("mg"), Some(3.0), Some(20.0), Some(35.0)));
        let ps = subs.iter().find(|s| s.name == "Psilocybin").unwrap();
        let mush = subs.iter().find(|s| s.name == "Psilocybin Mushrooms").unwrap();
        assert_eq!(ps.interactions.len(), mush.interactions.len(), "shares the mushrooms' interactions");
        assert!(ps.interactions.iter().any(|i| i.name == "Lithium" && i.severity == "danger"));
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
    fn adds_substances_dosewiki_lacks() {
        let subs = parse_slim(include_str!("../../resources/dosewiki.json")).unwrap();
        let s = subs.iter().find(|s| s.name == "SR-17018").expect("SR-17018 is in the directory");
        assert_eq!(s.psychoactive, ["Opioid"]);
        assert!(s.common_names.iter().any(|a| a == "SR17018"));
        let oral = &s.roas[..];
        assert_eq!(oral.len(), 1);
        assert_eq!((oral[0].name.as_str(), oral[0].total.as_deref()), ("oral", Some("6 hours")));
        assert_eq!(oral[0].onset.as_deref(), Some("20 minutes"));
        let r = &oral[0];
        assert!(r.units.is_none() && r.threshold.is_none() && r.common.min.is_none() && r.heavy.is_none(), "no dose figures");
        let note = s.dose_note.as_deref().unwrap();
        assert!(note.contains("hasn't been studied in people") && note.contains("maintainer's own use"));
        assert_eq!(subs.iter().filter(|s| s.name == "SR-17018").count(), 1);
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
