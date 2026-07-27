<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
# Releasing Field Notes

The checklist for cutting a release. CI does the building; the human (or agent)
does the verifying, the version bump, and the release notes.

## 1. Verify

```bash
cd src-tauri && cargo test          # all suites green
cd .. && npm run check              # svelte-check: 0 errors
npm run build                       # frontend builds
```

- If the release touches anything platform-specific, run the **"Windows build
  (no release)"** workflow from the Actions tab first (`workflow_dispatch`). It
  builds real NSIS/MSI installers and attaches them to the run without touching
  releases. (It deliberately skips `cargo test` — Windows test binaries can't
  launch due to [tauri#13419](https://github.com/tauri-apps/tauri/issues/13419);
  the shipped app is unaffected.)

## 2. Bump the version

The version lives in **three files** and two lockfiles. Bump all of them in one
commit:

```bash
# edit: package.json, src-tauri/Cargo.toml, src-tauri/tauri.conf.json
npm install --package-lock-only                        # syncs package-lock.json
(cd src-tauri && cargo metadata --format-version 1 >/dev/null)  # syncs Cargo.lock
```

Also update `ROADMAP.md` (mark shipped items with the version) and `README.md`
(the feature list) if features shipped.

## 2a. Write the changelog section (before tagging)

Add a `## vX.Y.Z` section to the top of **`CHANGELOG.md`** with the user-facing
"what's new", written for the person reading it **inside the app**. The release
workflow extracts this exact section and uses it as the release body — which is
also what goes into `latest.json` and what the in-app "a new version is
available" prompt now shows. So keep it to what changed; **no** download links
or Gatekeeper/SmartScreen help here (those go on the GitHub page in step 4).

The heading must be exactly `## vX.Y.Z`, matching the tag, or the build falls
back to a generic one-liner (and logs a warning).

## 3. Tag

Work lands on `main` first (merge feature branches, make sure CI-relevant
changes are in). Then:

```bash
git tag vX.Y.Z
git push origin main vX.Y.Z
```

- The `Release` workflow builds macOS (universal), Linux, and Windows in
  parallel (~15–25 min) and creates a **draft** release with all assets.
- A tag containing `-` (e.g. `v0.6.0-beta.1`) is automatically marked
  **prerelease**, which keeps it away from `/releases/latest` and therefore away
  from everyone's auto-updater. Plain tags become the update everyone is offered.

## 4. Round out the GitHub release page

The workflow already fills the draft body with the `CHANGELOG.md` section from
step 2a — that part is done, and it's what `latest.json` and the in-app prompt
carry. What the changelog deliberately leaves out is the web-page furniture, so
**append** it to the draft body (don't replace what's there):

1. **Downloads table** — direct links to the files a human wants
   (`_x64-setup.exe`, `.msi`, `_universal.dmg`, `.AppImage`, `.deb`, `.rpm`).
   Asset URLs follow the pattern:
   `https://github.com/sparkly-quasar/field-notes/releases/download/vX.Y.Z/<asset-name>`
2. **First-launch notes** — SmartScreen (Windows) and Gatekeeper (macOS)
   workarounds, since installers are unsigned.

Keep this download/first-launch furniture **out of `latest.json`** — it lives
only on the web page. That's why it's appended here rather than in `CHANGELOG.md`.

Constraints worth knowing:

- **Assets cannot be grouped, reordered, or renamed.** GitHub shows a flat
  alphabetical list, and `latest.json` references several assets by exact URL
  for the auto-updater — renaming anything breaks in-app updates. The downloads
  table in the body is the fix.
- The `.sig` files, `.app.tar.gz`, and `latest.json` belong to the updater; say
  so in the notes so nobody wonders.

## 5. Publish

```bash
gh release edit vX.Y.Z --draft=false --latest
```

## 6. Confirm

```bash
# The updater endpoint must serve the new version:
curl -sL https://github.com/sparkly-quasar/field-notes/releases/latest/download/latest.json | head -5
# Spot-check one download link from the notes (expect 200):
curl -sIL -o /dev/null -w '%{http_code}\n' \
  "https://github.com/sparkly-quasar/field-notes/releases/download/vX.Y.Z/Field.Notes_X.Y.Z_x64-setup.exe"
```

Existing installs are offered the update on next launch ("Install & restart").

## If a platform build fails

The draft release stays unpublished (never publish a partial release — the
updater would offer an update some platforms can't complete). Fix, delete the
draft and the tag, and re-tag:

```bash
gh release delete vX.Y.Z --yes
git tag -d vX.Y.Z && git push origin :refs/tags/vX.Y.Z
# fix, then tag again
```
