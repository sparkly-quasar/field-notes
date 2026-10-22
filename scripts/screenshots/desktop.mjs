// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// The README's desktop screenshots: the desktop page in Chromium, with a stand-in
// for Tauri's IPC that sends each command to the dev portal over HTTP. Commands
// the portal won't serve (desktop-only ones) fail as they would for a phone, so
// a few are answered here instead. See README.md in this folder.
// usage: node desktop.mjs <token> <outDir> [baseUrl]
import { chromium } from "playwright";

const [token, out, base = "http://127.0.0.1:5173"] = process.argv.slice(2);
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
const ctx = await browser.newContext({
  viewport: { width: 1200, height: 750 },
  deviceScaleFactor: 2,
  colorScheme: "light",
  timezoneId: "America/New_York",
  locale: "en-US",
});
await ctx.addInitScript((token) => {
  for (const k of ["hideDisclaimer", "companionChoiceMade", "firstStepsDismissed"]) localStorage.setItem(`fieldnotes.${k}`, "1");
  if (localStorage.getItem("fieldnotes.arcsChart") == null) localStorage.setItem("fieldnotes.arcsChart", "1");
  const stubs = { set_companion_enabled: null, data_dir: "/Users/demo/Library/Application Support/Field Notes" };
  let cb = 0;
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main", windowLabel: "main" } },
    transformCallback: (f) => {
      const id = ++cb;
      window[`_${id}`] = f;
      return id;
    },
    convertFileSrc: (p) => p,
    invoke: async (cmd, args = {}) => {
      if (cmd in stubs) return structuredClone(stubs[cmd]);
      if (cmd.startsWith("plugin:")) return null;
      const r = await fetch(`/api/${cmd}`, {
        method: "POST",
        headers: { "Content-Type": "application/json", Authorization: `Bearer ${token}` },
        body: JSON.stringify(args),
      });
      const t = await r.text();
      if (!r.ok) throw t || `${cmd} failed`;
      return t ? JSON.parse(t) : null;
    },
  };
}, token);
const page = await ctx.newPage();
const shot = async (name) => {
  await page.waitForTimeout(1200);
  await page.screenshot({ path: `${out}/${name}.png` });
  console.log("shot", name);
};
const home = async () => {
  await page.goto(`${base}/`);
  await page.waitForTimeout(2000);
};
const side = (label) => page.locator("aside, nav").getByText(label, { exact: true }).first().click();
/** Scroll the heading with this text to near the top of whatever scrolls it. */
const toHeading = (text) =>
  page.getByRole("heading", { name: text, exact: true }).first().evaluate((el) => {
    el.scrollIntoView({ block: "start" });
    for (let p = el.parentElement; p; p = p.parentElement) if (p.scrollTop > 0) return void (p.scrollTop -= 40);
    window.scrollBy(0, -40);
  });

// The journal, with the chart folded away so the entries show.
await page.goto(`${base}/`);
await page.evaluate(() => localStorage.setItem("fieldnotes.arcsChart", "0"));
await home();
await shot("journal-home");
await page.evaluate(() => localStorage.setItem("fieldnotes.arcsChart", "1"));

await home();
await page.getByText("Afternoon sit with Robin").first().click();
await page.getByRole("button", { name: "Live trip report" }).click();
await shot("live-session");

await home();
await page.getByText("Equinox ceremony").first().click();
await page.waitForTimeout(800);
await page.evaluate(() => document.querySelectorAll("*").forEach((el) => el.scrollTop > 0 && (el.scrollTop = 0)));
await shot("session-detail");

await side("Stats");
await page.getByRole("button", { name: "1 year", exact: true }).click();
await shot("stats");
await toHeading("Every dose, by substance");
await shot("substance-log");
await page.getByRole("button", { name: "Psilocybin Mushrooms", exact: true }).first().click();
await page.waitForTimeout(600);
await toHeading("Dose over time · Psilocybin Mushrooms");
await shot("stats-dose-chart");

await side("Check");
await page.fill("#combo-names", "Psilocybin mushrooms, lithium");
await page.press("#combo-names", "Enter");
await page.locator("#combo-names").blur();
await shot("substances-reference");
await toHeading("Search the reference");
const q = page.getByPlaceholder(/ketamine tolerance/);
await q.fill("psilocybin tolerance");
await q.press("Enter");
await q.blur();
await shot("reference-search");

await home();
await page.getByText("Get help", { exact: true }).first().click();
await shot("emergency-help");

await home();
await side("Settings");
await toHeading("Updates");
await shot("settings-updates");
await page.getByText("Devices & sync", { exact: true }).first().click();
await shot("settings-phone-access");
await page.getByText("Data & privacy", { exact: true }).first().click();
await shot("settings-encryption-backup");
await toHeading("Obsidian vault sync");
await shot("settings-obsidian-sync");
await toHeading("Erase & uninstall");
await shot("settings-data-location");

await browser.close();
