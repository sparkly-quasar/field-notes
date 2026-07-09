// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Dose reference importer. Reads a bundled snapshot of the DoseWiki substance
//! encyclopedia (dose ranges, durations, graded interactions) and maps it into
//! the local reference cache. The data ships *with* the app as an offline
//! resource, so there is no network call and every lookup is private.
//!
//! DoseWiki content is dedicated to the public domain under CC0 (the site code is
//! MIT); no attribution is legally required. We credit DoseWiki in-app as a
//! courtesy. See `data/dosewiki/README.md` for the snapshot + slimming pipeline.

use serde::{Deserialize, Serialize};

/// Date of the bundled DoseWiki snapshot (see `data/dosewiki/slim.py`). Bump this
/// whenever `resources/dosewiki.json` is regenerated from a fresh download.
pub const DOSEWIKI_SNAPSHOT: &str = "2026-07-08";

/// Path of the bundled reference file, relative to the Tauri resource dir.
const RESOURCE_PATH: &str = "resources/dosewiki.json";

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
        .find_map(|r| r.unit.clone().filter(|u| !u.is_empty()))
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

fn map_sub(s: DwSub) -> PwInfo {
    let roas = s
        .routes
        .into_iter()
        .map(|r| {
            let d = &r.dose_ranges;
            PwRoa {
                name: r.route,
                units: route_units(d),
                threshold: d.threshold.as_ref().and_then(|t| t.min),
                light: range(&d.light),
                common: range(&d.moderate), // DoseWiki calls our "common" tier "moderate"
                strong: range(&d.strong),
                heavy: d.heavy.as_ref().and_then(|h| h.min),
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

    PwInfo {
        name: s.title,
        common_names: s.alternative_names,
        psychoactive: s.psychoactive_class,
        chemical: s.chemical_class,
        roas,
        interactions,
    }
}

/// Parse the slimmed DoseWiki JSON (the bundled `dosewiki.json`) into our shape.
pub fn parse_slim(json: &str) -> Result<Vec<PwInfo>, String> {
    let subs: Vec<DwSub> =
        serde_json::from_str(json).map_err(|e| format!("Couldn't parse the bundled dose reference: {e}"))?;
    Ok(subs.into_iter().map(map_sub).collect())
}

/// Load the whole bundled reference from the app's resource directory.
pub fn load_bundled(app: &tauri::AppHandle) -> Result<Vec<PwInfo>, String> {
    use tauri::Manager;
    let path = app
        .path()
        .resolve(RESOURCE_PATH, tauri::path::BaseDirectory::Resource)
        .map_err(|e| format!("Couldn't locate the bundled dose reference: {e}"))?;
    let json = std::fs::read_to_string(&path)
        .map_err(|e| format!("Couldn't read the bundled dose reference at {}: {e}", path.display()))?;
    parse_slim(&json)
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
    fn parses_bundled_snapshot() {
        // The committed resource must parse and carry dose + interaction data.
        let json = include_str!("../resources/dosewiki.json");
        let subs = parse_slim(json).expect("parse bundled dosewiki.json");
        assert!(subs.len() > 500, "expected hundreds of substances, got {}", subs.len());
        assert!(subs.iter().any(|s| !s.roas.is_empty()), "expected some dose data");
        let graded: Vec<&PwInteraction> = subs.iter().flat_map(|s| &s.interactions).collect();
        assert!(graded.iter().any(|i| i.severity == "danger" || i.severity == "note"),
            "expected graded interactions");
        assert!(graded.iter().any(|i| i.reason.is_some()), "expected interaction reasons");
    }
}
