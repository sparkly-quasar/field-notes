# Field Notes

**An offline, private harm-reduction journal & trip-sitting workstation.**

Field Notes lets you catalogue substances, log experiences and the doses taken
during them, get flagged about dangerous interactions, and review your history
organized by substance — all stored **locally on your own device**, nothing sent
anywhere.

> ⚠️ **Harm-reduction and journaling tool — not medical advice, and not
> encouragement to use anything.** Dose and interaction information is a reference
> and safety backstop only: incomplete, possibly wrong, and no substitute for a
> qualified clinician. The interaction checker flags only some well-known dangerous
> combinations — **absence of a warning does not mean a combination is safe.** In
> an emergency, contact local emergency services or poison control.

> **Status:** early foundation. Local journal + dose logging + a deterministic
> safety-interaction checker + by-substance history. Built on the same Tauri +
> Svelte stack as [Cairn](https://github.com/sparkly-quasar/cairn).

## What works today

- **Journal** — create experiences (intention, set & setting), log doses with
  amount/unit/route and a live timeline of how you're feeling. Full edit & delete,
  and backdate anything so you can record past experiences accurately.
- **Import from text** — paste a past experience in your own words and a **local**
  model extracts the substances, doses, and timeline into a structured record for
  you to review before saving. Runs entirely on-device via Ollama.
- **Companion** — a calm, non-judgmental harm-reduction chat, also fully local
  (Ollama). It can be made aware of your current session (logged doses + interaction
  flags) and never encourages use.
- **Dose reference (PsychonautWiki)** — download dose ranges, durations, and
  dangerous-interaction data for hundreds of substances **once** into a local
  cache, then see them inline while logging — fully offline afterward. Data from
  [PsychonautWiki](https://psychonautwiki.org) (CC-BY-SA 4.0); attribution shown
  in-app.
- **Safety checker** — every dose is checked against the others in that experience
  for widely-documented dangerous combinations (opioid + benzodiazepine, MAOI +
  serotonin releaser, lithium + psychedelics, SSRI + MDMA, …), rated
  danger / caution / note. Works via coarse pharmacological *classes*, so it also
  covers substances you add yourself once they're classified.
- **Substances** — catalogue substances, assign interaction classes (common ones
  are auto-classified), keep your own dose notes.
- **By substance** — every dose grouped by substance, so you can see your history
  and typical dosages at a glance.

## Architecture

- **Tauri 2 + Svelte (TypeScript)** desktop app (macOS + Linux).
- **Local SQLite** (`rusqlite`, bundled) at the app data dir — `substances`,
  `experiences`, `doses`, `timeline_events`. No network, no accounts.
- `src-tauri/src/interactions.rs` — the deterministic interaction rules + class
  vocabulary (common-knowledge harm-reduction categories, not derived from any
  copyrighted source).

## Roadmap (not yet built)

- **Substance knowledge pack** — an offline RAG corpus from openly-licensed sources
  (PsychonautWiki CC-BY-SA, TripSit). *Licensing must be settled first; PiHKAL /
  TiHKAL are copyrighted and cannot be bundled — pointer / user-import only.*
- **Local LLM companion** — a calm trip-sitting assistant with tool access to the
  journal, running against a local model (via Cairn / Ollama). Fully offline.
- **Obsidian vault integration** — process journal entries, write structured
  summaries back.
- **User-added substances → opt-in PsychonautWiki contribution** (consent-gated).
- **Encrypted-at-rest database** (passphrase) for this sensitive data.

## Install & update

Download an installer for your platform from the
[Releases page](https://github.com/sparkly-quasar/field-notes/releases):

- **macOS** — open the `.dmg`, drag Field Notes to Applications. Not notarized yet,
  so on first launch **right-click → Open** (or `xattr -dr com.apple.quarantine
  "/Applications/Field Notes.app"`).
- **Linux** — the `.AppImage` (make it executable and run), or the `.deb` / `.rpm`.

**To update:** grab the latest release and install it over the old version
(replace the app in Applications, or the AppImage; reinstall the `.deb`/`.rpm`).
**Your journal is safe** — all data lives in the OS app-data directory
(`~/Library/Application Support/com.fieldnotes.journal` on macOS,
`~/.local/share/com.fieldnotes.journal` on Linux), separate from the app bundle,
so updating never touches it. Check the Releases page for new versions; there is
no auto-update yet.

## Development

```bash
npm install
npm run tauri dev
npm run tauri build
```

## License

Licensed under the **[PolyForm Noncommercial License 1.0.0](./LICENSE)**.

Free to use, modify, and share for **non-commercial** purposes (personal use,
research, education, nonprofits, government). **Commercial use requires a separate
commercial license — a contract with the author.** For commercial licensing,
contact the author via [github.com/sparkly-quasar](https://github.com/sparkly-quasar).
