// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// Build the phone's offline checker: `field_notes_core` compiled to WebAssembly
// (src-tauri/wasm), copied to static/offline/ so the frontend build ships it and
// the portal serves it to phones. Runs before every frontend build.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const manifest = join(root, "src-tauri", "Cargo.toml");
try {
  execFileSync(
    "cargo",
    ["build", "--manifest-path", manifest, "-p", "field_notes_wasm", "--release", "--target", "wasm32-unknown-unknown"],
    { stdio: "inherit" },
  );
} catch {
  console.error(
    "\nCouldn't build the phone's offline checker. If the error above mentions the target, run:\n" +
      "  rustup target add wasm32-unknown-unknown\n",
  );
  process.exit(1);
}
const out = join(root, "static", "offline");
mkdirSync(out, { recursive: true });
copyFileSync(
  join(root, "src-tauri", "target", "wasm32-unknown-unknown", "release", "field_notes_wasm.wasm"),
  join(out, "field-notes.wasm"),
);
