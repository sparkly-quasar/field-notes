// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// `npm test`: tagging someone in a dose from Edit dose (tagging.ts).
import { test } from "node:test";
import assert from "node:assert/strict";
import { sendOnSave, tagState } from "./tagging.ts";

const SAM = 2;
const KIT = 3;
const sent = (to, dose_id, status) => ({ to, dose_id, status });

test("a dose nobody was tagged in goes only to who was picked", () => {
  assert.deepEqual(sendOnSave([], 7, [SAM, KIT], []), []);
  assert.deepEqual(sendOnSave([], 7, [SAM, KIT], [KIT]), [KIT]);
});

test("saving an edit refreshes a tag still waiting, and never resends one added", () => {
  const out = [sent(SAM, 7, "waiting"), sent(KIT, 7, "accepted")];
  assert.deepEqual(sendOnSave(out, 7, [SAM, KIT], []), [SAM]);
  assert.deepEqual(sendOnSave(out, 7, [SAM, KIT], [SAM, KIT]), [SAM]);
});

test("someone who declined can be tagged again, but only if picked", () => {
  const out = [sent(SAM, 7, "declined")];
  assert.deepEqual(sendOnSave(out, 7, [SAM], []), []);
  assert.deepEqual(sendOnSave(out, 7, [SAM], [SAM]), [SAM]);
});

test("the newest tag of a dose decides, and other doses don't count", () => {
  const out = [sent(SAM, 7, "waiting"), sent(SAM, 7, "declined"), sent(KIT, 8, "accepted")];
  assert.equal(tagState(out, 7, SAM), "waiting");
  assert.equal(tagState(out, 7, KIT), "none");
  assert.deepEqual(sendOnSave(out, 7, [SAM, KIT], [KIT]), [SAM, KIT]);
});

test("only people who can be tagged now are sent anything", () => {
  assert.deepEqual(sendOnSave([sent(KIT, 7, "waiting")], 7, [SAM], [SAM, KIT]), [SAM]);
});
