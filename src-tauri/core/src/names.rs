// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Which substance a typed name means: "acid" is LSD, "dexamp" is
//! Dextroamphetamine. Used when a dose is logged (so the journal says what was
//! taken) and when Stats groups doses (so "acid" and "LSD" are one history).
//!
//! Deliberately conservative: only a whole name or street name counts, never a
//! prefix, and a street name only when it means exactly one substance and is
//! longer than two letters ("L", "E", "X" and "K" mean too many things). A name
//! typed without its hyphens or spaces ("3meopcp", "2cb") counts too, but only
//! when the exact spelling means nothing and the squashed one means just one
//! substance. The person's own catalogue comes first: a substance they added
//! themselves is never renamed after the reference.

use std::collections::HashMap;

/// Shortest street name that's converted.
const MIN_ALIAS_LEN: usize = 3;

pub struct NameIndex {
    /// Lowercase name or street name → the name it means, or `None` when it
    /// means more than one.
    own: HashMap<String, Option<String>>,
    reference: HashMap<String, Option<String>>,
    /// The same, keyed by [`squash`]ed spelling.
    own_squashed: HashMap<String, Option<String>>,
    reference_squashed: HashMap<String, Option<String>>,
}

fn exact(key: &str) -> String {
    key.trim().to_lowercase()
}

/// A spelling with only its letters and digits, lowercase: "3-MeO-PCP",
/// "3 meo pcp" and "3meopcp" are all "3meopcp".
fn squash(key: &str) -> String {
    key.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

fn add(map: &mut HashMap<String, Option<String>>, k: String, name: &str) {
    if k.is_empty() {
        return;
    }
    match map.get(&k) {
        None => {
            map.insert(k, Some(name.to_string()));
        }
        Some(Some(existing)) if !existing.eq_ignore_ascii_case(name) => {
            map.insert(k, None);
        }
        _ => {}
    }
}

impl NameIndex {
    /// `own`: the person's catalogue, `reference`: the dose reference, each as
    /// (name, street names).
    pub fn new(own: &[(String, Vec<String>)], reference: &[(String, Vec<String>)]) -> Self {
        NameIndex {
            own: own_map(own, exact),
            reference: reference_map(reference, exact),
            own_squashed: own_map(own, squash),
            reference_squashed: reference_map(reference, squash),
        }
    }

    /// The name `typed` means, spelled as its entry spells it, if it clearly means
    /// one. `None` for anything unknown or ambiguous: keep it as written.
    pub fn canonical(&self, typed: &str) -> Option<String> {
        let k = exact(typed);
        if k.is_empty() {
            return None;
        }
        let s = squash(typed);
        if let Some(hit) = self.own.get(&k).or_else(|| self.own_squashed.get(&s)) {
            return hit.clone();
        }
        self.reference.get(&k).or_else(|| self.reference_squashed.get(&s)).cloned().flatten()
    }
}

fn own_map(own: &[(String, Vec<String>)], key: fn(&str) -> String) -> HashMap<String, Option<String>> {
    let mut o = HashMap::new();
    for (name, _) in own {
        add(&mut o, key(name), name);
    }
    for (name, aliases) in own {
        for a in aliases {
            if !o.contains_key(&key(a)) {
                add(&mut o, key(a), name);
            }
        }
    }
    o
}

fn reference_map(reference: &[(String, Vec<String>)], key: fn(&str) -> String) -> HashMap<String, Option<String>> {
    let mut r = HashMap::new();
    for (name, _) in reference {
        add(&mut r, key(name), name);
    }
    let mut street: HashMap<String, Option<String>> = HashMap::new();
    for (name, aliases) in reference {
        for a in aliases.iter().filter(|a| a.trim().chars().count() >= MIN_ALIAS_LEN) {
            add(&mut street, key(a), name);
        }
    }
    // A reference name always beats someone else's street name.
    for (k, v) in street {
        r.entry(k).or_insert(v);
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(own: &[(&str, &[&str])]) -> NameIndex {
        let reference = crate::pw::parse_slim(include_str!("../../resources/dosewiki.json")).unwrap();
        let reference: Vec<(String, Vec<String>)> = reference.into_iter().map(|s| (s.name, s.common_names)).collect();
        let own: Vec<(String, Vec<String>)> =
            own.iter().map(|(n, a)| (n.to_string(), a.iter().map(|x| x.to_string()).collect())).collect();
        NameIndex::new(&own, &reference)
    }

    #[test]
    fn street_names_become_the_substance() {
        let i = index(&[]);
        assert_eq!(i.canonical("acid").as_deref(), Some("LSD"));
        assert_eq!(i.canonical(" Dexamp ").as_deref(), Some("Dextroamphetamine"));
        assert_eq!(i.canonical("molly").as_deref(), Some("MDMA"));
        assert_eq!(i.canonical("lsd").as_deref(), Some("LSD"), "a name's own spelling");
        assert_eq!(i.canonical("14b").as_deref(), Some("1,4-Butanediol"));
    }

    #[test]
    fn short_unknown_or_partial_names_stay_as_written() {
        let i = index(&[]);
        for s in ["L", "E", "X", "K", "", "my own blend", "keta"] {
            assert_eq!(i.canonical(s), None, "{s:?}");
        }
    }

    #[test]
    fn names_typed_without_hyphens_or_spaces_count() {
        let i = index(&[]);
        assert_eq!(i.canonical("3meopcp").as_deref(), Some("3-MeO-PCP"));
        assert_eq!(i.canonical("3 MeO PCP").as_deref(), Some("3-MeO-PCP"));
        assert_eq!(i.canonical("2cb").as_deref(), Some("2C-B"));
        assert_eq!(i.canonical("3-MeO-PCPr").as_deref(), Some("3-MeO-PCPr"), "an exact spelling still wins");
        assert_eq!(i.canonical("-"), None);
        // Squashed, this could mean two substances: keep it as written.
        let i = index(&[("Blend A-1", &[]), ("Blend A1", &[])]);
        assert_eq!(i.canonical("blend a 1"), None);
        assert_eq!(i.canonical("Blend A1").as_deref(), Some("Blend A1"), "unless spelled exactly");
    }

    #[test]
    fn the_persons_own_catalogue_comes_first() {
        let i = index(&[("Acid", &[]), ("House blend", &["hb"])]);
        assert_eq!(i.canonical("acid").as_deref(), Some("Acid"));
        assert_eq!(i.canonical("HB").as_deref(), Some("House blend"));
        assert_eq!(i.canonical("molly").as_deref(), Some("MDMA"));
    }
}
