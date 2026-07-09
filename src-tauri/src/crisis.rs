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

#[derive(Debug, Clone, Serialize)]
pub struct CrisisResult {
    pub level: Level,
    /// A short, calm headline for the banner (empty when `level` is `none`).
    pub headline: String,
    /// The phrases/signals that triggered detection — for transparency, not shown loudly.
    pub matched: Vec<String>,
    pub resources: Vec<Resource>,
}

fn res(label: &str, contact: &str, detail: &str) -> Resource {
    Resource { label: label.into(), contact: contact.into(), detail: detail.into() }
}

/// Physical-emergency resources.
fn medical_resources() -> Vec<Resource> {
    vec![
        res("Emergency services", "911 (US) · 112 (EU) · your local number", "If someone is in physical danger, call now — don't wait."),
        res("US Poison Control", "1-800-222-1222", "Free, confidential, 24/7 — for overdoses, bad reactions, and interactions."),
    ]
}

/// Psychiatric-emergency resources.
fn psychiatric_resources() -> Vec<Resource> {
    vec![
        res("Suicide & Crisis Lifeline (US)", "988 (call or text)", "24/7 support for suicidal thoughts, self-harm, or crisis."),
        res("Emergency services", "911 (US) · 112 (EU) · your local number", "If there's immediate danger to yourself or others, call now."),
        res("Get a trusted person present", "", "Ask a sober friend to be with you, in person if you can."),
    ]
}

/// Non-emergency peer-support resources.
fn peer_resources() -> Vec<Resource> {
    vec![
        res("Fireside Project", "62-FIRESIDE (623-473-7433) — call or text", "Free psychedelic peer-support line (US), 11am–11pm PT."),
    ]
}

/// The full resource list, most urgent first — used by the always-available
/// panic / "Get help now" screen.
pub fn all_resources() -> Vec<Resource> {
    let mut v = medical_resources();
    v.extend(psychiatric_resources());
    v.extend(peer_resources());
    v
}

fn resources_for(level: Level) -> Vec<Resource> {
    match level {
        Level::None => Vec::new(),
        Level::Peer => peer_resources(),
        Level::Psychiatric => {
            let mut v = psychiatric_resources();
            v.extend(peer_resources());
            v
        }
        Level::Medical => {
            let mut v = medical_resources();
            v.extend(psychiatric_resources());
            v
        }
    }
}

fn headline_for(level: Level) -> &'static str {
    match level {
        Level::None => "",
        Level::Peer => "That sounds really hard. You don't have to hold it alone — support is one call away.",
        Level::Psychiatric => "Please reach out to a person right now. You deserve real support, and help is available.",
        Level::Medical => "This may be a medical emergency. Please get real-world help now — calling for help is the right move.",
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

fn any_match<'a>(haystack: &str, needles: &[&'a str]) -> Vec<String> {
    needles.iter().filter(|n| haystack.contains(**n)).map(|n| n.to_string()).collect()
}

/// Scan a user message for crisis signals. Returns the highest level detected.
pub fn scan(text: &str) -> CrisisResult {
    let t = text.to_lowercase();
    let med = any_match(&t, MEDICAL);
    let psy = any_match(&t, PSYCHIATRIC);
    let peer = any_match(&t, PEER);

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
    }
}

/// Force a result to at least `floor` (e.g. a dangerous interaction flag elevates
/// a message to a medical concern regardless of its wording).
pub fn escalate(mut r: CrisisResult, floor: Level, reason: &str) -> CrisisResult {
    if floor > r.level {
        r.level = floor;
        r.headline = headline_for(floor).to_string();
        r.resources = resources_for(floor);
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
    fn escalation_floor_raises_level() {
        let r = escalate(scan("having a nice time"), Level::Medical, "dangerous interaction flagged");
        assert_eq!(r.level, Level::Medical);
        assert!(r.matched.iter().any(|m| m.contains("interaction")));
    }
}
