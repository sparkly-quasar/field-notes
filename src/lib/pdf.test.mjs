// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// `npm test`: the experience PDF (pdf.ts) is a well-formed file with the parts
// asked for, and without the parts left out.
import { test } from "node:test";
import assert from "node:assert/strict";
import { experiencePdf, pdfFilename } from "./pdf.ts";

export const sample = {
  id: 7,
  kind: "session",
  title: "Autumn sit with Jules",
  intention: "Rest, listen, and let the grief move.",
  setting: "Home, music, Jules sitting.",
  notes: "It came on gently. The (first) hour was mostly music.\n\nLater, a long cry that felt like relief — and then laughing at the cat. µ-dosing? Not this time. 🌿",
  rating: 8,
  started_at: "2026-10-03T20:00:00Z",
  ended_at: "2026-10-04T05:30:00Z",
  created_at: "2026-10-03T20:00:00Z",
  doses: [
    { id: 1, experience_id: 7, substance_id: null, substance_name: "LSD", amount: 100, unit: "µg", route: "oral", taken_at: "2026-10-03T20:10:00Z", note: "" },
    { id: 2, experience_id: 7, substance_id: null, substance_name: "LSD", amount: 50, unit: "µg", route: "oral", taken_at: "2026-10-03T22:40:00Z", note: "small booster" },
  ],
  timeline: [
    { id: 3, experience_id: 7, at: "2026-10-03T21:00:00Z", note: "Colours breathing, a bit nauseous.", mood: "curious", intensity: 4 },
    { id: 4, experience_id: 7, at: "2026-10-04T04:30:00Z", note: "Coming down, tea.", mood: "", intensity: null },
  ],
};

const text = (bytes) => Buffer.from(bytes).toString("latin1");

test("it's a complete PDF file", () => {
  const s = text(experiencePdf(sample));
  assert.ok(s.startsWith("%PDF-1.4"));
  assert.ok(s.trimEnd().endsWith("%%EOF"));
  // The cross-reference offsets point at the objects they name.
  const xref = Number(s.match(/startxref\n(\d+)/)[1]);
  assert.ok(s.slice(xref).startsWith("xref"));
  const offsets = [...s.slice(xref).matchAll(/^(\d{10}) 00000 n $/gm)].map((m) => Number(m[1]));
  offsets.forEach((o, i) => assert.ok(s.slice(o).startsWith(`${i + 1} 0 obj`), `object ${i + 1}`));
});

test("parts left out stay out", () => {
  const all = text(experiencePdf(sample));
  const bare = text(experiencePdf(sample, { intention: false, moments: false, writeup: false, rating: false }));
  for (const s of ["grief", "Colours breathing", "long cry", "Rating"]) {
    assert.ok(all.includes(s), `${s} is in the full report`);
    assert.ok(!bare.includes(s), `${s} is left out`);
  }
  assert.ok(bare.includes("LSD"), "doses are always there");
});

test("the filename is dated and safe", () => {
  assert.match(pdfFilename({ ...sample, title: "a/b: c?" }), /^2026-10-0\d a b c\.pdf$/);
});
