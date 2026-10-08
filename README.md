# Field Notes

**A private offline journal and safety companion for psychedelic and other substance experiences.**

Field Notes is a journal that understands what an experience is. Write plain diary
entries or log an experience as it happens — what you took, how much, and how
you're feeling over time. Before you combine substances, check them against a
built-in reference of known dangerous combinations. If you want one, an
optional AI companion can be there to talk — it runs entirely on your machine,
so the conversation never leaves the room.

There are no accounts, no cloud, and no network requests. Your journal can be
encrypted with a password, and nothing you write is ever scanned, analyzed, or
sent anywhere. It works on **Windows, macOS, and Linux**, with optional access
from your phone: that's the one exception, off until you turn it on, and it
uses a free Tailscale account to connect your own devices. Tailscale sees
which of your devices are connected, never what's in your journal.

### ⬇️ [Download Field Notes](https://github.com/sparkly-quasar/field-notes/releases/latest) · [How to install](#install--update)

Free for Windows, macOS and Linux. The installers aren't code-signed yet, so
each OS warns you once on first launch; [the install steps](#install--update)
show you how to get past it.

> ⚠️ **Harm-reduction and journaling tool — not medical advice, and not
> encouragement to use anything.** Dose and interaction information is a reference
> and safety backstop only: incomplete, possibly wrong, and no substitute for a
> qualified clinician. The interaction checker flags only some well-known dangerous
> combinations — **absence of a warning does not mean a combination is safe.** In
> an emergency, contact local emergency services or poison control.

## Features

- **Journal** — log experiences with intention, set & setting, doses, and a
  running timeline of how you feel. A street name is saved as the substance it
  means ("acid" as LSD, unless you choose to keep your wording), and Stats counts
  both spellings as one. Any entry can be saved or shared as a tidy **PDF**
  (from the phone's share sheet too), choosing whether it includes intention and
  setting, moments, the write-up and the rating. Alcohol logs in standard drinks (beer, wine
  and shot quick picks) and smoked cannabis in hits. Edit, delete, or backdate anything. Logging
  something that already happened is a first-class option — tick "this already
  happened" on a new experience and it's saved as a finished trip, with doses
  defaulting to when it occurred rather than now. Every timestamp in an experience
  also shows the time since your first dose (`14:02 (t+1:23)`), so the timeline
  reads against the clock that matters. An experience you never got round to titling
  takes the name of the first substance you log into it, so nothing sits in the
  journal as "Untitled" — rename it whenever you like.
- **Plain notes** — not everything is an experience. Write ordinary journal entries
  (a title, your words, a date) alongside them.
- **Active arcs** — a live trip report charts what each substance is likely
  doing now, from the dose reference's timings: one line per substance with every
  dose added in, its height the reference's dose tier, and a "Likely still active"
  list of phases. Drinks stack on their own scale. Tap a substance for its timing
  range and what a redose does for it. Anything logged in the last two days that
  may still be active shows on Today and in Check, live trip report or not, and a
  finished trip report keeps its arcs with your moments on them.
- **Combination warnings** — every dose is checked against the others taken
  around the same time for well-documented risky combinations. Each shows as a
  quiet note you can tap for what the risk is and what lowers it, and there's a
  standalone checker to consult *before* taking anything.
- **Dose reference** — dose ranges, durations, and graded interaction data for
  hundreds of substances, bundled with the app and available offline. Search the
  reference prose, or read any of the 575 substance entries **in full** —
  pharmacology, harm potential, tolerance, legality — with the exact dose figures
  alongside. Sourced from [DoseWiki](https://dose.wiki) (public domain).
- **Companion** (optional, off until you turn it on) — a calm, non-judgmental
  support chat that runs on a local AI
  model. It can be aware of your live trip report, look up references, and log
  things for you when you ask. Pick a support style ("just listen", "keep me
  grounded", …) and it honors it. It loads in the background so the window never
  freezes while it thinks, and it tells you up front if your machine is short on
  memory to run the model. It's improving but still rough in places — see
  [Companion quality](#companion-quality). It's off on a new install: turn it on
  in Settings, and the Companion tab walks you through the separate download.
  Nothing else in the app needs it.
- **Quick dose log** — most of what gets recorded isn't a trip you sit through and
  write up, it's "I took this, at about this time". **+ Dose** on the desktop, or
  the top of the phone's Now screen, takes a substance, an amount and a time —
  recent substances and times like "last night" are one tap — and leaves a real
  entry in the journal. Add the notes, rating and any other doses later, from
  either screen, or don't.
- **Live trip report** — a quiet, altered-state-friendly screen for an ongoing
  experience: elapsed time, one-tap logging, the companion, and an always-visible
  **Get help now** button. Timeline notes can be edited after the fact — on the
  desktop and from the phone.
- **Crisis resources** — if something you write to the Companion sounds like it
  might be urgent, real emergency and peer-support contacts are offered, calmly.
  What you've logged never raises this on its own. This is driven by fixed rules,
  never by the AI — and your journal writing is never scanned.
- **Import from text** — paste a past experience in any form, from a one-line note
  to a full trip report with T+ timestamps, and the local model pulls out the
  substances, doses, and timeline into a structured entry you review before saving.
- **Reference search** — search thousands of passages of substance information
  (pharmacology, tolerance, legality) offline.
- **Encryption & backups** — optional password encryption for the whole journal,
  plus one-file backup and restore.
- **Obsidian sync** — export entries to an Obsidian vault as readable Markdown
  notes and import them back; works both ways, fully offline. Any single entry
  can also be exported on its own ("Export this entry" on the desktop, "Export"
  on the phone) in the same format, so it drops straight into a vault.
- **Phone access** (optional, off by default) — Tailscale is built in, so the
  computer needs nothing else installed: press **Connect**, sign in to Tailscale
  in your browser, then install Tailscale on your phone, sign in with the same
  account, and scan the code. Then use the
  journal, combo checker, reference, and Companion from bed at 3am. The desktop
  shows a green **"Paired successfully"** light the moment the phone first uses
  the code, so you're not left guessing whether the scan took. Companion
  replies run as a background job on the desktop, so a slow local model — or a
  locked phone screen — no longer drops the answer. Starting a live trip report from the
  phone takes an optional title — or leave it blank and the first substance you
  log names it. Or skip the trip report entirely: **log something you took** —
  substance, amount, and when — as a one-shot entry, for any day you're catching
  up on. Past entries are editable from the phone too: tap any dose or note to
  correct it, add one you forgot, or open the entry to change its title, times,
  rating and write-up. **Works without the computer, too:** if it's asleep or
  out of reach, the phone still opens, Check and Look up answer on the phone
  (the same checker, compiled for it, over a saved copy of the reference), and
  new doses, moments and entries wait on the phone until the computer is back.
  The gear next to Help holds the phone's settings: discreet mode, what it keeps
  for offline use, updating the computer, the Home Screen link, and pairing or
  un-pairing your devices, which asks for your journal's password (or, if the
  journal isn't encrypted, a phone PIN set at the computer).
  Private by design: see [Architecture](#architecture) for how.
- **Paste a trip log** — copy a log from your notes app (`8:43am - 35mg mesc`,
  `T+1:30 peak`…) and it becomes a timed experience, doses recognised by name or
  street name. Lists of doses, other people's doses and the write-up after the
  log are sorted out too. Read on the device, no AI; every line shown before
  it's saved.
- **One journal, all your computers** (optional) — let a computer that stays on
  be your Field Notes server, and connect a laptop to it over Tailscale: same
  journal, everywhere. Each device gets its own revocable key. If the server
  can't be reached, new entries save on the laptop and are sent when it's back,
  and the interaction checker keeps working in the meantime. **Sync journal to
  server** brings the entries a laptop already had along with it, skipping any
  the server already has. In **Server
  Mode**, the server can start serving at login and — opt-in — unlock from the
  system keychain after a reboot.
- **Substance catalogue & log** — keep your own substance list with notes, and
  review your history grouped by substance.
- **Stats** (desktop and phone): dose over time against the reference's dose
  ranges, spacing between experiences, how often, a calendar, and what you've taken
  together, plus **Trends**: what changed lately against the period before (how
  often, time between, amount per experience, redosing), each with a small chart.
  Tap a day, bar, hour or dot to see the experiences behind it.
  **Dose-aware**: each dose shown as what it was logged as against the
  reference's ranges, microdoses recognised, mushroom forms (fresh, capsules,
  edibles), notes on what an amount tends to do, routine and as-needed doses,
  doses near bedtime and an optional morning sleep check-in, and a short note
  when a pattern linked to dependence or withdrawal shows up.
  No streaks or scores, and a toggle hides substance names for screen-sharing.
- **Updates** — automatic, a **Check for updates** button in Settings, and
  (opt-in, set at the computer) a paired phone can install an update on the
  computer it connects to.
- **Contribute upstream** (consent-gated) — export substances you've catalogued
  that DoseWiki doesn't cover as a draft to submit by hand. Never automatic,
  never includes journal data.
- **Send feedback** — a link in Settings opens a prefilled bug report or feature
  request on GitHub, in your browser. Nothing is sent from the app itself.

## Screenshots

All screenshots use a fictional demo journal.

### Desktop

| | |
|:---:|:---:|
| ![Journal home](docs/screenshots/journal-home.png) | ![Live trip report](docs/screenshots/live-session.png) |
| **Journal**: experiences and notes by month, actions in one quiet bar | **Live trip report**: last dose, quick log and one-tap moments |
| ![Experience detail](docs/screenshots/session-detail.png) | ![Stats](docs/screenshots/stats.png) |
| **Experience detail**: what you took, when, and how it went | **Stats**: dose over time against reference ranges, spacing, frequency, calendar |
| ![Companion chat](docs/screenshots/companion-chat.png) | ![Substance log](docs/screenshots/substance-log.png) |
| **Companion**: local AI support chat with selectable support styles | **Every dose, by substance**: the full history, under Stats |
| ![Check](docs/screenshots/substances-reference.png) | ![Reference search](docs/screenshots/reference-search.png) |
| **Check**: test a combination, search the reference, keep your own catalogue | **Reference search**: thousands of DoseWiki passages, searchable offline |
| ![Emergency help](docs/screenshots/emergency-help.png) | ![Updates](docs/screenshots/settings-updates.png) |
| **Get help now**: real crisis and peer-support contacts, always one tap away | **Updates**: automatic, or check by hand any time |
| ![Devices and server](docs/screenshots/settings-phone-access.png) | ![Encryption and backup](docs/screenshots/settings-encryption-backup.png) |
| **Devices & server**: optional, built-in Tailscale, off by default | **Encryption & backups**: AES-256 at rest, one-file backup and restore |
| ![Obsidian vault sync](docs/screenshots/settings-obsidian-sync.png) | ![Erase and uninstall](docs/screenshots/settings-data-location.png) |
| **Obsidian sync**: two-way Markdown export, fully offline | **Your data, one folder**: everything lives on your device, erase anytime |

### Phone

| | | |
|:---:|:---:|:---:|
| ![Today](docs/screenshots/phone-today.png) | ![Journal](docs/screenshots/phone-journal.png) | ![Entry](docs/screenshots/phone-entry.png) |
| **Today**: the live trip report at your thumb | **Journal**: search and filter everything | **Entry**: a past experience, editable |
| ![Stats](docs/screenshots/phone-stats.png) | ![Stats chart](docs/screenshots/phone-stats-chart.png) | ![Combo check](docs/screenshots/phone-check.png) |
| **Stats**: range, substances, totals | **Dose over time**: tap a dose for details | **Check**: combinations, from the deterministic checker |

## Install & update

Download the installer for your computer from the
**[latest release](https://github.com/sparkly-quasar/field-notes/releases/latest)**
(the files are listed under *How to install* on that page), then follow the
steps for your system. The installers aren't code-signed yet, so the first
launch needs one extra click.

### Windows

1. Download the file ending in **`_x64-setup.exe`** (or the `.msi`, if you prefer).
2. Open it. Windows shows **"Windows protected your PC"**: click **More info**,
   then **Run anyway**.
3. Follow the installer, then open **Field Notes** from the Start menu.

### macOS (Apple Silicon and Intel)

1. Download the file ending in **`_universal.dmg`**.
2. Open it and drag **Field Notes** into **Applications**.
3. Open Field Notes. macOS blocks it the first time because it isn't notarized
   yet: click **Done**.
4. Go to **System Settings → Privacy & Security**, scroll down and click
   **Open Anyway** next to *"Field Notes" was blocked*.
5. Open Field Notes again and click **Open**. You only do this once.

On macOS 15 Sequoia and later, right-click → Open no longer skips the block, so
use step 4. In Terminal, `xattr -dr com.apple.quarantine "/Applications/Field Notes.app"`
does the same thing. If the icon keeps bouncing and no window opens, select
Field Notes in Applications, press **Cmd+I**, and untick **Locked**.

### Linux

- **AppImage (any distribution):** download the `.AppImage`, make it executable
  (`chmod +x Field.Notes_*.AppImage`), and run it.
- **Debian / Ubuntu:** `sudo apt install ./Field.Notes_*_amd64.deb`
- **Fedora / RHEL:** `sudo dnf install ./Field.Notes-*.x86_64.rpm`

### Updating

The app checks for updates on launch and installs them in place
("Install & restart"), signed and verified, fully in-app. You can also install
any release over the old version by hand. **Your journal is safe either way.**
All data lives in the OS app-data directory
(`%APPDATA%\com.fieldnotes.journal` on Windows,
`~/Library/Application Support/com.fieldnotes.journal` on macOS,
`~/.local/share/com.fieldnotes.journal` on Linux), separate from the app itself,
so updating never touches it.

## Support Field Notes

Field Notes is free, with no ads, accounts or tracking. If it's been useful to
you, please consider [chipping in toward keeping it going](https://github.com/sponsors/sparkly-quasar).

Not in a position to give money? Reporting bugs, suggesting features, and
passing Field Notes along to a harm reduction group help too.

## Architecture

- **Tauri 2 + Svelte (TypeScript)** desktop app (Windows + macOS + Linux), built
  on the same stack as [Cairn](https://github.com/sparkly-quasar/cairn).
- **Local SQLite** (`rusqlite`, bundled) at the app data dir — `substances`,
  `experiences` (experiences with doses *and* plain notes, split by an explicit `kind` column),
  `doses`, `timeline_events`. No network, no accounts. Opt-in **SQLCipher**
  encryption at rest (AES-256).
- The safety-critical layers are **deterministic Rust, independent of any model** —
  `interactions.rs` (interaction rules + class vocabulary, common-knowledge
  harm-reduction categories not derived from any copyrighted source), `crisis.rs`
  (crisis signals in Companion chat + graded resources; journal prose is never
  scanned — owner's decision, recorded in `ROADMAP.md`), `pw.rs` (the dose
  reference; snapshot + slimming pipeline in [`data/dosewiki/`](./data/dosewiki/)).
  The checker, the reference and the corpus search live in the small
  `src-tauri/core` crate, which `src-tauri/wasm` compiles to WebAssembly for the
  phone's offline mode: one checker, not a second copy to drift.
- `knowledge.rs` — BM25 over the bundled DoseWiki prose corpus (7,800+ passages,
  575 substances), in-process, no embeddings.
- `portal.rs` — the optional device server. Binds **127.0.0.1 only** and is fronted
  by your **Tailscale tailnet**; every request needs a paired device's token
  (`devices.rs`: one per device, stored only as a hash, revocable); it refuses to
  serve a locked journal; and it exposes a strict **allowlist** — wiping the
  journal, the passphrase, backups, and filesystem access are unreachable from a
  phone by construction. Its module docs state four load-bearing rules, and tests
  pin all four.
- `tailnet.rs` + [`tailnet/`](./tailnet/) — the Tailscale built into the app: a
  small Go program around Tailscale's own `tsnet` library, shipped as a Tauri
  sidecar. It joins your tailnet as its own device (`field-notes`) and forwards
  HTTPS there to the portal on loopback, so the computer needs no Tailscale
  install. It never uses Funnel, forwards only to `127.0.0.1`, and uploads no
  logs; a test reads its source to keep it that way. A computer that already
  published through the Tailscale app keeps using the app.
- `remote.rs` — the client half: a desktop that uses another computer as its
  journal. Journal commands go over that computer's portal; new entries queue in
  the local (encrypted) journal while it's unreachable, with the interaction
  checker and crisis scan still running locally. The phone's equivalent is
  `src/lib/offline.ts` + `outbox.ts` (same rules: only new entries wait, edits
  and deletes don't) and a service worker that keeps `/m` on the phone. It keeps
  as little as it can in browser storage: what's waiting to be sent, the
  experiences in progress, and the last day's entries by substance and time, for
  the offline check. `keychain.rs` is the opt-in
  "remember the password" for a server that must unlock itself after a reboot.
- `contribute.rs` — upstream draft exports, with no HTTP client in the file at all.
- The AI features (Companion, text import) talk only to a local
  [Ollama](https://ollama.com) instance on `127.0.0.1` — the app can install it
  and download a model for you on first use.

## Companion quality

The Companion is the least finished part of this app, and it's worth being
straight about that. v0.9.0 made real improvements — it no longer invents
durations or cites references it never read, and the default model was changed
to one that will actually engage when things get hard. There's an evaluation
harness (`src-tauri/eval/scenarios.json`, 30 scenarios) so changes are measured
rather than guessed at.

It still has rough edges. It runs long when it should be brief, occasionally
misses a tool call it should have made, and its register drifts from the calm,
non-directive tone it's aiming for. Prompt work to address that is the next
step. Quality also depends on your hardware — a small model on an older machine
will be noticeably worse.

None of this touches the safety-critical layers. The interaction checker, crisis
detection and dose reference are deterministic Rust that run regardless of which
model is loaded, or whether one is loaded at all. If the Companion isn't earning
its keep on your machine, turn it off in Settings; everything else is unaffected.

## Roadmap

- **Companion register** — prompt restructuring and worked examples, so it stays
  brief and non-directive. Measured against the eval harness.

See [`ROADMAP.md`](./ROADMAP.md) for the full picture and design history.

## Development

```bash
npm install
scripts/build-tailnet.sh   # the bundled Tailscale helper; needs Go 1.27+
npm run tauri dev
npm run tauri build
```

Build the helper once before any `cargo` command (`cargo test` included): Tauri
checks that `src-tauri/binaries/fieldnotes-tailnet-<target>` exists at compile
time. Rebuild it after changing anything in `tailnet/`.

Releases are cut per [`RELEASING.md`](./RELEASING.md).

### Development notes

Things that aren't obvious from the code, and that have already cost a debugging
session at least once:

- **`src/lib/api.ts` is the only file allowed to import Tauri's `invoke`.** It is the
  single seam where the desktop (`invoke`) and the phone (`fetch` to the portal) diverge,
  which is why the UI never had to learn that the portal exists. Import `invoke`
  anywhere else and the phone silently breaks.
- **`portal.rs`'s `EXPOSED` is an allowlist.** A new command in `commands.rs` is
  unreachable from the phone until someone adds it there *on purpose* — that's the point.
  The four rules in that file's module docs are load-bearing; tests pin all four.
- **Adding a command to `EXPOSED` is a security decision, not a plumbing one.** Ask what
  it does in the hands of someone holding a phone that isn't yours. `ai_start` is exposed
  (it wakes a loopback server the app already owns); `ai_install` and `ai_pull` are not
  (they install software and download gigabytes). The one exception is
  `server_update_install`: it installs only a signed Field Notes release, is off until
  enabled at the computer, and refuses unless the server will come back by itself
  (see `server_update.rs`). The switch that enables it is not exposed.
- **Known rough edge:** browsing to the portal's `/` (rather than `/m`) from a phone
  serves the *desktop* page, which half-renders and throws console errors. Harmless —
  the allowlist is server-side — and fixed as part of Phase 3b.
- **Verifying the UI without a screen.** macOS may withhold Screen Recording /
  Accessibility, which blocks screenshots and AppleScript. Headless Chromium needs
  neither: point it at the running portal (`http://127.0.0.1:<port>/m#t=<token>`) and
  drive the real app over a real socket, phone viewport and all. This is how `/m` and the
  desktop Substances tab are actually verified, rather than merely typechecked.
- **Two bugs got past "the tests pass" and were caught by a human on a phone** — the
  Tailscale step being too much to ask of an end user, and the Companion tab giving up
  forever if Ollama was asleep at page load. Typechecks and unit tests don't open tabs.
  Drive the thing.
- **Windows CI skips `cargo test`** — Windows test binaries die at launch with
  `STATUS_ENTRYPOINT_NOT_FOUND` because tauri-build links the app manifest only to
  the main binary ([tauri#13419](https://github.com/tauri-apps/tauri/issues/13419)).
  The shipped app is unaffected; the suite runs on macOS/Linux.

## License

Licensed under the **[PolyForm Noncommercial License 1.0.0](./LICENSE)**.

Free to use, modify, and share for **non-commercial** purposes (personal use,
research, education, nonprofits, government). **Commercial use requires a separate
commercial license — a contract with the author.** For commercial licensing,
contact the author via [github.com/sparkly-quasar](https://github.com/sparkly-quasar).
