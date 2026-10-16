# Erowid source record

The dose figures marked "from Erowid" in `src-tauri/core/src/pw.rs` (`ROUTE_PATCHES`,
`ROUTE_OVERRIDES`, `EXTRA_SUBSTANCES`) were read off saved copies of Erowid's pages.
`manifest.json` lists each page: its URL, when it was saved, and a sha256 fingerprint
of the saved copy.

The copies themselves are **not** kept here. Erowid's pages say "Permission required
before publicly reproducing Erowid info", and this repository is public. Keep saved
copies privately and record them in the manifest. `.gitignore` keeps any copy dropped
into this folder out of commits.
