// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Offline substance-knowledge search over the bundled DoseWiki prose corpus.
//!
//! The corpus (`resources/dosewiki-corpus.json`, built by `data/dosewiki/corpus.py`)
//! is CC0 DoseWiki prose — harm potential, pharmacology, tolerance, interactions,
//! legality, history — split into ~7.8k retrievable chunks. It is searched with
//! **BM25, in-process**: no embedding model, no indexing pass, no network. Every
//! lookup is private, and the whole thing works with the journal still locked
//! (the corpus is public reference data, not user data).
//!
//! # What this is NOT
//!
//! **This is not the source of dose or interaction facts.** Those come from the
//! deterministic layers — `pw.rs` for dose ranges, `interactions.rs` for combo
//! verdicts — and retrieval must never be able to override them. This corpus
//! answers *"how does this work, what are the risks, what's the tolerance
//! profile"*. It never supplies a number or a combo verdict. See ROADMAP.md #1.
//!
//! # Trust the hits, but check `thin`
//!
//! DoseWiki's coverage is deeply uneven and it tracks *fame*, not *risk*: 93% of
//! entries are marked `editorial_review: needed` upstream, prose volume varies
//! ~400x, and the obscure research chemicals — exactly where a user has nowhere
//! else to look — are the thinnest. So every [`Hit`] carries `thin` and
//! `reviewed`, and callers **must** surface them rather than presenting a hit
//! from an 86-character entry with the same confidence as one from LSD.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Path of the bundled corpus, relative to the Tauri resource dir.
const RESOURCE_PATH: &str = "resources/dosewiki-corpus.json";

/// Standard BM25 term-frequency saturation / length-normalisation constants.
const K1: f64 = 1.2;
const B: f64 = 0.75;

/// A hit must clear this to be returned at all. Below it, callers should say "I
/// have no good data on that" rather than pass thin noise to a language model,
/// which will happily write confident prose around it (ROADMAP.md #1, rule 3).
const MIN_SCORE: f64 = 1.0;

/// Matching a substance's *name* is a much stronger signal than matching a word in
/// its prose ("LSD interactions" should not rank a passing mention of LSD in the
/// ketamine entry above LSD's own interactions chunk).
const TITLE_BOOST: f64 = 2.5;

/// Words too common in this corpus to discriminate between substances.
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "can", "for", "from", "has", "have", "in",
    "is", "it", "its", "may", "of", "on", "or", "that", "the", "this", "to", "with", "which",
    "effects", "effect", "substance", "drug", "use", "used",
];

// ---- bundled corpus shape (see data/dosewiki/corpus.py) ----

#[derive(Deserialize)]
struct Corpus {
    chunks: Vec<Chunk>,
}

#[derive(Deserialize, Clone)]
struct Chunk {
    title: String,
    slug: String,
    section: String,
    text: String,
    /// Substance has < 2 kB of prose upstream — treat its content as sparse.
    thin: bool,
    /// DoseWiki marked this entry editorially reviewed (true for ~1 of 577).
    reviewed: bool,
}

/// One retrieved passage, with the provenance a caller needs to weigh it.
#[derive(Serialize, Clone, Debug)]
pub struct Hit {
    pub title: String,
    pub slug: String,
    pub section: String,
    pub text: String,
    pub thin: bool,
    pub reviewed: bool,
    pub score: f64,
}

/// One substance in the corpus, for browsing the reference by name rather than
/// by query. Carries the same coverage flags as a [`Hit`] so a sparse entry is
/// marked as such in the browse list, before it is opened.
#[derive(Serialize, Clone, Debug)]
pub struct Entry {
    pub title: String,
    pub slug: String,
    pub thin: bool,
    pub reviewed: bool,
    /// How many passages the substance has — the honest measure of coverage.
    pub sections: usize,
}

/// In-memory BM25 index. Built once at launch (~7.8k chunks; a few ms).
pub struct Index {
    chunks: Vec<Chunk>,
    /// term -> [(chunk index, term frequency)]
    postings: HashMap<String, Vec<(usize, u32)>>,
    /// Terms of each chunk's substance name, for the title boost.
    titles: Vec<Vec<String>>,
    lengths: Vec<u32>,
    avg_len: f64,
}

/// Split text into search terms.
///
/// Chemical names are the whole point here, so the tokenizer must not shred them:
/// `2C-B`, `5-HT2A` and `5F-PB-22` have to survive. We keep alphanumeric-and-hyphen
/// runs whole *and also* emit their hyphen-separated parts, so a query for `2C-B`
/// matches, and so does one for `2C`.
fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for tok in whole_tokens(text) {
        // Also index the parts of a hyphenated name, so a query for `2C` finds
        // `2C-B`. The whole token is emitted too, and only the whole token earns
        // the title boost — see `titles` below.
        if tok.contains('-') {
            for part in tok.split('-').filter(|p| !p.is_empty()) {
                if !STOPWORDS.contains(&part) {
                    out.push(part.to_string());
                }
            }
        }
        out.push(tok);
    }
    out
}

/// Tokens without hyphen-splitting: `1V-LSD` -> ["1v-lsd"], not ["1v","lsd"].
///
/// Used for substance names. If titles were indexed with their parts split, a
/// search for "LSD" would give the full title boost to every analogue whose name
/// merely *contains* "lsd" — 1V-LSD, 1P-LSD, ALD-52 — and those chunks, being
/// shorter, would outrank LSD's own. Only a whole-name match is name evidence.
fn whole_tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .filter(|s| !s.is_empty())
        .map(|raw| raw.trim_matches('-').to_ascii_lowercase())
        .filter(|t| !t.is_empty() && !STOPWORDS.contains(&t.as_str()))
        .collect()
}

impl Index {
    fn build(chunks: Vec<Chunk>) -> Self {
        let mut postings: HashMap<String, Vec<(usize, u32)>> = HashMap::new();
        let mut lengths = Vec::with_capacity(chunks.len());
        let mut titles = Vec::with_capacity(chunks.len());

        for (i, chunk) in chunks.iter().enumerate() {
            // Index the section label alongside the body so "LSD legality" or
            // "ketamine harm potential" can hit on the section name too.
            let terms = tokenize(&format!(
                "{} {} {}",
                chunk.title, chunk.section, chunk.text
            ));
            lengths.push(terms.len() as u32);
            titles.push(whole_tokens(&chunk.title));

            let mut tf: HashMap<&str, u32> = HashMap::new();
            for t in &terms {
                *tf.entry(t.as_str()).or_default() += 1;
            }
            for (term, freq) in tf {
                postings.entry(term.to_string()).or_default().push((i, freq));
            }
        }

        let total: u64 = lengths.iter().map(|&l| l as u64).sum();
        let avg_len = if lengths.is_empty() {
            0.0
        } else {
            total as f64 / lengths.len() as f64
        };

        Index { chunks, postings, titles, lengths, avg_len }
    }

    /// Rank chunks against `query`, best first. Returns at most `limit` hits, and
    /// only those clearing [`MIN_SCORE`] — an empty result is meaningful and must
    /// be reported as "no data", not papered over.
    pub fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
        let terms = tokenize(query);
        if terms.is_empty() || self.chunks.is_empty() {
            return Vec::new();
        }

        let n = self.chunks.len() as f64;
        let mut scores: HashMap<usize, f64> = HashMap::new();

        for term in &terms {
            let Some(posting) = self.postings.get(term) else {
                continue;
            };
            // Classic BM25 IDF, floored at zero so a term present in nearly every
            // chunk can't push a score negative.
            let df = posting.len() as f64;
            let idf = (((n - df + 0.5) / (df + 0.5)) + 1.0).ln().max(0.0);

            for &(i, freq) in posting {
                let tf = freq as f64;
                let len_norm = 1.0 - B + B * (self.lengths[i] as f64 / self.avg_len.max(1.0));
                let mut contribution = idf * (tf * (K1 + 1.0)) / (tf + K1 * len_norm);

                // Name match: strong evidence this chunk is *about* the query.
                if self.titles[i].contains(term) {
                    contribution *= TITLE_BOOST;
                }
                *scores.entry(i).or_default() += contribution;
            }
        }

        let mut hits: Vec<(usize, f64)> =
            scores.into_iter().filter(|&(_, s)| s >= MIN_SCORE).collect();
        hits.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        hits.truncate(limit);

        hits.into_iter()
            .map(|(i, score)| {
                let c = &self.chunks[i];
                Hit {
                    title: c.title.clone(),
                    slug: c.slug.clone(),
                    section: c.section.clone(),
                    text: c.text.clone(),
                    thin: c.thin,
                    reviewed: c.reviewed,
                    score,
                }
            })
            .collect()
    }

    /// Every passage of one substance's entry, in corpus order.
    ///
    /// Search returns fragments; this returns the whole thing, so a reader who
    /// found a promising excerpt can go read what it was excerpted *from*. The
    /// corpus stores a substance's chunks contiguously and already ordered
    /// (Summary first, then harm potential, pharmacology, and so on), so corpus
    /// order is the reading order — no re-sorting.
    ///
    /// `score` is meaningless here and is reported as 0.0: nothing was ranked.
    pub fn entry(&self, slug: &str) -> Vec<Hit> {
        self.chunks
            .iter()
            .filter(|c| c.slug == slug)
            .map(|c| Hit {
                title: c.title.clone(),
                slug: c.slug.clone(),
                section: c.section.clone(),
                text: c.text.clone(),
                thin: c.thin,
                reviewed: c.reviewed,
                score: 0.0,
            })
            .collect()
    }

    /// Every substance in the corpus, alphabetical — the browse list.
    pub fn entries(&self) -> Vec<Entry> {
        let mut out: Vec<Entry> = Vec::new();
        for c in &self.chunks {
            // Chunks of a substance are contiguous, so only the boundary needs a
            // new record; everything else just increments the section count.
            match out.last_mut() {
                Some(e) if e.slug == c.slug => e.sections += 1,
                _ => out.push(Entry {
                    title: c.title.clone(),
                    slug: c.slug.clone(),
                    thin: c.thin,
                    reviewed: c.reviewed,
                    sections: 1,
                }),
            }
        }
        out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        out
    }

    pub fn len(&self) -> usize {
        self.chunks.len()
    }
}

/// Load and index the bundled corpus from the app's resource directory.
pub fn load_bundled(app: &tauri::AppHandle) -> Result<Index, String> {
    use tauri::Manager;
    let path = app
        .path()
        .resolve(RESOURCE_PATH, tauri::path::BaseDirectory::Resource)
        .map_err(|e| format!("knowledge corpus not found: {e}"))?;
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("could not read knowledge corpus: {e}"))?;
    parse(&raw)
}

/// Index a corpus held in memory, with no Tauri resource dir in play — used by
/// the evaluation harness, which runs outside the app bundle.
pub fn load_str(raw: &str) -> Result<Index, String> {
    parse(raw)
}

fn parse(raw: &str) -> Result<Index, String> {
    let corpus: Corpus =
        serde_json::from_str(raw).map_err(|e| format!("malformed knowledge corpus: {e}"))?;
    Ok(Index::build(corpus.chunks))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> Index {
        parse(include_str!("../resources/dosewiki-corpus.json")).expect("corpus parses")
    }

    #[test]
    fn corpus_is_bundled_and_substantial() {
        let idx = index();
        assert!(idx.len() > 5_000, "expected a few thousand chunks, got {}", idx.len());
    }

    #[test]
    fn finds_the_substance_it_is_asked_about() {
        let idx = index();
        let hits = idx.search("LSD tolerance", 5);
        assert!(!hits.is_empty(), "LSD tolerance should retrieve something");
        assert_eq!(hits[0].title, "LSD", "the top hit should be about LSD itself");
    }

    #[test]
    fn chemical_names_survive_tokenization() {
        // The whole corpus is chemical names; a tokenizer that shreds them on
        // hyphens and digits is useless here.
        let toks = tokenize("2C-B and 5-HT2A");
        assert!(toks.contains(&"2c-b".to_string()), "hyphenated name kept whole: {toks:?}");
        assert!(toks.contains(&"2c".to_string()), "and split into parts: {toks:?}");
        assert!(toks.contains(&"5-ht2a".to_string()), "receptor name kept: {toks:?}");
    }

    #[test]
    fn nonsense_retrieves_nothing_rather_than_noise() {
        // Rule 3: empty retrieval must be possible, so the caller can say "I don't
        // know" instead of handing a model thin noise to confabulate around.
        let idx = index();
        assert!(idx.search("zzzzq xxqqzz", 5).is_empty());
    }

    #[test]
    fn hits_carry_their_coverage_flags() {
        let idx = index();
        // Substances thin enough to be flagged must exist and be searchable, with
        // the flag intact — that flag is the only thing standing between a sparse
        // upstream entry and a confidently-worded answer.
        let hits = idx.search("2-Chloroephenidine", 3);
        assert!(!hits.is_empty());
        assert!(hits[0].thin, "sparse DoseWiki entry should arrive flagged thin");
    }

    #[test]
    fn an_entry_reads_whole_from_a_search_hit() {
        // The point of the feature: a hit must be openable into the full entry
        // it came from, and that entry must contain the hit.
        let idx = index();
        let hit = idx.search("LSD tolerance", 1).remove(0);
        let entry = idx.entry(&hit.slug);
        assert!(entry.len() > 1, "a real entry has several sections");
        assert!(entry.iter().all(|c| c.title == hit.title));
        assert!(
            entry.iter().any(|c| c.section == hit.section && c.text == hit.text),
            "the excerpt must appear in the entry it was excerpted from"
        );
    }

    #[test]
    fn unknown_slug_reads_empty_rather_than_panicking() {
        assert!(index().entry("not-a-substance").is_empty());
    }

    #[test]
    fn every_substance_is_browsable() {
        let idx = index();
        let entries = idx.entries();
        assert!(entries.len() > 500, "got {} substances", entries.len());
        // Alphabetical, and each one actually opens.
        let titles: Vec<String> = entries.iter().map(|e| e.title.to_lowercase()).collect();
        let mut sorted = titles.clone();
        sorted.sort();
        assert_eq!(titles, sorted, "browse list should be alphabetical");
        assert_eq!(
            entries.iter().map(|e| e.sections).sum::<usize>(),
            idx.len(),
            "section counts must account for every chunk (slugs are contiguous)"
        );
        let lsd = entries.iter().find(|e| e.title == "LSD").expect("LSD is in the corpus");
        assert_eq!(idx.entry(&lsd.slug).len(), lsd.sections);
    }

    #[test]
    fn licensing_excluded_field_is_absent() {
        // `subjective_effects` is CC BY-SA-derived and must never enter the corpus
        // (ROADMAP.md #1). If someone re-adds it to corpus.py, this fails.
        //
        // Assert on the *fingerprint* of that content -- its attribution block --
        // not on the phrase "subjective effects", which occurs innocently all over
        // the CC0 prose ("its subjective effects resemble LSD's"). Matching the
        // phrase would fail on clean data and teach the next person to delete the
        // test.
        let raw = include_str!("../resources/dosewiki-corpus.json").to_lowercase();
        for marker in ["josie kins", "forked from subjective effect", "disregardeverythingisay"] {
            assert!(
                !raw.contains(marker),
                "CC BY-SA subjective_effects content leaked into the corpus \
                 (found {marker:?}) — licensing violation, see ROADMAP.md #1"
            );
        }
    }

    #[test]
    fn a_substance_outranks_its_own_analogues() {
        // "LSD" must not be beaten by 1V-LSD / 1P-LSD merely because their names
        // contain "lsd" and their chunks are shorter. Name evidence requires a
        // whole-name match; see `whole_tokens`.
        let idx = index();
        let hits = idx.search("LSD", 5);
        assert_eq!(hits[0].title, "LSD", "got {:?}", hits[0].title);
    }
}
