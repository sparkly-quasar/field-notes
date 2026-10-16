// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Dose profiles (dose-aware Stats, step 5 in ROADMAP.md): what an amount of a
//! substance tends to do, for the few whose effects change with the amount.
//! Field Notes' own small table, owner-approved 2026-10-05:
//!
//! | Substance | By amount |
//! |---|---|
//! | Kratom (leaf) | under about 3 g more stimulating; 3 to 5 g mixed; over 5 g more opioid-like and sedating |
//! | DXM | plateaus 1 to 4, from the reference's light, common, strong and heavy ranges |
//! | Diphenhydramine | below the reference's common range, the sleep-aid range; from it, the deliriant range |
//! | Ketamine | below dissociative, dissociative, and from the strong range, a "hole", per route |
//!
//! A profile is a label, never a reason to worry less: the checker only ever
//! *adds* context from one (see `db::log_dose`). Kratom extract and 7-OH
//! products get no profile, since leaf's amounts don't apply to them.
//!
//! The cautions' wording is to be checked by a clinician before release, with
//! the dependence notes (ROADMAP.md).

use crate::check::Reference;
use crate::pw::PwRoa;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Profile {
    /// Stable name: "stimulating", "mixed", "opioid", "plateau1".."plateau4",
    /// "sleep", "deliriant", "subdissociative", "dissociative", "hole".
    pub key: String,
    /// Short, for beside the dose: "more stimulating at this amount".
    pub label: String,
    /// A sentence saying what the amount tends to do.
    pub note: String,
    /// A safety note, for the amounts where that matters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caution: Option<String>,
}

fn p(key: &str, label: &str, note: &str, caution: Option<&str>) -> Profile {
    Profile { key: key.into(), label: label.into(), note: note.into(), caution: caution.map(String::from) }
}

/// Grams in `amount` of `unit`, for units of mass.
fn grams(amount: f64, unit: &str) -> Option<f64> {
    match unit.trim().to_lowercase().as_str() {
        "g" | "gram" | "grams" => Some(amount),
        "mg" | "milligram" | "milligrams" => Some(amount / 1_000.0),
        "µg" | "μg" | "ug" | "mcg" => Some(amount / 1_000_000.0),
        _ => None,
    }
}

/// The reference's ranges for a route, in `unit`: the route's own when listed,
/// else the only one in a unit that converts.
fn ranges(r: &impl Reference, name: &str, route: &str, unit: &str) -> Option<(PwRoa, f64)> {
    let info = r.lookup(name)?;
    let scale = |roa: &PwRoa| -> Option<f64> {
        let ru = roa.units.as_deref()?;
        // How many of `unit` one of the reference's unit is.
        Some(grams(1.0, ru)? / grams(1.0, unit)?)
    };
    let fits: Vec<&PwRoa> = info.roas.iter().filter(|x| scale(x).is_some()).collect();
    let roa = fits
        .iter()
        .find(|x| !route.is_empty() && x.name.eq_ignore_ascii_case(route))
        .or(if fits.len() == 1 { fits.first() } else { None })?;
    Some(((*roa).clone(), scale(roa)?))
}

/// What this amount of `name` tends to do, if it's one of the substances with a
/// profile and the amount can be read. `form` is the dose's form (kratom
/// extract and 7-OH have none).
pub fn profile(r: &impl Reference, name: &str, amount: Option<f64>, unit: &str, route: &str, form: &str) -> Option<Profile> {
    let a = amount.filter(|a| a.is_finite() && *a > 0.0)?;
    let n = name.trim().to_lowercase();
    let form = form.trim().to_lowercase();

    if n == "kratom" {
        if form == "extract" || form == "7-oh" {
            return None;
        }
        let g = grams(a, unit)?;
        return Some(if g < 3.0 {
            p("stimulating", "more stimulating at this amount", "At this amount kratom usually feels more stimulating than sedating.", None)
        } else if g <= 5.0 {
            p("mixed", "mixed at this amount", "Around this amount kratom is often a mix of stimulating and opioid-like.", None)
        } else {
            p("opioid", "more opioid-like at this amount", "At this amount kratom usually acts more like an opioid: more sedating, less stimulating.", None)
        });
    }

    let is = |names: &[&str]| names.contains(&n.as_str()) || r.lookup(&n).is_some_and(|i| names.contains(&i.name.to_lowercase().as_str()));

    if is(&["dxm", "dextromethorphan"]) {
        let (roa, s) = ranges(r, "Dextromethorphan", route, unit)?;
        let at = |x: Option<f64>| x.map(|v| v * s);
        let plateau = if at(roa.heavy).is_some_and(|h| a >= h) {
            4
        } else if at(roa.strong.min).is_some_and(|m| a >= m) {
            3
        } else if at(roa.common.min).is_some_and(|m| a >= m) {
            2
        } else if at(roa.light.min).is_some_and(|m| a >= m) {
            1
        } else {
            return None;
        };
        let ord = ["", "first", "second", "third", "fourth"][plateau];
        let caution = (plateau >= 3).then_some(
            "Third-plateau amounts and above are strongly dissociative: have someone sober with you, and don't mix DXM with serotonergic drugs.",
        );
        return Some(p(
            &format!("plateau{plateau}"),
            &format!("{ord} plateau"),
            &format!("By the reference's ranges, this amount is in the {ord} DXM plateau."),
            caution,
        ));
    }

    if is(&["diphenhydramine", "dph", "benadryl"]) {
        let (roa, s) = ranges(r, "Diphenhydramine", route, unit)?;
        let common = roa.common.min.map(|m| m * s)?;
        return Some(if a < common {
            p("sleep", "sleep-aid range", "This amount is in diphenhydramine's sleep-aid and antihistamine range.", None)
        } else {
            p(
                "deliriant",
                "deliriant range",
                "This amount is in diphenhydramine's deliriant range.",
                Some("At deliriant amounts diphenhydramine can cause seizures and dangerous heart rhythms. Have someone sober with you; a racing or irregular heartbeat, chest pain or a seizure means emergency help."),
            )
        });
    }

    // Ibogaine slows the heart's recovery between beats (QT prolongation) at
    // any dose, microdoses included (owner's review, 2026-10-06): every dose
    // carries the caution, whatever its amount.
    if is(&["ibogaine"]) {
        return Some(p(
            "cardiac",
            "affects heart rhythm at any dose",
            "Ibogaine affects heart rhythm at any amount, microdoses included.",
            Some("Ibogaine can cause dangerous heart rhythms (it prolongs the QT interval) at any dose, microdoses included. It's riskier with a heart condition, other drugs that affect heart rhythm, or low potassium or magnesium. A heart check (ECG) first is the safer route; fainting, a racing or irregular heartbeat, or chest pain means emergency help."),
        ));
    }

    if is(&["ketamine"]) {
        let (roa, s) = ranges(r, "Ketamine", route, unit)?;
        let at = |x: Option<f64>| x.map(|v| v * s);
        return Some(if at(roa.strong.min).is_some_and(|m| a >= m) {
            p("hole", "can reach a \u{201c}hole\u{201d}", "For this route, this amount is in the reference's strong range, where a \u{201c}hole\u{201d} can happen.", None)
        } else if at(roa.common.min).is_some_and(|m| a >= m) {
            p("dissociative", "dissociative", "For this route, this amount is in the reference's dissociative (common) range.", None)
        } else {
            p("subdissociative", "below dissociative", "For this route, this amount is below the reference's dissociative range.", None)
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pw::PwInfo;

    struct Ref(Vec<PwInfo>);
    impl Reference for Ref {
        fn lookup(&self, name: &str) -> Option<PwInfo> {
            self.0.iter().find(|i| i.name.eq_ignore_ascii_case(name)).cloned()
        }
        fn classes(&self, _: &str) -> Vec<String> {
            Vec::new()
        }
    }

    fn roa(route: &str, unit: &str, l: f64, c: f64, s: f64, h: f64) -> PwRoa {
        serde_json::from_value(serde_json::json!({
            "name": route, "units": unit, "threshold": l,
            "light": {"min": l, "max": c}, "common": {"min": c, "max": s}, "strong": {"min": s, "max": h}, "heavy": h
        }))
        .unwrap()
    }

    fn info(name: &str, roas: Vec<PwRoa>) -> PwInfo {
        PwInfo {
            name: name.into(),
            common_names: vec![],
            psychoactive: vec![],
            chemical: vec![],
            roas,
            interactions: vec![],
            dose_note: None,
            mine: false,
        }
    }

    fn reference() -> Ref {
        Ref(vec![
            info("Dextromethorphan", vec![roa("oral", "mg", 100.0, 200.0, 300.0, 600.0)]),
            info("Diphenhydramine", vec![roa("oral", "mg", 25.0, 150.0, 300.0, 500.0)]),
            info("Ketamine", vec![roa("insufflated", "mg", 15.0, 30.0, 60.0, 100.0), roa("oral", "mg", 50.0, 100.0, 200.0, 300.0)]),
        ])
    }

    fn key(name: &str, a: f64, unit: &str, route: &str, form: &str) -> Option<String> {
        profile(&reference(), name, Some(a), unit, route, form).map(|p| p.key)
    }

    #[test]
    fn kratom_by_amount_and_only_leaf() {
        assert_eq!(key("Kratom", 2.0, "g", "oral", "").as_deref(), Some("stimulating"));
        assert_eq!(key("Kratom", 2500.0, "mg", "oral", "leaf").as_deref(), Some("stimulating"));
        assert_eq!(key("Kratom", 4.0, "g", "oral", "").as_deref(), Some("mixed"));
        assert_eq!(key("Kratom", 6.0, "g", "oral", "").as_deref(), Some("opioid"));
        assert_eq!(key("Kratom", 1.0, "g", "oral", "extract"), None);
        assert_eq!(key("Kratom", 15.0, "mg", "oral", "7-oh"), None);
    }

    #[test]
    fn dxm_plateaus_from_the_reference() {
        assert_eq!(key("DXM", 50.0, "mg", "oral", ""), None);
        assert_eq!(key("DXM", 150.0, "mg", "oral", "").as_deref(), Some("plateau1"));
        assert_eq!(key("Dextromethorphan", 250.0, "mg", "oral", "").as_deref(), Some("plateau2"));
        let third = profile(&reference(), "DXM", Some(400.0), "mg", "oral", "").unwrap();
        assert_eq!(third.key, "plateau3");
        assert!(third.caution.is_some(), "third plateau and up carry a caution");
        assert_eq!(key("DXM", 0.7, "g", "oral", "").as_deref(), Some("plateau4"));
    }

    #[test]
    fn diphenhydramine_sleep_aid_or_deliriant() {
        let sleep = profile(&reference(), "Diphenhydramine", Some(50.0), "mg", "oral", "").unwrap();
        assert_eq!((sleep.key.as_str(), sleep.caution.is_none()), ("sleep", true));
        let delirium = profile(&reference(), "Benadryl", Some(300.0), "mg", "oral", "").unwrap();
        assert_eq!(delirium.key, "deliriant");
        assert!(delirium.caution.unwrap().contains("seizures"));
    }

    #[test]
    fn ketamine_per_route() {
        assert_eq!(key("Ketamine", 20.0, "mg", "insufflated", "").as_deref(), Some("subdissociative"));
        assert_eq!(key("Ketamine", 40.0, "mg", "insufflated", "").as_deref(), Some("dissociative"));
        assert_eq!(key("Ketamine", 80.0, "mg", "insufflated", "").as_deref(), Some("hole"));
        // The same 80 mg by mouth is below dissociative.
        assert_eq!(key("Ketamine", 80.0, "mg", "oral", "").as_deref(), Some("subdissociative"));
    }

    #[test]
    fn ibogaine_always_carries_its_cardiac_caution() {
        for amount in [2.0, 20.0, 1000.0] {
            let p = profile(&reference(), "Ibogaine", Some(amount), "mg", "oral", "").unwrap();
            assert_eq!(p.key, "cardiac");
            assert!(p.caution.unwrap().contains("QT"));
        }
    }

    #[test]
    fn everything_else_has_none() {
        assert_eq!(key("LSD", 100.0, "µg", "oral", ""), None);
        assert_eq!(profile(&reference(), "Kratom", None, "g", "oral", ""), None);
    }
}
