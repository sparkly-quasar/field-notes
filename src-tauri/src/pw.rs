// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! PsychonautWiki reference importer. Fetches substance data (dose ranges,
//! durations, dangerous interactions) from the PsychonautWiki GraphQL API ONCE,
//! on explicit user request, and caches it locally so all later lookups are
//! offline and private. Nothing is queried live per-lookup.
//!
//! PsychonautWiki content is licensed CC-BY-SA 4.0; cached data carries that
//! license and attribution (see NOTICE), separate from this app's license.

use serde::{Deserialize, Serialize};

const ENDPOINT: &str = "https://api.psychonautwiki.org/";

// ---- GraphQL response shapes ----

#[derive(Deserialize)]
struct GqlResp {
    data: Option<GqlData>,
}
#[derive(Deserialize)]
struct GqlData {
    substances: Option<Vec<GqlSub>>,
}
#[derive(Deserialize)]
struct GqlSub {
    name: String,
    #[serde(rename = "commonNames")]
    common_names: Option<Vec<String>>,
    class: Option<GqlClass>,
    roas: Option<Vec<GqlRoa>>,
    #[serde(rename = "dangerousInteractions")]
    dangerous: Option<Vec<GqlNamed>>,
}
#[derive(Deserialize)]
struct GqlClass {
    psychoactive: Option<Vec<String>>,
    chemical: Option<Vec<String>>,
}
#[derive(Deserialize)]
struct GqlNamed {
    name: Option<String>,
}
#[derive(Deserialize)]
struct GqlRoa {
    name: Option<String>,
    dose: Option<GqlDose>,
    duration: Option<GqlDuration>,
}
#[derive(Deserialize)]
struct GqlDose {
    units: Option<String>,
    threshold: Option<f64>,
    light: Option<GqlRange>,
    common: Option<GqlRange>,
    strong: Option<GqlRange>,
    heavy: Option<f64>,
}
#[derive(Deserialize)]
struct GqlRange {
    min: Option<f64>,
    max: Option<f64>,
}
#[derive(Deserialize)]
struct GqlDuration {
    onset: Option<GqlDur>,
    total: Option<GqlDur>,
}
#[derive(Deserialize)]
struct GqlDur {
    min: Option<f64>,
    max: Option<f64>,
    units: Option<String>,
}

// ---- stored / exposed shape ----

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Range {
    pub min: Option<f64>,
    pub max: Option<f64>,
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
    pub total: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PwInfo {
    pub name: String,
    pub common_names: Vec<String>,
    pub psychoactive: Vec<String>,
    pub chemical: Vec<String>,
    pub roas: Vec<PwRoa>,
    pub interactions: Vec<String>,
}

fn range(g: Option<GqlRange>) -> Range {
    g.map(|r| Range { min: r.min, max: r.max }).unwrap_or_default()
}

fn fmt_dur(g: Option<GqlDur>) -> Option<String> {
    let d = g?;
    let units = d.units.unwrap_or_default();
    match (d.min, d.max) {
        (Some(a), Some(b)) if (a - b).abs() > f64::EPSILON => Some(format!("{a}–{b} {units}")),
        (Some(a), _) | (_, Some(a)) => Some(format!("{a} {units}")),
        _ => None,
    }
}

fn map_sub(s: GqlSub) -> PwInfo {
    let (psychoactive, chemical) = s
        .class
        .map(|c| (c.psychoactive.unwrap_or_default(), c.chemical.unwrap_or_default()))
        .unwrap_or_default();
    let roas = s
        .roas
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            let dose = r.dose;
            let (units, threshold, light, common, strong, heavy) = match dose {
                Some(d) => (d.units, d.threshold, range(d.light), range(d.common), range(d.strong), d.heavy),
                None => (None, None, Range::default(), Range::default(), Range::default(), None),
            };
            let (onset, total) = r.duration.map(|d| (fmt_dur(d.onset), fmt_dur(d.total))).unwrap_or((None, None));
            PwRoa { name: r.name.unwrap_or_default(), units, threshold, light, common, strong, heavy, onset, total }
        })
        .collect();
    PwInfo {
        name: s.name,
        common_names: s.common_names.unwrap_or_default(),
        psychoactive,
        chemical,
        roas,
        interactions: s
            .dangerous
            .unwrap_or_default()
            .into_iter()
            .filter_map(|n| n.name)
            .collect(),
    }
}

/// Fetch the full substance list from PsychonautWiki (one network call).
pub fn fetch_all() -> Result<Vec<PwInfo>, String> {
    let query = "{ substances(limit: 2000) { name commonNames class { psychoactive chemical } \
        roas { name dose { units threshold light { min max } common { min max } strong { min max } heavy } \
        duration { onset { min max units } total { min max units } } } dangerousInteractions { name } } }";
    let resp = ureq::post(ENDPOINT)
        .send_json(serde_json::json!({ "query": query }))
        .map_err(|e| format!("Couldn't reach PsychonautWiki: {e}"))?;
    let parsed: GqlResp = resp
        .into_json()
        .map_err(|e| format!("Bad response from PsychonautWiki: {e}"))?;
    let subs = parsed
        .data
        .and_then(|d| d.substances)
        .ok_or_else(|| "PsychonautWiki returned no substances".to_string())?;
    Ok(subs.into_iter().map(map_sub).collect())
}
