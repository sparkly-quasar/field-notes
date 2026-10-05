// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
/**
 * The offhand path: "I took this, at about this time."
 *
 * Most of what anyone wants to record is not a trip they sit through and write
 * up — it's a dose, and a rough time, and nothing else. Going through a session
 * (start it, log into it, remember to end it) to record that is enough friction
 * that it doesn't get recorded at all, and an unrecorded dose is one the
 * interaction checker can never see.
 *
 * So: one call that leaves a complete, correctly-timed entry in the journal.
 * It's a **session that is already ended**, which means it reads as history
 * rather than something you forgot to close, and — the point of doing it this
 * way rather than inventing a fourth kind of row — everything else in the app
 * already understands it. It can be opened, added to, written up, rated,
 * exported to Obsidian, and read by the Companion, all with no special cases.
 *
 * Both the desktop and the phone call this. The rule about warnings below is
 * why it lives here rather than being written twice.
 */

import {
  addTimelineEvent,
  checkCombo,
  createExperience,
  endExperience,
  getExperience,
  listExperiences,
  logDose,
  updateExperience,
  type ExperienceSummary,
  type TimedDose,
  type Warning,
} from "./api";
import { deferStretch, NeedsComputerError } from "./offline";

export interface QuickLogInput {
  substance: string;
  amount: number | null;
  unit: string;
  route: string;
  /** ISO 8601, UTC. When it was taken — often not now. */
  at: string;
  /**
   * Add to an entry that already exists instead of starting a new one: the
   * second thing taken on the same night belongs with the first, both because
   * that's how it reads back and because `log_dose` only checks a dose against
   * the rest of *its own* entry.
   */
  intoId?: number | null;
}

export interface QuickLogResult {
  /** The entry the dose landed in — new, or the one it was added to. */
  id: number;
  /** Its title, which for a fresh entry the backend has just named after the substance. */
  title: string;
  warnings: Warning[];
  /** The dose just logged, so it can be undone (null for a pasted log). */
  doseId: number | null;
  /** True when this dose made a new entry (undoing it removes the entry too). */
  fresh: boolean;
}

/**
 * How far either side of a dose we look for something else to check it against.
 * Long enough to catch an evening; short enough that last week's entry doesn't
 * produce a warning about a combination that never happened.
 */
export const NEARBY_HOURS = 12;

export async function quickLog(input: QuickLogInput): Promise<QuickLogResult> {
  const substance = input.substance.trim();
  if (!substance) throw new Error("Nothing to log — name the substance.");

  const fresh = input.intoId == null;
  // A blank title on purpose: `name_after_first_dose` (db.rs) names the entry
  // after the substance, which is the only name an entry like this wants.
  const id = fresh
    ? (await createExperience({ title: "", started_at: input.at })).id
    : input.intoId!;

  const logged = await logDose({
    experience_id: id,
    substance_name: substance,
    amount: input.amount,
    unit: input.unit,
    route: input.route,
    taken_at: input.at,
  });

  if (fresh) {
    await endExperience(id, input.at, null, "");
  } else {
    await stretchToCover(id, input.at);
  }

  const entry = await getExperience(id);
  return {
    id,
    title: entry.title,
    // The dose is saved whatever happens here; if the wider check can't run, the
    // entry's own check still stands.
    warnings: await allWarnings(substance, input.at, logged.warnings, id, input.route).catch(() => logged.warnings),
    doseId: logged.dose.id,
    fresh,
  };
}

/**
 * Keep an entry's span honest when a dose is added outside it. Without this the
 * journal shows an entry that ended an hour before something in it was taken,
 * and every t+ offset in that entry is measured from the wrong moment.
 */
export async function stretchToCover(id: number, at: string) {
  const e = await getExperience(id);
  const before = new Date(at) < new Date(e.started_at);
  const after = e.ended_at != null && new Date(at) > new Date(e.ended_at);
  if (!before && !after) return;
  const update = {
    title: e.title,
    intention: e.intention,
    setting: e.setting,
    notes: e.notes,
    rating: e.rating,
    started_at: before ? at : e.started_at,
    ended_at: after ? at : e.ended_at,
  };
  try {
    await updateExperience(id, update);
  } catch (err) {
    // A phone without its computer can't edit, but this edit is safe to make
    // later: it's worked out again against the computer's copy when it's sent.
    if (err instanceof NeedsComputerError) deferStretch(id, at);
    else throw err;
  }
}

/**
 * `log_dose` compares a dose against the rest of **its own entry**, which for a
 * quick log is nothing at all — so on its own it would go quiet on exactly the
 * combination that matters. Widen the question to "what else was in you around
 * then" and run the same timed checker over every dose in the entries nearby, at
 * the times they were actually taken: pairs that never overlapped drop out, and
 * pairs that only met once one was past its peak are softened.
 *
 * An entry whose doses can't be read (on a phone, one that isn't saved there)
 * counts as taken when the entry started. That can flag a pair that was hours
 * apart, which is the right way to be wrong: a warning you can dismiss beats
 * silence you can't.
 */
export async function allWarnings(
  substance: string,
  at: string,
  own: Warning[],
  entryId?: number,
  route = "",
): Promise<Warning[]> {
  const t = new Date(at).getTime();
  const minutes = (iso: string) => (Number.isFinite(Date.parse(iso)) ? Date.parse(iso) / 60_000 : null);
  // The entry's own check (`own`) already covers it, and knows when each dose was
  // taken; checking its doses again here would bring back pairs that never met.
  const nearby = (await listExperiences()).filter(
    (e) => e.id !== entryId && Math.abs(new Date(e.started_at).getTime() - t) < NEARBY_HOURS * 3600_000,
  );
  const doses: TimedDose[] = [{ substance_name: substance, route, at_min: minutes(at) }];
  for (const e of nearby) {
    try {
      for (const d of (await getExperience(e.id)).doses) {
        doses.push({ substance_name: d.substance_name, route: d.route, at_min: minutes(d.taken_at) });
      }
    } catch {
      for (const n of e.substances) doses.push({ substance_name: n, route: "", at_min: minutes(e.started_at) });
    }
  }

  const names = [...new Set(doses.map((d) => d.substance_name.trim()).filter(Boolean))];
  const wider = names.length > 1 ? await checkCombo(names, doses) : [];

  // A pair is the same pair in either order: the entry's own check may report
  // "Heroin + Alcohol" where the wider one says "Alcohol + Heroin", and showing the
  // same danger twice reads as two dangers.
  const seen = new Set<string>();
  return [...own, ...wider].filter((w) => {
    const pair = [w.a, w.b].map((n) => n.trim().toLowerCase()).sort().join("|");
    const key = `${w.severity}|${pair}|${w.message}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

/** Warnings that say the same thing, as one: "Stimulant + psychedelic" for five
 *  pairs is one thing to know, not five. Most severe first, each with its pairs. */
export interface WarningGroup {
  severity: string;
  message: string;
  pairs: string[];
  advice: string[];
}

export function groupWarnings(list: Warning[]): WarningGroup[] {
  const rank = (s: string) => (s === "danger" ? 0 : s === "caution" ? 1 : 2);
  const groups = new Map<string, WarningGroup>();
  for (const w of list) {
    const key = `${w.severity}|${w.message}`;
    const pair = `${w.a} + ${w.b}`;
    const g = groups.get(key) ?? { severity: w.severity, message: w.message, pairs: [], advice: [] };
    if (!g.pairs.includes(pair)) g.pairs.push(pair);
    for (const a of w.advice ?? []) if (!g.advice.includes(a)) g.advice.push(a);
    groups.set(key, g);
  }
  return [...groups.values()].sort((a, b) => rank(a.severity) - rank(b.severity));
}

/** What you've logged lately, most recent first — the list worth offering as a tap. */
export function recentSubstances(entries: ExperienceSummary[], limit = 8): string[] {
  return [...new Set(entries.flatMap((e) => e.substances))].slice(0, limit);
}

/**
 * Times you actually reach for. "Last night" is the previous evening rather
 * than a number of hours back, because that's how the thing being logged is
 * remembered — and the exact value always lands in a visible field, so a preset
 * can never quietly file a dose under the wrong moment.
 */
export const whenPresets: { label: string; at: () => Date }[] = [
  { label: "Now", at: () => new Date() },
  { label: "1h ago", at: () => new Date(Date.now() - 3600_000) },
  { label: "3h ago", at: () => new Date(Date.now() - 3 * 3600_000) },
  {
    label: "Last night",
    at: () => {
      const d = new Date();
      d.setDate(d.getDate() - 1);
      d.setHours(22, 0, 0, 0);
      return d;
    },
  },
];

/**
 * The unit and route you last used for a substance, remembered per device.
 *
 * Defaulting cannabis to milligrams oral every single time is the kind of small
 * friction that stops a log being kept. This is a convenience only — it lives in
 * browser storage, never in the journal, so losing it costs nothing and it is
 * never mistaken for something the user recorded.
 */
const SHAPE_KEY = "fieldnotes.doseShapes";

export interface DoseShape {
  unit: string;
  route: string;
}

function shapes(): Record<string, DoseShape> {
  try {
    return JSON.parse(localStorage.getItem(SHAPE_KEY) ?? "{}");
  } catch {
    return {};
  }
}

export function recallDoseShape(substance: string): DoseShape | null {
  return shapes()[substance.trim().toLowerCase()] ?? null;
}

/** The units every dose form offers. One list, so the phone and desktop agree. */
export const UNITS = ["mg", "µg", "g", "ml", "tab", "capsule", "pill", "piece", "drink", "hit"];

/** Units that count things, which can say how much each one holds ("00 caps, 0.45 g"). */
export const COUNTED_UNITS = ["capsule", "pill", "tab", "piece"];

export interface FormChoice {
  value: string;
  label: string;
}

/**
 * The forms a dose can take, by substance (owner's decisions, 2026-10-05). Stats
 * reads them in `measure` (stats.rs): fresh mushrooms count as about a tenth of
 * their weight dried, an edible goes on its estimate, and kratom extract and
 * 7-OH stay off leaf's ranges. A Rust test checks these names.
 */
export const FORMS: Record<"mushrooms" | "kratom", FormChoice[]> = {
  mushrooms: [
    { value: "dried", label: "Dried" },
    { value: "fresh", label: "Fresh" },
    { value: "powdered", label: "Powdered" },
    { value: "edible", label: "Edible" },
  ],
  kratom: [
    { value: "leaf", label: "Leaf powder" },
    { value: "extract", label: "Extract" },
    { value: "7-oh", label: "7-OH product" },
  ],
};

/** Which forms to offer for a substance: mushrooms and truffles, or kratom. */
export function formsFor(substance: string): FormChoice[] {
  const s = substance.trim().toLowerCase();
  if (/mushroom|shroom|mushies|truffle/.test(s)) return FORMS.mushrooms;
  if (s === "kratom") return FORMS.kratom;
  return [];
}

/** Psilocybin mushrooms, which have a fresh-to-dried estimate. Truffles hold less
 *  water, so theirs isn't converted. Matches `is_mushroom` in stats.rs. */
export function isMushroom(substance: string): boolean {
  const s = substance.trim().toLowerCase();
  return !s.includes("truffle") && /mushroom|shroom|mushies/.test(s);
}

/** Fresh psilocybin mushrooms are about 90% water. `FRESH_TO_DRIED` in stats.rs. */
export const FRESH_TO_DRIED = 10;

/**
 * One standard drink, as the app counts it: the US definition (14 g of alcohol).
 * Shown wherever "drink" is the unit, so a number of drinks always means the same
 * thing. DoseWiki's alcohol ranges mix UK units (8 g) with drinks, so a logged
 * number of drinks is never compared against them.
 */
export const STANDARD_DRINK =
  "1 drink = one standard drink: a 12 oz beer (5%), a 5 oz glass of wine (12%), or a 1.5 oz shot of spirits (40%). A mixed drink is as many drinks as it has shots.";

/** A hit is not a fixed amount, so it's never compared with mg ranges. */
export const HIT_NOTE =
  "A hit varies a lot with strength, device and how deep you inhale, so hits aren't compared with the reference's mg ranges.";

/** Quick picks for counting drinks: each adds one standard drink. */
export const DRINK_PICKS = ["Beer", "Glass of wine", "Shot"];

/**
 * Substances nobody measures in milligrams. Picking one of these switches the
 * unit for you when there's no remembered shape — "100 mg LSD" is a thousandfold
 * mistake, and "3.5" of mushrooms left on the form's mg default is a thousandfold
 * under-record. Names are the DoseWiki titles plus common aliases, lowercased.
 */
const DEFAULT_UNITS: Record<string, string> = Object.fromEntries([
  ...[
    "lsd", "lsd-25", "acid",
    "1a-lsd", "1b-lsd", "1cp-lsd", "1d-lsd", "1p-lsd", "1v-lsd", "ald-52",
    "al-lad", "1cp-al-lad", "eth-lad", "1p-eth-lad", "pro-lad", "pargy-lad",
  ].map((n) => [n, "µg"]),
  ...[
    "psilocybin mushrooms", "magic mushrooms", "mushrooms", "shrooms", "mushies",
    "psychedelic mushrooms", "magic truffles", "truffles",
  ].map((n) => [n, "g"]),
  ...[
    "alcohol", "ethanol", "booze", "beer", "wine", "cider", "vodka", "whiskey", "whisky",
    "rum", "gin", "tequila", "mezcal", "liquor", "spirits", "sake", "champagne",
  ].map((n) => [n, "drink"]),
]);

const CANNABIS = new Set(["cannabis", "weed", "marijuana", "thc", "pot", "bud", "flower", "herb", "ganja"]);

/** The unit a substance is usually counted in, when it isn't mg. Smoked or vaped
 *  cannabis is counted in hits; eaten, it stays in mg of THC. */
export function defaultUnitFor(substance: string, route = ""): string | null {
  const s = substance.trim().toLowerCase();
  if (CANNABIS.has(s)) return /^(vaporized|smoked|inhaled)$/i.test(route) ? "hit" : null;
  return DEFAULT_UNITS[s] ?? null;
}

/** DoseWiki writes micrograms as both "ug" and "µg"; the forms only offer "µg". */
export const sameUnit = (a: string, b: string) => {
  const n = (u: string) => u.trim().toLowerCase().replace(/^(ug|mcg)$/, "µg");
  return n(a) === n(b);
};

export function rememberDoseShape(substance: string, shape: DoseShape): void {
  const name = substance.trim().toLowerCase();
  if (!name) return;
  try {
    localStorage.setItem(SHAPE_KEY, JSON.stringify({ ...shapes(), [name]: shape }));
  } catch {
    // A phone with storage disabled just doesn't get the convenience.
  }
}

/**
 * Save a pasted trip log (see tripimport.ts) as one past session: the entry, then
 * every line in order — doses through `log_dose`, the rest as timeline moments — then
 * ended at its last line so it reads as history rather than a session left open,
 * with whatever was written after the log as its write-up.
 *
 * Every dose gets the same wider interaction check as a quick log. The warnings are
 * returned all together, deduplicated, because a log pasted in one go is read in one go.
 */
export interface TripLine {
  kind: "dose" | "moment";
  /** ISO 8601, UTC. */
  at: string;
  text: string;
  substance: string;
  amount: number | null;
  unit: string;
  route: string;
  intensity: number | null;
}

export async function saveTripLog(title: string, lines: TripLine[], writeup = ""): Promise<QuickLogResult> {
  if (!lines.length) throw new Error("Nothing to import.");
  const sorted = [...lines].sort((a, b) => new Date(a.at).getTime() - new Date(b.at).getTime());
  for (const l of sorted) {
    if (l.kind === "dose" && !l.substance.trim()) throw new Error("A dose line has no substance — name it, or make it a moment.");
  }
  const start = sorted[0].at;
  const end = sorted[sorted.length - 1].at;
  const { id } = await createExperience({ title: title.trim(), started_at: start });

  const all: Warning[] = [];
  for (const l of sorted) {
    if (l.kind === "dose") {
      const res = await logDose({
        experience_id: id,
        substance_name: l.substance.trim(),
        amount: l.amount,
        unit: l.unit,
        route: l.route,
        taken_at: l.at,
        // Keep the words around the dose when there were any worth keeping.
        note: l.text.split(/\s+/).length > 4 ? l.text : "",
      });
      all.push(...(await allWarnings(l.substance.trim(), l.at, res.warnings, id, l.route)));
    } else if (l.text.trim()) {
      await addTimelineEvent({ experience_id: id, at: l.at, note: l.text.trim(), intensity: l.intensity });
    }
  }
  await endExperience(id, end, null, writeup.trim());

  const seen = new Set<string>();
  const warnings = all.filter((w) => {
    const pair = [w.a, w.b].map((n) => n.trim().toLowerCase()).sort().join("|");
    const key = `${w.severity}|${pair}|${w.message}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
  const entry = await getExperience(id);
  return { id, title: entry.title, warnings, doseId: null, fresh: true };
}
