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
- **Dose reference** (`pw.rs`) — an **offline, on-request cache** of dose ranges,
  durations, routes, and dangerous interactions. One "update database" fetch, then
  all lookups are offline/private. Currently sourced from **PsychonautWiki**
  (~373 substances, CC-BY-SA 4.0, attribution in-app + NOTICE). **Being migrated to
  DoseWiki** — see roadmap item #1 below.
- **Deterministic safety checker** (`interactions.rs`) — flags dangerous combos,
  wired to the dose-reference interaction data; inline dose-range + interaction
  warnings appear while logging a dose.
- **Distribution** — cross-platform signed installers (macOS universal `.dmg` +
  Linux `.AppImage`/`.deb`/`.rpm`) via `tauri-action` CI on `v*` tags, plus
  **in-app auto-update** (Tauri updater; "Install & restart" banner). macOS is
  currently **unsigned** (right-click → Open on first launch) pending an Apple
  Developer ID.

---

## Roadmap (not yet built)

1. **Switch the dose reference from PsychonautWiki to DoseWiki (CC0).** DoseWiki
   (<https://dose.wiki>) publishes its whole encyclopedia as a single **public-domain
   (CC0)** static file, `SubstanceIndex.json` — **577 substances**, richer than what
   we scrape from PsychonautWiki's live GraphQL API: graded interactions
   (dangerous / unsafe / caution), full duration stages (onset, come-up, peak,
   offset, after-effects, half-life), and per-route dose ranges. The snapshot is
   **already staged in `data/dosewiki/`** with a full schema→`PwInfo` mapping and
   integration plan in [`data/dosewiki/README.md`](data/dosewiki/README.md).
   Because it's one CC0 file, we can **bundle it offline** and drop the live-API
   dependency (and the CC-BY-SA share-alike constraint) entirely. **Do this first —
   it simplifies the licensing story below.** Work: rewrite `pw.rs`'s fetch/parse to
   read DoseWiki's JSON, map graded interactions onto our danger/caution/note model,
   reword the `interactions.rs` message + in-app attribution, update NOTICE to CC0.

2. **Substance knowledge pack — offline RAG corpus.** Beyond the structured dose
   data, add a **retrieval corpus** over openly-licensed full-text sources for
   richer Q&A (semantic search, not just dose lookups). Sources: **DoseWiki (CC0)**,
   **PsychonautWiki (CC-BY-SA)**, **TripSit**. ⚠️ **Licensing must be settled first**
   for the share-alike sources — DoseWiki's CC0 text is unencumbered, but mixing in
   CC-BY-SA material re-imposes attribution + share-alike. **PiHKAL / TiHKAL are
   copyrighted and cannot be bundled — pointer / user-import only.** Keep any
   CC-BY-SA corpus as a **separately-licensed data pack**, never mixed into the
   PolyForm code.

3. **Local LLM companion with tool access + live-session mode.** Today the
   Companion only *reads* injected context. Give the model **tools** to actually
   drive the journal from chat ("log 100 mg MDMA", "how am I doing?"), plus a
   calm, altered-state-friendly **live session UI** (large text, one-tap logging,
   running timeline, panic → grounding). Fully offline, against a local model
   (via Cairn / Ollama). **The support model, session intake, and crisis
   guardrails are specified in [Companion design principles](#companion-design-principles-peer-support-model)
   below — that spec is load-bearing, not optional polish.**

4. **Obsidian vault integration.** Read/parse journal entries from an Obsidian
   vault and write **structured experience summaries** back. Bidirectional, fully
   offline. (Reuse the filesystem/obsidian-MCP patterns from prior work.)

5. **User-added substances → opt-in upstream contribution.** Local CRUD for
   uncatalogued substances already exists; add a **consent-gated** export that
   generates a draft the user reviews and submits manually to the upstream source
   (**DoseWiki** once the migration lands; it's CC0 and open-source). **Never
   auto-upload** — this is sensitive, legally fraught data.

6. **Encrypted-at-rest database (passphrase).** Protect this sensitive, local-only
   data with an encrypted DB (e.g. SQLCipher + passphrase). Pair with
   **export / backup & restore** so data survives a lost DB or a machine switch.

---

## Companion design principles (peer-support model)

The Companion is modeled on established **psychedelic peer-support** practice —
the **Zendo Project's Four Principles** and the **Fireside Project's**
non-directive, compassionate approach. It is a *peer sitter*, **not** a therapist,
guide, or medical authority, and it says so.

**The Four Principles (Zendo):**
1. **Create a safe space** — calm, warm, reassuring, non-judgmental.
2. **Sitting, not guiding** — follow the person's experience; don't steer,
   interpret, analyze, or impose an agenda.
3. **Talk through, not down** — stay present *with* difficult material instead of
   trying to shut it down or "rescue" the person; be a companion, not a fixer.
4. **Difficult is not the same as bad** — hard moments can be meaningful; don't
   pathologize them.

**Fireside-inspired stance:** meet people exactly where they are; empower their
own process; active, present listening; never medical or legal advice; fully
confidential and on-device.

**Session intake / check-in (before and at the start of a session):**
- *What kind of session are you planning?* — substance(s), rough dose, setting,
  solo or with others, intention (reuse journal data where it already exists).
- Experience level and any worries going in.
- **What kind of support do you want?** Offer concrete modes and let the user
  pick (and change anytime): *mostly just listen · help me stay grounded · talk
  me through the hard parts · stay quiet unless I reach out · gentle periodic
  check-ins · practical reminders (water, rest, breathing)*.
- Store the chosen support style and **honor it**; proactively re-offer to adjust
  ("want more space, or more check-ins?"). The user sets the tone; the Companion
  calibrates to it — consent-based support.

**Crisis guardrails — direct to real-world help (deterministic, not left to the
model's judgment):**
- The Companion is **not** an emergency service and must state that plainly. When
  red flags appear it **calmly directs to IRL help** rather than trying to manage
  the situation itself, and it never discourages seeking help or tries to talk
  someone out of calling for it.
- **Medical emergency signs → call emergency services (911 / local number):**
  unresponsiveness, seizures, chest pain, trouble breathing, dangerously high
  body temperature, signs of serotonin syndrome (see the interaction checker),
  relentless vomiting, or anything the interaction checker flags as dangerous.
  US Poison Control: **1-800-222-1222**.
- **Psychiatric emergency signs → get IRL help now:** suicidal or self-harm
  intent, intent to harm others, or acute distress that isn't easing. US Suicide
  & Crisis Lifeline: **988**. Encourage getting a trusted sober person present.
- **Non-emergency peer support:** **Fireside Project** psychedelic peer-support
  line — call/text **62-FIRESIDE (623-473-7433)** (US).
- **Implementation:** a **deterministic escalation layer** — symptom/intent
  detection (plus interaction-checker signals) that surfaces a persistent,
  unmissable **"Get help now"** banner with the right numbers, *independent of the
  model*. The live-session **panic button** opens the same always-available
  emergency-resources screen. Localize numbers where feasible (911 US, 112 EU,
  etc.). The model's system prompt reinforces the same escalation behavior but is
  never the sole safety net.

**Hard boundaries:** no medical or dosing prescriptions; never encourage (re)dosing;
no synthesis/sourcing help; always defer to trained humans for anything beyond
emotional presence.

## Cross-cutting constraints (read before building)

- **Data licensing** — the dose reference is moving to **DoseWiki (CC0, public
  domain)**, which can be bundled freely with only a courtesy credit — no
  share-alike, no attribution obligation. Any *additional* CC-BY-SA sources (e.g. a
  RAG corpus from PsychonautWiki/TripSit) must still ship as a separate,
  attributed, share-alike pack kept out of the PolyForm-licensed source.
  PiHKAL/TiHKAL: pointer/user-import only.
- **Safety** — the model must **retrieve** dosage/interaction facts, never invent
  them; keep the interaction checker **deterministic**; harm-reduction framing
  (never encouragement, never synthesis/sourcing help); always surface emergency
  guidance.
- **Privacy** — everything stays on-device and offline; treat the journal as
  sensitive (encryption, no telemetry, no network lookups that leak what a user
  is researching).

## Suggested next increment

Do **#1 (DoseWiki migration)** next — it's staged in `data/dosewiki/`, low-risk, and
clears the licensing story for everything after it. Then highest value / lowest cost:
**#6 encryption + export** (protect the data), then **#4 Obsidian** or **#3
tool-enabled Companion** as the next headline feature. **#2 (RAG corpus)** stays
gated on the licensing decision for any share-alike sources.

---

## Related project — Cairn

`sparkly-quasar/cairn` (public) — the guided local-LLM installer Field Notes
builds on. Shipped through Phase 3 (Simple setup + Explore catalog + Remote
access) with signed installers + auto-update. **Remaining:** Phase 4 — advanced
mode (quantization / context length / logs) and a bundled Python sidecar to drop
the Docker dependency. (App self-update already shipped.)
