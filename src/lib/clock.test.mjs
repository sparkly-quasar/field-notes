// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// `npm test`: times show on the 12-hour clock unless Settings switches to 24-hour.
import { test } from "node:test";
import assert from "node:assert/strict";
import { clockHour, clockTime, readClock24From } from "./clock.ts";

const evening = new Date(2026, 9, 8, 21, 5);

test("12-hour by default, 24-hour when chosen", () => {
  assert.match(clockTime(evening), /^9:05\s?p\.?m\.?$/i);
  assert.match(clockHour(evening), /^9\s?p\.?m\.?$/i);
  readClock24From(() => true);
  assert.match(clockTime(evening), /^21:05$/);
  readClock24From(() => false);
  assert.match(clockTime(evening), /^9:05\s?p\.?m\.?$/i);
});
