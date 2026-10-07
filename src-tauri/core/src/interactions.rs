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
    pub message: String,
    /// What lowers the risk, for the person deciding: plain harm-reduction notes
    /// for the classes involved (see [`advice_for`]). Shown behind a tap.
    #[serde(default)]
    pub advice: Vec<String>,
}

/// The class vocabulary the UI offers when classifying a substance.
pub const CLASSES: &[&str] = &[
    "maoi",
    "ssri",
    "serotonin_releaser",
    "entactogen",
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
    "qt_prolonging",
    "cyp2d6_inhibitor",
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
    // Ibogaine blocks hERG and prolongs QT, and so does noribogaine (half-life
    // 28–49 h); it is cleared mainly by CYP2D6 (Koenig & Hilber 2015,
    // PMC4382526). Clearance rises steeply with CYP2D6 activity (Knuijver 2024,
    // PMC11102648). DoseWiki lists no interactions for it at all.
    ("ibogaine", "qt_prolonging", "danger",
        "Ibogaine + a QT-prolonging drug (e.g. methadone, cocaine, alcohol) — both lengthen the heart's QT interval, and together the risk of a fatal arrhythmia rises. Ibogaine's metabolite keeps doing this for days."),
    ("ibogaine", "cyp2d6_inhibitor", "danger",
        "Ibogaine + a CYP2D6 inhibitor — CYP2D6 is how the body clears ibogaine, so blocking it leaves much more ibogaine in the blood, and more strain on the heart."),
    ("lithium", "psychedelic", "danger",
        "Lithium + psychedelics — reports of seizures and serious reactions. Treated as contraindicated."),
    ("lithium", "stimulant", "danger",
        "Lithium + stimulants — increased seizure and neurotoxicity risk."),
    ("opioid", "depressant", "danger",
        "Opioid + depressant (alcohol/GHB/etc.) — additive respiratory depression, a leading overdose cause."),
    ("opioid", "benzodiazepine", "danger",
        "Opioid + benzodiazepine — additive respiratory depression. Frequently fatal in overdose."),
    // Two benzos first: of two rules equally severe, the first that fits is shown.
    ("benzodiazepine", "benzodiazepine", "caution",
        "Multiple depressants stack unpredictably — heightened sedation and memory loss."),
    ("depressant", "benzodiazepine", "caution",
        "Depressant + benzodiazepine — additive sedation and blackout/respiratory risk."),
    ("ssri", "serotonin_releaser", "caution",
        "SSRI + serotonin releaser (e.g. MDMA) — serotonin-syndrome risk, and SSRIs also blunt the effect."),
    ("serotonin_releaser", "serotonin_releaser", "caution",
        "Two serotonin releasers — additive serotonin-syndrome and neurotoxicity risk."),
    ("stimulant", "stimulant", "caution",
        "Two stimulants — additive cardiovascular strain (heart rate, blood pressure, temperature)."),
    ("dissociative", "depressant", "caution",
        "Dissociative + depressant — additive sedation; nausea/vomiting while sedated is a choke risk."),
    // Before stimulant + psychedelic: MDMA is a stimulant too, but with a psychedelic
    // it acts as an empathogen and usually eases anxiety rather than adding to it.
    ("entactogen", "psychedelic", "note",
        "Empathogen + psychedelic — MDMA often eases the anxiety a psychedelic can bring, but together they still add up on heart rate, temperature and serotonin."),
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
                out.push(Warning { severity: rule.2, a: na.clone(), b: nb.clone(), message: rule.3.to_string(), advice: Vec::new() });
            }
        }
    }
    out.sort_by_key(|w| std::cmp::Reverse(rank(w.severity)));
    out
}

/// Pairs where we rate the risk differently from the reference, with our reason.
/// MDMA + mephedrone is two serotonin releasers, and DoseWiki calls it dangerous
/// outright; how risky it is depends heavily on the amounts and the spacing, so a
/// flat "dangerous" overstates a small second dose hours later and understates
/// nothing the advice doesn't cover. (a, b, severity, message), names lowercase.
pub const SEVERITY_OVERRIDES: &[(&str, &str, &str, &str)] = &[(
    "mdma",
    "mephedrone",
    "caution",
    "Two serotonin releasers: a risk of serotonin toxicity, overheating and added neurotoxicity that grows with the amounts and how close together they're taken. DoseWiki rates this combination as dangerous.",
)];

/// Harm-reduction notes for a pair, from the classes on each side. Plain things
/// that lower the risk, not instructions to stop: the person has already decided
/// or is deciding, and what helps is knowing how to do it more safely.
pub fn advice_for(ca: &[String], cb: &[String]) -> Vec<String> {
    let either = |x: &str, y: &str| (has(ca, x) && has(cb, y)) || (has(ca, y) && has(cb, x));
    let mut out: Vec<&str> = Vec::new();
    if has(ca, "maoi") || has(cb, "maoi") {
        out.push("MAOIs change how much of the other drug reaches you, unpredictably. This is one to avoid rather than adjust; if it's already happened, watch for a severe headache, a racing heart or overheating, and get help if they appear.");
    }
    if either("ibogaine", "qt_prolonging") || either("ibogaine", "cyp2d6_inhibitor") {
        out.push("This is one to avoid rather than adjust. Ibogaine's effect on the heart outlasts the experience by days, so leave a long gap either side, and get an ECG check beforehand if you can.");
    }
    if has(ca, "lithium") || has(cb, "lithium") {
        out.push("Seizures have been reported with lithium at ordinary doses of the other drug. This is one to avoid rather than adjust.");
    }
    // Serotonergic here means tramadol, DXM and the like. Every classic psychedelic
    // is also tagged serotonergic, but with MDMA the meaningful risks are the
    // stimulant ones below, not serotonin toxicity.
    let serotonergic_not_psychedelic = |c: &[String]| has(c, "serotonergic") && !has(c, "psychedelic");
    let serotonin_pair = either("serotonin_releaser", "serotonin_releaser")
        || either("ssri", "serotonin_releaser")
        || (has(ca, "serotonin_releaser") && serotonergic_not_psychedelic(cb))
        || (has(cb, "serotonin_releaser") && serotonergic_not_psychedelic(ca));
    if serotonin_pair {
        out.push("Effects build on each other, so a smaller amount of the second goes further. Leave real time between them and go easy on redoses.");
        out.push("Keep cool and take breaks from dancing. Overheating, rigid or twitching muscles, or confusion are signs of serotonin toxicity: cool down, and get help if they don't settle.");
    }
    if either("ssri", "serotonin_releaser") {
        out.push("SSRIs blunt MDMA-like drugs, which can lead to taking more to feel it. Taking more doesn't get past the block; it adds risk.");
    }
    let depressants = |c: &[String]| has(c, "depressant") || has(c, "benzodiazepine") || has(c, "opioid");
    if depressants(ca) && depressants(cb) {
        out.push("Each makes the other stronger, often more than expected. Use less of both, measure carefully, and don't redose either on top.");
        out.push("Have someone with you who knows what you took. If someone can't be woken, or their breathing is slow or noisy, put them on their side in the recovery position and call for help.");
    }
    if either("dissociative", "depressant") {
        out.push("Feeling sick while heavily sedated is a choking risk: lie on your side, not your back.");
    }
    if either("stimulant", "stimulant") {
        out.push("Heart rate, blood pressure and temperature add up. Lower amounts of each and more time between them make the biggest difference.");
        out.push("Sip water steadily rather than a lot at once, and rest somewhere cool. Chest pain or a racing heart that doesn't settle with rest is a reason to get checked.");
    }
    if either("entactogen", "psychedelic") {
        out.push("MDMA tends to soften a psychedelic's anxiety rather than add to it. Together it's still a long, full experience: many people take less of each than they would on its own.");
        out.push("Heart rate and temperature both climb. Take breaks somewhere cool and go easy on redoses.");
    } else if either("stimulant", "psychedelic") {
        out.push("Stimulants can tip a psychedelic experience toward anxiety or thought loops. A calm setting, a trusted sitter and a smaller stimulant dose help.");
    }
    if either("stimulant", "dissociative") {
        out.push("A stimulant can hide how sedated you are. Go slow with redoses of either.");
    }
    let mut v: Vec<String> = out.into_iter().map(String::from).collect();
    v.dedup();
    if !v.is_empty() || !ca.is_empty() || !cb.is_empty() {
        v.push("Amounts and spacing matter: lower doses and more time between them lower the risk of almost any combination.".into());
    }
    v
}

fn rank(sev: &str) -> u8 {
    match sev {
        "danger" => 3,
        "caution" => 2,
        _ => 1,
    }
}

/// Human-readable message for a pair flagged by DoseWiki's graded interaction
/// lists. `severity` is our danger/caution/note; `reason` is DoseWiki's optional
/// parenthetical explanation.
pub fn dosewiki_message(severity: &str, reason: Option<&str>) -> String {
    let lead = match severity {
        "danger" => "DoseWiki lists this as a dangerous combination",
        "caution" => "DoseWiki lists this as unsafe",
        _ => "DoseWiki notes caution with this combination",
    };
    match reason {
        Some(r) if !r.is_empty() => format!("{lead}: {}", correct_reason(r)),
        _ => format!("{lead} — treat as risky and check trusted sources."),
    }
}

/// Fix known slips in DoseWiki's wording, here rather than in the vendored data so
/// a refresh can't bring them back. Its stimulant entries reuse the opioid sentence
/// for GHB/GBL and other sedatives ("...a higher dose of sedatives. If the stimulant
/// wears off first then the opiate may overcome..."), and GHB isn't an opiate.
pub fn correct_reason(r: &str) -> String {
    if r.contains("sedatives") {
        r.replace("the opiate", "the sedative")
    } else {
        r.to_string()
    }
}

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
    // The MDMA-like empathogens. Mephedrone releases serotonin too but behaves more
    // like a stimulant, so it isn't one of these.
    if n.contains("mdma") || n.contains("molly") || n.contains("ecstasy") || n.contains("mda")
        || n.contains("mdea") || n.contains("methylone")
    {
        add("entactogen", &mut c);
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
    // depressants. Drinks by name too ("Beer"), matched whole so "ginseng" or
    // "rum raisin" isn't alcohol; same list as the phone's drink unit (quicklog.ts).
    const DRINKS: &[&str] = &[
        "booze", "beer", "wine", "cider", "vodka", "whiskey", "whisky", "rum", "gin", "tequila",
        "mezcal", "liquor", "spirits", "sake", "champagne", "glass of wine", "shot",
    ];
    if DRINKS.contains(&n.trim()) {
        add("depressant", &mut c);
    }
    if n.contains("alcohol") || n.contains("ethanol") || n.contains("ghb") || n.contains("gbl")
        || n.contains("butanediol") || n == "1,4-b" || n == "14b" || n.contains("barbiturate") || n.contains("phenibut")
    {
        add("depressant", &mut c);
    }
    // benzodiazepines
    if n.contains("benzo") || n.contains("alprazolam") || n.contains("xanax")
        || n.contains("diazepam") || n.contains("valium") || n.contains("clonazepam")
        || n.contains("etizolam") || n.contains("lorazepam") || n.contains("zolam") || n.contains("zepam")
        || n.contains("rilmazafone")
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
    // Ibogaine's heart risks (see RULES). Only the drugs the sources name: alcohol,
    // cocaine and methadone prolong QT; methadone also inhibits CYP2D6.
    if n.contains("iboga") {
        add("ibogaine", &mut c);
    }
    if n.contains("methadone") {
        add("opioid", &mut c);
        add("qt_prolonging", &mut c);
        add("cyp2d6_inhibitor", &mut c);
    }
    if n.contains("cocaine") || n.contains("coke") || n.contains("alcohol") || n.contains("ethanol") || DRINKS.contains(&n.trim()) {
        add("qt_prolonging", &mut c);
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
    fn ibogaine_with_qt_drugs_or_cyp2d6_inhibitors_is_danger() {
        for other in ["methadone", "cocaine", "Beer", "alcohol"] {
            let w = check(&[sub("Ibogaine"), sub(other)]);
            assert_eq!(w.first().map(|w| w.severity), Some("danger"), "ibogaine + {other}: {w:?}");
            assert!(w[0].message.starts_with("Ibogaine + a QT"), "{w:?}");
        }
        let w = check(&[sub("iboga root bark"), sub("my med")]);
        assert!(w.is_empty(), "an unclassified med isn't guessed at: {w:?}");
        let mine = ("my med".to_string(), vec!["cyp2d6_inhibitor".to_string()]);
        let w = check(&[sub("Ibogaine"), mine]);
        assert!(w[0].message.contains("CYP2D6"), "{w:?}");
        // The new classes don't change anything without ibogaine.
        let w = check(&[sub("cocaine"), sub("Beer")]);
        assert!(w.iter().all(|w| !w.message.contains("QT")), "{w:?}");
    }

    #[test]
    fn flags_lithium_lsd_danger() {
        let w = check(&[sub("lithium"), sub("LSD")]);
        assert!(w.iter().any(|w| w.severity == "danger"));
    }

    #[test]
    fn mdma_with_a_psychedelic_is_an_empathogen_note() {
        let w = check(&[sub("MDMA"), sub("LSD")]);
        assert!(w[0].message.starts_with("Empathogen + psychedelic"), "{w:?}");
        let a = advice_for(&builtin_classes("MDMA"), &builtin_classes("LSD"));
        assert!(a.iter().any(|x| x.contains("soften")), "{a:?}");
        assert!(!a.iter().any(|x| x.contains("thought loops")), "{a:?}");
        // A plain stimulant keeps the stimulant note.
        let w = check(&[sub("amphetamine"), sub("LSD")]);
        assert!(w[0].message.starts_with("Stimulant + psychedelic"), "{w:?}");
    }

    #[test]
    fn drinks_by_name_are_alcohol() {
        for d in ["Beer", "wine", "Vodka", " gin ", "Glass of wine", "shot"] {
            assert!(builtin_classes(d).contains(&"depressant".to_string()), "{d} should be a depressant");
        }
        for not in ["ginseng", "rum raisin", "winery tour"] {
            assert!(builtin_classes(not).is_empty(), "{not} isn't a drink");
        }
        let w = check(&[sub("Beer"), sub("alprazolam")]);
        assert!(!w.is_empty(), "beer + a benzo must be flagged: {w:?}");
    }

    #[test]
    fn unrelated_is_quiet() {
        let w = check(&[sub("caffeine"), sub("cannabis")]);
        assert!(w.iter().all(|w| w.severity != "danger"));
    }
}
