// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Deterministic crisis-escalation layer for the Companion.
//!
//! This is the load-bearing safety net specified in the roadmap: symptom/intent
//! detection that runs **independently of the language model**, so a red flag
//! surfaces real-world help even if the model says nothing useful (or nothing at
//! all). The model's prompt reinforces the same behaviour, but is never the sole
//! safeguard. Matching is intentionally simple and errs toward surfacing help.

use serde::Serialize;

/// Escalation levels, ordered so the strongest match wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// Nothing detected.
    None,
    /// General acute distress — offer calm peer support + a warmline.
    Peer,
    /// Suicidal / self-harm / harm-to-others intent — direct to IRL help now.
    Psychiatric,
    /// Physical medical emergency — direct to emergency services / poison control.
    Medical,
}

#[derive(Debug, Clone, Serialize)]
pub struct Resource {
    pub label: String,
    pub contact: String,
    pub detail: String,
}

/// How the banner should present itself.
///
/// Flashing emergency numbers at someone having a hard but ordinary time is its own
/// harm: it pathologises a difficult experience, which is precisely what the Zendo
/// principles say not to do. So at `Peer` we *offer* — trusted person first, numbers
/// behind a tap they choose.
///
/// That reasoning inverts as severity rises. Asking "would you like emergency info?"
/// of someone confused with heat stroke asks a person whose judgement is impaired to
/// triage themselves, and asking a suicidal person whether they want the number
/// invites "no". At `Psychiatric` and `Medical` the information is simply present.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Presentation {
    /// Offer first: a question, and resources only if they say yes.
    Offer,
    /// Show the resources straight away, without asking.
    Direct,
}

#[derive(Debug, Clone, Serialize)]
pub struct CrisisResult {
    pub level: Level,
    /// A short, calm headline for the banner (empty when `level` is `none`).
    pub headline: String,
    /// The phrases/signals that triggered detection — for transparency, not shown loudly.
    pub matched: Vec<String>,
    pub resources: Vec<Resource>,
    /// Whether to ask before showing resources. See [`Presentation`].
    pub presentation: Presentation,
}

fn res(label: &str, contact: &str, detail: &str) -> Resource {
    Resource { label: label.into(), contact: contact.into(), detail: detail.into() }
}

// Each resource has one definition, so the same wording and number appear
// wherever it's shown. The level lists and the full panic-screen list are then
// just orderings over these — which is what stops "Emergency services" from
// turning up twice when a list concatenates two categories that both include it.
fn r_trusted_person() -> Resource {
    res("Someone you trust", "", "Is there a friend you could call, or who could sit with you? That's often the best first move.")
}
fn r_sober_present() -> Resource {
    res("Get a trusted sober person present", "", "Ask a sober friend to be with you, in person if you can.")
}
fn r_emergency() -> Resource {
    res("Emergency services", "911 (US) · 112 (EU) · your local number", "If someone is in physical danger, call now — don't wait.")
}
fn r_lifeline() -> Resource {
    res("Suicide & Crisis Lifeline (US)", "988 (call or text)", "24/7 support for suicidal thoughts, self-harm, or crisis.")
}
fn r_poison() -> Resource {
    res("US Poison Control", "1-800-222-1222", "Free, confidential, 24/7 — for overdoses, bad reactions, and interactions.")
}
fn r_fireside() -> Resource {
    res("Fireside Project", "62-FIRESIDE (623-473-7433) — call or text", "Free psychedelic peer-support line (US), 11am–11pm PT.")
}

/// Drop later entries that repeat an earlier label, keeping order. A composed
/// level list can name the same resource from two categories (Emergency services
/// is both physical and psychiatric); the person should see it once.
fn dedup_by_label(mut v: Vec<Resource>) -> Vec<Resource> {
    let mut seen = std::collections::BTreeSet::new();
    v.retain(|r| seen.insert(r.label.clone()));
    v
}

/// Physical-emergency resources.
fn medical_resources() -> Vec<Resource> {
    vec![r_emergency(), r_poison()]
}

/// Psychiatric-emergency resources.
fn psychiatric_resources() -> Vec<Resource> {
    vec![r_lifeline(), r_emergency(), r_sober_present()]
}

/// Non-emergency peer-support resources. A trusted person comes first deliberately:
/// for most hard moments someone in the room beats a phone number, and naming it as
/// an option is often what makes it thinkable.
fn peer_resources() -> Vec<Resource> {
    vec![r_trusted_person(), r_fireside()]
}

/// The full panic-screen list, for the always-available "Get help now" screen.
/// Ordered gentlest-first — the people who could already be in the room, then
/// the phone lines — so the most reachable help is what someone reads first when
/// they're in no state to read far.
pub fn all_resources() -> Vec<Resource> {
    vec![
        r_trusted_person(),
        r_sober_present(),
        r_fireside(),
        r_emergency(),
        r_lifeline(),
        r_poison(),
    ]
}

fn resources_for(level: Level) -> Vec<Resource> {
    match level {
        Level::None => Vec::new(),
        Level::Peer => peer_resources(),
        Level::Psychiatric => dedup_by_label({
            let mut v = psychiatric_resources();
            v.extend(peer_resources());
            v
        }),
        Level::Medical => dedup_by_label({
            let mut v = medical_resources();
            v.extend(psychiatric_resources());
            v
        }),
    }
}

fn headline_for(level: Level) -> &'static str {
    match level {
        Level::None => "",
        // An offer, not a verdict — and phrased so it can be waved away.
        Level::Peer => "That sounds really hard. Would it help to have someone to talk to? I can show you a couple of options.",
        Level::Psychiatric => "You don't have to be alone with this. Please reach out to someone — a person you trust, or one of these.",
        Level::Medical => "This may be a medical emergency. Please get real-world help now — calling for help is the right move.",
    }
}

fn presentation_for(level: Level) -> Presentation {
    match level {
        Level::None | Level::Peer => Presentation::Offer,
        Level::Psychiatric | Level::Medical => Presentation::Direct,
    }
}

// Phrase lists. Lowercased substring matches against the user's text.
const MEDICAL: &[&str] = &[
    "can't breathe", "cant breathe", "can't breath", "cant breath", "trouble breathing",
    "hard to breathe", "stopped breathing", "not breathing", "chest pain", "chest hurts",
    "pain in my chest", "seizure", "seizing", "convulsing", "convulsion", "unconscious",
    "unresponsive", "won't wake", "wont wake", "passed out", "collapsed", "overdose",
    "overdosed", "od'd", "overheating", "burning up", "high fever", "serotonin syndrome",
    "can't stop vomiting", "cant stop vomiting", "keep vomiting", "won't stop throwing up",
    "throwing up blood", "turning blue", "blue lips",
];

const PSYCHIATRIC: &[&str] = &[
    "kill myself", "killing myself", "end my life", "want to die", "wanna die", "suicidal",
    "suicide", "hurt myself", "harm myself", "self harm", "self-harm", "cut myself",
    "end it all", "don't want to be alive", "dont want to be alive", "no reason to live",
    "kill someone", "hurt someone", "hurt others", "hurt people",
];

const PEER: &[&str] = &[
    "panic", "panicking", "freaking out", "freaking me out", "bad trip", "terrified",
    "can't calm down", "cant calm down", "losing control", "losing my mind", "so scared",
    "i'm scared", "im scared", "really scared", "this is too much", "overwhelmed",
];

/// Medical emergencies people **describe** rather than name. Someone with heat
/// stroke says "really hot and I've stopped sweating", not "overheating"; the
/// single-phrase lists above sail straight past that. Each cluster fires only when
/// a sign from *both* halves is present, which keeps a hot room or an ordinary
/// headache from tripping it.
///
/// This is the layer that must not depend on how capable the local model is. A
/// small model that misreads heat stroke as dehydration is a real possibility on
/// low-spec hardware; the person still gets the banner and the resources.
const MEDICAL_CLUSTERS: &[(&str, &[&str], &[&str])] = &[
    (
        "possible heat stroke",
        &["really hot", "so hot", "burning", "overheat", "boiling", "can't cool", "cant cool"],
        &["stopped sweating", "not sweating", "no longer sweating", "confused", "disoriented",
          "dizzy", "cramping", "can't stand", "cant stand"],
    ),
    (
        "possible serotonin toxicity",
        &["shaking", "shivering", "twitching", "muscles", "jaw", "rigid", "stiff"],
        &["fever", "burning up", "so hot", "sweating a lot", "heart racing", "confused"],
    ),
    (
        "possible cardiac event",
        &["heart", "chest", "pulse"],
        &["racing", "pounding", "irregular", "skipping", "won't slow", "wont slow", "tight",
          "crushing", "numb arm", "left arm"],
    ),
    (
        "possible respiratory depression",
        &["breathing", "breaths", "lips", "skin"],
        &["shallow", "slow", "blue", "grey", "gray", "gurgling", "rattling", "snoring"],
    ),
];

/// Signals expressed as word stems that must appear *near* each other, rather than
/// as one exact phrase. Fixed substrings are brittle in the ways that matter most:
/// "my chest **really** hurts" misses `"chest hurts"`, and "it's terrif**ying**"
/// misses `"terrified"`. People in trouble do not phrase things canonically.
///
/// `(label, stems, window)` — every stem must match some token as a prefix, and all
/// matches must fall inside a `window`-token span.
type Near = (&'static str, &'static [&'static str], usize);

const MEDICAL_NEAR: &[Near] = &[
    ("chest pain", &["chest", "hurt"], 4),
    ("chest pain", &["chest", "pain"], 4),
    ("chest tightness", &["chest", "tight"], 4),
    ("heart pain", &["heart", "hurt"], 4),
    ("trouble breathing", &["get", "breath"], 4),
    ("trouble breathing", &["catch", "breath"], 4),
    ("trouble breathing", &["breath", "shallow"], 4),
    ("trouble breathing", &["breath", "short"], 4),
];

const PSYCHIATRIC_NEAR: &[Near] = &[
    ("does not want to be here", &["want", "here", "anymore"], 6),
    ("does not want to exist", &["want", "exist"], 4),
    ("intent to end life", &["end", "tonight"], 5),
    ("intent to end life", &["end", "it", "all"], 4),
    ("intent to end life", &["ending", "it"], 3),
    ("intent to end life", &["take", "own", "life"], 5),
    ("feels others better off", &["better", "off", "without", "me"], 6),
    ("sees no reason to live", &["no", "point", "liv"], 5),
];

const PEER_NEAR: &[Near] = &[
    ("feels they are dying", &["think", "dying"], 4),
    ("feels they are dying", &["feel", "dying"], 4),
    ("feels they are dying", &["i'm", "dying"], 2),
    ("feels they are dying", &["im", "dying"], 2),
    ("feels they are dying", &["going", "die"], 3),
    ("feels they are dying", &["gonna", "die"], 3),
    ("wants it to stop", &["want", "stop"], 5),
    ("wants it to stop", &["make", "stop"], 4),
    ("at their limit", &["can't", "take"], 3),
    ("at their limit", &["cant", "take"], 3),
    ("something feels wrong", &["something", "wrong"], 4),
    // Stems, anchored to a first-person cue so a terrifying *film* stays a film.
    ("terrified", &["it's", "terrif"], 3),
    ("terrified", &["its", "terrif"], 3),
    ("terrified", &["i'm", "terrif"], 3),
    ("terrified", &["im", "terrif"], 3),
    ("terrified", &["so", "terrif"], 3),
    ("terrified", &["really", "terrif"], 3),
    ("terrified", &["feel", "terrif"], 4),
];

/// Peer signals that describe how a moment *feels* rather than asking for help.
/// Someone writing "this is horrible, i want it to stop" is very often just naming
/// an unpleasant experience — which is what a journal is for, and a hard experience
/// is not the same as a bad one. These therefore need repetition before they raise
/// anything. See [`scan_recent`].
const PEER_EXPRESSIVE: &[&str] = &["wants it to stop", "at their limit", "something feels wrong"];

/// Negation flips some signals and is essential to others. "i **didn't** want it
/// to stop" is not distress; "i **don't** want to be here anymore" very much is.
/// So this is per-signal rather than a general rule — only these labels are
/// suppressed when one of their negating phrases is present.
const NEGATORS: &[(&str, &[&str])] = &[
    ("wants it to stop", &["didn't want", "didnt want", "did not want", "never want", "don't want it to stop"]),
    ("at their limit", &["can't take my eyes", "cant take my eyes", "can't take that away"]),
    ("something feels wrong", &["nothing", "isn't wrong", "isnt wrong", "not wrong"]),
];

/// Drop signals whose meaning the surrounding text negates.
fn strip_negated(text: &str, labels: Vec<String>) -> Vec<String> {
    labels
        .into_iter()
        .filter(|l| {
            !NEGATORS
                .iter()
                .any(|(label, phrases)| label == l && phrases.iter().any(|p| text.contains(p)))
        })
        .collect()
}

fn tokens(text: &str) -> Vec<&str> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .filter(|t| !t.is_empty())
        .collect()
}

/// Positions where some token starts with `stem`.
fn stem_positions(toks: &[&str], stem: &str) -> Vec<usize> {
    toks.iter().enumerate().filter(|(_, t)| t.starts_with(stem)).map(|(i, _)| i).collect()
}

/// Do all stems occur within a `window`-token span? Stem lists are short, so the
/// exhaustive walk is cheaper than being clever.
fn near_hit(toks: &[&str], stems: &[&str], window: usize) -> bool {
    let mut combos: Vec<Vec<usize>> = vec![Vec::new()];
    for stem in stems {
        let pos = stem_positions(toks, stem);
        if pos.is_empty() {
            return false;
        }
        combos = combos
            .iter()
            .flat_map(|c| {
                pos.iter().map(move |p| {
                    let mut next = c.clone();
                    next.push(*p);
                    next
                })
            })
            .take(512) // guard against a pathological message
            .collect();
    }
    combos.iter().any(|c| {
        let (min, max) = (c.iter().min().unwrap(), c.iter().max().unwrap());
        max - min <= window
    })
}

fn near_matches(toks: &[&str], sigs: &[Near]) -> Vec<String> {
    let mut out: Vec<String> = sigs
        .iter()
        .filter(|(_, stems, window)| near_hit(toks, stems, *window))
        .map(|(label, _, _)| (*label).to_string())
        .collect();
    out.dedup();
    out
}


fn any_match<'a>(haystack: &str, needles: &[&'a str]) -> Vec<String> {
    needles.iter().filter(|n| haystack.contains(**n)).map(|n| n.to_string()).collect()
}

/// Clusters that fired: both halves present in the same message.
fn cluster_matches(haystack: &str) -> Vec<String> {
    MEDICAL_CLUSTERS
        .iter()
        .filter(|(_, a, b)| {
            a.iter().any(|n| haystack.contains(n)) && b.iter().any(|n| haystack.contains(n))
        })
        .map(|(label, _, _)| (*label).to_string())
        .collect()
}

/// Scan a user message for crisis signals. Returns the highest level detected.
pub fn scan(text: &str) -> CrisisResult {
    scan_recent(&[text.to_string()])
}

/// Scan a run of recent messages from the person, oldest first, the newest last.
///
/// Acute signals fire on the newest message alone. **Expressive** distress —
/// "this is horrible", "i want it to stop" — deliberately does not. Said once it is
/// usually someone naming an unpleasant feeling, which is the whole point of the
/// app; answering that with a hotline pathologises it. Said again across a
/// conversation it starts to look like someone who is not coming out of it, and
/// that is worth a quiet offer. Once is expression; twice is a pattern.
pub fn scan_recent(recent: &[String]) -> CrisisResult {
    let latest = recent.last().cloned().unwrap_or_default().to_lowercase();
    let toks = tokens(&latest);

    let mut med = any_match(&latest, MEDICAL);
    med.extend(cluster_matches(&latest));
    med.extend(near_matches(&toks, MEDICAL_NEAR));

    let mut psy = any_match(&latest, PSYCHIATRIC);
    psy.extend(near_matches(&toks, PSYCHIATRIC_NEAR));

    // Acute peer signals: panic, terror, feeling like you're dying.
    let mut peer: Vec<String> = any_match(&latest, PEER);
    peer.extend(
        strip_negated(&latest, near_matches(&toks, PEER_NEAR))
            .into_iter()
            .filter(|l| !PEER_EXPRESSIVE.contains(&l.as_str())),
    );

    // Expressive distress counts across the recent run, not within one message —
    // saying it twice in one breath is still saying it once.
    let repeats = recent
        .iter()
        .filter(|m| {
            let m = m.to_lowercase();
            let mt = tokens(&m);
            strip_negated(&m, near_matches(&mt, PEER_NEAR))
                .iter()
                .any(|l| PEER_EXPRESSIVE.contains(&l.as_str()))
        })
        .count();
    if repeats >= 2 {
        peer.push("distress repeated across several messages".into());
    }

    let (level, matched) = if !med.is_empty() {
        (Level::Medical, med)
    } else if !psy.is_empty() {
        (Level::Psychiatric, psy)
    } else if !peer.is_empty() {
        (Level::Peer, peer)
    } else {
        (Level::None, Vec::new())
    };

    CrisisResult {
        level,
        headline: headline_for(level).to_string(),
        matched,
        resources: resources_for(level),
        presentation: presentation_for(level),
    }
}

/// Force a result to at least `floor` (e.g. a dangerous interaction flag elevates
/// a message to a medical concern regardless of its wording).
pub fn escalate(mut r: CrisisResult, floor: Level, reason: &str) -> CrisisResult {
    if floor > r.level {
        r.level = floor;
        r.headline = headline_for(floor).to_string();
        r.resources = resources_for(floor);
        r.presentation = presentation_for(floor);
    }
    if floor >= Level::Medical {
        r.matched.push(reason.to_string());
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_medical_over_everything() {
        let r = scan("i'm panicking and i can't breathe");
        assert_eq!(r.level, Level::Medical);
        assert!(!r.resources.is_empty());
    }

    #[test]
    fn detects_suicidal_intent() {
        let r = scan("i just want to die");
        assert_eq!(r.level, Level::Psychiatric);
    }

    #[test]
    fn detects_distress() {
        let r = scan("this is a really bad trip and i'm scared");
        assert_eq!(r.level, Level::Peer);
    }

    #[test]
    fn quiet_when_calm() {
        let r = scan("feeling good, just logged some water");
        assert_eq!(r.level, Level::None);
        assert!(r.resources.is_empty());
    }

    #[test]
    fn detects_heat_stroke_described_rather_than_named() {
        // The wording that got past both the phrase list and an 8B model.
        let r = scan("i've been dancing for hours, i'm really hot and i've stopped sweating and feel confused");
        assert_eq!(r.level, Level::Medical);
        assert!(r.matched.iter().any(|m| m.contains("heat stroke")));
    }

    #[test]
    fn detects_described_respiratory_depression() {
        let r = scan("his breathing has gone really shallow and his lips look blue");
        assert_eq!(r.level, Level::Medical);
    }

    #[test]
    fn clusters_need_both_halves() {
        // A hot room is not a medical emergency, and neither is being confused.
        assert_eq!(scan("it's really hot in this room").level, Level::None);
        assert_eq!(scan("i feel a bit confused about the time").level, Level::None);
    }

    /// Every one of these reached `Level::None` before the stem/proximity matcher.
    /// They are not exotic phrasings — they are how people actually talk.
    #[test]
    fn catches_natural_phrasings_the_phrase_lists_missed() {
        let cases: &[(&str, Level)] = &[
            ("my chest really hurts and i can't get a full breath", Level::Medical),
            ("my chest is so tight", Level::Medical),
            ("i don't want to be here anymore. i've been thinking about ending it tonight", Level::Psychiatric),
            ("everyone would be better off without me", Level::Psychiatric),
            ("i think i'm dying. something is really wrong with me", Level::Peer),
            ("i can't find myself. i don't know who i am and it's terrifying", Level::Peer),
        ];
        for (text, want) in cases {
            assert_eq!(scan(text).level, *want, "wrong level for {text:?}");
        }
    }

    /// The matcher is looser, so the quiet cases matter more than before.
    #[test]
    fn stays_quiet_on_ordinary_talk() {
        for text in [
            "the music was so good i didn't want it to stop",
            "we took a breath of fresh air on the balcony",
            "my chest feels warm and open, it's lovely",
            "watched a terrifying movie last week, unrelated",
            "feeling good, just logged some water",
        ] {
            let r = scan(text);
            assert_eq!(r.level, Level::None, "false positive on {text:?}: {:?}", r.matched);
        }
    }

    /// Naming an unpleasant feeling is what the app is *for*. Saying it once must
    /// not summon a hotline; saying it repeatedly is a different signal.
    #[test]
    fn expressive_distress_needs_repeating_before_it_raises_anything() {
        let once = "this is horrible. i hate this. i want it to stop right now";
        assert_eq!(scan(once).level, Level::None);

        let run = vec![
            once.to_string(),
            "still going".to_string(),
            "i really want this to stop".to_string(),
        ];
        let r = scan_recent(&run);
        assert_eq!(r.level, Level::Peer);
        assert_eq!(r.presentation, Presentation::Offer);
    }

    /// Repetition gates the expressive signals only — never the acute ones.
    #[test]
    fn acute_distress_still_fires_on_the_first_message() {
        assert_eq!(scan("i think i'm dying. something is really wrong with me").level, Level::Peer);
        assert_eq!(scan("i'm panicking and i can't calm down").level, Level::Peer);
        assert_eq!(scan("i can't find myself and it's terrifying").level, Level::Peer);
    }

    /// A hard trip gets an offer; an emergency does not wait to be asked.
    #[test]
    fn severity_decides_whether_we_ask_first() {
        assert_eq!(scan("i'm freaking out").presentation, Presentation::Offer);
        assert_eq!(scan("i've been thinking about ending it tonight").presentation, Presentation::Direct);
        assert_eq!(scan("my chest really hurts and i can't get a full breath").presentation, Presentation::Direct);
    }

    #[test]
    fn a_trusted_person_is_offered_before_a_hotline() {
        let r = scan("i'm freaking out and i can't calm down");
        assert!(r.resources[0].detail.contains("friend"), "trusted person should come first");
    }

    #[test]
    fn the_panic_screen_lists_each_resource_once_people_first() {
        let all = all_resources();
        let labels: Vec<&str> = all.iter().map(|r| r.label.as_str()).collect();
        // The duplicate that prompted this: Emergency services appeared twice
        // when the list concatenated the physical and psychiatric categories.
        let mut seen = std::collections::BTreeSet::new();
        for l in &labels {
            assert!(seen.insert(*l), "resource listed twice: {l}");
        }
        assert_eq!(labels[0], "Someone you trust", "reachable, in-the-room help first");
        assert_eq!(labels[1], "Get a trusted sober person present");
        assert_eq!(labels[2], "Fireside Project", "peer support sits with the trusted-people options");
        assert_eq!(labels[3], "Emergency services");
    }

    #[test]
    fn a_medical_banner_does_not_repeat_emergency_services() {
        let r = scan("i'm burning up and i've stopped sweating and i'm confused");
        let n = r.resources.iter().filter(|x| x.label == "Emergency services").count();
        assert_eq!(n, 1, "emergency services should appear once");
    }

    #[test]
    fn escalation_to_medical_stops_asking() {
        let r = escalate(scan("i'm freaking out"), Level::Medical, "dangerous interaction");
        assert_eq!(r.presentation, Presentation::Direct);
    }

    #[test]
    fn escalation_floor_raises_level() {
        let r = escalate(scan("having a nice time"), Level::Medical, "dangerous interaction flagged");
        assert_eq!(r.level, Level::Medical);
        assert!(r.matched.iter().any(|m| m.contains("interaction")));
    }
}
