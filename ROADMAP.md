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
- **Distribution** — cross-platform installers (macOS universal `.dmg` +
  Linux `.AppImage`/`.deb`/`.rpm` + **Windows NSIS `.exe`/`.msi` from v0.5.0**)
  via `tauri-action` CI on `v*` tags, plus **in-app auto-update** (Tauri updater;
  "Install & restart" banner). macOS is currently **unsigned** (right-click →
  Open on first launch) pending an Apple Developer ID; Windows is unsigned too
  (SmartScreen "More info → Run anyway").

---

## Shipped in v0.4.0 / v0.4.1

### Offline knowledge corpus — DoseWiki prose, BM25, no embedding model

`data/dosewiki/corpus.py` → `src-tauri/resources/dosewiki-corpus.json`
(**7,823 chunks / 575 substances / 3.6 MB**), searched in-process by
`knowledge.rs`. Reachable three ways: the Companion's `search_knowledge` tool, the
`knowledge_search` command, and a **Search the reference** card in the Substances
tab. The index is held in `Knowledge` state **independent of `Db`**, so it works
while the journal is locked — it's public CC0 data, not user data.

Search returns *excerpts*, which is the wrong unit when you want to understand a
substance rather than answer one question. So the same corpus is also readable
**whole**: `knowledge_entry(slug)` returns every passage of one substance in
corpus order, `knowledge_entries()` lists all 575 alphabetically for browsing
without a query, and the Substances tab pairs the opened entry with its
`pw_lookup` dose panel. The panel and the prose stay visually separate on
purpose — rule 1 below still holds, the prose never supplies a number.

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

## Shipped in v0.6.0

- **Edit timeline notes** — in-session timeline entries could only be added or
  deleted; now they can be edited end-to-end (`update_timeline_event`:
  `TimelineUpdate` + db fn/test in `db.rs`, command in `commands.rs`, registered
  in `lib.rs`, portal allowlist + dispatch in `portal.rs`, `updateTimelineEvent`
  in `src/lib/api.ts`). Desktop: ✎ on each timeline entry opens inline edit
  (note, mood, intensity, time). Phone: tap a timeline note to edit note +
  intensity, with delete in the panel.
- **Bug fix: phone portal showed times in UTC** — `hhmm`/`day` in
  `src/routes/m/+page.svelte` sliced raw UTC ISO strings, so times were off by
  the timezone offset and evening sessions showed the wrong date. Now converted
  to local time; stored data was always correct, no migration needed.

---

## Shipped in v0.11.5

**Offhand dose tracking, on both screens.** v0.11.4 put a quick log on the phone;
this makes it the path of least resistance everywhere, and makes the entry it
leaves behind easy to finish later. The logic is shared — `src/lib/quicklog.ts`
is called by the desktop and the phone alike, so the warning rule below can't
drift between them.

- **`quickLog()` in `src/lib/quicklog.ts`** — creates the already-ended entry,
  logs the dose, and returns the entry plus its warnings. Also `stretchToCover`
  (a dose added outside an entry's span moves its start or end, or the journal
  shows an entry that ended before something in it happened, and every t+ offset
  in it is measured from the wrong moment) and `recentSubstances` / `whenPresets`
  / `recallDoseShape` so both UIs offer the same shortcuts.
- **Fewer taps to the same entry** — recently-logged substances as one-tap chips;
  **Now / 1h ago / 3h ago / Last night** presets that *fill the visible time
  field* rather than replacing it, so a preset can never quietly file a dose
  under the wrong moment; and unit+route remembered per substance in browser
  storage (`recallDoseShape`) so cannabis stops defaulting to milligrams oral.
  The memory is a convenience, never journal data.
- **"Add notes to it" / "Log another into it"** on the confirmation. The first
  opens the entry with its write-up focused — "log it now, write it up later"
  only works if later is one click away. The second logs the next substance into
  the **same** entry rather than beside it, which is both how the evening reads
  back and what lets `log_dose` compare the two against each other.
- **Desktop: `+ Dose` leads the Journal header** (Session demoted to ghost) and
  opens the same panel. The live session's own one-tap logger was renamed
  `logIntoSession` to stop it colliding with the shared `quickLog`.
- **"no notes yet"** against a finished entry in both journal lists, so an entry
  still waiting for its story says so.

---

## Shipped in v0.11.4

Two gaps in the phone portal, both about the journal being a record you keep
rather than a session you sit through. Frontend only — every command involved
was already on the portal allowlist (`src/routes/m/+page.svelte`).

- **Quick log: a substance and a time, and that's the whole entry.** Logging
  something from a phone meant starting a session, logging into it, and
  remembering to end it — a trip-report shape imposed on "I took this at nine".
  The Now screen now leads with a one-shot form (substance, amount, route, and a
  `datetime-local` defaulting to now) that creates an **already-ended** session
  so it lands in the journal as history and never appears as a session someone
  forgot to close. The title is left blank on purpose: `name_after_first_dose`
  names it after what was taken. Because a standalone entry has nothing for
  `log_dose`'s own check to compare against, the same deterministic checker is
  run across everything logged within 12 hours either side — the checker must
  not go quiet on exactly the path that skips the session.
- **A past entry is editable from the phone, not just readable.** The Journal
  detail screen was read-only apart from rename and export. Now every dose and
  note in it is a tap-to-edit target (the same editors the live session uses,
  which gained a time field so a mistimed dose can be corrected), an entry can be
  added to after the fact (dose or note, at a time you choose), and **Edit entry**
  opens title, start, end, rating, and the write-up — plus delete. The write-up
  field is the point: it's how a quick log becomes a full entry later, if it ever
  does.
- **The phone's timeline is in time order.** Doses and notes are two tables and
  the phone shows them as one list; it rendered all doses, then all notes. Merged
  and sorted now, which matters once times are editable.

---

## Shipped in v0.11.3

- **An untitled session is named after its first dose.** The phone starts a
  session with a single tap and had nowhere to put a title, so phone-started
  entries lived in the journal as "Untitled" — the one thing that makes a journal
  hard to read back. `name_after_first_dose` (in `db.rs`, called from `log_dose`)
  fills a blank title with the substance name of the session's **first** dose.
  Deliberate limits: only the first dose (after that the session has a name, and a
  title cleared back to blank was cleared on purpose), never over a title the user
  typed, and renaming still wins. It sits at the db layer, so it holds for the
  desktop, the phone, and the Companion's `log_dose` tool alike.
  - The desktop's "+ Session" form used to stamp the literal string
    `"Untitled experience"` into the title column when the field was left blank,
    which would have made the auto-name unreachable there. It now stores a blank
    title and leans on the display fallback the journal already had.
  - The phone's start-a-session pane gained an **optional** title field. Offered,
    never demanded: typing a title is often the last thing you want to be doing at
    the moment you're starting a session.
- **Still untitled:** a session that is started and never gets a dose. Nothing to
  name it after; left alone rather than guessed at.

---

## Shipped in v0.11.0

- **First-class past-experience logging.** Writing up a trip that already happened
  meant either starting a live session and backdating it (which stamped a wrong
  `ended_at`) or leaning on the LLM text-import. The "+ Session" form now has a
  "This already happened" checkbox: it reveals an end-time field and creates the
  session already ended (via `end_experience`, falling back to the start time when
  no end is given), so it reads as history. New doses on an already-ended session
  default their time to the session's start rather than now (`defaultDoseTime`),
  so a write-up doesn't require correcting every dose timestamp. Desktop-only —
  the phone is the in-hand companion, this is a desk activity.
- **Report-a-bug moved to the top bar.** The prefilled-GitHub-issue feedback flow
  (added in v0.10.0) lived only in a Settings card. A "🐛 Report a bug" button on
  the header now opens it in a modal, one click from anywhere. The Settings card
  stays; this is just a faster way in.

## Shipped in v0.10.2

- **The in-app update prompt shows what changed.** It used to show only a version
  number, and `latest.json`'s `notes` carried the workflow's generic install
  template — the real changelog lived only on the GitHub release page. Now
  `CHANGELOG.md` is the single source of truth: the release workflow extracts the
  section matching the tag (portable awk, so it runs on the macOS and Windows
  runners too) and passes it as `tauri-action`'s `releaseBody`, which is also what
  lands in `latest.json`. The update banner renders `update.body`. Download links
  and Gatekeeper/SmartScreen help stay out of the changelog — appended to the
  GitHub page after publishing (see `RELEASING.md`) so they don't clutter the
  in-app notes. Seeds the renderer: rich notes appear on the *next* update after
  this one, since the prompt belongs to the installed version.
- **Update checks now run on a timer, not just at startup.** `checkForUpdate` was
  called once in `onMount`, so an app left open for days never noticed a release.
  A `setInterval` (6 h) re-checks while running; it skips a check that's mid-install
  and clears a prior dismissal when a genuinely newer version turns up, so
  dismissing one version doesn't hide the next. Silent — it only surfaces when
  `check()` returns something.

## Shipped in v0.10.1

- **The pairing screen says when a phone paired.** Scanning the QR gave the desktop
  no feedback at all — you scanned, and then you guessed. The portal now records the
  first request that arrives with the right token (`Running.paired` in `portal.rs`,
  surfaced on `PortalStatus` and emitted as a `portal-paired` event), and Settings
  shows a green light reading **"Paired successfully"** the moment it happens. A
  *rejected* token deliberately doesn't light it — otherwise anyone probing the port
  could tell you your phone was paired. It reports "a phone has paired since you
  turned this on", not live presence.

## Shipped in v0.10.0

Responsiveness, honesty about hardware, and a text-import that actually works —
plus a way for people to send feedback.

- **The Companion no longer freezes the app.** `companion_chat` was a synchronous
  command, so a turn — including the tens-of-seconds cold model load on the first
  message — ran on the UI thread and locked the whole window until it returned,
  which read as a crash. It's now async and runs off the UI thread (like the phone
  portal already did). The same fix landed on **text import** (`parse_experience`),
  which had the identical problem.
- **The model pre-warms.** When the Companion comes into view the app quietly loads
  the model into memory (`ollama::warm` → `companion_warm`), so the first real
  message arrives at warm speed instead of paying a cold multi-second load.
- **Compute watcher** (`compute.rs`) — local LLMs are memory-bound, so before
  someone leans on the Companion the app reads the machine (`sysinfo`) and gives a
  plain verdict: *ample / tight / insufficient*. It weighs available RAM against the
  model's footprint (using Ollama's `/api/ps` real resident size when the model is
  loaded, and flagging a GPU→CPU spill), preflights a model **before** the multi-GB
  download in setup, and folds in the **measured tokens/sec** of the last reply so a
  machine that fits the model but runs it too slowly is caught honestly. Read-only,
  advisory, never a gate.
- **Import from text, fixed.** It was unreliable and opaque — on the default model
  (qwen3:8b) the parse ran with *thinking on*, took minutes (looked hung), and still
  under-extracted, so people saw "No doses were detected." Now it runs with
  `think: false` (a full trip report went from timing out to ~30 s with every dose
  found), grammar-constrains the reply to a **JSON schema** for a guaranteed shape,
  and uses the substance's standard name so the interaction checker recognises it
  (acid→LSD, shrooms→psilocybin, xanax→alprazolam). The UI says what to paste and
  what works best, shows the extracted intention/setting/notes (not just doses), and
  explains the no-dose case. When no real date was found, fabricated-but-spaced dose
  times are rebased onto the start you confirm.
- **First-run Companion opt-in.** The disclaimer splash on a fresh install now asks
  whether to enable the Companion, once (it's optional, on-device, and the safety
  layers work either way).
- **Send feedback → GitHub.** A Settings card opens a prefilled bug report or feature
  request as a GitHub issue in the browser. No backend, no telemetry — nothing leaves
  the journal; GitHub handles identity and the reports collect in one place.

## Shipped in v0.9.1

A round of Companion and Settings polish on top of v0.9.0, mostly driven by
using the model switch and reading the screens as a first-timer would.

- **Companion explains itself and shows a warm-up hint.** The empty-state text
  now says what the Companion is for (planning, an experience in progress, or
  integration) and that sharing a session lets it see doses and log for you. A
  hint appears only while the first reply is loading — the model loads into
  memory then, and that reply is slow — and disappears once it's warm.
- **Pick which session to share.** The share checkbox now reveals a dropdown:
  the current (newest) session by default, or any past session by name and date,
  so an experience can be attached for integration, not just live support. The
  backend frames an ended session as past ("wants to talk it through") rather
  than current, so the model doesn't offer stay-hydrated advice for doses long
  worn off — decided from the session's own `ended_at`, no new parameter.
- **Dropped the "Gentle periodic check-ins" support style.** It promised
  proactive outreach the Companion can't do; it only replies when spoken to.
- **Substances tab clarified.** Removed the standalone "Dose reference" box (its
  provenance line moved under "Search the reference"), and renamed the last
  section to "Substances you track" with copy that makes clear it's a personal
  roster feeding the interaction checker, not a notes feature.
- **Phone access requires Tailscale, up front.** A prerequisite panel with a link
  and a live installed/signed-in/ready status now sits in the intro, and the
  "Turn on" button is disabled until Tailscale is detected — serving on plain
  localhost had no real use. Removed the raw `tailscale serve …` command line
  under the Publish button.
- **Crisis resources de-duplicated and reordered.** The panic screen listed
  Emergency services twice (the physical and psychiatric categories both carry
  it); every resource now has a single definition and a dedup pass covers the
  composed banners too. Order is gentlest-first: someone you trust, a sober
  person present, Fireside, emergency services, the crisis lifeline, poison
  control. The "calling is always okay" note moved below the in-person options,
  above the phone lines it describes.

## Shipped in v0.9.0

- **Companion evaluation harness** (`src-tauri/examples/companion_eval.rs`,
  `eval/scenarios.json`) — 30 scenarios replayed through the real tool loop
  against a real seeded journal and the real bundled reference data, emitting a
  markdown report with full transcripts. `eval/scenarios.json` doubles as the
  written behavioural spec: what the Companion should do about facts, Zendo
  register, redosing, crises, boundaries and journal tools. Reports land in
  `eval/runs/` and are **git-ignored** — they quote journal prose.
  A green run means "no hard failure", never "this was good": the checks are
  substring and word-count matches, and a bad answer containing the right word
  still passes. Read the transcripts.
- **Default model is now `qwen3:8b`.** Measured against the harness,
  `llama3.1:8b` declined to engage with six safety scenarios — including a
  witnessed seizure, and an alcohol + diazepam stack where the interaction
  checker had *already* returned a `[danger]` flag naming respiratory
  depression, to which it replied "I'm so sorry to hear that." Qwen3 answered
  both correctly. A model that refuses this app's subject matter cannot sit with
  someone having a hard time. An in-app **switch button** (`ai_switch_model`)
  downloads the new model before removing the old one — never the reverse, which
  would strand someone mid-session if the download failed.
- **Companion-free mode** — the Companion can be turned off entirely in
  Settings, for slower machines or by preference. Deliberately switched from
  Settings rather than the Companion tab, so turning it off doesn't hide the
  control that turns it back on.
- **`lookup_dose` now reports durations.** It formatted only light/common/strong
  ranges and silently dropped the onset/peak/total that `pw.rs` already parses
  and `db.rs` already stores — so the Companion guessed, and said things like
  "LSD lasts about 3 hours." When a substance has no duration data the tool now
  says so explicitly rather than leaving room to estimate.
- **Fabricated-citation guard** — a reply claiming "the dose reference says…"
  without having called the tool gets one corrective round trip; if it fabricates
  again, the sentences carrying the claim are stripped deterministically.
  Sentence-level, not phrase-level: removing just the attribution leaves the
  assertion standing as the Companion's own knowledge. The first version of the
  correction offered "call the tool now and report what it returns" and small
  models took that as a *template* — asserting the call and inventing the result.
  Removing the option removed the failure.
- **Crisis detection matches how people actually write** (`crisis.rs`) — literal
  substring matching missed "my chest really hurts", "i don't want to be here
  anymore", "ending it tonight", "i think i'm dying". Replaced with stem and
  proximity matching plus four two-half medical clusters (heat stroke, serotonin
  toxicity, cardiac, respiratory depression), with a per-signal negator list.
  Negation is handled per signal rather than globally on purpose: a blanket rule
  would swallow the true positive "i don't want to be here anymore".
- **Expressive distress no longer alarms on first utterance.** "This is horrible,
  I want it all to stop" is often someone logging how they feel, not a crisis.
  Said once it is expression; repeated across messages it becomes a pattern worth
  offering help for (`scan_recent`, `repeats >= 2`). Peer-level results are now
  presented as an **offer** ("Would it help to have someone to talk to?") that
  leads with a trusted person rather than a hotline; medical and psychiatric
  levels stay direct.
- **Still unfinished.** v0.9.0 fixed correctness and honesty, not register. The
  Companion still runs long when it should be brief, sometimes misses a tool call
  it should have made, and drifts out of the calm non-directive voice it's aiming
  for. The prompt restructure and worked Zendo examples — the original "it reads
  like a completely untrained person" complaint — remain the largest untouched
  lever, and are only now worth pulling: they were never going to land on a model
  that refused the subject matter. Measure against the harness before and after.
- **The crisis verdict now reaches the Companion.** The scan ran in the frontend
  and `companion_chat` never saw its result, so the two halves could disagree in
  front of someone in trouble: the banner said get help now while the chat, having
  re-derived the situation from scratch, said see how you feel in half an hour.
  Measured on `overheating` (heat stroke after MDMA) the scan returned `medical`
  in 5 of 5 runs and the model gave wait-and-see advice in 5 of 5. Passing the
  verdict into the prompt took it to 4 of 5. Only `medical` and `psychiatric`
  produce a brief — `peer` is excluded deliberately, because acute distress is a
  moment to sit with someone, not to steer them toward resources.
- **Seven eval checks were silently vacuous.** Scenarios are written with ASCII
  apostrophes ("you're not dying"); models emit typographic ones ("you’re not
  dying"). `expect_any` failed on correct answers and, far worse, `forbid_any`
  passed on the exact phrases it was written to catch — including
  `fear-of-dying`'s Zendo guard. Matching now folds the punctuation. Worth
  remembering when reading any green in a report: a check that cannot fail looks
  exactly like a check that passed.
- **Safety no longer depends on model capability.** The crisis scan and
  interaction checker are deterministic Rust that run regardless of which model
  is loaded, or whether one is loaded at all — which is what makes the low-spec
  tier and companion-free mode viable rather than quietly less safe.

## Shipped in v0.8.0

- **`t+` offsets on session timestamps** — every dose and timeline note now
  carries the time since the **first dose** alongside its wall-clock time
  (`14:02 (t+1:23)`), on the desktop detail view, the live session, and the
  phone portal. T-zero is deliberately the first dose rather than the session
  start: sessions get opened well before anything is taken, and a peak, a redose
  window, or a comedown is measured from ingestion. No dose logged means no
  t-zero and no offset shown, rather than a number counted from nothing. Notes
  backdated before the first dose read `t−0:20`, not a clamped zero. The live
  header also carries a ticking `now t+2:41`.
- **Read reference entries in full** — the DoseWiki corpus was only reachable as
  ranked excerpts, which is the wrong unit when you want to understand a
  substance rather than answer one question. Every search hit now opens into the
  whole entry (`knowledge_entry`), and all 575 substances are browsable by name
  without a query (`knowledge_entries`). An opened entry pairs the prose with
  its `pw_lookup` dose panel — kept in a visually separate box, because rule 1
  above still holds: the prose never supplies a number or a combo verdict.

## Shipped in v0.7.0

- **Phone Companion chat survives long replies** — a slow local model can take
  minutes per reply, and mobile Safari kills a silent request at ~60 s (a locked
  screen kills it instantly), so the phone showed "Can't reach the desktop app"
  while the desktop was fine. The portal now runs the reply as a **background
  job** on the desktop and the phone **polls** every couple of seconds until it's
  done — locking the phone mid-reply is fine. Test pins that a job outlives its
  originating request and delivers exactly once
  (`a_companion_job_outlives_its_request_and_delivers_once` in `portal.rs`).
- **Export a single entry as Markdown** — an "Export this entry" button at the
  end of an entry on the desktop (save dialog) and an "Export" button on the
  phone (downloads the file). Same format and filename as the Obsidian vault
  sync, so an exported note drops straight into a vault.
- **Rename from the phone** — tap the live-session title in the header, or an
  opened entry's title in the journal, to rename it.
- **New footer** — the tagline is now the quote *"The greatest intention is to
  be open to learning."* with the subline "for mindful exploration and
  contemplation".

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

0. **Backend timestamp normalization for imports.** Shipped in v0.10.0, text import
   rebases fabricated dose/timeline times onto the confirmed start **in the frontend**
   (`rebaseTimestamps` in `+page.svelte`), because that's where the `t+` date math
   already lives and the backend has no date library. That's solid for the common case
   but leaves the invariant split across two layers. A cleaner home is the backend: give
   `import_experience` (`commands.rs`) real timestamp parsing (add `chrono`, or the
   `time` crate) so it owns normalization — parse each `taken_at`/`at`, drop the ones it
   can't, and when no absolute `started_at` was extracted, shift the parseable ones so the
   earliest lands on the chosen start while preserving spacing. Then the phone portal's
   import path (if it ever gains one) and the desktop share one rule. Small, self-contained,
   worth a test that a T+ report rebases correctly and a real-dated one is left untouched.

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
   - **Mobile route** — `/m`: Now (start/end a session, log a dose, edit or delete one,
     notes with the crisis scan), Journal (history + substance log), Combo, Look up (dose
     table + corpus prose + your catalogue), Talk. A phone-shaped **mirror** of the
     desktop, re-laid out for one hand — not the desktop page made responsive. It started
     as a four-button subset; beta feedback was that the subset was the wrong call.
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

   **Publishing to the tailnet is one button** (`portal_serve` / `portal_unserve`, wired to
   Settings → Phone access). It was originally a command for the user to run by hand, on the
   theory that the step deserved to be visible; beta feedback was that this is too much to
   ask of an end user, and the honest fix is to keep it *visible* — the button says what it
   runs, shows the resulting `*.ts.net` URL, and is reversible — rather than to keep it
   *manual*. Tailscale's own refusals ("not logged in", "HTTPS must be enabled in the admin
   console") are surfaced verbatim, because that message is the fix.

   **Note the asymmetry:** the phone mirrors the journal, but it cannot mirror the *portal's
   own controls*. `portal_serve`, `portal_unserve`, `portal_enable`, and `portal_disable` are
   not in `EXPOSED` — a phone may not publish, unpublish, or reconfigure its own access. That
   decision is made at the desk.

   ### Phase 3b — offline capture. Lets you log while the Mac is asleep or off-tailnet.
   The journal is **append-only** in practice (a dose/note is a new row), so an outbox
   that queues *only new entries* sidesteps real bidirectional sync entirely — **keep it
   that way**; queuing edits/deletes re-opens conflict resolution and is not worth it.
   - **Outbox** — service worker + IndexedDB; local temp IDs reconciled against
     server-assigned IDs on replay. A visible **"N entries pending"** marker — never
     leave the user guessing whether a dose was recorded.
   - **Redirect `/` → `/m` for non-Tauri clients.** The portal's SPA fallback serves
     `index.html` for any non-file path, so a phone that browses to the tailnet root gets
     the **desktop** page — which half-renders and throws console errors, because it
     expects Tauri APIs that don't exist in a browser. Not a security hole (the allowlist
     is enforced server-side, so nothing dangerous is reachable either way), just untidy.
     It belongs here rather than as a one-off: the service worker has to decide what the
     PWA's start URL and scope are anyway, and both should answer to `/m`.
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

**v0.4.0 and v0.4.1 are shipped and public** — the knowledge corpus, contribution
drafts, the phone portal (Phase 3a), the combo-checker fix, and the phone Companion fix.

### Plain journal entries (not a drug session) — ✅ shipped in v0.5.0

Today an entry **has to be a session**. If you just want to write about your day, the
app has nowhere to put it, which quietly narrows a journal into a drug log. It should
be possible to write a plain text entry, in the same journal, alongside the sessions.

- **Model it explicitly, don't infer it.** The tempting shortcut — "an experience with
  zero doses is a note" — is wrong: a session where you haven't logged the first dose
  *yet* looks exactly like a note, and it would flip type under you mid-session. Add a
  `kind` column (`'session' | 'note'`, defaulting to `'session'` so every existing row
  keeps its meaning) and branch on it.
- **The crisis scan does NOT run on journal prose — owner's decision (2026-07-14).**
  The journal is private; the app must not read over the user's shoulder. `crisis.rs`
  fires in exactly two places: (1) what the user *says to* the Companion in an active
  chat (self-harm / harm-to-others intent), and (2) the deterministic combo checker
  when a dangerous interaction is flagged. Journal entries — session notes, timeline
  notes, and plain entries — are saved as written and never scanned. (The phone's
  timeline-note scan was removed for the same reason.) Don't relitigate this by
  "adding safety"; the guardrails live where the user is talking to something, not
  where they're talking to themselves.
- **The UI should get quieter, not just different.** A plain entry has no doses, no
  timeline, no combo warnings, no elapsed-time header. It's a title, a body, a date.
  Resist re-using the session layout with the drug parts hidden.
- **It's the phone's missing verb.** `/m` currently can't take a note at all without a
  live session — the Now tab starts from "log a dose". A plain entry is the obvious way
  to jot something at 3am without pretending it's a session.
- **Downstream, mostly free:** Obsidian export writes them as ordinary Markdown notes;
  the Companion can read them for context; the substance log ignores them by definition.

**Then stop and let 3a be used before building 3b.** Offline capture is a *separate
project*, not a follow-up commit: it's gated on the WASM port of `interactions.rs` and
`crisis.rs` (so the safety checks don't go dark offline — that's a blocker, not polish)
and on a real decision about phone-side encryption. The "known constraint" above —
the desktop must be awake to serve — is the thing 3b exists to fix. **Find out how
often that actually bites** before committing to it.

---

## Related project — Cairn

`sparkly-quasar/cairn` (public) — the guided local-LLM installer Field Notes
builds on. Shipped through Phase 3 (Simple setup + Explore catalog + Remote
access) with signed installers + auto-update. **Remaining:** Phase 4 — advanced
mode (quantization / context length / logs) and a bundled Python sidecar to drop
the Docker dependency. (App self-update already shipped.)
