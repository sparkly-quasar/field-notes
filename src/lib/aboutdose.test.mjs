// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// `npm test`: the warnings shown on saving a dose are about that dose (aboutdose.ts).
import { test } from "node:test";
import assert from "node:assert/strict";
import { aboutDose } from "./aboutdose.ts";

const w = (a, b, severity = "caution") => ({ a, b, severity, message: "" });

test("pairs from earlier in the day don't come back under a new dose", () => {
  const all = [
    w("1,4-Butanediol", "Rilmazafone"),
    w("Caffeine", "Dextroamphetamine"),
    w("Caffeine", "LSD", "note"),
    w("1,4-Butanediol", "Dextroamphetamine", "note"),
    w("Kratom", "Caffeine", "note"),
  ];
  assert.deepEqual(aboutDose(all, "Rilmazafone"), [all[0]]);
});

test("either side of the pair, any case or spacing", () => {
  const all = [w("rilmazafone ", "GHB"), w("Alcohol", "Rilmazafone")];
  assert.equal(aboutDose(all, " Rilmazafone").length, 2);
});

test("a note about the dose's own amount is kept; another's is not", () => {
  const all = [w("Rilmazafone", ""), w("LSD", "")];
  assert.deepEqual(aboutDose(all, "Rilmazafone"), [all[0]]);
});

test("no substance named: nothing is hidden", () => {
  const all = [w("Caffeine", "LSD")];
  assert.deepEqual(aboutDose(all, "  "), all);
});
