// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// `npm test`: what the substance box suggests as you type (suggest.ts).
import { test } from "node:test";
import assert from "node:assert/strict";
import { suggest } from "./suggest.ts";

const ref = (name, aliases = []) => ({ name, aliases, mine: false });
const ALL = [ref("3-MeO-PCE"), ref("3-MeO-PCP", ["3-MeO"]), ref("3-MeO-PCPr"), ref("3-MeO-PCPy"), ref("LSD", ["Acid"])];
const names = (typed) => suggest(ALL, typed).map((s) => s.name);

test("a name typed in full stays listed, first, beside longer names that start with it", () => {
  assert.deepEqual(names("3-MeO-PCP"), ["3-MeO-PCP", "3-MeO-PCPr", "3-MeO-PCPy"]);
  assert.deepEqual(names("3-meo-pcp"), ["3-MeO-PCP", "3-MeO-PCPr", "3-MeO-PCPy"]);
});

test("a name typed in full stays listed when nothing else matches", () => {
  assert.deepEqual(names("3-MeO-PCPr"), ["3-MeO-PCPr"]);
  assert.deepEqual(names("lsd"), ["LSD"]);
});

test("partial names and street names still suggest", () => {
  assert.deepEqual(names("3-meo-pc"), ["3-MeO-PCE", "3-MeO-PCP", "3-MeO-PCPr", "3-MeO-PCPy"]);
  assert.deepEqual(suggest(ALL, "acid"), [{ name: "LSD", via: "Acid" }]);
});

test("hyphens and spaces don't matter", () => {
  assert.deepEqual(names("3meopcp"), ["3-MeO-PCP", "3-MeO-PCPr", "3-MeO-PCPy"]);
  assert.deepEqual(names("3 meo pcp"), ["3-MeO-PCP", "3-MeO-PCPr", "3-MeO-PCPy"]);
  assert.deepEqual(names("3meopcpy"), ["3-MeO-PCPy"], "shown so picking it fixes the spelling");
  assert.deepEqual(names("-"), []);
});
