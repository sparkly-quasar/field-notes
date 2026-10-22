// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// Render the iPhone launch images (static/splash/) and their <link> tags.
//
// A Home Screen app on iOS shows a launch image while it starts, white unless
// the page names one for that exact screen size. These are the loading splash
// from src/app.html, drawn at each iPhone's size in its light and dark colours,
// so the launch image hands over to the splash without a jump. Run it after
// changing the splash or to add a new iPhone, then paste the tags it prints into
// app.html. Needs Playwright (set CHROMIUM_PATH if its browser isn't installed).
//
// usage: node scripts/build-launch-images.mjs
import { chromium } from "playwright";
import { mkdirSync, readFileSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outDir = join(root, "static", "splash");

// Portrait screen sizes in CSS pixels, and the pixel ratio. One entry covers
// every iPhone sharing that screen.
const PHONES = [
  [440, 956, 3], // 16 Pro Max, 17 Pro Max
  [430, 932, 3], // 14 Pro Max, 15 Plus, 15 Pro Max, 16 Plus
  [428, 926, 3], // 12 Pro Max, 13 Pro Max, 14 Plus
  [420, 912, 3], // Air
  [414, 896, 3], // XS Max, 11 Pro Max
  [414, 896, 2], // XR, 11
  [414, 736, 3], // 6 Plus to 8 Plus
  [402, 874, 3], // 16 Pro, 17, 17 Pro
  [393, 852, 3], // 14 Pro, 15, 15 Pro, 16
  [390, 844, 3], // 12, 12 Pro, 13, 13 Pro, 14, 16e
  [375, 812, 3], // X, XS, 11 Pro, 12 mini, 13 mini
  [375, 667, 2], // 6 to 8, SE (2nd and 3rd)
];

// The splash exactly as app.html has it: its <style> and its markup.
const html = readFileSync(join(root, "src", "app.html"), "utf8");
const style = html.match(/<style>[\s\S]*?#splash[\s\S]*?<\/style>/)?.[0];
const markup = html.match(/<div id="splash"[\s\S]*?<\/p>\s*<\/div>/)?.[0];
if (!style || !markup) throw new Error("couldn't find the splash in src/app.html");
const fonts = readFileSync(join(root, "static", "fonts", "fonts.css"), "utf8").replaceAll(
  /url\(["']?([^"')]+)["']?\)/g,
  (_, f) => `url("data:font/woff2;base64,${readFileSync(join(root, "static", "fonts", f)).toString("base64")}")`,
);
const page = `<!doctype html><html class="phone"><head><style>${fonts}</style>${style}
<style>#splash .dots span { animation: none; }</style></head><body>${markup}</body></html>`;

rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM_PATH || undefined });
const tags = [];
for (const [w, h, r] of PHONES) {
  for (const scheme of ["light", "dark"]) {
    const name = `launch-${w * r}x${h * r}-${scheme}.png`;
    const ctx = await browser.newContext({ viewport: { width: w, height: h }, deviceScaleFactor: r, colorScheme: scheme });
    const p = await ctx.newPage();
    await p.setContent(page);
    await p.evaluate(() => document.fonts.ready);
    await p.screenshot({ path: join(outDir, name) });
    await ctx.close();
    tags.push(
      `<link rel="apple-touch-startup-image" href="%sveltekit.assets%/splash/${name}" media="(device-width: ${w}px) and (device-height: ${h}px) and (-webkit-device-pixel-ratio: ${r}) and (orientation: portrait) and (prefers-color-scheme: ${scheme})" />`,
    );
  }
}
await browser.close();
console.log(tags.map((t) => `    ${t}`).join("\n"));
