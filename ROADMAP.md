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

## Shipped so far (v0.3.1 — DoseWiki, encryption, Obsidian & tool-enabled Companion)

> v0.3.1 follow-ups on top of the v0.3.0 batch: in-app **model management**
> (switch/download models any time), **encrypted backups** even for a plaintext
> journal, an **Obsidian "sync at your own risk"** notice (exported notes are
> plaintext outside the app's encryption), **erase-all-data & uninstall** helpers,
> "password" wording, and a **Settings** tab (formerly "Data & security").

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
  single-file **VACUUM INTO backups** + restore, all in a **Settings** tab.
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

## Built, not yet released (in `main`)

### Offline knowledge corpus — DoseWiki prose, BM25, no embedding model

`data/dosewiki/corpus.py` → `src-tauri/resources/dosewiki-corpus.json`
(**7,823 chunks / 575 substances / 3.6 MB**), searched in-process by
`knowledge.rs`. Reachable three ways: the Companion's `search_knowledge` tool, the
`knowledge_search` command, and a **Search the reference** card in the Substances
tab. The index is held in `Knowledge` state **independent of `Db`**, so it works
while the journal is locked — it's public CC0 data, not user data.

The rules below are **load-bearing**. They are why this is safe to ship; read
them before touching any of it.

### Upstream contribution drafts (`contribute.rs`)

A user-added substance DoseWiki doesn't cover becomes a **DoseWiki-shaped JSON
draft** the user reads, saves to a file, and submits **by hand**. Three
non-negotiables, each with a test:
- **No network. Ever.** There is no HTTP client in `contribute.rs` and there must
  never be one — not even an opt-in auto-upload. The data is legally fraught and
  it is not ours to send.
- **No journal data in a draft.** Built from the *catalogue* row only — never a
  dose, an experience, or a timestamp (including `created_at`: when you first
  catalogued a compound is itself a disclosure). `draft_carries_no_journal_data`
  enforces it.
- **No invented numbers.** Dose/duration blocks ship **empty**. Parsing the user's
  free-text dose note into structured ranges would mean guessing at figures and
  sending the guess upstream with authority it hasn't earned.

---

## Roadmap (not yet built)

> ✅ **Shipped in v0.3.0:** DoseWiki migration, encryption-at-rest + backup/restore,
> Obsidian vault sync, and the tool-enabled Companion + live session + crisis
> guardrails — see "Shipped so far" above. The remaining items are below.

<details>
<summary><strong>Reference — the knowledge-corpus rules (item now built; keep these)</strong></summary>

   **Licensing — settled, both ways (2026-07-12):**
   - ⛔ **PiHKAL / TiHKAL: dropped entirely.** Not worth the encumbrance. The corpus is
     **DoseWiki-only**. *(Previously this item proposed bundling Shulgin's Part 2
     compound data as a separate CC BY-NC-SA pack; that path is closed — don't
     relitigate it.)*
   - ⚠️ **DoseWiki `subjective_effects`: EXCLUDED — do not ingest it.** DoseWiki
     blankets its export as CC0, but this one field (132 of 577 substances, ~370 kB)
     ships an embedded attribution block — `author: "Josie Kins"`, *"Forked from
     Subjective Effect Documentation"* — pointing at archived **PsychonautWiki** pages
     (**CC BY-SA**) and disregardeverythingisay.com. **No other prose field carries any
     attribution block.** Content forked from a BY-SA source isn't CC0-able unless the
     author relicensed, and the `license`/`source` sub-fields are both `null`, so
     provenance is unresolved. It may well be fine (Josie Kins founded PsychonautWiki
     and could relicense her own work) — but we are **not** betting a public repo on
     someone else's licensing assertion over content that ships with a named-author
     credit. It costs only "what does it feel like" prose, which is **not
     safety-critical**.
   - ✅ **Everything else in DoseWiki is unencumbered CC0** and bundles freely with a
     courtesy credit: `summary`, `harm_potential`, `pharmacology`, `interactions`,
     `tolerance`, `legality`, `history_culture`. That's the corpus.
   - *Note: `slim.py` already drops `subjective_effects`, so the shipped dose reference
     was never exposed. The risk only appears when ingesting prose — keep the exclusion
     enforced in `corpus.py`.*

   **Retrieval approach — no embedding model.** Search is **BM25 over the bundled
   corpus, in pure Rust** (`knowledge.rs`), built in memory at launch. Deliberately
   **not** vector embeddings: those would add an Ollama embed-model dependency, a
   multi-minute first-run indexing pass, and a model the user must pull — for a corpus
   of 577 substances where lexical search over named compounds and named interactions is
   what people actually query. Also avoids depending on FTS5 being compiled into the
   SQLCipher amalgamation. Semantic rerank stays available as a later enhancement if
   lexical search proves insufficient in practice.

   ⚠️ **Accuracy is uneven — and it is worst where it matters most.** Measured across
   the snapshot (2026-07-12):
   - **93% of entries (537/577) are marked `editorial_review: "needed"` by DoseWiki's
     own editors.** Exactly **one** is `completed`.
   - **Prose volume varies ~400×:** min 86 chars, median 2,705, max 33,332. **23
     substances are near-empty**, 238 total fall under the 2 kB "thin" line.
   - **Only 338/577 carry citations at all.**
   - **Coverage tracks fame, not risk.** Richest: LSD, MDMA, ketamine, cocaine,
     amphetamine. Thinnest: alpha-PCYP, 5f-PB-22, 3,4-Dichloromethylphenidate — i.e. the
     obscure research chemicals where a user has nowhere else to look and where being
     wrong is most likely to hurt them. **Naive RAG makes this worse**, because a model
     writes equally fluent, equally confident prose whether it retrieved 33,000
     characters or 86.
   - Entries are written by different contributors, so claims can **conflict between
     substances**. Retrieval surfaces chunks; it does not reconcile them.

   **Containment rules (build these in — do not rely on the prompt alone):**
   1. **Dose and interaction facts NEVER come from the corpus.** They come from the
      deterministic layers — `pw.rs` (dose ranges) and `interactions.rs` (combos). The
      corpus answers *"how does this work, what are the risks, what's the tolerance
      profile"*; it is **never** the source of a number or a combo verdict. Keep the
      paths physically separate so retrieval *cannot* override the deterministic
      checker.
   2. **Coverage signals travel with every chunk.** `corpus.py` stamps each chunk with
      `thin` (substance has <2 kB of prose) and `reviewed` (DoseWiki editorial status).
      A retrieved chunk from a thin/unreviewed entry arrives **flagged**, and the
      Companion must say so ("DoseWiki's entry for this is sparse and unreviewed")
      rather than smoothing over it.
   3. **Empty retrieval ⇒ "I don't know", never prose.** Below the score threshold the
      Companion states it has no good data instead of generating.

   All three are implemented: (1) the corpus and the deterministic layers are separate
   code paths that cannot override each other; (2) `corpus.py` stamps `thin`/`reviewed`
   on every chunk and they travel through `knowledge.rs` → the tool result → the UI
   badges; (3) `run_companion_tool` returns an explicit *"no reference material found —
   don't guess"* on empty retrieval.

</details>

1. **Phone portal — log from your phone over Tailscale.** ***Entirely optional; off by
   default.*** Field Notes stays a **fully offline, on-device app as it ships** — this
   adds an opt-in way to reach the journal from a phone during a session (the desktop is
   the trip-sitting workstation; the phone is what's actually in your hand). Ships as a
   **web portal served by the desktop app**, installable to the home screen as a **PWA**
   — *not* a native iOS app (see the decision note below). Two separable phases.

   ### ✅ Phase 3a — the portal (online-only). **BUILT** (`portal.rs`, `/m`).

   The desktop app is the server; the phone is a thin client. It was small, because the
   code already had the right seams. What shipped:
   - **Server** (`src-tauri/src/portal.rs`) — `tiny_http`, `POST /api/<command>`,
     handlers calling the **same** `commands::` functions the desktop calls, so there is
     no second implementation of any safety rule. It also serves the app's own embedded
     frontend (SPA fallback to `index.html`), so the phone runs the same build.
   - **Transport swap** — `src/lib/api.ts` picks Tauri `invoke` on the desktop and
     `fetch` on the phone (`src/lib/portal.ts`). One function; the 1,900-line UI never
     learned about it. **`api.ts` remains the only file allowed to import `invoke`.**
   - **Mobile route** — `/m`: dose, note, combo, Companion. A phone-shaped *subset*, not
     the desktop page made responsive.
   - **PWA shell** — `static/manifest.webmanifest` + Apple meta tags: home-screen icon
     and standalone chrome. The **service worker is Phase 3b**, deliberately absent.
   - **Auth** — a 256-bit bearer token, compared in constant time, paired by scanning a
     QR code in Settings. The token rides in the URL **fragment** (never sent to a
     server, never in a log) and the phone strips it from its address bar on arrival.
   - **Lifecycle** — off by default; refuses to start against a locked journal and
     re-checks on **every** request; dies on quit.

   ⚠️ **The four rules in `portal.rs`'s module docs are load-bearing — read them before
   touching it.** Bind `127.0.0.1` only; token on every request even on the tailnet;
   never serve a locked journal; and `EXPOSED` is an **allowlist**, so a new command in
   `commands.rs` is unreachable from the phone until someone adds it there on purpose.
   Tests pin all four, including that `wipe_all_data`, `unlock_db`, the encryption
   commands, the filesystem commands, and the `portal_*` commands themselves stay
   unreachable.

   **Still to do for 3a:** the user runs `tailscale serve --bg <port>` themselves — the
   app detects Tailscale, shows the exact command, and shows the resulting `*.ts.net`
   URL, but does not run it for them. That's deliberate for now (it's the step that makes
   the journal reachable from another device, and it should be visible), but a one-click
   version behind a confirmation is a reasonable follow-up.

   ### Phase 3b — offline capture. Lets you log while the Mac is asleep or off-tailnet.
   The journal is **append-only** in practice (a dose/note is a new row), so an outbox
   that queues *only new entries* sidesteps real bidirectional sync entirely — **keep it
   that way**; queuing edits/deletes re-opens conflict resolution and is not worth it.
   - **Outbox** — service worker + IndexedDB; local temp IDs reconciled against
     server-assigned IDs on replay. A visible **"N entries pending"** marker — never
     leave the user guessing whether a dose was recorded.
   - ⚠️ **Safety features must not silently go dark offline.** `interactions.rs` and
     `crisis.rs` are deterministic and run on the desktop — offline, the phone would
     happily log a dose while unable to warn that it interacts with what was taken an
     hour ago, and the crisis scan would never fire. **This is a blocker, not a polish
     item.** Fix: compile those two modules to **WASM** and ship them in the PWA (one
     source of truth, still deterministic); cache the bundled DoseWiki file for offline
     dose lookups. The **Companion is genuinely desktop-only** (it needs Ollama) — it
     must **visibly grey out** offline, never fail silently.
   - ⚠️ **Phone-side storage is outside SQLCipher.** Cached journal fragments sit in
     Safari's IndexedDB in the clear, which partly defeats encryption-at-rest. Treat the
     phone as a strict **write-through cache** (flush + purge on sync, cache as little
     for reading as possible); consider encrypting the outbox under a short PIN entered
     on open. **Decide this deliberately** rather than inheriting it by accident.

   **Decision recorded — why not a native iOS app.** Tauri 2 does target iOS, so it's
   possible; it's still the wrong tool. (a) **No App Store** — a substance journal is a
   near-certain rejection under Apple's drug guidelines, and PolyForm-NC complicates it
   further; distribution collapses to sideloading (re-signing every 7 days) or $99/yr.
   (b) **It buys nothing** — a native app is either a thin client to this same server (a
   PWA with an Xcode toolchain bolted on) or it keeps its own DB, which means
   **bidirectional encrypted sync** — conflict resolution plus cross-device key
   management, plausibly a bigger project than all of v0.3.0. (c) The **Companion can't
   run on-device anyway** (no Ollama on iPhone), so the phone is inherently a client.
   The PWA gives the home-screen icon, full-screen chrome, and offline capture without
   Xcode, an Apple account, or a second codebase. Revisit **only** if on-phone-only
   operation (no desktop at all) ever becomes a goal.

   **Known constraint:** the desktop must be **awake** to serve. During a live sit it is,
   by definition. Phase 3b is what makes the asleep case survivable — ship 3a first and
   see how often that actually bites before committing to it.

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

- **Data licensing** — **the project ships CC0 data only.** Both the dose reference and
  the RAG corpus come from **DoseWiki (CC0, public domain)**, bundled freely with a
  courtesy credit — no share-alike, no attribution obligation, nothing to keep separate.
  - ⛔ **PiHKAL / TiHKAL: closed, do not revisit.** Dropped 2026-07-12. Part 1 is
    all-rights-reserved; Part 2 is only reproducible non-commercially with notices
    attached, which would forfeit any future commercial-license path. Not worth it.
  - ⚠️ **"CC0" on a source is a claim, not a guarantee — check the records themselves.**
    DoseWiki declares its whole export CC0, yet its `subjective_effects` field ships a
    named-author attribution block crediting work forked from **CC BY-SA**
    PsychonautWiki. That field is **excluded** (see #1). The general rule this teaches:
    **an embedded attribution/author credit in supposedly-CC0 data is a red flag** —
    grep for one before ingesting any new field or source.
  - Any future **CC-BY-SA** source must ship as a separate, attributed, share-alike pack
    kept out of the PolyForm-licensed source — the bar for taking one on is now high,
    since nothing currently requires it.
- **Safety** — the model must **retrieve** dosage/interaction facts, never invent
  them; keep the interaction checker **deterministic**; harm-reduction framing
  (never encouragement, never synthesis/sourcing help); always surface emergency
  guidance.
- **Privacy** — everything stays on-device and offline; treat the journal as
  sensitive (encryption, no telemetry, no network lookups that leak what a user
  is researching).
  - **The phone portal (#3) does not change this default.** Field Notes remains
    **fully offline as it ships**: the portal is **opt-in and off by default**, and with
    the toggle off the app **opens no socket and makes no network connection at all** —
    the offline guarantee above holds exactly as written, unweakened. Enabling it is a
    **conscious, reversible choice by the user**, not a change to what the app is.
  - **When the user does enable it**, the invariant it must preserve is *"your data
    never reaches a third party"*: traffic goes **device-to-device on your own tailnet**
    — no server on the internet, no account, no relay that can read it, still **no
    telemetry**. Non-negotiables for that to hold: bind **`127.0.0.1` only** (never
    `0.0.0.0` — that would put the journal on whatever café LAN you're on), tailnet-only
    reach via `tailscale serve`, **token auth even on the tailnet**, and **never served
    while the DB is locked**. If a future change can't hold all of those, it doesn't
    ship — but the thing being protected is the *opted-in* state, not the default, which
    stays offline regardless.

## Suggested next increment

The v0.3.0 batch is **shipped**. The **knowledge corpus**, the **contribution
drafts**, and the **phone portal (Phase 3a)** are **built and in `main`**, unreleased —
cut them as **v0.4.0**.

**Stop here and let 3a be used before building 3b.** Offline capture is a *separate
project*, not a follow-up commit: it's gated on the WASM port of `interactions.rs` and
`crisis.rs` (so the safety checks don't go dark offline — that's a blocker, not polish)
and on a real decision about phone-side encryption. The "known constraint" below —
the desktop must be awake to serve — is the thing 3b exists to fix. **Find out how
often that actually bites** before committing to it.

---

## Related project — Cairn

`sparkly-quasar/cairn` (public) — the guided local-LLM installer Field Notes
builds on. Shipped through Phase 3 (Simple setup + Explore catalog + Remote
access) with signed installers + auto-update. **Remaining:** Phase 4 — advanced
mode (quantization / context length / logs) and a bundled Python sidecar to drop
the Docker dependency. (App self-update already shipped.)
