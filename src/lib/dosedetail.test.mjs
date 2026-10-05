// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// `npm test`: what a dose works out to (dosedetail.ts), by the same rules as
// Stats (`measure` in stats.rs).
import { test } from "node:test";
import assert from "node:assert/strict";
import { describeAmount, formsFor, inUnit, measure } from "./dosedetail.ts";

const M = "Psilocybin Mushrooms";

test("fresh mushrooms are about a tenth of their weight dried", () => {
  assert.deepEqual(measure(M, 3, "g", { form: "fresh" }), { amount: 0.3, unit: "g", approx: true });
  assert.deepEqual(measure(M, 2500, "mg", { form: "fresh" }), { amount: 0.25, unit: "g", approx: true });
  assert.equal(describeAmount(M, 3, "g", { form: "fresh" }), "3 g fresh (about 0.3 g dried)");
  // Dried (or not said) is just the weight.
  assert.deepEqual(measure(M, 1, "g", {}), { amount: 1, unit: "g", approx: false });
  assert.equal(describeAmount(M, 1, "g", { form: "dried" }), "1 g");
});

test("fresh truffles aren't converted, so they aren't compared", () => {
  assert.equal(measure("Magic truffles", 10, "g", { form: "fresh" }), null);
});

test("capsules with an amount each are that weight in all", () => {
  const caps = { per_unit: 0.45, per_unit_unit: "g", unit_label: "00 caps" };
  assert.deepEqual(measure(M, 2, "capsule", caps), { amount: 0.9, unit: "g", approx: false });
  assert.equal(describeAmount(M, 2, "capsule", caps), "2 × 00 caps, 0.45 g each (0.9 g)");
  assert.equal(
    describeAmount(M, 2, "capsule", { ...caps, form: "powdered" }),
    "2 × 00 caps, 0.45 g each, powdered (0.9 g)",
  );
  // Not sure how much is in each: nothing to compare.
  assert.equal(measure(M, 2, "capsule", {}), null);
  assert.equal(describeAmount(M, 2, "capsule", {}), "2 capsule");
});

test("an edible is its estimate, never converted between grams and psilocybin", () => {
  const g = { form: "edible", estimate: 1.5, estimate_unit: "g" };
  assert.deepEqual(measure(M, 2, "piece", g), { amount: 1.5, unit: "g", approx: true });
  assert.equal(describeAmount(M, 2, "piece", g), "2 piece edible (about 1.5 g dried mushroom)");
  const p = { form: "edible", estimate: 10, estimate_unit: "mg psilocybin" };
  assert.equal(measure(M, 2, "piece", p), null);
  assert.equal(describeAmount(M, 2, "piece", p), "2 piece edible (about 10 mg psilocybin)");
  assert.equal(measure(M, 20, "g", { form: "edible" }), null, "a weight of chocolate says nothing");
});

test("kratom extract and 7-OH are never read against leaf's ranges", () => {
  assert.deepEqual(measure("Kratom", 3, "g", { form: "leaf" }), { amount: 3, unit: "g", approx: false });
  assert.equal(measure("Kratom", 1, "g", { form: "extract" }), null);
  assert.equal(measure("Kratom", 15, "mg", { form: "7-oh" }), null);
  assert.equal(describeAmount("Kratom", 15, "mg", { form: "7-oh" }), "15 mg 7-OH product");
});

test("only mushrooms, truffles and kratom have forms", () => {
  assert.equal(formsFor("shrooms").length, 4);
  assert.equal(formsFor("Kratom").length, 3);
  assert.equal(formsFor("LSD").length, 0);
});

test("units of mass convert; nothing else does", () => {
  assert.equal(inUnit(600, "mg", "g"), 0.6);
  assert.equal(inUnit(1, "tab", "µg"), null);
  assert.equal(inUnit(1, "ml", "g"), null);
});
