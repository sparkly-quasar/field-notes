#!/usr/bin/env python3
# SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
"""Build the offline knowledge corpus from the DoseWiki snapshot.

Reads `SubstanceIndex.json` (the CC0 DoseWiki export) and writes
`src-tauri/resources/dosewiki-corpus.json` — the prose, split into retrievable
chunks, bundled as a Tauri resource and searched offline by `knowledge.rs`
(BM25, no embedding model, no network).

    python3 corpus.py

--------------------------------------------------------------------------------
LICENSING — `subjective_effects` IS DELIBERATELY EXCLUDED. DO NOT ADD IT.
--------------------------------------------------------------------------------
DoseWiki declares its whole export CC0, but every substance carrying a
`subjective_effects` block (132 of 577) also carries an embedded attribution:

    author: "Josie Kins"
    text:   "Forked from Subjective Effect Documentation by Josie Kins, ..."
    url:    <archived PsychonautWiki page>  # PsychonautWiki is CC BY-SA

No other prose field has an attribution block. Content forked from a CC BY-SA
source is not CC0-able unless the author relicensed it, and the record's own
`license`/`source` fields are null, so provenance is unresolved. It may well be
fine -- but the project ships CC0-only data and does not bet a public repo on
someone else's licensing assertion over content that arrives with a named-author
credit. See ROADMAP.md #1. The cost is "what does it feel like" prose, which is
not safety-critical.

`ALLOWED_FIELDS` is an allowlist, not a denylist, so a new DoseWiki field can
never be ingested by accident -- it has to be added here on purpose.
"""

import json
import pathlib
import re
import sys

HERE = pathlib.Path(__file__).parent
SRC = HERE / "SubstanceIndex.json"
OUT = HERE.parent.parent / "src-tauri" / "resources" / "dosewiki-corpus.json"

# Bump when regenerating from a fresh download (keep in sync with pw.rs).
SNAPSHOT = "2026-07-08"

# Allowlist. Every field ingested into the corpus must be listed here, and every
# one below is unambiguously CC0 (no attribution block anywhere in the export).
ALLOWED_FIELDS = [
    "summary",
    "harm_potential",
    "pharmacology",
    "tolerance",
    "interactions",
    "legality",
    "history_culture",
]

# Named so a future reader trips over it before re-adding the field.
EXCLUDED_FOR_LICENSING = {"subjective_effects"}

# Chunks are kept small enough to be a useful retrieval unit and to sit
# comfortably in a local model's context when several are returned at once.
MAX_CHARS = 1200

CITE_RE = re.compile(r"\[cite:[^\]]*\]")
WS_RE = re.compile(r"[ \t]+")


def clean(text: str) -> str:
    """Strip DoseWiki's inline [cite:...] markers and normalise whitespace."""
    text = CITE_RE.sub("", text or "")
    text = WS_RE.sub(" ", text)
    text = re.sub(r"\n{3,}", "\n\n", text)
    return text.strip()


def split(text: str, limit: int = MAX_CHARS) -> list[str]:
    """Split overlong prose on paragraph, then sentence, boundaries."""
    text = text.strip()
    if len(text) <= limit:
        return [text] if text else []

    out, buf = [], ""
    units = [p for p in text.split("\n\n") if p.strip()]
    # A single paragraph can still exceed the limit; fall back to sentences.
    expanded = []
    for u in units:
        if len(u) <= limit:
            expanded.append(u)
        else:
            expanded.extend(re.split(r"(?<=[.!?]) +", u))

    for u in expanded:
        if buf and len(buf) + len(u) + 1 > limit:
            out.append(buf.strip())
            buf = u
        else:
            buf = f"{buf} {u}".strip() if buf else u
    if buf.strip():
        out.append(buf.strip())
    return [c for c in out if c]


def label(*parts: str) -> str:
    """Human-readable section label, e.g. 'Harm potential — addiction'.

    The leading field name is capitalised so walked sections match the hand-written
    ones ('Pharmacology', not 'pharmacology') — they are shown to the user and fed
    to the model, and two spellings of one section reads like two sections.
    """
    clean_parts = [p.replace("_", " ").strip() for p in parts if p]
    if not clean_parts:
        return ""
    head, *rest = clean_parts
    return " — ".join([head[:1].upper() + head[1:], *rest])


def walk_descriptions(node, trail=()):
    """Yield (trail, text) for every prose string nested under a dict/list.

    DoseWiki nests prose irregularly (`harm_potential.addiction.psychological
    .description`, `history_culture.sections[].content`, ...), so rather than
    hard-coding every path we walk and pick up the strings, keeping the key trail
    as the section label.
    """
    if isinstance(node, str):
        if len(node.strip()) > 40:  # skip enum-ish values ("extremely_low")
            yield trail, node
    elif isinstance(node, dict):
        for k, v in node.items():
            # `level`/`status`/`tag` are short enum labels, not prose.
            if k in ("level", "status", "tag", "attribution"):
                continue
            yield from walk_descriptions(v, trail + (k,))
    elif isinstance(node, list):
        for item in node:
            yield from walk_descriptions(item, trail)


def chunks_for(sub: dict) -> list[dict]:
    title = sub.get("title") or ""
    slug = sub.get("slug") or ""
    out = []

    def add(section: str, text: str):
        for piece in split(clean(text)):
            out.append({"title": title, "slug": slug, "section": section, "text": piece})

    # summary — a plain string.
    add("Summary", sub.get("summary") or "")

    # harm_potential / pharmacology / tolerance / history_culture — nested prose.
    for field in ("harm_potential", "pharmacology", "tolerance", "history_culture"):
        for trail, text in walk_descriptions(sub.get(field)):
            # Drop the noisy leaf keys that carry no meaning in a label.
            keep = [p for p in trail if p not in ("description", "content", "sections")]
            add(label(field, *keep), text)

    # tolerance also has a cross_tolerance list of bare names — useful to match on.
    cross = (sub.get("tolerance") or {}).get("cross_tolerance") or []
    if cross:
        add("Tolerance — cross-tolerance", "Cross-tolerant with: " + ", ".join(cross))

    # pharmacology.receptor_profile — bare tags, worth indexing as one chunk.
    receptors = (sub.get("pharmacology") or {}).get("receptor_profile") or []
    tags = [r.get("tag") or r.get("receptor") or "" for r in receptors]
    tags = [t for t in tags if t]
    if tags:
        add("Pharmacology — receptor profile", "Binding profile: " + "; ".join(tags))

    # interactions — one chunk per severity tier, so a retrieved chunk always
    # carries its own severity. Never merge the tiers.
    inter = sub.get("interactions") or {}
    for tier in ("dangerous", "unsafe", "caution"):
        entries = inter.get(tier) or []
        if entries:
            body = "\n".join(f"- {e}" for e in entries)
            add(f"Interactions — {tier}", f"{title} — {tier} combinations:\n{body}")

    # legality — one chunk per substance. Per-country chunks would explode the
    # corpus (389 substances x ~20 countries) for little retrieval benefit, so
    # collapse to "Country — status: notes" with the notes trimmed.
    countries = (sub.get("legality") or {}).get("countries") or {}
    if countries:
        lines = []
        for country, info in sorted(countries.items()):
            info = info or {}
            status = (info.get("status") or "").strip()
            notes = clean(info.get("notes") or "")
            if len(notes) > 200:
                notes = notes[:200].rsplit(" ", 1)[0] + "…"
            lines.append(f"- {country}: {status}{' — ' + notes if notes else ''}")
        add("Legality", f"Legal status of {title} by country:\n" + "\n".join(lines))

    return out


# Below this many characters of prose, DoseWiki's entry is too thin to answer
# from with any confidence. Chunks from such substances are flagged `thin` so the
# Companion can say "the entry for this is sparse" instead of confabulating around
# it. Chosen from the observed distribution: min 86 / median 2,705 / max 33,332,
# with 23 substances under 500 chars and 233 under 2k.
THIN_CHARS = 2000


def prose_chars(sub: dict) -> int:
    """Total CC0 prose available for a substance — our coverage proxy."""
    def size(node):
        if isinstance(node, str):
            return len(node)
        if isinstance(node, dict):
            return sum(size(v) for v in node.values())
        if isinstance(node, list):
            return sum(size(v) for v in node)
        return 0

    return sum(size(sub.get(f)) for f in ALLOWED_FIELDS)


def main() -> int:
    if not SRC.exists():
        print(f"missing {SRC} — see README.md for the download URL", file=sys.stderr)
        return 1

    subs = json.loads(SRC.read_text())

    corpus, dropped, thin_subs = [], 0, set()
    for sub in subs:
        # Guard: prove the excluded field never reaches a chunk.
        for field in EXCLUDED_FOR_LICENSING:
            if sub.pop(field, None) is not None:
                dropped += 1

        # Coverage signals travel WITH every chunk. DoseWiki's own editors mark
        # 93% of entries `review: needed`, and prose volume varies ~400x between
        # substances -- and it tracks fame, not risk (LSD/MDMA are rich; obscure
        # research chemicals are nearly empty). A retrieved chunk must therefore
        # carry enough context for the caller to know how much to trust it.
        # See ROADMAP.md #1 "Accuracy is uneven".
        chars = prose_chars(sub)
        reviewed = ((sub.get("editorial_review") or {}).get("status")) == "completed"
        thin = chars < THIN_CHARS
        if thin:
            thin_subs.add(sub.get("slug"))

        for c in chunks_for(sub):
            c["id"] = len(corpus)
            c["thin"] = thin
            c["reviewed"] = reviewed
            corpus.append(c)

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(
        json.dumps(
            {
                "snapshot": SNAPSHOT,
                "source": "DoseWiki (dose.wiki) — CC0 public domain",
                "excluded": sorted(EXCLUDED_FOR_LICENSING),
                "thin_chars": THIN_CHARS,
                "chunks": corpus,
            },
            ensure_ascii=False,
        )
    )

    subs_with = len({c["slug"] for c in corpus})
    size_mb = OUT.stat().st_size / 1_048_576
    print(f"wrote {OUT.relative_to(HERE.parent.parent)}")
    print(f"  {len(corpus):,} chunks across {subs_with} substances ({size_mb:.1f} MB)")
    print(f"  {len(thin_subs)} substances flagged thin (<{THIN_CHARS} chars of prose)")
    print(f"  excluded {dropped} subjective_effects blocks (licensing — see docstring)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
