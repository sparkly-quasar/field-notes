// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// `npm test`: active arcs (arcs.ts), on reference timings and tiers.
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  DRINKS_PER_HOUR,
  amountAt,
  buildSeries,
  drinksAt,
  drinksClearAt,
  heightFor,
  parseSpan,
  phaseOf,
  redoseOf,
  routeFor,
  tierOf,
  TIER_HEIGHT,
} from "./arcs.ts";

const H = 3_600_000;
const T0 = Date.UTC(2026, 9, 8, 21, 0);
const iso = (h) => new Date(T0 + h * H).toISOString();
const range = (min, max) => ({ min, max });

const MDMA = {
  name: "MDMA",
  common_names: [],
  psychoactive: ["Entactogen", "Stimulant", "Psychedelic (mild)"],
  chemical: [],
  interactions: [],
  roas: [
    {
      name: "oral", units: "mg", threshold: 30,
      light: range(40, 75), common: range(75, 125), strong: range(125, 175), heavy: 175,
      onset: "20–70 minutes", come_up: "30–60 minutes", peak: "2–3.5 hours", offset: "1–2 hours",
      after_effects: "2–24 hours", total: "3–6 hours", half_life: "7-10 hours",
    },
  ],
};
const ALCOHOL = {
  name: "Alcohol", common_names: [], psychoactive: [], chemical: [], interactions: [],
  roas: [
    {
      name: "Oral", units: "units", threshold: 1,
      light: range(1, 2), common: range(2, 4), strong: range(6, null), heavy: 5,
      onset: "15–30 minutes", come_up: null, peak: null, offset: null,
      after_effects: "6–48 hours", total: "1.5–3 hours", half_life: null,
    },
  ],
};
const NOTHING = { name: "4-HO-McPT", common_names: [], psychoactive: ["Psychedelic"], chemical: [], interactions: [], roas: [] };
const REF = { mdma: MDMA, alcohol: ALCOHOL, beer: ALCOHOL, "4-ho-mcpt": NOTHING };
const info = (n) => REF[n.trim().toLowerCase()] ?? null;

let id = 0;
const dose = (substance_name, amount, unit, h, route = "oral", extra = {}) => ({
  id: ++id, experience_id: 1, substance_id: null, substance_name, amount, unit, route, taken_at: iso(h), note: "", ...extra,
});

test("reference durations read as hours, whatever the unit", () => {
  assert.deepEqual(parseSpan("20–70 minutes"), [20 / 60, 70 / 60]);
  assert.deepEqual(parseSpan("2-3.5 hours"), [2, 3.5]);
  assert.deepEqual(parseSpan("3 hours"), [3, 3]);
  assert.deepEqual(parseSpan("10–60 seconds"), [10 / 3600, 60 / 3600]);
  assert.equal(parseSpan(""), null);
  assert.equal(parseSpan("varies"), null);
  assert.equal(parseSpan("2-4"), null, "no unit, no guess");
});

test("a route is matched by name, IM as IV, and never swapped for another", () => {
  const iv = { ...MDMA.roas[0], name: "Intravenous" };
  const inf = { ...MDMA, roas: [MDMA.roas[0], iv] };
  assert.equal(routeFor(inf, "Oral").name, "oral");
  assert.equal(routeFor(inf, "IM").name, "Intravenous");
  assert.equal(routeFor(inf, "").name, "oral", "no route logged: the first listed");
  assert.equal(routeFor(inf, "smoked"), null);
});

test("a dose on the bottom of a tier is that tier", () => {
  const roa = MDMA.roas[0];
  assert.equal(tierOf(roa, 75), "common");
  assert.equal(tierOf(roa, 74.9), "light");
  assert.equal(tierOf(roa, 175), "heavy");
  assert.equal(tierOf(roa, 10), "threshold");
});

test("every dose of a substance adds into one line, and a top-up lifts it a tier", () => {
  const [s] = buildSeries([dose("MDMA", 110, "mg", 0), dose("MDMA", 40, "mg", 2.25)], info);
  assert.equal(s.doses.length, 2);
  // Before the top-up is absorbed: just the first dose, between common and strong.
  const before = amountAt(s, T0 + 2.2 * H, T0 + 2.2 * H);
  assert.equal(before, 110);
  assert.ok(heightFor(s, before) > TIER_HEIGHT.common && heightFor(s, before) < TIER_HEIGHT.strong);
  // Both at their peak: 150 mg, the strong range.
  const both = amountAt(s, T0 + 3.75 * H, T0 + 3.75 * H);
  assert.equal(both, 150);
  assert.equal(tierOf(s.roa, both), "strong");
  // Nothing counts once every dose is past its slowest offset.
  assert.equal(amountAt(s, T0 + 12 * H, T0 + 12 * H), 0);
});

test("a dose not yet taken doesn't count", () => {
  const [s] = buildSeries([dose("MDMA", 110, "mg", 0), dose("MDMA", 40, "mg", 2.25)], info);
  assert.equal(amountAt(s, T0 + 3.75 * H, T0 + 2 * H), 110);
});

test("micrograms and milligrams compare against the reference's unit", () => {
  const [s] = buildSeries([dose("MDMA", 0.11, "g", 0)], info);
  assert.equal(s.doses[0].amount, 110);
});

test("an amount that can't be compared leaves the line untiered, never guessed", () => {
  const [s] = buildSeries([dose("MDMA", 110, "mg", 0), dose("MDMA", 1, "pill", 1)], info);
  assert.equal(s.tiers, null);
});

test("drinks of any name are one line, counted in drinks, never against DoseWiki's units", () => {
  const series = buildSeries([dose("Beer", 1, "drink", 0), dose("Alcohol", 2, "drink", 0.5)], info);
  assert.equal(series.length, 1);
  const [s] = series;
  assert.equal(s.kind, "drinks");
  assert.equal(s.tiers, null);
  assert.equal(s.name, "Alcohol");
});

test("drinks stack and clear at a steady rate", () => {
  assert.equal(DRINKS_PER_HOUR, 1);
  const [s] = buildSeries([dose("Alcohol", 3, "drink", 0)], info);
  // Absorbed over ~22 minutes while clearing: a little under 3 at the top.
  const top = drinksAt(s, T0 + 0.4 * H, T0 + 0.4 * H);
  assert.ok(top > 2.5 && top < 3, `top ${top}`);
  // An hour later, about one drink lower.
  const later = drinksAt(s, T0 + 1.4 * H, T0 + 1.4 * H);
  assert.ok(Math.abs(top - later - 1) < 0.05, `top ${top} later ${later}`);
  // Clear about three hours after the drink.
  const clear = drinksClearAt(s, T0 + 1 * H);
  assert.ok(Math.abs((clear - T0) / H - 3) < 0.05, `clear at ${(clear - T0) / H} h`);
  // A second round restarts the climb.
  const [two] = buildSeries([dose("Alcohol", 3, "drink", 0), dose("Alcohol", 2, "drink", 1)], info);
  assert.ok(drinksAt(two, T0 + 1.5 * H, T0 + 1.5 * H) > drinksAt(s, T0 + 1.5 * H, T0 + 1.5 * H) + 1.5);
});

test("how a redose behaves follows the substance's class", () => {
  assert.equal(redoseOf(MDMA, "MDMA"), "blunted");
  assert.equal(redoseOf(ALCOHOL, "Alcohol"), "additive");
  assert.equal(redoseOf(null, "GHB"), "additive");
  assert.equal(redoseOf(null, "1,4-B"), "additive");
  assert.equal(redoseOf({ ...NOTHING, psychoactive: ["Opioid", "Stimulant (low doses)"] }, "Kratom"), "additive");
  assert.equal(redoseOf({ ...NOTHING, psychoactive: ["Dissociative"] }, "Ketamine"), "additive");
  assert.equal(redoseOf({ ...NOTHING, psychoactive: ["Psychedelic"] }, "LSD"), "partial");
  const weed = { ...NOTHING, psychoactive: ["Cannabinoid", "Hallucinogen (mild)"] };
  assert.equal(redoseOf(weed, "Cannabis", { unit: "mg", route: "oral", form: "" }), "delayed");
  assert.equal(redoseOf(weed, "Cannabis", { unit: "hit", route: "smoked", form: "" }), "additive");
  assert.equal(redoseOf(null, "Mystery"), null);
});

test("phases name both ends when fast and slow timings disagree, and never count down", () => {
  const [s] = buildSeries([dose("MDMA", 110, "mg", 0)], info);
  assert.equal(phaseOf(s, T0 + 0.1 * H).label, "Not felt yet");
  // 1.5 h in: fast timing is peaking, slow is still coming up.
  assert.equal(phaseOf(s, T0 + 1.5 * H).label, "Coming up or peaking");
  assert.match(phaseOf(s, T0 + 1.5 * H).detail, /^The peak usually arrives \d{1,2}:\d\d\s?[ap]\.?m\.?–\d{1,2}:\d\d\s?[ap]\.?m\.?\.$/i);
  assert.equal(phaseOf(s, T0 + 30 * H).label, "After-effects or likely past", "after-effects run to 24 h past the slowest offset");
  assert.equal(phaseOf(s, T0 + 40 * H).label, "Likely past");
});

test("a redose on a blunted substance says the line runs ahead of the feeling", () => {
  const [s] = buildSeries([dose("MDMA", 110, "mg", 0), dose("MDMA", 40, "mg", 2.25)], info);
  assert.match(phaseOf(s, T0 + 3.5 * H).detail, /raises levels in your body more than it raises how it feels/);
});

test("a substance with no timings stays listed", () => {
  const [s] = buildSeries([dose("4-HO-McPT", 15, "mg", 0)], info);
  assert.equal(s.kind, "marker");
  assert.equal(phaseOf(s, T0 + 20 * H).label, "No duration data");
});

test("recent entries: live, or started or ended in the last two days, never notes", async () => {
  const { recentEntries } = await import("./arcs.ts");
  const now = T0;
  const e = (id, kind, startH, endH) => ({ id, kind, started_at: iso(startH), ended_at: endH == null ? null : iso(endH) });
  const list = [
    e(1, "session", -1, null), // live
    e(2, "session", -30, -20), // yesterday
    e(3, "session", -100, -60), // long gone
    e(4, "note", -1, -1), // a note
    e(5, "session", -60, -40), // started long ago, ended within two days
  ];
  assert.deepEqual(recentEntries(list, now).map((x) => x.id), [1, 2, 5]);
});

test("only felt effects count as active, not after-effects", async () => {
  const { isActive } = await import("./arcs.ts");
  const [s] = buildSeries([dose("MDMA", 110, "mg", 0)], info);
  assert.equal(isActive(s, T0 - 0.1 * H), false, "not taken yet");
  assert.equal(isActive(s, T0 + 3 * H), true);
  // Slowest felt end: 70 min + 1 h + 3.5 h + 2 h = 7.67 h. After that it's only after-effects.
  assert.equal(isActive(s, T0 + 7.5 * H), true);
  assert.equal(isActive(s, T0 + 8 * H), false);
  const [drinks] = buildSeries([dose("Alcohol", 2, "drink", 0)], info);
  assert.equal(isActive(drinks, T0 + 1 * H), true);
  assert.equal(isActive(drinks, T0 + 3 * H), false, "processed by now, even though after-effects run on");
  const [marker] = buildSeries([dose("4-HO-McPT", 15, "mg", 0)], info);
  assert.equal(isActive(marker, T0 + 7 * H), true);
  assert.equal(isActive(marker, T0 + 9 * H), false, "no timings: the combination check's 8 hours");
});

test("yesterday's doses leave nothing active, and today's window centres on now", async () => {
  const { isActive, nowWindow } = await import("./arcs.ts");
  const now = T0 + 30 * H;
  const series = buildSeries(
    [dose("MDMA", 110, "mg", 0), dose("Alcohol", 1, "drink", 2), dose("MDMA", 40, "mg", 28.5)],
    info,
  );
  const active = series.filter((s) => isActive(s, now));
  assert.deepEqual(active.map((s) => s.name), ["MDMA"], "alcohol from yesterday isn't active");
  const [start, end] = nowWindow(active, now);
  // Starts just before the dose still counting (28.5 h), not yesterday's (0 h).
  assert.equal(start, T0 + 28.5 * H - 15 * 60_000);
  assert.ok(end > now && end <= T0 + 28.5 * H + 8.2 * H, `end ${(end - T0) / H} h`);
  assert.deepEqual(nowWindow([], now), [now - H, now + H]);
});

test("today's row counts only the doses still active, not yesterday's", async () => {
  const { amountsText, tierText } = await import("./arcs.ts");
  const now = T0 + 30 * H;
  const [s] = buildSeries([dose("MDMA", 110, "mg", 0), dose("MDMA", 40, "mg", 28.5)], info);
  assert.equal(amountsText(s, now, true), "40 mg");
  assert.equal(tierText(s, now, true), "light range");
  assert.equal(amountsText(s, now), "110 + 40 mg", "a finished trip report still lists every dose");
  assert.doesNotMatch(phaseOf(s, now).detail, /second dose/);
});

test("heightFor curves smoothly through the tier anchors", () => {
  const s = { kind: "shape", tiers: [[0, 0], [20, 0.18], [50, 0.4], [100, 0.62], [150, 0.82], [300, 1]] };
  for (const [a, h] of s.tiers) assert.ok(Math.abs(heightFor(s, a) - h) < 1e-9);
  let prev = -1, prevSlope = null;
  for (let a = 0; a <= 400; a += 0.5) {
    const h = heightFor(s, a);
    assert.ok(h >= prev - 1e-9, `falls at ${a}`);
    const slope = h - prev;
    if (prevSlope !== null && a > 1) assert.ok(Math.abs(slope - prevSlope) < 1e-3, `kinks at ${a}`);
    prevSlope = slope;
    prev = h;
  }
});

test("a block's height counts only the doses still felt, not yesterday's", () => {
  // Kratom's reference gives only onset and a total, so it's drawn as a block.
  const KRATOM = {
    name: "Kratom", common_names: [], psychoactive: ["Opioid"], chemical: [], interactions: [],
    roas: [
      {
        name: "oral", units: "g", threshold: 0.5,
        light: range(0.5, 1.5), common: range(1.5, 3), strong: range(3, 6), heavy: 6,
        onset: "10–40 minutes", come_up: null, peak: null, offset: null,
        after_effects: "1–12 hours", total: "2–5 hours", half_life: null,
      },
    ],
  };
  const now = T0 + 24.5 * H;
  const [s] = buildSeries([dose("Kratom", 3, "g", 0), dose("Kratom", 3, "g", 24)], () => KRATOM);
  assert.equal(s.kind, "block");
  assert.equal(amountAt(s, now, now), 3, "yesterday's 3 g is long past");
  assert.equal(heightFor(s, amountAt(s, now, now)), TIER_HEIGHT.strong);
  assert.equal(tierOf(s.roa, 3), "strong");
});
