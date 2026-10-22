<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
# README screenshots

How the images in `docs/screenshots/` are made: the real app, a fictional demo
journal, headless Chromium. Nothing here ships.

- **`seed.mjs`** writes the demo journal through the portal API: a few months of
  past entries, and a live trip report started 2h20m ago with two doses and
  moments, so the charts are mid-session whenever you run it.
- **`serve.mjs`** serves the built frontend and forwards `/api` to the dev
  portal, which serves no frontend of its own under test.
- **`phone.mjs`** drives `/m` at a phone viewport, as a paired phone.
- **`desktop.mjs`** drives the desktop page, with a stand-in for Tauri's IPC that
  sends each command to the portal. A few desktop-only commands are answered in
  the script instead (the data folder shows `/Users/demo/…`). The Companion chat
  needs Ollama, so `companion-chat.png` isn't made here.

## Run

```bash
npm run build                                  # the frontend to serve
cd src-tauri && cargo test --lib portal::tests::dev_portal -- --ignored --nocapture
# prints DEV_PORTAL port=<port> token=<token>; it serves for an hour

# in another terminal, from scripts/screenshots/ (Playwright on the path)
node seed.mjs <port> <token>                   # once per dev_portal run
node serve.mjs ../../build <port> 5173 &
node phone.mjs <token> ../../docs/screenshots
node desktop.mjs <token> ../../docs/screenshots
```

Set `CHROMIUM_PATH` if Playwright's own browser isn't installed. Times show in
America/New_York; the phone is in light mode, and the desktop as it ships.
