<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
# Releasing Field Notes

The checklist for cutting a release. CI does the building; the human (or agent)
does the verifying, the version bump, and the release notes.

## 1. Verify

```bash
scripts/build-tailnet.sh            # the bundled Tailscale helper (CI builds its own)
cd src-tauri && cargo test          # all suites green
cd .. && npm run check              # svelte-check: 0 errors
npm run build                       # frontend builds
```

- If the release touches `tailnet/` or `tailnet.rs`, test phone access by hand on a
  real computer and phone before tagging a plain release: Connect, sign in, scan,
  and check the phone works on cellular. CI can't sign in to Tailscale.
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
or Gatekeeper/SmartScreen help here (the workflow adds those to the GitHub page; see step 4).

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
- **The workflow never publishes.** A draft is invisible to the in-app updater.
  The release goes out to everyone at step 5, and only then. If you (or a script
  or agent on your machine) run steps 4 and 5 as soon as the build finishes, the
  tag is effectively the point of no return: v0.18.1 and v0.19.0 were public
  within a minute of their builds. Decide before tagging.
- A tag containing `-` (e.g. `v0.6.0-beta.1`) is automatically marked
  **prerelease**, which keeps it away from `/releases/latest` and therefore away
  from everyone's auto-updater. Plain tags become the update everyone is offered.

### Holding a release back

To get builds onto your own devices without offering them to everyone:

- **Tag a prerelease**, e.g. `v0.20.0-beta.1`. Any tag with a `-` is marked
  prerelease, and the updater reads `/releases/latest`, which skips prereleases.
  It needs its own `## v0.20.0-beta.1` changelog section, or the body falls back
  to a one-line note (fine for a beta).
- **Already published a plain tag too early?** Edit the release on GitHub and
  tick *Set as a pre-release*. That takes it out of `latest`, so the updater stops
  offering it. Anyone who already updated keeps it.
- **Mid-build?** Cancel the run from the Actions tab, then delete the draft and
  the tag as in *If a platform build fails* below.

## 4. Check the GitHub release page

This step is automatic now. When all three platform builds have uploaded, the
workflow's `install-guide` job appends **`.github/release-install.md`** to the
draft body, below the `CHANGELOG.md` section, with `@VERSION@` / `@TAG@` filled
in. The guide has step-by-step install instructions per OS with direct
download links, the SmartScreen and Gatekeeper workarounds, and a note on which
files belong to the updater. To change what every release page says, edit that
file.

The guide is added *after* `latest.json` is written, so it stays **out of the
in-app update prompt**. That's why it isn't in `CHANGELOG.md`.

Before publishing, open the draft and check:

- The guide is there, under the changelog section.
- The `install-guide` job logged no warnings. It warns if the guide links an
  asset name that isn't on the release (a toolchain change can rename one). If
  it did, fix the name in `.github/release-install.md` and correct the draft by
  hand.

The job skips a release whose body already contains `<!-- install-guide -->`,
so re-running it is safe. If a platform build failed, the job doesn't run, but
you won't publish that draft anyway (see below).

Constraints worth knowing:

- **Assets cannot be grouped, reordered, or renamed.** GitHub shows a flat
  alphabetical list, and `latest.json` references several assets by exact URL
  for the auto-updater — renaming anything breaks in-app updates. The download
  links in the guide are the fix.
- The `.sig` files, `.app.tar.gz`, and `latest.json` belong to the updater; the
  guide says so, so nobody wonders.

## 5. Publish

This is the step that ships: the moment it runs, every install is offered the
update. Only once all three platforms' assets are on the draft.

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
