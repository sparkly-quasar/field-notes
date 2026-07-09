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

## Shipped so far (v0.3.0 — DoseWiki, encryption, Obsidian & tool-enabled Companion)

- **Journal** — experiences, doses, and a live timeline in a local **SQLite** DB
  (`db.rs`), with **edit/delete** everywhere and **backdating** (log past
  experiences at their real time).
- **Local Companion** (`ollama.rs`) — a calm, non-judgmental harm-reduction chat
  that talks **only** to a model on your machine via Ollama (`127.0.0.1`, nothing
  leaves the device). It's *session-aware*: it reads a read-only summary of the
  current experience's doses and interaction flags (opt-in "share session").
- **Text import** — paste a past experience in plain words; the local model
  extracts substances, doses, and timeline into a **review-before-save** preview.
- **Dose reference** (`pw.rs`) — a **bundled, fully-offline** reference of dose
  ranges, durations (onset/come-up/peak/offset/after-effects/total + half-life),
  routes, and **graded** interactions. Sourced from **DoseWiki** (**577 substances,
  CC0 public domain**), slimmed to ~0.9 MB (`data/dosewiki/slim.py`) and shipped as a
  Tauri resource loaded into the cache on launch — **no network call at all**. A
  courtesy DoseWiki credit is shown in-app. *(Migrated off PsychonautWiki's live
  CC-BY-SA GraphQL scrape — shipped in v0.3.0.)*
- **Encryption at rest + backup/restore** (`crisis.rs` aside, in `db.rs`/`commands.rs`) —
  opt-in **SQLCipher** passphrase encryption (AES-256); the app opens to an **unlock
  screen** when the journal is encrypted. Enable/disable/change-passphrase and
  single-file **VACUUM INTO backups** + restore, all in a **Data & security** tab.
  The startup disclaimer can be dismissed ("don't show again"). *(v0.3.0.)*
- **Obsidian vault sync** (`obsidian.rs`) — **bidirectional, fully offline**. Export
  each experience as a readable Markdown note (frontmatter + doses/timeline) with a
  canonical ```fieldnotes``` block for lossless round-trips; import reads that block
  back (vault wins on conflicts), leaving hand-written notes untouched. *(v0.3.0.)*
- **Tool-enabled Companion + live session + crisis guardrails** — the Companion can
  now **act** via tools (log doses/notes, session status, dose/interaction lookups)
  at the user's request; a calm **live-session** workspace (elapsed time, running
  timeline, one-tap logging, panic button) supports altered states; and a
  **deterministic crisis layer** (`crisis.rs`) surfaces graded, localized emergency
  resources **independent of the model**. System prompt follows the Zendo four
  principles + Fireside stance with consent-based support-style intake. *(v0.3.0.)*
- **Deterministic safety checker** (`interactions.rs`) — flags dangerous combos,
  wired to the dose-reference interaction data; DoseWiki's dangerous/unsafe/caution
  tiers map onto our danger/caution/note severities (with the reason text), and
  inline dose-range + interaction warnings appear while logging a dose.
- **Distribution** — cross-platform signed installers (macOS universal `.dmg` +
  Linux `.AppImage`/`.deb`/`.rpm`) via `tauri-action` CI on `v*` tags, plus
  **in-app auto-update** (Tauri updater; "Install & restart" banner). macOS is
  currently **unsigned** (right-click → Open on first launch) pending an Apple
  Developer ID.

---

## Roadmap (not yet built)

> ✅ **Shipped in v0.3.0:** DoseWiki migration, encryption-at-rest + backup/restore,
> Obsidian vault sync, and the tool-enabled Companion + live session + crisis
> guardrails — see "Shipped so far" above. The remaining items are below.

1. **Substance knowledge pack — offline RAG corpus.** Beyond the structured dose
   data, add a **retrieval corpus** over openly-licensed full-text sources for
   richer Q&A (semantic search, not just dose lookups). Sources: **DoseWiki (CC0)**
   and **Shulgin's PiHKAL/TiHKAL Part 2 compound data** (Shulgin's qualitative
   bioassay reports are unique full-text that DoseWiki's structured data doesn't have
   — additive here, not for the dose reference). ⚠️ **Licensing must be settled
   first:** DoseWiki's CC0 text is unencumbered, but **PiHKAL / TiHKAL are *not*
   public domain.** **Part 1 (the autobiographical narrative) is all-rights-reserved
   — pointer / user-import only.** **Part 2 (the compound entries)** may be reproduced
   **non-commercially** with the copyright / cautionary / ordering notices attached;
   the Isomer Design database (isomerdesign.com) presents this as **CC BY-NC-SA 4.0**.
   So Part 2 *can* be bundled as a **separate non-commercial, share-alike pack with
   the required notices** — but NC forecloses any future commercial-license path for
   that pack, so keep it **separately licensed**, never mixed into the PolyForm code.

2. **User-added substances → opt-in upstream contribution.** Local CRUD for
   uncatalogued substances already exists; add a **consent-gated** export that
   generates a draft the user reviews and submits manually to the upstream source
   (**DoseWiki** — it's CC0 and open-source). **Never auto-upload** — this is
   sensitive, legally fraught data.

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

- **Data licensing** — the dose reference now ships from **DoseWiki (CC0, public
  domain)**, bundled freely with only a courtesy credit — no share-alike, no
  attribution obligation. Any *additional* CC-BY-SA sources (e.g. a
  RAG corpus from PsychonautWiki/TripSit) must still ship as a separate,
  attributed, share-alike pack kept out of the PolyForm-licensed source.
  **PiHKAL/TiHKAL** are *not* public domain: Part 1 (narrative) is all-rights-reserved
  (pointer/user-import only); Part 2 (compound data, e.g. via Isomer Design's
  CC BY-NC-SA) is bundleable non-commercially **as a separate pack with the required
  notices attached**.
- **Safety** — the model must **retrieve** dosage/interaction facts, never invent
  them; keep the interaction checker **deterministic**; harm-reduction framing
  (never encouragement, never synthesis/sourcing help); always surface emergency
  guidance.
- **Privacy** — everything stays on-device and offline; treat the journal as
  sensitive (encryption, no telemetry, no network lookups that leak what a user
  is researching).

## Suggested next increment

The v0.3.0 batch (DoseWiki, encryption + backup, Obsidian sync, tool-enabled
Companion + live session + crisis guardrails) is **shipped**. Of what remains:
**#2 (opt-in upstream contribution of user-added substances)** is the smaller,
self-contained next step; **#1 (RAG corpus)** stays gated on the licensing
decision for any share-alike sources (the DoseWiki CC0 slice is unencumbered, but
a PiHKAL/TiHKAL Part 2 pack must ship separately with its NC/share-alike notices).

---

## Related project — Cairn

`sparkly-quasar/cairn` (public) — the guided local-LLM installer Field Notes
builds on. Shipped through Phase 3 (Simple setup + Explore catalog + Remote
access) with signed installers + auto-update. **Remaining:** Phase 4 — advanced
mode (quantization / context length / logs) and a bundled Python sidecar to drop
the Docker dependency. (App self-update already shipped.)
