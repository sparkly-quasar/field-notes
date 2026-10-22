// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// The README's phone screenshots: /m at a phone viewport, as a paired phone sees
// it. See README.md in this folder.
// usage: node phone.mjs <token> <outDir> [baseUrl]
import { chromium } from "playwright";

const [token, out, base = "http://127.0.0.1:5173"] = process.argv.slice(2);
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
const ctx = await browser.newContext({
  viewport: { width: 390, height: 844 },
  deviceScaleFactor: 2,
  isMobile: true,
  hasTouch: true,
  colorScheme: "light",
  timezoneId: "America/New_York",
  locale: "en-US",
});
await ctx.addInitScript(() => {
  localStorage.setItem("fieldnotes.navSlot", "stats");
  localStorage.setItem("fieldnotes.arcsChart", "1");
  localStorage.setItem("fieldnotes.homeHintDismissed", "1");
});
const page = await ctx.newPage();
page.on("pageerror", (e) => console.log("pageerror", e.message));
const shot = async (name) => {
  await page.waitForTimeout(1200);
  await page.screenshot({ path: `${out}/${name}.png` });
  console.log("shot", name);
};
const nav = (label) => page.locator("nav").getByText(label, { exact: true }).first().click();
/** Scroll so the first chart with moment dots sits `gap` px below the top. */
const toArcs = (gap) =>
  page.evaluate((gap) => {
    const svg = document.querySelector("svg circle.moment")?.closest("svg");
    if (svg) window.scrollTo(0, svg.getBoundingClientRect().top + window.scrollY - gap);
  }, gap);

await page.goto(`${base}/m#t=${token}`);
await page.waitForTimeout(2500);
await toArcs(230);
await shot("phone-today");

await nav("Journal");
await shot("phone-journal");

await page.getByText("Equinox ceremony").first().click();
await shot("phone-entry");
await toArcs(120);
await shot("phone-entry-arcs");

await nav("Stats");
await page.getByRole("button", { name: "1 year", exact: true }).click();
await page.getByRole("button", { name: "Psilocybin Mushrooms", exact: true }).first().click();
await page.waitForTimeout(800);
await page
  .getByRole("heading", { name: /Dose over time/ })
  .first()
  .evaluate((el) => window.scrollTo(0, el.getBoundingClientRect().top + window.scrollY - 140));
// Open one dose's details, as a tap would.
await page.evaluate(() => [...document.querySelectorAll('svg circle[role="button"]')].at(-2)?.dispatchEvent(new MouseEvent("click", { bubbles: true })));
await shot("phone-stats-chart");

await nav("Check");
await page.fill("#combo", "Psilocybin mushrooms, lithium");
await page.press("#combo", "Enter");
await page.locator("#combo").blur();
await shot("phone-check");

await browser.close();
