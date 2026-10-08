// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Combination checking over a dose reference, independent of where that reference
//! is kept. The desktop answers from its journal database; a phone that can't reach
//! it answers from a copy of the bundled reference held in memory
//! ([`MemReference`]). Both run this code, so the checker people consult offline is
//! the same checker, not a second one that could drift.

use crate::interactions::{self, dosewiki_message, Warning};
use crate::pw::{PwInfo, PwInteraction};

/// Where reference data and classes come from.
pub trait Reference {
    /// DoseWiki data for a name or street name.
    fn lookup(&self, name: &str) -> Option<PwInfo>;
    /// The coarse classes the rule backstop reasons over.
    fn classes(&self, name: &str) -> Vec<String>;
}

/// Every warning we know about for a set of substances taken together, from both
/// sources: the class-rule backstop in `interactions.rs` and DoseWiki's graded
/// interaction lists. The most severe warning per pair survives.
///
/// This is the *only* way warnings should be produced. Logging a dose and asking
/// the combo checker "is this safe?" must answer identically — the checker is the
/// one people consult *before* taking something, so it can't know less.
pub fn combo_warnings(r: &impl Reference, names: &[String]) -> Vec<Warning> {
    let with_classes: Vec<(String, Vec<String>)> = names.iter().map(|n| (n.clone(), classes_of(r, n))).collect();
    let mut warnings = interactions::check(&with_classes);
    warnings.extend(pw_interaction_warnings(r, names));
    let mut warnings = interactions::dedup_pairs(warnings);
    for w in &mut warnings {
        let (a, b) = (w.a.to_lowercase(), w.b.to_lowercase());
        if let Some(o) =
            interactions::SEVERITY_OVERRIDES.iter().find(|o| (o.0 == a && o.1 == b) || (o.0 == b && o.1 == a))
        {
            w.severity = o.2;
            w.message = o.3.to_string();
        }
        w.advice = interactions::advice_for(&classes_of(r, &w.a), &classes_of(r, &w.b));
    }
    warnings
}

/// The classes a logged name belongs to: its own, plus those of the reference
/// entry it means, so "3-MeO" or "3meo" is a dissociative because 3-MeO-PCP is.
/// Only ever adds classes, so only ever adds warnings.
fn classes_of(r: &impl Reference, name: &str) -> Vec<String> {
    let mut classes = r.classes(name);
    if let Some(info) = r.lookup(name).filter(|i| !i.name.eq_ignore_ascii_case(name.trim())) {
        for c in r.classes(&info.name) {
            if !classes.contains(&c) {
                classes.push(c);
            }
        }
    }
    classes
}

/// One logged dose, for [`session_warnings`]: substance, route, and when it was
/// taken in minutes since the epoch (`None` if the time couldn't be read).
pub struct TimedDose {
    pub name: String,
    pub route: String,
    pub at_min: Option<f64>,
}

/// Interaction warnings for one session, counting only pairs that were in the
/// body at the same time. Each dose is active from when it was taken for the
/// reference's total duration on that route (or its longest, if the route isn't
/// listed; `UNKNOWN_DURATION_MIN` with no reference at all), plus
/// `OVERLAP_MARGIN_MIN` for after-effects and loose timestamps. Logged live, every
/// dose is "now" and this is the same as [`combo_warnings`]; for a past session,
/// a stimulant at 6pm and a benzo at 3am a day later aren't a combination.
///
/// Pairs that only meet once one of them is past its peak are softened rather than
/// dropped (see [`soften_for_tail`]): the substance is fading, not gone, and for
/// some pairs the fading is the risk.
pub fn session_warnings(r: &impl Reference, doses: &[TimedDose]) -> Vec<Warning> {
    let mut names: Vec<String> = doses.iter().map(|d| d.name.clone()).collect();
    names.sort();
    names.dedup();
    let mut warnings = combo_warnings(r, &names);

    // Each dose's windows, in minutes since the epoch: (name, taken, end of peak, end).
    let windows: Vec<(String, f64, f64, f64)> = doses
        .iter()
        .filter_map(|d| {
            let t = d.at_min?;
            let info = r.lookup(&d.name);
            let dur = info.as_ref().and_then(|i| active_minutes(i, &d.route)).unwrap_or(UNKNOWN_DURATION_MIN);
            // No phase data: the whole window counts as the strong part.
            let peak = info.as_ref().and_then(|i| peak_end_minutes(i, &d.route)).map_or(dur, |p| p.min(dur));
            Some((d.name.to_lowercase(), t, t + peak, t + dur + OVERLAP_MARGIN_MIN))
        })
        .collect();
    let of = |n: &str| windows.iter().filter(|w| w.0 == n.to_lowercase()).collect::<Vec<_>>();
    let meets = |x0: f64, x1: f64, y0: f64, y1: f64| x0 <= y1 && y0 <= x1;

    warnings.retain_mut(|w| {
        let (wa, wb) = (of(&w.a), of(&w.b));
        // A dose with no usable time can't be ruled out.
        if wa.is_empty() || wb.is_empty() {
            return true;
        }
        if !wa.iter().any(|x| wb.iter().any(|y| meets(x.1, x.3, y.1, y.3))) {
            return false;
        }
        let at_peak = wa.iter().any(|x| wb.iter().any(|y| meets(x.1, x.2, y.1, y.2)));
        if !at_peak {
            // The one that had peaked first is the one fading when they meet.
            let a_first = wa.iter().map(|x| x.2).fold(f64::MAX, f64::min) <= wb.iter().map(|y| y.2).fold(f64::MAX, f64::min);
            let fading = if a_first { w.a.clone() } else { w.b.clone() };
            soften_for_tail(r, w, &fading);
        }
        true
    });
    warnings
}

/// Unknown substances are assumed to last this long: long enough not to miss a
/// combination with most things, short enough that a whole night isn't one dose.
pub const UNKNOWN_DURATION_MIN: f64 = 8.0 * 60.0;
pub const OVERLAP_MARGIN_MIN: f64 = 60.0;

/// The longest end of a DoseWiki duration like "2-4 hours", in minutes.
fn max_minutes(s: &str) -> Option<f64> {
    let max = s.split(|c: char| !(c.is_ascii_digit() || c == '.')).filter_map(|n| n.parse::<f64>().ok()).next_back()?;
    let unit = s.to_lowercase();
    Some(if unit.contains("min") { max } else if unit.contains("day") { max * 1440.0 } else { max * 60.0 })
}

/// The reference's entry for a route, IM read as IV, the nearest route DoseWiki lists.
fn roa<'a>(info: &'a PwInfo, route: &str) -> Option<&'a crate::pw::PwRoa> {
    let want = match route.to_lowercase().as_str() {
        "im" | "iv" => "intravenous".to_string(),
        "vaporized" => "vaporized".to_string(),
        r => r.to_string(),
    };
    info.roas
        .iter()
        .find(|r| r.name.eq_ignore_ascii_case(route))
        .or_else(|| info.roas.iter().find(|r| r.name.eq_ignore_ascii_case(&want)))
}

/// How long a dose of this lasts by the reference's "total" for the route, longest
/// end of the range.
pub fn active_minutes(info: &PwInfo, route: &str) -> Option<f64> {
    roa(info, route)
        .and_then(|r| r.total.as_deref())
        .and_then(max_minutes)
        .or_else(|| info.roas.iter().filter_map(|r| r.total.as_deref().and_then(max_minutes)).reduce(f64::max))
}

/// When the peak is over: onset, come-up and peak added, longest ends. `None` when
/// the reference doesn't give a peak for the route, so nothing is called a tail
/// on a guess. A missing come-up is read as already counted in the onset.
pub fn peak_end_minutes(info: &PwInfo, route: &str) -> Option<f64> {
    let r = roa(info, route)?;
    let peak = r.peak.as_deref().and_then(max_minutes)?;
    let onset = r.onset.as_deref().and_then(max_minutes)?;
    Some(onset + r.come_up.as_deref().and_then(max_minutes).unwrap_or(0.0) + peak)
}

/// A pair that only meets while one of them is past its peak becomes a note,
/// saying so, unless it is one where the tail is no safer:
/// - anything rated dangerous;
/// - MAOIs, which go on interacting long after they're felt;
/// - two serotonergic drugs, where the risk follows blood levels, not the high;
/// - a stimulant with an opioid, benzo or GHB-type sedative, where the stimulant
///   wearing off is exactly when breathing can stop.
pub fn soften_for_tail(r: &impl Reference, w: &mut Warning, fading: &str) {
    if w.severity == "danger" {
        return;
    }
    let (ca, cb) = (r.classes(&w.a), r.classes(&w.b));
    let has = |c: &[String], k: &str| c.iter().any(|x| x == k);
    let either = |k: &str| has(&ca, k) || has(&cb, k);
    let sero = |c: &[String]| ["ssri", "serotonin_releaser", "serotonergic"].iter().any(|k| has(c, k));
    let ghb_like = |n: &str| {
        let n = n.to_lowercase();
        ["ghb", "gbl", "1,4-butanediol", "1,4-bd", "sodium oxybate"].iter().any(|g| n.contains(g))
    };
    let sed = |c: &[String], n: &str| ["opioid", "benzodiazepine"].iter().any(|k| has(c, k)) || ghb_like(n);
    if either("maoi")
        || (sero(&ca) && sero(&cb))
        || (has(&ca, "stimulant") && sed(&cb, &w.b))
        || (has(&cb, "stimulant") && sed(&ca, &w.a))
    {
        return;
    }
    w.severity = "note";
    w.message = format!(
        "{fading} was past its peak by the time these overlapped, so this matters less than it would at full strength. It is fading, not gone. {}",
        w.message
    );
}

/// Active metabolites sold or taken in their own right, and prodrugs, with the drug
/// whose interaction warnings apply to them. Whole-word matching can't see that
/// "O-Desmethyltramadol" is tramadol's active form, or that 1,4-butanediol becomes
/// GHB in the body, and neither reference entry lists anything to match on, so
/// without this no warning naming tramadol or GHB (etizolam: "GHB/GBL", dangerous)
/// would ever reach them. (metabolite or prodrug, the drug it acts as), lowercase.
const ACTIVE_METABOLITES: &[(&str, &str)] = &[("o-desmethyltramadol", "tramadol"), ("1,4-butanediol", "ghb")];

/// A DoseWiki family name with wildcards, like "5-MeO-xxT" or "2C-x": an `x`
/// segment stands for any one segment, and `xx` inside one for one to four
/// characters. Does `id` (a name or alias) belong to the family?
pub fn family_match(pattern: &str, id: &str) -> bool {
    let (ps, is): (Vec<&str>, Vec<&str>) = (pattern.split('-').collect(), id.split('-').collect());
    ps.len() == is.len()
        && ps.iter().zip(&is).all(|(p, i)| {
            if *p == "x" {
                !i.is_empty() && i.chars().all(char::is_alphanumeric)
            } else if let Some((pre, post)) = p.split_once("xx") {
                let mid = i.len() as isize - pre.len() as isize - post.len() as isize;
                i.starts_with(pre) && i.ends_with(post) && (1..=4).contains(&mid)
            } else {
                p == i
            }
        })
}

/// The wildcard family a DoseWiki entry names, if any ("5-MeO-xxT tryptamines" →
/// "5-meo-xxt"). Such an entry means that family, not every member of the class
/// word after it: "5-MeO-xxT tryptamines" is not about DMT.
fn family_of(interaction: &str) -> Option<&str> {
    interaction
        .split_whitespace()
        .find(|t| t.contains('-') && t.split('-').any(|seg| seg == "x" || seg.contains("xx")))
}

/// Does a DoseWiki interaction entry (a substance name or a class like
/// "Stimulants"/"MAOIs") refer to `other`? Matches `other`'s name, aliases, and
/// psychoactive/chemical classes, with light singular/substring tolerance.
pub fn matches_interaction(interaction: &str, other: &PwInfo) -> bool {
    let i = interaction.to_lowercase();
    if let Some(fam) = family_of(&i) {
        // By name or alias only, or a class that *is* the family ("2C-X").
        let ids = std::iter::once(&other.name).chain(&other.common_names).chain(&other.chemical);
        return ids.map(|s| s.to_lowercase()).any(|id| id == fam || family_match(fam, &id));
    }
    let i_sing = i.trim_end_matches('s');
    let mut ids: Vec<String> = vec![other.name.to_lowercase()];
    ids.extend(
        ACTIVE_METABOLITES
            .iter()
            .filter(|(m, _)| other.name.eq_ignore_ascii_case(m))
            .map(|(_, parent)| parent.to_string()),
    );
    ids.extend(other.common_names.iter().map(|s| s.to_lowercase()));
    ids.extend(other.psychoactive.iter().map(|s| s.to_lowercase()));
    ids.extend(other.chemical.iter().map(|s| s.to_lowercase()));
    ids.iter().any(|id| {
        let id_sing = id.trim_end_matches('s');
        // Partial matches are whole words only, and never on a one- or two-letter
        // street name: LSD is also "L", MDMA "E" and "X", ketamine "K", and a plain
        // substring test made "Lithium", "Tramadol" and "Alcohol" all match LSD.
        id == &i
            || id_sing == i_sing
            || (id.len() >= 3 && has_word(&i, id))
            || (i.len() >= 4 && has_word(id, &i))
            || (i_sing.len() >= 4 && has_word(id, i_sing))
    })
}

/// `needle` appears in `hay` as a whole word (a plural "s" after it is allowed):
/// no letter or digit directly on either side.
fn has_word(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    hay.match_indices(needle).any(|(at, _)| {
        let before = hay[..at].chars().next_back();
        let rest = &hay[at + needle.len()..];
        let rest = rest.strip_prefix('s').filter(|r| !r.starts_with(char::is_alphanumeric)).unwrap_or(rest);
        !before.is_some_and(char::is_alphanumeric) && !rest.starts_with(char::is_alphanumeric)
    })
}

/// Rank a DoseWiki-derived severity for picking the most severe match per pair.
fn sev_rank(sev: &str) -> u8 {
    match sev {
        "danger" => 3,
        "caution" => 2,
        _ => 1,
    }
}

/// Map a stored severity string back to the static set the `Warning` type uses.
fn static_sev(sev: &str) -> &'static str {
    match sev {
        "danger" => "danger",
        "caution" => "caution",
        _ => "note",
    }
}

/// Graded warnings from DoseWiki's interaction lists for every pair of the given
/// substances that has reference data. Keeps the most severe match per pair and
/// carries DoseWiki's reason text.
pub fn pw_interaction_warnings(r: &impl Reference, names: &[String]) -> Vec<Warning> {
    let infos: Vec<(String, PwInfo)> = names.iter().filter_map(|n| r.lookup(n).map(|info| (n.clone(), info))).collect();
    let mut out = Vec::new();
    for a in 0..infos.len() {
        for b in (a + 1)..infos.len() {
            let (na, ia) = &infos[a];
            let (nb, ib) = &infos[b];
            // Consider graded matches in both directions; keep the most severe.
            let matches = ia
                .interactions
                .iter()
                .filter(|x| matches_interaction(&x.name, ib))
                .chain(ib.interactions.iter().filter(|x| matches_interaction(&x.name, ia)));
            let mut best: Option<&PwInteraction> = None;
            for x in matches {
                if best.map_or(0, |b| sev_rank(&b.severity)) < sev_rank(&x.severity) {
                    best = Some(x);
                }
            }
            if let Some(x) = best {
                out.push(Warning {
                    severity: static_sev(&x.severity),
                    a: na.clone(),
                    b: nb.clone(),
                    message: dosewiki_message(&x.severity, x.reason.as_deref()),
                    advice: Vec::new(),
                });
            }
        }
    }
    out
}

/// The bundled reference held in memory, answering lookups exactly as the
/// desktop's cache table does: an exact name first (ASCII case-insensitive), then
/// the first entry whose stored JSON contains the name as a quoted string — the
/// same `LIKE '%"name"%'` the database runs, so a street name, a class or a route
/// name finds what it finds there — and failing both, the name the spelling means
/// ("3meopcp" is 3-MeO-PCP), as the desktop does. Classes come from the built-in
/// list only: a phone keeps no copy of the person's own catalogue.
pub struct MemReference {
    subs: Vec<PwInfo>,
    /// Each entry's JSON as the desktop stores it, ASCII-lowercased for `LIKE`.
    json: Vec<String>,
    names: crate::names::NameIndex,
}

impl MemReference {
    pub fn new(subs: Vec<PwInfo>) -> Self {
        let json = subs.iter().map(|s| serde_json::to_string(s).unwrap_or_default().to_ascii_lowercase()).collect();
        let reference: Vec<_> = subs.iter().map(|s| (s.name.clone(), s.common_names.clone())).collect();
        let names = crate::names::NameIndex::new(&[], &reference);
        MemReference { subs, json, names }
    }

    fn lookup_as_written(&self, name: &str) -> Option<PwInfo> {
        if let Some(s) = self.subs.iter().find(|s| s.name.eq_ignore_ascii_case(name)) {
            return Some(s.clone());
        }
        let pattern = format!("%\"{name}\"%").to_ascii_lowercase();
        self.json.iter().position(|j| like(&pattern, j)).map(|i| self.subs[i].clone())
    }

    pub fn len(&self) -> usize {
        self.subs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.subs.is_empty()
    }

    /// Every name with its street names (the desktop's `pw_names`).
    pub fn names(&self) -> Vec<(String, Vec<String>)> {
        let mut out: Vec<_> = self.subs.iter().map(|s| (s.name.clone(), s.common_names.clone())).collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}

impl Reference for MemReference {
    fn lookup(&self, name: &str) -> Option<PwInfo> {
        self.lookup_as_written(name).or_else(|| {
            let meant = self.names.canonical(name)?;
            if meant.eq_ignore_ascii_case(name.trim()) {
                return None;
            }
            self.lookup_as_written(&meant)
        })
    }

    fn classes(&self, name: &str) -> Vec<String> {
        interactions::builtin_classes(name)
    }
}

/// SQL `LIKE` over already-lowercased text: `%` is any run, `_` any one character.
fn like(pattern: &str, text: &str) -> bool {
    // The usual case, `%"name"%` with no wildcard inside: a plain substring test.
    if let Some(inner) = pattern.strip_prefix('%').and_then(|p| p.strip_suffix('%')) {
        if !inner.contains(['%', '_']) {
            return text.contains(inner);
        }
    }
    let (p, t): (Vec<char>, Vec<char>) = (pattern.chars().collect(), text.chars().collect());
    // Classic two-pointer glob with backtracking to the last `%`.
    let (mut pi, mut ti) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '_' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '%' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '%' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundled() -> MemReference {
        MemReference::new(crate::pw::parse_slim(include_str!("../../resources/dosewiki.json")).unwrap())
    }

    #[test]
    fn like_matches_as_sqlite_does() {
        assert!(like("%\"molly\"%", "{\"common_names\":[\"molly\"]}"));
        assert!(!like("%\"mol\"%", "{\"common_names\":[\"molly\"]}"));
        assert!(like("%\"m_lly\"%", "[\"molly\"]"));
        assert!(like("%", ""));
    }

    #[test]
    fn a_street_name_finds_its_entry() {
        let r = bundled();
        assert_eq!(r.lookup("molly").unwrap().name, "MDMA");
        assert_eq!(r.lookup("lsd").unwrap().name, "LSD");
        assert!(r.lookup("not a real substance").is_none());
        assert_eq!(r.lookup("3meopcp").unwrap().name, "3-MeO-PCP", "spelled without hyphens");
        assert_eq!(r.lookup("3 meo").unwrap().name, "3-MeO-PCP", "a street name without hyphens");
    }

    #[test]
    fn mdma_with_tramadol_is_flagged_offline() {
        let w = combo_warnings(&bundled(), &["MDMA".into(), "Tramadol".into()]);
        assert!(w.iter().any(|w| w.severity == "danger"), "{w:?}");
    }

    #[test]
    fn a_name_logged_by_another_spelling_is_still_checked() {
        let as_named = combo_warnings(&bundled(), &["3-MeO-PCP".into(), "Alcohol".into()]);
        assert!(!as_named.is_empty());
        for typed in ["3meopcp", "3meo", "3-MeO"] {
            let w = combo_warnings(&bundled(), &[typed.into(), "Alcohol".into()]);
            assert_eq!(w.len(), as_named.len(), "{typed}: {w:?}");
        }
    }

    #[test]
    fn doses_apart_in_time_do_not_combine() {
        let r = bundled();
        let d = |name: &str, h: f64| TimedDose { name: name.into(), route: "oral".into(), at_min: Some(h * 60.0) };
        assert!(!session_warnings(&r, &[d("MDMA", 0.0), d("Tramadol", 1.0)]).is_empty());
        assert!(session_warnings(&r, &[d("MDMA", 0.0), d("Tramadol", 72.0)]).is_empty());
    }

    fn d(name: &str, h: f64) -> TimedDose {
        TimedDose { name: name.into(), route: "oral".into(), at_min: Some(h * 60.0) }
    }

    #[test]
    fn alcohol_at_the_peak_of_dexamphetamine_is_not_softened() {
        // Oral dexamphetamine's peak runs to 5.5h on DoseWiki's longest figures.
        let w = session_warnings(&bundled(), &[d("Dextroamphetamine", 0.0), d("Alcohol", 5.0)]);
        assert!(!w.is_empty(), "{w:?}");
        assert!(w.iter().all(|w| !w.message.contains("past its peak")), "{w:?}");
    }

    #[test]
    fn alcohol_in_the_tail_of_dexamphetamine_is_a_softened_note() {
        let w = session_warnings(&bundled(), &[d("Dextroamphetamine", 0.0), d("Alcohol", 7.0)]);
        assert!(!w.is_empty(), "{w:?}");
        for w in &w {
            assert_eq!(w.severity, "note", "{w:?}");
            assert!(w.message.starts_with("Dextroamphetamine was past its peak"), "{w:?}");
        }
    }

    #[test]
    fn a_fading_stimulant_with_ghb_is_never_softened() {
        let w = session_warnings(&bundled(), &[d("Amphetamine", 0.0), d("GHB", 7.0)]);
        assert!(!w.is_empty(), "{w:?}");
        assert!(w.iter().all(|w| !w.message.contains("past its peak")), "{w:?}");
    }

    #[test]
    fn ghb_with_stimulants_does_not_call_it_an_opiate() {
        let w = combo_warnings(&bundled(), &["Amphetamine".into(), "GHB".into()]);
        assert!(!w.is_empty(), "{w:?}");
        assert!(w.iter().all(|w| !w.message.contains("opiate")), "{w:?}");
    }
}
