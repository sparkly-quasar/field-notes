// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// `npm test`: where doses fall against bedtime (stats.ts), on the viewer's clock.
import { test } from "node:test";
import assert from "node:assert/strict";
import { bedMinutes, closeToBed, hourBeforeBed, minutesBeforeBed } from "./stats.ts";

const at = (h, m = 0) => new Date(2026, 8, 1, h, m).getTime();

test("a bedtime is a time of day, or nothing to count against", () => {
  assert.equal(bedMinutes("23:30"), 23 * 60 + 30);
  assert.equal(bedMinutes("0:15"), 15);
  assert.equal(bedMinutes("varies"), null);
  assert.equal(bedMinutes("skip"), null);
  assert.equal(bedMinutes(null), null);
});

test("doses are counted within a half-life before bed, across midnight", () => {
  const bed = bedMinutes("23:00");
  assert.equal(minutesBeforeBed(at(18), bed), 300);
  // Caffeine, half-life ~5 hours: 18:30 and 22:00 count, 9:00 doesn't.
  assert.equal(closeToBed([at(9), at(18, 30), at(22)], bed, 5), 2);
  // A 1am bedtime: 17:00 is 8 hours before it.
  assert.equal(closeToBed([at(17), at(16, 45)], bedMinutes("01:00"), 8.5), 2);
  assert.equal(closeToBed([at(16)], bedMinutes("01:00"), 8.5), 0);
  // Exactly one half-life before bed counts: half of it is still there.
  assert.equal(closeToBed([at(14)], bed, 9), 1);
});

test("the hours shaded are the ones leading up to bed", () => {
  const bed = bedMinutes("23:00");
  const shaded = [...Array(24).keys()].filter((h) => hourBeforeBed(h, bed, 5));
  assert.deepEqual(shaded, [18, 19, 20, 21, 22]);
  assert.deepEqual([...Array(24).keys()].filter((h) => hourBeforeBed(h, bedMinutes("01:00"), 3)), [0, 22, 23]);
});

test("a morning asks about the night before, and bedtime can fall after midnight", async () => {
  const { lastNight, bedOf } = await import("./stats.ts");
  assert.equal(lastNight(new Date(2026, 9, 6, 8, 0).getTime()), "2026-10-05");
  assert.equal(bedOf("2026-10-05", 23 * 60), new Date(2026, 9, 5, 23, 0).getTime());
  assert.equal(bedOf("2026-10-05", 60), new Date(2026, 9, 6, 1, 0).getTime(), "a 1am bedtime is the next morning");
});

test("sleep after a dose near bedtime, against other nights", async () => {
  const { sleepCompare } = await import("./stats.ts");
  const bed = 23 * 60;
  const nights = [
    { night: "2026-10-01", rating: 2 },
    { night: "2026-10-02", rating: 4 },
    { night: "2026-10-03", rating: 2 },
    { night: "2026-10-04", rating: 5 },
  ];
  // Caffeine (5 hours) at 8pm on the 1st and 3rd; at 9am on the 4th, long gone by bed.
  const doses = [
    { t: new Date(2026, 9, 1, 20, 0).getTime(), hours: 5 },
    { t: new Date(2026, 9, 3, 20, 0).getTime(), hours: 5 },
    { t: new Date(2026, 9, 4, 9, 0).getTime(), hours: 5 },
  ];
  assert.deepEqual(sleepCompare(nights, doses, bed), {
    near: { nights: 2, average: 2 },
    other: { nights: 2, average: 4.5 },
  });
});
