// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// Seed the README screenshots' fictional demo journal through the portal API, on
// top of dev_portal's own entries. Times are relative to now, so the live trip
// report is always mid-session. See README.md in this folder.
// usage: node seed.mjs <portalPort> <token>
const [port, token] = process.argv.slice(2);
const call = async (cmd, args) => {
  const r = await fetch(`http://127.0.0.1:${port}/api/${cmd}`, {
    method: "POST",
    headers: { "Content-Type": "application/json", Authorization: `Bearer ${token}` },
    body: JSON.stringify(args),
  });
  const t = await r.text();
  if (!r.ok) throw new Error(`${cmd} ${r.status}: ${t}`);
  return t ? JSON.parse(t) : null;
};
const now = Date.now();
const ago = (h, m = 0) => new Date(now - (h * 60 + m) * 60_000).toISOString();
const daysAgo = (d, hour, min = 0) => {
  const t = new Date(now - d * 86_400_000);
  t.setUTCHours(hour, min, 0, 0);
  return t.toISOString();
};
const plus = (iso, h, m = 0) => new Date(Date.parse(iso) + (h * 60 + m) * 60_000).toISOString();

async function entry({ title, start, end, intention = "", setting = "", notes = "", rating = null, doses = [], moments = [], kind = "session" }) {
  const e = await call("create_experience", { input: { kind, title, intention, setting, started_at: start } });
  for (const d of doses) await call("log_dose", { input: { experience_id: e.id, route: "oral", unit: "g", ...d } });
  for (const m of moments) await call("add_timeline_event", { input: { experience_id: e.id, intensity: null, ...m } });
  if (end || notes || rating != null) {
    await call("update_experience", { id: e.id, update: { title, intention, setting, notes, rating, started_at: start, ended_at: end ?? null } });
  }
  return e.id;
}

// dev_portal's oral cannabis from yesterday would share a line with the live
// entry's vaporized cannabis, and a line takes its first dose's route.
for (const e of await call("list_experiences", {})) {
  if (e.title === "Cannabis" && e.substances.join() === "Cannabis") await call("delete_experience", { id: e.id });
}

// Past entries, so the journal and stats have a history.
for (const [d, g, title, rating] of [
  [104, 1.0, "First time with mushrooms", 7],
  [78, 2.0, "Rainy afternoon, headphones", 6],
]) {
  const s = daysAgo(d, 15);
  await entry({ title, start: s, end: plus(s, 6), rating, notes: "Took it slow. Wrote a page afterwards.", doses: [{ substance_name: "Psilocybin mushrooms", amount: g, taken_at: s }] });
}
{
  const s = daysAgo(55, 14);
  await entry({
    title: "Equinox ceremony",
    start: s,
    end: plus(s, 7),
    rating: 9,
    intention: "Grief, and letting it move.",
    setting: "Friend's land, small circle, sitter present.",
    notes: "Hard first hour. Cried a lot, then a long stretch of feeling held.\n\nWhat stayed: I don't have to carry it alone.",
    doses: [{ substance_name: "Psilocybin mushrooms", amount: 2.5, taken_at: s }],
    moments: [
      { at: plus(s, 0, 50), mood: "heavy", note: "Grief arriving all at once", intensity: 7 },
      { at: plus(s, 2, 10), mood: "held", note: "Hand on my back. Breathing together", intensity: 8 },
      { at: plus(s, 4, 30), mood: "open", note: "Sun through the trees", intensity: 5 },
    ],
  });
}
{
  const s = daysAgo(26, 9);
  await entry({ title: "Low dose, writing day", start: s, end: plus(s, 8), rating: 6, notes: "Focused morning, scattered afternoon.", doses: [{ substance_name: "LSD", amount: 15, unit: "µg", taken_at: s }] });
}
{
  const s = daysAgo(12, 13);
  await entry({
    title: "Mushroom walk in the woods",
    start: s,
    end: plus(s, 6),
    rating: 8,
    intention: "Be outside. Notice things.",
    setting: "State park trail, with Robin.",
    notes: "Gentle and bright. Lay in the leaves for a long time.",
    doses: [{ substance_name: "Psilocybin mushrooms", amount: 1.5, taken_at: s }],
    moments: [
      { at: plus(s, 0, 45), mood: "giggly", note: "Everything is funny", intensity: 4 },
      { at: plus(s, 1, 40), mood: "awed", note: "The moss is breathing", intensity: 6 },
      { at: plus(s, 3, 20), mood: "calm", note: "Walking back slowly", intensity: 3 },
    ],
  });
}
await entry({ kind: "note", title: "Integration: what Robin said", start: daysAgo(10, 20), notes: "That I get quieter when I'm scared, not louder. Worth noticing at work too." });

// The live trip report: two lines on the chart, moments along the way.
const live = ago(2, 20);
await entry({
  title: "Afternoon sit with Robin",
  start: live,
  intention: "Gentle and curious. Stay with whatever comes.",
  setting: "Living room, then the garden. Robin sitting.",
  doses: [
    { substance_name: "Psilocybin mushrooms", amount: 1.8, taken_at: live },
    { substance_name: "Cannabis", amount: 1.5, unit: "mg", route: "vaporized", taken_at: ago(0, 35) },
  ],
  moments: [
    { at: ago(1, 55), mood: "uneasy", note: "Coming up, a bit restless", intensity: 3 },
    { at: ago(1, 30), mood: "curious", note: "Colours breathing on the curtains", intensity: 5 },
    { at: ago(1, 5), mood: "joyful", note: "Laughing with Robin about nothing", intensity: 7 },
    { at: ago(0, 30), mood: "calm", note: "Lying in the grass", intensity: 6 },
  ],
});
console.log("seeded");
