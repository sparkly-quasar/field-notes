# Field Notes — Roadmap

**Field Notes** is an offline, private harm-reduction journal & trip-sitting
workstation for psychonauts and all other explorers. It runs entirely on-device
(Tauri + Svelte + local Ollama), and is licensed **PolyForm Noncommercial 1.0.0**
(free for non-commercial use; commercial use requires a contract with the author).

> This file is the durable reference for where the project stands and what's next
> — start here when picking the project back up in a new session.

Repo: `sparkly-quasar/field-notes` (public). Built on the local-LLM stack from
its sibling project **`sparkly-quasar/cairn`** (the general-purpose local-LLM
installer).

---

## Shipped so far (through v0.2.0)

- **Journal** — experiences, doses, and a live timeline in a local **SQLite** DB
  (`db.rs`), with **edit/delete** everywhere and **backdating** (log past
  experiences at their real time).
- **Local Companion** (`ollama.rs`) — a calm, non-judgmental harm-reduction chat
  that talks **only** to a model on your machine via Ollama (`127.0.0.1`, nothing
  leaves the device). It's *session-aware*: it reads a read-only summary of the
  current experience's doses and interaction flags (opt-in "share session").
- **Text import** — paste a past experience in plain words; the local model
  extracts substances, doses, and timeline into a **review-before-save** preview.
- **PsychonautWiki dose reference** (`pw.rs`) — an **offline, on-request cache**
  (~373 substances) of dose ranges, durations, routes, and dangerous
  interactions. One "update database" fetch, then all lookups are offline/private.
  Attribution shown in-app + NOTICE (CC-BY-SA 4.0).
- **Deterministic safety checker** (`interactions.rs`) — flags dangerous combos,
  now wired to PsychonautWiki's interaction data; inline dose-range + interaction
  warnings appear while logging a dose.
- **Distribution** — cross-platform signed installers (macOS universal `.dmg` +
  Linux `.AppImage`/`.deb`/`.rpm`) via `tauri-action` CI on `v*` tags, plus
  **in-app auto-update** (Tauri updater; "Install & restart" banner). macOS is
  currently **unsigned** (right-click → Open on first launch) pending an Apple
  Developer ID.

---

## Roadmap (not yet built)

1. **Substance knowledge pack — offline RAG corpus.** Beyond the structured
   PsychonautWiki dose data already shipped, add a **retrieval corpus** over
   openly-licensed full-text sources for richer Q&A (semantic search, not just
   dose lookups). Sources: **PsychonautWiki (CC-BY-SA)** + **TripSit**.
   ⚠️ **Licensing must be settled first.** **PiHKAL / TiHKAL are copyrighted and
   cannot be bundled — pointer / user-import only.** Ship the corpus as a
   **separately-licensed CC-BY-SA data pack**, never mixed into the PolyForm code.

2. **Local LLM companion with tool access + live-session mode.** Today the
   Companion only *reads* injected context. Give the model **tools** to actually
   drive the journal from chat ("log 100 mg MDMA", "how am I doing?"), plus a
   calm, altered-state-friendly **live session UI** (large text, one-tap logging,
   running timeline, panic → grounding). Fully offline, against a local model
   (via Cairn / Ollama).

3. **Obsidian vault integration.** Read/parse journal entries from an Obsidian
   vault and write **structured experience summaries** back. Bidirectional, fully
   offline. (Reuse the filesystem/obsidian-MCP patterns from prior work.)

4. **User-added substances → opt-in PsychonautWiki contribution.** Local CRUD for
   uncatalogued substances already exists; add a **consent-gated** export that
   generates a PsychonautWiki-formatted draft the user reviews and submits
   manually. **Never auto-upload** — this is sensitive, legally fraught data.

5. **Encrypted-at-rest database (passphrase).** Protect this sensitive, local-only
   data with an encrypted DB (e.g. SQLCipher + passphrase). Pair with
   **export / backup & restore** so data survives a lost DB or a machine switch.

---

## Cross-cutting constraints (read before building)

- **Data licensing** — ship substance data as a separate CC-BY-SA pack with
  attribution + share-alike; keep it out of the PolyForm-licensed source.
  PiHKAL/TiHKAL: pointer/user-import only.
- **Safety** — the model must **retrieve** dosage/interaction facts, never invent
  them; keep the interaction checker **deterministic**; harm-reduction framing
  (never encouragement, never synthesis/sourcing help); always surface emergency
  guidance.
- **Privacy** — everything stays on-device and offline; treat the journal as
  sensitive (encryption, no telemetry, no network lookups that leak what a user
  is researching).

## Suggested next increment

Highest value / lowest cost: **#5 encryption + export** (protect the data), then
**#3 Obsidian** or **#2 tool-enabled Companion** as the next headline feature.
**#1 (RAG corpus)** is gated on the data-licensing decision — settle that first.

---

## Related project — Cairn

`sparkly-quasar/cairn` (public) — the guided local-LLM installer Field Notes
builds on. Shipped through Phase 3 (Simple setup + Explore catalog + Remote
access) with signed installers + auto-update. **Remaining:** Phase 4 — advanced
mode (quantization / context length / logs) and a bundled Python sidecar to drop
the Docker dependency. (App self-update already shipped.)
