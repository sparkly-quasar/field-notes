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
  amount/unit/route and a live timeline of how you're feeling.
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

## Development

```bash
npm install
npm run tauri dev
npm run tauri build
```
