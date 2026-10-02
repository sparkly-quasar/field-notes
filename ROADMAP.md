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

## Shipped in v0.13.0 — phone redesign + paste a trip log

**Phone redesign** (`src/routes/m/+page.svelte`, rewritten) from a three-lens UX
review run 2026-09-26: task flows/IA, impaired-state accessibility, and reading/
editing. Decisions:
- **IA:** Today · Journal · ＋ · Check · Talk. ＋ opens a "New" sheet (dose, live
  session, past session, paste a log, journal note). Combo + Look up merged as Check.
- **Vocabulary, one word per thing:** entry / session / journal note / moment /
  write-up. ("Note" used to mean three things.)
  **v0.21.4:** a session in progress is called a **live trip report** in the UI
  (was "live session"); a finished one is still a session. Today keeps one
  "+ Log a dose" button; the other "new" actions live only under ＋, now
  labelled "＋ New", which also offers "Add a moment" during a live trip report.
- **Bottom sheets for every edit** (they open at the thumb; editors used to render
  below the whole timeline, off-screen). Entry actions in a sticky bar.
- **Deferred delete + Undo (8s)** instead of `confirm()`; Delete separated from Save,
  entry delete is two deliberate taps.
- **Every dose goes through `quickLog`**, so doses into existing entries get the
  12-hour wider check and `stretchToCover` too. After a dose, `crisisScan("", id)`
  runs as on the desktop — combinations only; **journal text is still never
  scanned** (crisis policy unchanged).
- **Help button on every screen**, `emergency_resources` cached in localStorage for
  when the server is unreachable. `telOf()` dials the first phone number in a
  contact, not every digit in it (the old `replace(/[^\d+]/g,"")` produced 911112).
- **Data integrity:** `num()` accepts comma decimals and refuses NaN (was silently
  saved as null); blank times are refused rather than becoming "now"; `portal.ts`
  times out after 20s with "may or may not have saved" wording (no blind retry).
- Tokens with a light scheme, safe-area insets, ≥44px targets, visible labels,
  `aria-live` toasts, `aria-pressed`/`aria-current`, reduced-motion.
- `allWarnings` dedup is now order-insensitive per pair (desktop benefits too).

**Paste a trip log** (`src/lib/tripimport.ts` + `src/lib/TripImport.svelte`, on
both screens). **Deterministic** — no model — so it works on the phone and every
line is shown before saving. Clock times (am/pm, 24h, bracketed), T+/`+1h30`/`90
min in` offsets, midnight rollover, continuation lines, `n/10` intensity. Doses are
an amount+unit plus a name; names resolve against the user's catalogue then the
dose reference's 577 names and street names via the new read-only `pw_names`
command (portal-allowlisted; reference data only). A unique *main*-name prefix
wins before street names ("mesc" → Mescaline, not TMA's "Mescalamphetamine");
street names that are ordinary words ("pot", "beans", "tabs") never match
mid-sentence. `saveTripLog` (quicklog.ts) creates an ended session and runs the
wider check per dose.

v0.20.1: percent-encoding is decoded a word at a time (a log with one encoded
paragraph pasted in); a date-only line before the first timed line
(`Date: 02-28-2026 12:30pm`) dates the log, with day/month order from the locale
when ambiguous; and the first line under a bare time (`12:30pm` on its own) is
read as that line, so a dose there is a dose.

v0.21.5: 12-hour logs with no am/pm roll half a day, not a day, and the preview
asks am or pm (guessed from a time on the date line; an am/pm further down
settles it). `+` and comma lists split into a dose per item when every item reads
as a substance and one is anchored (an amount, or a whole catalogue name). Doses
are tagged with whose they were (`- Name`, `(11 A 6 B)`, then `Name 40mg`); the
preview asks which is you and saves the rest as notes. Untimed paragraphs after
the last timed line and a blank line become the write-up. Route words stay out
of names; an unwritten route comes from the same substance earlier in the log,
then quick log's remembered shape, then `RARELY_ORAL` (DMT/5-MeO-DMT vaporized
unless an MAOI is mentioned, ketamine insufflated). `EXTRA_ALIASES` in `pw.rs`
adds names DoseWiki lacks (4mmc, 14b, dexamp…) on load, surviving a refresh.

**Dev harness:** `cargo test --lib portal::tests::dev_portal -- --ignored
--nocapture` serves a real portal on a seeded throwaway journal (with the real dose
reference) for driving the phone UI in a browser.

## Shipped in v0.12.0 — one computer as the server

**The ask:** a Mac mini that's always on runs the journal; the laptop stores and
syncs everything there; the phone pairs with the Mac mini as before. Built as an
extension of the phone portal — the laptop is just another client of it — not as
a sync engine.

- **Per-device pairing** (`devices.rs`). The single in-memory token is gone. Each
  device gets its own 256-bit token, named, shown once, stored **only as SHA-256**
  in `devices.json` beside the journal, and revocable on its own. Pairings survive
  restarts — a server that reboots can't make everything re-pair.
  - ⚠️ **Why the registry is *outside* SQLCipher:** rule 2 checks the token before
    anything else, including the lock. Inside the encrypted DB, a locked server
    would have to answer strangers before it could tell who they are. Names and
    hashes aren't journal data.
- **Serve on launch** (`prefs.rs`, `server.json`) — start device access when the
  journal opens (at launch or on unlock) and republish to the tailnet on the
  **same HTTPS port** as before (`was_ours` reclaims our own stale handler; never
  someone else's). Plus **open at login** (`tauri-plugin-autostart`).
- **Keychain unlock** (`keychain.rs`) — **owner's decision 2026-09-25: offered, opt-in.**
  The passphrase can be saved in the OS keychain so the server unlocks itself
  after a reboot. The Settings copy states the trade-off (anyone who can log in to
  that account can open the journal; encryption still covers a pulled disk or a
  copied file). It's verified against the journal before saving, kept in step on
  a password change, and forgotten on disabling encryption or wiping.
- **Client mode** (`remote.rs`). A desktop connects with the server's pairing link
  (https only, loopback excepted). `api.ts` sends the `ROUTED` commands to
  `remote_call`; a test keeps the TS and Rust lists identical, and another checks
  every routed command is on the server's `EXPOSED` list. A client refuses to
  serve (no chained servers). Backup, Obsidian, text import and contribution
  drafts are hidden in client mode — they belong on the server.
- **Offline: new entries queue** — **owner's decision 2026-09-25**, the same rule
  as Phase 3b below. `create_experience`, `log_dose`, `add_timeline_event` and
  `end_experience` go to an outbox **inside the laptop's own journal** (so it's
  under SQLCipher, which answers Phase 3b's storage concern for this client);
  edits and deletes are refused offline. Offline IDs are negative and translated
  to the server's on send. 5xx (Tailscale's 502 when the server app is down) and
  503 (locked) mean "retry"; 401 means un-paired; any other refusal is kept,
  **visibly**, with a Discard button — never dropped or retried forever.
  - The **safety layers don't go dark offline** — the Phase 3b blocker doesn't
    arise here, because the laptop runs the same Rust. A queued dose is checked
    against the cached session + everything queued since; open sessions are
    prefetched whenever the list loads so that cache exists when it's needed.
    `crisis_scan_names` is the crisis scan over a name list, shared by both paths.

- **Sync journal to server** (`upload_local`, owner's request 2026-09-25) — copies
  the laptop's own entries up: custom substances first, then each entry whole
  (create → doses → timeline → `update_experience` last, so a blank title stays
  blank). `remote_synced` records (local id, server) and **survives disconnect**,
  so re-running or reconnecting never duplicates; a failure mid-entry deletes the
  half-copied entry on the server. Local copies are kept — nothing is deleted.
  **v0.13.3:** entries are checked against the server by content before sending
  (`db::fingerprint` — everything written, not IDs), so an entry the server
  already has is recorded as synced and skipped even when `remote_synced` doesn't
  know it; half-copied entries that couldn't be deleted go to `remote_orphans`
  and are removed next sync; exact duplicates are removed once on the first open
  after updating (`user_version` 1) and on the server after each sync
  (`remove_duplicate_entries`, exposed).
- **Settings naming:** "Use another computer as your server" (client) and
  "Server Mode" (serve on launch / open at login / keychain) — owner's wording.

**Not built, on purpose:** queued edits (see above);
the phone's own offline outbox (still Phase 3b).

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

0. ✅ **Shipped in v0.14.0.** `normalize_import` in `commands.rs` now owns
   the rule (chrono added); `rebaseTimestamps` is gone from `+page.svelte`, and
   `import_experience` takes the confirmed `start` separately so the model's own
   `started_at` still tells it whether the report was dated. Unparseable times fall back
   to the start instead of reaching the journal as text. Five tests. Original plan below.

   **Backend timestamp normalization for imports.** Shipped in v0.10.0, text import
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

2. ✅ **v1 shipped in v0.14.0.** `stats.rs` (`usage_stats`, in `EXPOSED` and
   `ROUTED`, nine tests), `src/lib/stats.ts` (bucketing), and one responsive component,
   `src/lib/UsageStats.svelte`, used by the desktop **Stats** tab and the phone's
   **Journal → Entries | Stats** switch. Everything in "What goes on the page" is in
   except the "later" ideas. **One change from the plan below:** day/week/hour bucketing
   happens on the frontend, in the viewer's time zone (a phone and its server need not
   agree), via the one shared `stats.ts`; all grouping rules stay in Rust. **Not built,
   waiting on the owner:** export, and Companion access. In client mode the page needs
   the server (it isn't cached offline).

   **Usage stats: a page of patterns from your own journal.** A read-only view that
   turns the doses already in the journal into a few honest pictures: how much, how
   often, and how far apart. It works on **both the desktop and the phone (`/m`)** from
   day one, not desktop first with a phone port later. No new data is collected; this
   only reads what's already logged.

   **What goes on the page (in rough priority order):**
   - **Dose over time, per substance**: the headline chart. One dot per dose (x = date,
     y = amount), one substance at a time, picked from chips. Where `pw.rs` has a dose
     range for that substance + route + unit, draw the **light / common / strong bands**
     behind the dots. That gives a dose context a number alone doesn't, and it comes from
     the deterministic layer, never the corpus (containment rule 1 still holds).
   - **Frequency**: sessions per week or month as bars, over a range picker (30 days ·
     90 days · 1 year · all). Count **sessions**, not dose rows, so a redose doesn't
     double-count a night.
   - **Spacing between sessions**: days since the last use of each substance, and the
     gap between consecutive sessions. For psychedelics this is the tolerance question
     people actually ask ("is it too soon?"), and it's more useful than a raw count.
     Show the gap as a fact. Any tolerance guidance stays with the Companion and the
     reference, not baked into the chart.
   - **Calendar heatmap**: a year at a glance, one cell per day, shaded by number of
     sessions. It reads well on desktop. On the phone it collapses to the current month
     with swipe/arrows (a 53-column year doesn't fit 360px).
   - **Combinations**: which substances show up together in the same session, most
     common first. It pairs naturally with the combo checker: tapping a pair opens Check
     with it prefilled.
   - **Smaller tiles:** route breakdown per substance, time of day doses are taken, and
     average session rating where ratings exist.
   - *Ideas worth considering later, not in v1:* overlaying timeline `intensity` on a
     session's dose times (a personal dose-response curve), and rating vs. dose. Both
     invite reading causation into a handful of points, so they'd need careful wording
     and a minimum-N before they show anything.

   **Data rules (the parts that will bite if skipped):**
   - **Never sum or average across units.** `doses.unit` is free text (`mg`, `µg`, `g`,
     `tab`, `ml`, `hits`…). Group by **substance + unit** and chart each group on its own
     axis. Don't convert units (a "tab" has no mg value, and guessing one is inventing a
     number). If one substance has several units, the chips split it ("LSD · µg",
     "LSD · tab").
   - **`amount` is nullable.** A dose with no amount still counts toward frequency and
     spacing but is left off the dose chart. Say how many were left off ("3 doses without
     an amount aren't shown") instead of dropping them silently.
   - **Group by `substance_id` when it's set, else by normalized `substance_name`,** so a
     catalogue rename or an alias doesn't split one substance into two series.
   - **Plain journal entries (`kind = 'note'`) are excluded by definition**, same as the
     substance log.
   - **Local time.** Bucket days and weeks in the device's timezone (the v0.6.0 UTC bug
     on the phone portal is the precedent to avoid).

   **Architecture:**
   - **One aggregation, in Rust.** A new read-only `usage_stats(range, substance?)`
     command in `commands.rs` does the grouping and returns chart-ready series. The
     desktop and phone render the same payload, so there's no second implementation of
     the unit/grouping rules.
   - **Add it to `EXPOSED` on purpose.** It's read-only, so it belongs on the phone, but
     per `portal.rs`'s rules it has to be allowlisted deliberately, with a test.
   - **No chart library.** Hand-rolled SVG in Svelte. The app is offline and ships no
     CDN code, and the charts above are dots, bars and a grid, which don't need a
     dependency. Every chart gets a **data table fallback** (a "show as table" toggle)
     for screen readers and for exact numbers.

   **Desktop and phone:**
   - **Where it lives:** desktop gets a **Stats** view alongside Journal. On the phone,
     the v0.13.0 IA (Today · Journal · ＋ · Check · Talk) stays at five; Stats goes
     **inside Journal** as a segmented "Entries | Stats" switch, not a sixth tab.
   - **Phone layout is a single column of cards**, charts at full width, no horizontal
     page scroll. **Tap, not hover**: tapping a dot or bar opens the existing bottom
     sheet with that dose/session and a link to the entry. Desktop shows the same
     detail on hover and click.
   - Substance chips and the range picker sit at the top, within thumb reach on the
     phone, and they're remembered per device.

   **Framing and privacy (decide these deliberately):**
   - **Descriptive, never judgmental.** No streaks, scores, goals, badges, or red/green
     "good/bad" coloring. Harm-reduction framing means showing the pattern and trusting
     the user with it. It also follows the 2026-07-14 decision that the app doesn't
     read over the user's shoulder: **the stats page never raises alerts on its own**.
     It's there when opened and quiet otherwise.
   - **Shoulder-surfing.** This page is a summary of someone's drug use on one screen.
     Offer a **"hide substance names"** toggle (labels become "Substance A/B") for
     viewing in public or screen-sharing.
   - **Export is the open question.** A "save as PDF / image" for bringing to a
     therapist, prescriber, or integration session is genuinely useful, but it moves the
     data outside the encrypted journal. If it ships, it gets the same **plaintext
     warning** as Obsidian export, and it's an explicit action, never automatic.
   - **Companion access: off by default.** The Companion could answer "how often have I
     been using X?" from this payload, but that's a new read path into the journal. If
     it's added, it's opt-in, like "share session", and it uses the same
     `usage_stats` output rather than its own queries.

   **Tests:** mixed units never merge; null amounts count for frequency but not for
   the dose chart; `note` entries are excluded; a session with three doses counts once
   for frequency; day bucketing respects local time across midnight; `usage_stats` is in
   `EXPOSED`.

3. **UX review and redesign (2026-10-02).** Four reviewers (visual design, use while
   altered, trauma-informed mental health UX, information architecture) read the
   code and screenshots. Their combined plan, in shippable phases:

   **Phase 0: live-session bugs. ✅ Shipped in v0.16.0.** The desktop live screen hid
   interaction warnings and the crisis banner; the phone's "now T+…" never updated;
   phone Help scrolled away and hit the status bar; desktop Help closed on a stray
   click and didn't lead with the emergency line.

   **Owner's decisions (2026-10-02), binding for the phases below:**
   - **Warm, not cool.** A warm near-black and one soft accent: "a field notebook,
     not a dashboard or an ER." Not the blue-black the app uses today.
   - **A serif for reflection: Literata** (owner's pick, 2026-10-02, from six OFL
     candidates) for the reflective fields (intention, write-up, moments), with a
     sans for data (Atkinson Hyperlegible recommended for its unambiguous I/l/1 and
     O/0). Bundled, never fetched.
   - **Discreet mode is opt-in** (owner, 2026-10-02): enabled in Settings on the
     computer that holds the journal (`discreet_available` in server.json, readable
     by the phone); once enabled, an eye toggle sits next to Help on every device.
   - **A reflection is never owed.** Any session can be marked as not needing a
     write-up (a dose of 1,4-butanediol may need none), in one tap, from the entry
     and from the "Waiting for a write-up" list. Marked entries leave that list and
     the "No write-up" filter, and can be unmarked.
   - **"What's your intention?"** is the prompt when a session starts. Plain, one
     question, optional.

   **Phase 1: tone and wording (S). ✅ Shipped in v0.16.0**, with the write-up skip.
   - Red means only interaction danger. Help gets its own calm, unmistakable filled
     style ("Get help"); Delete is a neutral text button that turns red only inside
     its confirm step.
   - "No write-up yet" stops being amber (it reads as overdue homework): muted text,
     "Add a reflection when you're ready", plus the skip option above.
   - Warning wording matches its weight: "Use care with:" (caution), "Known dangerous
     with:" (danger); a heavy dose is caution amber, not alarm red.
   - Stats: "days since last session"; calendar titled "Days with a session"; with
     one gap, "One gap so far: N days" instead of the same number twice; the dose
     chart's axis shows years when the range crosses one.
   - Desktop vocabulary matches the phone: session, moment, write-up (not
     "experience", "Add note", "End experience").

   **Phase 2: live session and reflection (M). ✅ Shipped in v0.16.0** (the desktop's night theme came with Phase 3).
   - "Last: LSD 100 µg · 1h 12m ago" on the live card, and time since the last dose
     of that substance in the dose sheet.
   - Undo on the "Saved" receipt.
   - Moments without typing: intensity alone, or one-tap chips.
   - "What's your intention?" at start; the write-up opens with "You set out to: …";
     rating comes after the write-up.
   - End and Exit move away from Help and + Dose.
   - Dim red night theme (true black, red text, keeps the screen awake).
   - App-wide discreet mode (today's "Hide substance names" covers Stats only).

   **Phase 3: the sleeker look (M–L). ✅ Shipped in v0.16.0**, except the component
   split (deferred: it's a refactor with no visible change, better done on its own).
   Built: warm tokens on both pages, borderless buttons and tinted chips, flat journal
   rows, `Icon.svelte` line icons with labels, the desktop sidebar (Journal, Stats with
   "Every dose, by substance" folded in, Check with a standalone combo checker, Talk;
   Settings and Get help at the foot; narrow windows fall back to a row), and the dim
   red theme on the desktop live screen. The desktop has no Today view; its live
   screen plays that role. *Fonts shipped early, in v0.16.0:* Atkinson
   Hyperlegible for data and UI, Literata for reflection (`static/fonts`, OFL, Latin +
   Latin Extended subsets, ~300 KB; `--font-data` / `--font-reflect`, class `reflect`). One shared token set for both pages (warm),
   the bundled fonts, flatter layout (no cards inside cards), calmer chips, a small
   line-icon set with labels, and a desktop sidebar with the phone's places: Today,
   Journal (Entries | Stats, absorbing Substance Log), Check (absorbing the
   Directory, plus a standalone combo checker), Talk, then Settings and Get help.
   Split `+page.svelte` into one component per view first.

   **Must survive any redesign:** tap targets 44–48px (56px for + Dose, + Moment,
   Help); body text ≥17px on the phone; no thin weights or dimmer grays; Help, + Dose,
   + Moment, Undo, elapsed and last dose always one tap away, never in a ⋯ menu; the
   crisis wording and offer-first flow; "Nothing flagged … isn't the same as 'safe'";
   the never-scan-the-journal rule; warnings that stay until dismissed; undo deletes;
   the phone's date-column journal; the Shulgin quote.

4. **Tailscale built into the app — phone access without installing anything on the
   computer.** ✅ **Shipped in v0.21.0** (built 2026-10-02: `tailnet.rs`, `tailnet/`,
   `scripts/build-tailnet.sh`). Changes from the plan below:
   - **Library: Go `tsnet`, as a Tauri sidecar**, not `tailscale-rs`. At build time
     `tailscale-rs` 0.6.1 listed HTTPS certificates, MagicDNS and Intel Macs as
     unsupported and had no direct connections, so it couldn't serve the phone's
     `https://…ts.net` page. `tsnet` is the code the Tailscale app itself runs.
     A separate small program (rather than a linked Go library) keeps the Go
     runtime out of the app's process and cross-compiles with `CGO_ENABLED=0`,
     so no C toolchain is involved on any OS. Adds about 22 MB per platform.
   - **It's its own tailnet device**, `field-notes` (Tailscale adds `-1` for a
     second computer), answering on port 443. No port juggling, unlike `tailscale
     serve`, which shares the computer's ports with whatever else it publishes.
   - **HTTPS is still a step when a tailnet doesn't have it on.** Step 1 shows it
     only in that case, with a button to the exact page. Settle with a fresh
     account whether new tailnets need it (still open, see risks below).
   - **Sign-out removes this computer from the tailnet** and deletes its keys;
     "Disconnect" only stops answering. "Erase all data" deletes the keys but
     can't sign out (that would block on the network), so the device lingers in
     the Tailscale admin until it expires.
   - **Verified here:** the helper builds for all three OSes, starts, reports
     "waiting for sign-in" through the Rust side (`drives_the_real_helper`), stops
     and signs out; the setup screen renders every state (headless Chromium, with
     a mocked Tauri bridge). **Not verifiable in CI:** a real sign-in, the
     certificate, and a phone on cellular. Those need a person, an account and a
     phone; see RELEASING.md.
   - **Also in v0.21.0: the Companion is opt-in** (owner, 2026-10-02). Off on a
     fresh install, including the first-run box; installs that ran before keep
     their setting. The portal reports it off until the desktop says otherwise.

   *Original proposal (2026-10-02):* Today a new user installs Tailscale
   on the computer, signs in, installs it on the phone, signs in again, then pairs. That
   first half is where people get lost (the README's "too much to ask of an end user").
   Bundling Tailscale into Field Notes removes it: the computer joins the user's tailnet
   by itself, and the only thing anyone installs is Tailscale on the phone.

   **Owner's decisions (2026-10-02):**
   - **Reachable from anywhere**, not only on home Wi-Fi. A LAN-only mode is out.
   - **Keep the home-server model** (one computer holds the journal; phones and laptops
     reach it, as built in v0.12.0).
   - **Bundle Tailscale** rather than build a new transport (see "Considered: a
     peer-to-peer tunnel" below for what this was weighed against).
   - **First-time setup must be really easy**, and the screen must say plainly, in
     order, what to do on the phone: install Tailscale, sign in, scan the QR code.

   **What the user sees (the target flow).** One screen, Settings → Devices & server →
   **Set up phone access**, with numbered steps and a live check beside each. Draft copy:

   > **Use Field Notes on your phone, anywhere.**
   > Your phone connects to this computer through Tailscale, a private connection
   > between your own devices. Tailscale can see which of your devices are connected,
   > never what's in your journal.
   >
   > **1. Connect this computer.** [Connect] opens Tailscale's sign-in page. Sign in
   > with Google, Apple, Microsoft or GitHub. No account yet? Signing in creates one.
   > ✓ *Connected as you@example.com*
   >
   > **2. Install Tailscale on your phone.** Get it from the App Store or Google Play.
   > (QR codes for both store pages, so the phone camera opens the right one.)
   >
   > **3. Sign in on your phone with the same account: you@example.com.**
   > Allow the VPN when your phone asks. Make sure Tailscale shows *Connected*.
   >
   > **4. Scan this code with your phone's camera.** (The pairing QR.) Field Notes
   > opens, already set up. Tip: Share → Add to Home Screen to keep it as an app.
   > ✓ *Paired successfully* (the existing green light)

   - Steps 2–4 stay visible but muted until step 1 is done, so the whole path is
     readable up front. The account name from step 1 is repeated in step 3 because
     **signing in with a different account on the phone is the most likely mistake**
     (Google on the computer, Apple on the phone).
   - No terminal, no admin console, no command shown. If a Tailscale setting does have
     to change (see "HTTPS" below), the screen gives a single button that opens the
     exact page, says what to click, and re-checks on return.

   **When it doesn't work, say which step broke.** The phone page (`portal.ts`) and the
   desktop error copy each name a cause the user can act on:
   - Phone can't reach the computer at all → "Is Tailscale switched on on this phone,
     and signed in as you@example.com? Another VPN app can switch it off."
   - Computer offline or asleep → "Your computer isn't answering. Is it on, with Field
     Notes open?" (existing 502 handling, reworded).
   - Locked journal → existing 503 wording.
   - Token refused → existing "no longer paired" wording.

   **How it's built.**
   - **Library:** Tailscale's **`tailscale-rs`** (native Rust, announced as a preview
     April 2026) is the natural fit for a Tauri app. The fallback is **`libtailscale`**
     (C bindings over Go `tsnet`), which works today but embeds a Go runtime in the
     process and needs Go in the build on all three OSes. Pick at build time by
     maturity: don't ship on a preview crate without checking its status and
     security posture first.
   - **State** (the node key) lives beside the journal in its own file, like
     `devices.json`, so it survives restarts and is removed by "Erase all data". The
     computer appears in the user's Tailscale admin as "Field Notes (<computer name>)".
   - **Replaces `tailscale serve`.** `portal_serve` / `portal_unserve` stop shelling out
     to the `tailscale` CLI; the embedded node listens on the tailnet over HTTPS and
     hands requests to the same loopback handler.
   - **Existing Tailscale installs keep working.** If the computer already runs the
     Tailscale app, offer "Use the Tailscale app already on this computer" and keep
     today's path, so current users don't have to re-pair.
   - **Client mode** (`remote.rs`, laptop → server) needs no change: the laptop still
     reaches the server's `*.ts.net` address through its own Tailscale. Bundling there
     too is a later, separate decision.

   ⚠️ **The four rules in `portal.rs` don't move.** The tailnet listener forwards to
   the same token-checked, lock-checked, allowlisted dispatcher. Rule 1's wording
   changes from "only reachable via `tailscale serve`" to "only reachable via the
   embedded tailnet node"; the point (never a LAN or public listener) holds, and a
   test should assert the embedded node never enables Funnel (public exposure).

   ⚠️ **Open risks to settle before building:**
   - **HTTPS certificates.** A `*.ts.net` HTTPS cert needs MagicDNS and HTTPS enabled
     on the tailnet. New tailnets have MagicDNS on; HTTPS may need one click in the
     admin console. Confirm what a brand-new account needs, and make that click a
     guided step if it can't be avoided.
   - **Build weight and signing.** Measure binary size and confirm notarization
     (macOS) and SmartScreen (Windows) still pass with the library in.
   - **Phone side unchanged:** iOS/Android allow one VPN at a time, so a user on
     Mullvad/Proton/work VPN still has a conflict. The copy should say so rather than
     hide it.

   **Tests:** state file created beside the journal and erased with it; Funnel never
   enabled; a server that already runs the Tailscale app keeps the old path; the
   step checks report correctly for signed-out / signed-in / paired; error copy maps
   each failure (no route, 502, 503, 401) to its own message.

   **Considered: a peer-to-peer tunnel with a relay (e.g. iroh). Not chosen; revisit
   later.** Devices would dial each other by public key and fall back to an encrypted
   relay; the QR code alone would carry everything, with **no account and no app to
   install on the phone**, which is the setup this item can't reach. Why not now:
   - **The phone is a browser page.** Browsers can't send raw UDP, so iroh in a browser
     (WASM, since 0.33) is **relay-only**: every phone request goes through a relay
     server. That's a dependency on n0's relays or on running our own, which is a
     public service to operate, and it sits awkwardly with "no network requests" even
     though relays can't read the traffic.
   - **It's a new transport layer**: replacing `portal.ts`'s `fetch` and the HTTP
     server with an iroh connection on both ends, plus key handling, reconnects and
     relay failure cases, all on the path the safety features depend on. Tailscale's
     security, NAT traversal and key rotation are already proven; ours wouldn't be.
   - **Revisit if** a native phone app ever happens (the native side can hole-punch
     directly, so relays become a fallback), or if Tailscale's account requirement
     turns out to be the thing that stops users. Pairing tokens and `EXPOSED` would
     carry over unchanged either way, since they sit above the transport.

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

**Current release: v0.14.1** (2026-10-01; v0.14.0 plus code cleanup, no behaviour change). v0.14.0 brought usage stats, import timestamps in the
backend, the whole-word combo-matching fix, split date/time fields, a manual update
check, and phone-installed server updates. Next:

1. **Real-device checks the release couldn't do:** tap a dose time on iPhone and in
   the Mac app (no calendar should open); install the *next* release from a phone.
2. **Owner decisions still open:** stats export, and Companion access to stats.
3. **#1 Phase 3b — on hold.** See the note at the end of this section.

The plain-entries write-up below is kept for the decisions it records (above all the
2026-07-14 crisis-scan decision), not because it's pending.

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

**Phase 3b stays on hold until 3a has been used enough to know.** Since v0.12 a
*laptop* client queues offline under SQLCipher with the safety checks intact, so the
only gap left is the **phone** while the server is asleep. Offline capture is a *separate
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
