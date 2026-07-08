// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! A small, deterministic interaction checker for the most dangerous, widely
//! documented combinations. This is a safety backstop — NOT a complete
//! interaction reference and NOT medical advice. It reasons over coarse
//! pharmacological *classes* assigned to each substance, so it also works for
//! user-added substances once they're classified.
//!
//! Classes are common-knowledge harm-reduction categories, deliberately not
//! derived from any copyrighted source.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Warning {
    /// "danger" | "caution" | "note"
    pub severity: &'static str,
    pub a: String,
    pub b: String,
    pub message: &'static str,
}

/// The class vocabulary the UI offers when classifying a substance.
pub const CLASSES: &[&str] = &[
    "maoi",
    "ssri",
    "serotonin_releaser",
    "serotonergic",
    "stimulant",
    "depressant",
    "benzodiazepine",
    "opioid",
    "psychedelic",
    "dissociative",
    "lithium",
    "cannabinoid",
    "deliriant",
];

/// (class A, class B, severity, message). Checked against unordered pairs.
const RULES: &[(&str, &str, &str, &str)] = &[
    ("maoi", "serotonin_releaser", "danger",
        "MAOI + serotonin releaser (e.g. MDMA) — high risk of serotonin syndrome and hypertensive crisis. Widely considered contraindicated."),
    ("maoi", "ssri", "danger",
        "MAOI + SSRI — serious serotonin-syndrome risk. Long washout periods apply."),
    ("maoi", "serotonergic", "danger",
        "MAOI + serotonergic drug — serotonin-syndrome risk."),
    ("maoi", "stimulant", "danger",
        "MAOI + stimulant — risk of hypertensive crisis."),
    ("maoi", "opioid", "danger",
        "MAOI + certain opioids (e.g. tramadol, meperidine, dextromethorphan) — serotonin-syndrome risk."),
    ("lithium", "psychedelic", "danger",
        "Lithium + psychedelics — reports of seizures and serious reactions. Treated as contraindicated."),
    ("lithium", "stimulant", "danger",
        "Lithium + stimulants — increased seizure and neurotoxicity risk."),
    ("opioid", "depressant", "danger",
        "Opioid + depressant (alcohol/GHB/etc.) — additive respiratory depression, a leading overdose cause."),
    ("opioid", "benzodiazepine", "danger",
        "Opioid + benzodiazepine — additive respiratory depression. Frequently fatal in overdose."),
    ("depressant", "benzodiazepine", "caution",
        "Depressant + benzodiazepine — additive sedation and blackout/respiratory risk."),
    ("benzodiazepine", "benzodiazepine", "caution",
        "Multiple depressants stack unpredictably — heightened sedation and memory loss."),
    ("ssri", "serotonin_releaser", "caution",
        "SSRI + serotonin releaser (e.g. MDMA) — serotonin-syndrome risk, and SSRIs also blunt the effect."),
    ("serotonin_releaser", "serotonin_releaser", "caution",
        "Two serotonin releasers — additive serotonin-syndrome and neurotoxicity risk."),
    ("stimulant", "stimulant", "caution",
        "Two stimulants — additive cardiovascular strain (heart rate, blood pressure, temperature)."),
    ("dissociative", "depressant", "caution",
        "Dissociative + depressant — additive sedation; nausea/vomiting while sedated is a choke risk."),
    ("stimulant", "psychedelic", "note",
        "Stimulant + psychedelic — can amplify anxiety and cardiovascular load."),
    ("stimulant", "dissociative", "note",
        "Stimulant + dissociative — masks sedation and raises cardiovascular load."),
];

fn has(classes: &[String], c: &str) -> bool {
    classes.iter().any(|x| x.eq_ignore_ascii_case(c))
}

/// Check every unordered pair of substances for the most severe matching rule.
pub fn check(substances: &[(String, Vec<String>)]) -> Vec<Warning> {
    let mut out = Vec::new();
    for i in 0..substances.len() {
        for j in (i + 1)..substances.len() {
            let (na, ca) = &substances[i];
            let (nb, cb) = &substances[j];

            // Find the most severe rule that applies to this pair.
            let mut best: Option<&(&str, &str, &str, &str)> = None;
            for rule in RULES {
                let (x, y, ..) = rule;
                let matches = (has(ca, x) && has(cb, y)) || (has(ca, y) && has(cb, x));
                if matches && rank(rule.2) > best.map_or(0, |b| rank(b.2)) {
                    best = Some(rule);
                }
            }
            if let Some(rule) = best {
                out.push(Warning { severity: rule.2, a: na.clone(), b: nb.clone(), message: rule.3 });
            }
        }
    }
    out.sort_by_key(|w| std::cmp::Reverse(rank(w.severity)));
    out
}

fn rank(sev: &str) -> u8 {
    match sev {
        "danger" => 3,
        "caution" => 2,
        _ => 1,
    }
}

/// Message for a pair flagged by PsychonautWiki's dangerous-interaction list.
pub const PW_MESSAGE: &str =
    "Listed as a dangerous interaction on PsychonautWiki — treat as high risk and check trusted sources.";

/// Merge warnings from multiple sources, keeping the most severe per unordered
/// substance pair (so a combination isn't reported twice).
pub fn dedup_pairs(mut warnings: Vec<Warning>) -> Vec<Warning> {
    warnings.sort_by_key(|w| std::cmp::Reverse(rank(w.severity)));
    let mut seen = std::collections::HashSet::new();
    warnings.retain(|w| {
        let key = if w.a <= w.b { (w.a.clone(), w.b.clone()) } else { (w.b.clone(), w.a.clone()) };
        seen.insert(key)
    });
    warnings
}

/// Built-in pharmacological classes for well-known substances, so the safety
/// checker works out of the box before the user classifies anything. Matched on
/// lowercased name/substring. Common knowledge — not a dosage or content source.
pub fn builtin_classes(name: &str) -> Vec<String> {
    let n = name.to_lowercase();
    let mut c: Vec<&str> = Vec::new();
    let add = |x: &'static str, c: &mut Vec<&str>| {
        if !c.contains(&x) {
            c.push(x)
        }
    };

    // serotonin releasers / empathogens
    if n.contains("mdma") || n.contains("molly") || n.contains("ecstasy") || n.contains("mda")
        || n.contains("mdea") || n.contains("methylone") || n.contains("mephedrone") || n.contains("4-mmc")
    {
        add("serotonin_releaser", &mut c);
        add("stimulant", &mut c);
    }
    // classic psychedelics
    if n.contains("lsd") || n.contains("acid") || n.contains("psiloc") || n.contains("mushroom")
        || n.contains("shroom") || n.contains("dmt") || n.contains("mescaline") || n.contains("2c-")
        || n.contains("ayahuasca")
    {
        add("psychedelic", &mut c);
        add("serotonergic", &mut c);
    }
    // dissociatives
    if n.contains("ketamine") || n == "k" || n.contains("mxe") || n.contains("dxm")
        || n.contains("pcp") || n.contains("n2o") || n.contains("nitrous") || n.contains("dck")
    {
        add("dissociative", &mut c);
    }
    if n.contains("dxm") {
        add("serotonergic", &mut c);
    }
    // stimulants
    if n.contains("amphetamine") || n.contains("adderall") || n.contains("meth")
        || n.contains("cocaine") || n.contains("coke") || n.contains("caffeine")
        || n.contains("modafinil") || n.contains("ritalin") || n.contains("methylphenidate")
    {
        add("stimulant", &mut c);
    }
    // depressants
    if n.contains("alcohol") || n.contains("ethanol") || n.contains("ghb") || n.contains("gbl")
        || n.contains("barbiturate") || n.contains("phenibut")
    {
        add("depressant", &mut c);
    }
    // benzodiazepines
    if n.contains("benzo") || n.contains("alprazolam") || n.contains("xanax")
        || n.contains("diazepam") || n.contains("valium") || n.contains("clonazepam")
        || n.contains("etizolam") || n.contains("lorazepam")
    {
        add("benzodiazepine", &mut c);
        add("depressant", &mut c);
    }
    // opioids
    if n.contains("opio") || n.contains("heroin") || n.contains("fentanyl") || n.contains("oxycodone")
        || n.contains("morphine") || n.contains("codeine") || n.contains("kratom") || n.contains("tramadol")
    {
        add("opioid", &mut c);
    }
    if n.contains("tramadol") {
        add("serotonergic", &mut c);
    }
    // maois / ssris / lithium
    if n.contains("maoi") || n.contains("harmal") || n.contains("syrian rue") || n.contains("moclobemide") || n.contains("phenelzine") {
        add("maoi", &mut c);
    }
    if n.contains("ssri") || n.contains("fluoxetine") || n.contains("sertraline")
        || n.contains("escitalopram") || n.contains("citalopram") || n.contains("paroxetine")
    {
        add("ssri", &mut c);
    }
    if n.contains("lithium") {
        add("lithium", &mut c);
    }
    if n.contains("cannabis") || n.contains("weed") || n.contains("thc") || n.contains("marijuana") {
        add("cannabinoid", &mut c);
    }

    c.into_iter().map(String::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(name: &str) -> (String, Vec<String>) {
        (name.to_string(), builtin_classes(name))
    }

    #[test]
    fn flags_mdma_ssri() {
        let w = check(&[sub("MDMA"), sub("sertraline (SSRI)")]);
        assert!(w.iter().any(|w| w.severity == "caution"), "expected an SSRI+MDMA caution: {w:?}");
    }

    #[test]
    fn flags_opioid_benzo_danger() {
        let w = check(&[sub("heroin"), sub("alprazolam")]);
        assert_eq!(w.first().map(|w| w.severity), Some("danger"));
    }

    #[test]
    fn flags_lithium_lsd_danger() {
        let w = check(&[sub("lithium"), sub("LSD")]);
        assert!(w.iter().any(|w| w.severity == "danger"));
    }

    #[test]
    fn unrelated_is_quiet() {
        let w = check(&[sub("caffeine"), sub("cannabis")]);
        assert!(w.iter().all(|w| w.severity != "danger"));
    }
}
