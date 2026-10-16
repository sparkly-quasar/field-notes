// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// `npm test`: which drug families experiences fell in, and which way each moved (stats.ts).
import { test } from "node:test";
import assert from "node:assert/strict";
import { familyMix, rhythm, rhythmWord, splitP } from "./stats.ts";

const MID = 100;
const exp = (t, ...families) => ({ t, families });
const many = (n, t, ...families) => Array.from({ length: n }, () => exp(t, ...families));
const row = (rows, f) => rows.find((r) => r.family === f);

test("families are counted per period and sorted by the latest", () => {
  const rows = familyMix([...many(1, 10, "stimulants"), ...many(9, 150, "stimulants"), ...many(8, 10, "dissociatives"), exp(150, "dissociatives")], MID);
  assert.deepEqual(rows.map((r) => r.family), ["stimulants", "dissociatives"]);
  assert.deepEqual(row(rows, "stimulants"), { family: "stimulants", before: 1, lately: 9, trend: "up" });
  assert.deepEqual(row(rows, "dissociatives"), { family: "dissociatives", before: 8, lately: 1, trend: "down" });
});

test("an experience counts once per family, and toward each family it has", () => {
  const rows = familyMix([exp(150, "entactogens", "stimulants", "stimulants")], MID, 1);
  assert.equal(row(rows, "stimulants").lately, 1);
  assert.equal(row(rows, "entactogens").lately, 1);
});

test("small or similar counts aren't called a change", () => {
  assert.equal(row(familyMix([exp(10, "opioids"), exp(150, "opioids"), exp(151, "opioids")], MID), "opioids").trend, "few");
  assert.equal(row(familyMix([...many(4, 10, "cannabinoids"), ...many(5, 150, "cannabinoids")], MID), "cannabinoids").trend, "same");
  assert.equal(row(familyMix([...many(3, 10, "depressants"), ...many(2, 150, "depressants")], MID), "depressants").trend, "few");
  assert.equal(row(familyMix([...many(5, 10, "dissociatives"), exp(150, "dissociatives")], MID), "dissociatives").trend, "few");
  assert.equal(row(familyMix(many(3, 150, "psychedelics"), MID), "psychedelics").trend, "few");
  assert.equal(row(familyMix(many(5, 150, "psychedelics"), MID), "psychedelics").trend, "new");
  assert.equal(row(familyMix(many(5, 10, "depressants"), MID), "depressants").trend, "gone");
});

test("ties fall back to display order", () => {
  const rows = familyMix([exp(150, "stimulants"), exp(150, "psychedelics")], MID);
  assert.deepEqual(rows.map((r) => r.family), ["psychedelics", "stimulants"]);
});

test("an uneven split of two counts is weighed by how many there are", () => {
  assert.equal(splitP(0, 0), 1);
  assert.equal(splitP(2, 3), 1);
  assert.ok(Math.abs(splitP(9, 1) - 22 / 1024) < 1e-9);
  assert.ok(Math.abs(splitP(1, 5) - 14 / 64) < 1e-9);
});

const D = 86_400_000;
const NOW = Date.UTC(2026, 9, 16);
const ago = (...days) => days.map((d) => NOW - d * D);

test("once a month over months reads as a steady rhythm", () => {
  const r = rhythm(ago(5, 33, 66, 95, 128, 160), NOW);
  assert.ok(r && r.times === 6);
  assert.equal(rhythmWord(r.gapDays), "about once a month");
  assert.equal(r.since, NOW - 160 * D);
});

test("a weekend counts as one occasion", () => {
  const r = rhythm(ago(5, 6, 33, 34, 66, 95), NOW);
  assert.ok(r && r.times === 4);
});

test("not a rhythm: too few, too irregular, too frequent, or stopped", () => {
  assert.equal(rhythm(ago(5, 35, 65), NOW), null, "three times");
  assert.equal(rhythm(ago(2, 5, 9, 110, 200), NOW), null, "uneven");
  assert.equal(rhythm(ago(3, 6, 9, 12, 15), NOW), null, "every few days isn't a slow rhythm");
  assert.equal(rhythm(ago(100, 130, 160, 190), NOW), null, "stopped months ago");
});
