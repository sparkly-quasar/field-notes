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
  checkCombo,
  createExperience,
  endExperience,
  getExperience,
  listExperiences,
  logDose,
  updateExperience,
  type ExperienceSummary,
  type Warning,
} from "./api";

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
  return { id, title: entry.title, warnings: await allWarnings(substance, input.at, logged.warnings) };
}

/**
 * Keep an entry's span honest when a dose is added outside it. Without this the
 * journal shows an entry that ended an hour before something in it was taken,
 * and every t+ offset in that entry is measured from the wrong moment.
 */
async function stretchToCover(id: number, at: string) {
  const e = await getExperience(id);
  const before = new Date(at) < new Date(e.started_at);
  const after = e.ended_at != null && new Date(at) > new Date(e.ended_at);
  if (!before && !after) return;
  await updateExperience(id, {
    title: e.title,
    intention: e.intention,
    setting: e.setting,
    notes: e.notes,
    rating: e.rating,
    started_at: before ? at : e.started_at,
    ended_at: after ? at : e.ended_at,
  });
}

/**
 * `log_dose` compares a dose against the rest of **its own entry**, which for a
 * quick log is nothing at all — so on its own it would go quiet on exactly the
 * combination that matters. Widen the question to "what else was in you around
 * then" and run the same deterministic checker over that.
 *
 * Entry start times are the coarse grain available without reading every dose
 * in the journal, so this can flag a pair that was hours apart. That is the
 * right way to be wrong: a warning you can dismiss beats silence you can't.
 */
async function allWarnings(substance: string, at: string, own: Warning[]): Promise<Warning[]> {
  const t = new Date(at).getTime();
  const nearby = (await listExperiences())
    .filter((e) => Math.abs(new Date(e.started_at).getTime() - t) < NEARBY_HOURS * 3600_000)
    .flatMap((e) => e.substances);

  const names = [...new Set([substance, ...nearby].map((n) => n.trim()).filter(Boolean))];
  const wider = names.length > 1 ? await checkCombo(names) : [];

  const seen = new Set<string>();
  return [...own, ...wider].filter((w) => {
    const key = `${w.severity}|${w.a}|${w.b}|${w.message}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
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

export function rememberDoseShape(substance: string, shape: DoseShape): void {
  const name = substance.trim().toLowerCase();
  if (!name) return;
  try {
    localStorage.setItem(SHAPE_KEY, JSON.stringify({ ...shapes(), [name]: shape }));
  } catch {
    // A phone with storage disabled just doesn't get the convenience.
  }
}
