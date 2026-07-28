<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
# Changelog

User-facing notes, one section per released version. The release workflow reads
the section whose heading matches the tag (`## vX.Y.Z`) and uses it as the
release body — which is also what lands in `latest.json` and what the in-app
"a new version is available" prompt shows. So: write for the person who will
read it inside the app, keep it to what changed, and don't put download links
or Gatekeeper/SmartScreen help here (those are added to the GitHub release page
after publishing — see RELEASING.md).

The heading must be exactly `## vX.Y.Z`, matching the tag. Newest on top.

## v0.11.1

- **Top-bar tidy-up.** The menu is reordered into a more natural flow — Journal,
  Substance Log, Substance Directory, Companion, Settings, then Emergency
  Resources and Report a bug at the end. The old "Substances" reference tab is now
  labelled **Substance Directory** so it's clearer what it is, and the "Report a
  bug" button lost its emoji so it sits cleanly alongside the rest.

## v0.11.0

- **Logging a past experience is now a first-class option.** The "+ Session" form
  has a **"This already happened"** checkbox — tick it and you get an end-time
  field, and the session is saved as a finished trip (not an ongoing one) rather
  than having to start a live session and backdate it. Doses you add to a
  finished session default to when it happened instead of "now", so writing up an
  old trip doesn't mean fixing every timestamp.
- **A "Report a bug" button now lives on the top bar**, so filing a bug or feature
  request is one click from anywhere instead of buried in Settings. It opens the
  same prefilled GitHub issue in your browser — nothing leaves your journal.

## v0.10.2

- **This update prompt now tells you what changed.** Until now it only showed a
  version number; from here on it shows the actual list of what's new — the notes
  you're reading. (You're seeing this because you updated *to* the version that
  added it, so this is the first time it appears.)
- **The app now checks for updates on its own while it's open**, every few hours,
  so a machine left running for days still finds out about a new version without
  being restarted. The check is silent — you'll only ever notice it when there's
  actually something new.

## v0.10.1

- **You can see when your phone has paired.** Setting up phone access used to
  give no feedback — you showed the QR code, scanned it, and then guessed. Now a
  green "Paired successfully" light appears the moment your phone first connects.
  It confirms a phone has paired since you turned phone access on (not that one
  is connected this second), and resets when you turn phone access off. Phone
  access is unchanged otherwise: off by default, tailnet-only, code on every
  request.

## v0.10.0

- **The Companion stays responsive.** It no longer locks the window while it
  thinks (worst on the first message, when the model loads), and the model
  pre-loads when you open the Companion so the first reply comes back at normal
  speed. Same fix for Import from text.
- **It's honest about your hardware.** The app checks whether your machine can
  run the model you picked and says so plainly — before a multi-gigabyte
  download, and again using real reply speed once you've chatted.
- **Import from text actually works now.** It reads any format, pulls out every
  dose, and uses each substance's standard name so the interaction checker
  recognises it. The screen shows everything it found for review.
- **A first-run choice for the Companion**, asked once on a fresh install.
- **Send feedback in one click** — a Settings card opens a prefilled GitHub
  issue in your browser. Nothing is sent from the app itself.
