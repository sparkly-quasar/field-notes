// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// Time bucketing for the usage stats page. The backend (stats.rs) owns the grouping
// rules; this file only places its timestamps on the *viewer's* calendar, because the
// phone and the computer serving it need not share a time zone. Desktop and phone
// both use this, so there is one bucketing rule on the frontend too.

import { clockTime } from "./clock.ts";

export type RangeKey = "30d" | "90d" | "1y" | "all";

export const RANGES: { key: RangeKey; label: string; days: number | null }[] = [
  { key: "30d", label: "30 days", days: 30 },
  { key: "90d", label: "90 days", days: 90 },
  { key: "1y", label: "1 year", days: 365 },
  { key: "all", label: "All", days: null },
];

const DAY = 86_400_000;

/** Local midnight at the start of the day containing `t`. */
export function startOfDay(t: number): number {
  const d = new Date(t);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

/** Local Monday 00:00 of the week containing `t`. */
export function startOfWeek(t: number): number {
  const d = new Date(startOfDay(t));
  d.setDate(d.getDate() - ((d.getDay() + 6) % 7));
  return d.getTime();
}

/** `YYYY-MM-DD` in local time — a day's identity for bucketing. */
export function dayKey(t: number): string {
  const d = new Date(t);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

export function sinceFor(range: RangeKey, now = Date.now()): string | null {
  const days = RANGES.find((r) => r.key === range)?.days;
  return days == null ? null : new Date(startOfDay(now) - (days - 1) * DAY).toISOString();
}

export function ts(s: string): number | null {
  const t = Date.parse(s);
  return Number.isNaN(t) ? null : t;
}

/** Sessions per bucket, oldest first, empty buckets included so gaps show. */
export function frequency(
  sessionTimes: number[],
  from: number,
  to: number,
): { start: number; count: number; unit: "week" | "month" }[] {
  const spanDays = (to - from) / DAY;
  const unit: "week" | "month" = spanDays > 200 ? "month" : "week";
  const begin = (t: number) => {
    if (unit === "week") return startOfWeek(t);
    const d = new Date(t);
    return new Date(d.getFullYear(), d.getMonth(), 1).getTime();
  };
  const next = (t: number) => {
    const d = new Date(t);
    return unit === "week"
      ? new Date(d.getFullYear(), d.getMonth(), d.getDate() + 7).getTime()
      : new Date(d.getFullYear(), d.getMonth() + 1, 1).getTime();
  };
  const out: { start: number; count: number; unit: "week" | "month" }[] = [];
  for (let b = begin(from); b <= to; b = next(b)) out.push({ start: b, count: 0, unit });
  for (const t of sessionTimes) {
    const b = begin(t);
    const slot = out.find((o) => o.start === b);
    if (slot) slot.count++;
  }
  return out;
}

/** Sessions per local day. */
export function perDay(sessionTimes: number[]): Map<string, number> {
  const m = new Map<string, number>();
  for (const t of sessionTimes) m.set(dayKey(t), (m.get(dayKey(t)) ?? 0) + 1);
  return m;
}

/** Doses per local hour, 0–23. */
export function byHour(doseTimes: number[]): number[] {
  const h = new Array(24).fill(0);
  for (const t of doseTimes) h[new Date(t).getHours()]++;
  return h;
}

/** Whole days between `t` and now, by local calendar day. */
export function daysSince(t: number, now = Date.now()): number {
  return Math.round((startOfDay(now) - startOfDay(t)) / DAY);
}

export function median(xs: number[]): number | null {
  if (!xs.length) return null;
  const s = [...xs].sort((a, b) => a - b);
  const m = Math.floor(s.length / 2);
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
}

const logFact = (n: number) => {
  let s = 0;
  for (let i = 2; i <= n; i++) s += Math.log(i);
  return s;
};

/**
 * Two-sided Fisher's exact test: how likely a split at least this uneven is
 * when the two groups don't really differ. `a` of `n1` in one group, `b` of `n2`
 * in the other. Exact at any size, so it holds for the handful of entries Trends
 * starts from.
 */
export function fisherP(a: number, n1: number, b: number, n2: number): number {
  const k = a + b, n = n1 + n2;
  if (!n1 || !n2 || k === 0 || k === n) return 1;
  const base = logFact(n1) + logFact(n2) + logFact(k) + logFact(n - k) - logFact(n);
  const p = (x: number) => Math.exp(base - logFact(x) - logFact(n1 - x) - logFact(k - x) - logFact(n2 - k + x));
  const seen = p(a) * (1 + 1e-7);
  let sum = 0;
  for (let x = Math.max(0, k - n2); x <= Math.min(k, n1); x++) {
    const px = p(x);
    if (px <= seen) sum += px;
  }
  return Math.min(1, sum);
}

/**
 * Two-sided exact test for two counts over periods of the same length: how
 * likely a split at least this uneven is if the rate didn't change. With `n`
 * events in all, each falls in either period with even odds.
 */
export function splitP(a: number, b: number): number {
  const n = a + b;
  if (n === 0) return 1;
  const p = (x: number) => Math.exp(logFact(n) - logFact(x) - logFact(n - x) - n * Math.LN2);
  const seen = p(a) * (1 + 1e-7);
  let sum = 0;
  for (let x = 0; x <= n; x++) if (p(x) <= seen) sum += p(x);
  return Math.min(1, sum);
}

/** A "nice" axis maximum and tick step for 0..max. */
export function niceScale(max: number, ticks = 4): { max: number; step: number } {
  if (!(max > 0)) return { max: 1, step: 1 };
  const raw = max / ticks;
  const mag = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 2.5, 5, 10].map((m) => m * mag).find((s) => s >= raw) ?? 10 * mag;
  return { max: Math.ceil(max / step) * step, step };
}

export function fmtNum(n: number): string {
  return Number.isInteger(n) ? String(n) : n.toFixed(n < 10 ? 2 : 1).replace(/\.?0+$/, "");
}

// ---------- drug families ----------

/** The drug families Stats sorts substances into, in display order (stats.rs FAMILIES). */
export const FAMILY_ORDER = ["psychedelics", "entactogens", "dissociatives", "stimulants", "depressants", "opioids", "cannabinoids", "other"];
export const FAMILY_LABEL: Record<string, string> = {
  psychedelics: "Psychedelics", entactogens: "Entactogens", dissociatives: "Dissociatives", stimulants: "Stimulants",
  depressants: "Depressants", opioids: "Opioids", cannabinoids: "Cannabinoids", other: "Other",
};
/** "days since the last psychedelic" */
export const FAMILY_ONE: Record<string, string> = {
  psychedelics: "psychedelic", entactogens: "entactogen", dissociatives: "dissociative", stimulants: "stimulant",
  depressants: "depressant", opioids: "opioid", cannabinoids: "cannabinoid", other: "of these",
};

/** Where one family went between the two periods. "few": too little to tell
 *  either way; "new" and "gone": none in one period, clearly some in the other. */
export type FamilyTrend = "up" | "down" | "same" | "new" | "gone" | "few";
export type FamilyRow = { family: string; before: number; lately: number; trend: FamilyTrend };

/**
 * How many experiences had each drug family in them, before `mid` and from `mid`
 * on, most lately first. An experience counts once per family however many doses
 * it held, so a night of redosing reads as one time, not many (Trends has its own
 * redosing card). One with substances from two families (MDMA: entactogen and
 * stimulant) counts toward both, so the rows can add up to more than the total.
 * A family has moved only when the bigger period has at least `minEach` of it,
 * one period has at least `by` times the other, and a split that uneven would
 * be unlikely by chance (`splitP` under `pMax`). With a handful of entries, 3
 * then 2, or even 5 then 1, is too few to tell.
 */
export function familyMix(exps: { t: number; families: string[] }[], mid: number, minEach = 3, by = 1.5, pMax = 0.1): FamilyRow[] {
  const count = new Map<string, { before: number; lately: number }>();
  for (const e of exps) {
    for (const f of new Set(e.families)) {
      const c = count.get(f) ?? { before: 0, lately: 0 };
      if (e.t >= mid) c.lately++;
      else c.before++;
      count.set(f, c);
    }
  }
  const trend = (b: number, l: number): FamilyTrend => {
    if (Math.max(b, l) < minEach) return "few";
    if (l < b * by && l * by > b) return "same";
    if (splitP(l, b) >= pMax) return "few";
    if (b === 0) return "new";
    if (l === 0) return "gone";
    return l > b ? "up" : "down";
  };
  const order = (f: string) => (FAMILY_ORDER.includes(f) ? FAMILY_ORDER.indexOf(f) : FAMILY_ORDER.length);
  return [...count]
    .map(([family, c]) => ({ family, ...c, trend: trend(c.before, c.lately) }))
    .sort((a, b) => b.lately - a.lately || b.before - a.before || order(a.family) - order(b.family));
}

/** How a steady rhythm reads, by its typical gap in days; null past what
 *  reads as a rhythm at all. */
export function rhythmWord(days: number): string | null {
  // From 6: a weekly habit logged a little early now and then has a typical gap
  // just under 7.
  if (days < 6) return null;
  if (days <= 10) return "about once a week";
  if (days <= 20) return "about every two weeks";
  if (days <= 45) return "about once a month";
  if (days <= 75) return "about every two months";
  if (days <= 120) return "about every three months";
  return null;
}

/**
 * A steady rhythm in a set of times, for what's too infrequent for two short
 * periods to compare: once a month is 1 then 1 in a 30-day view, but a pattern
 * over a year. Times within 2 days of an occasion's start are one occasion (a
 * weekend). A rhythm needs at least `minTimes` occasions, a typical gap of about a
 * week or more, three in four gaps within half to double that, and the last one
 * recent enough to still be going (no more than two gaps ago).
 */
export function rhythm(times: number[], now = Date.now(), minTimes = 4): { gapDays: number; since: number; times: number } | null {
  const occ: number[] = [];
  for (const t of [...times].sort((a, b) => a - b)) if (!occ.length || t - occ[occ.length - 1] > 2 * DAY) occ.push(t);
  if (occ.length < minTimes) return null;
  const gaps = occ.slice(1).map((t, i) => (t - occ[i]) / DAY);
  const g = median(gaps)!;
  if (!rhythmWord(g)) return null;
  const even = gaps.filter((x) => x >= g / 2 && x <= g * 2).length;
  if (even < gaps.length * 0.75) return null;
  if ((now - occ[occ.length - 1]) / DAY > g * 2) return null;
  return { gapDays: g, since: occ[0], times: occ.length };
}

/**
 * What harm-reduction sources usually say about spacing, per drug family: shown
 * beside the gaps on the stats page. Reference text, not a verdict on this
 * person's numbers. It never compares their gaps to it or colours anything.
 * Families with no widely given spacing guidance (stimulants, other) have none.
 *
 * Sources, for whoever edits these:
 * - psychedelics: Isbell et al. 1961 (LSD–psilocybin cross-tolerance); tolerance
 *   mostly gone after 3–7 days.
 * - entactogens: the "three-month rule" is community convention (credited to Ann
 *   Shulgin), not a tested limit; MAPS phase 3 (Mitchell et al. 2021) dosed
 *   monthly, three sessions.
 * - cannabinoids: D'Souza et al. 2016 and Hirvonen et al. 2012 (CB1 availability
 *   recovers from about 2 days, near normal by about 4 weeks).
 * - dissociatives: one note for the whole family (ketamine, PCP analogues such
 *   as 3-MeO-PCP, DXM...), so only ketamine is named for bladder damage, the
 *   one with the evidence for it.
 * - kratom (in place of the opioid note): Hiranita et al. 2020 and Hill et al.
 *   2022 (mitragynine's ceiling on respiratory depression in rodents); Olsen et
 *   al. 2019, CDC MMWR (kratom-positive overdose deaths almost all involved
 *   other substances). 7-OH is a far stronger opioid agonist than leaf.
 */
export const SPACING_NOTE: Record<string, string> = {
  psychedelics:
    "Tolerance builds after a single experience and carries across psychedelics: LSD one day dulls mushrooms the next. It mostly fades over 3 to 7 days, so a common guide is at least 3 days apart, and a week or more for the full effect. Many people leave longer still for integration.",
  entactogens:
    "A common harm-reduction guide is at least a month between MDMA experiences, and three months is widely recommended. The three-month rule is a community convention rather than a tested limit. It allows time for serotonin to recover and for the experience not to dull. Clinical MDMA trials spaced dosing sessions about a month apart.",
  dissociatives:
    "Tolerance to dissociatives builds quickly, and frequent use, more than any single dose, is what's tied to dependence. With ketamine, the best studied, frequent use is also tied to bladder damage. A common guide is to keep experiences days apart rather than back to back, and to notice if the gaps are shrinking.",
  cannabinoids:
    "With regular use, the brain's cannabinoid receptors turn down. They begin recovering within about 2 days of stopping and are close to normal after about 4 weeks, so a few days off eases tolerance and a month resets it.",
  opioids:
    "Tolerance falls during a break, faster than many expect. After days or weeks without, a dose that used to be usual can be an overdose. Start lower than before, don't use alone, and have naloxone nearby.",
  depressants:
    "With GHB/GBL and benzodiazepines, use on most days can bring dependence within weeks, and stopping suddenly after that can be dangerous. Spacing uses apart, not on consecutive days, keeps that from building. Tolerance also falls during a break, so start lower after one.",
};

/** Kratom sits with the opioids but the opioid note overstates its own risk. */
export const KRATOM_SPACING_NOTE =
  "Daily kratom builds tolerance and can lead to dependence, with withdrawal if you stop suddenly. On its own, kratom is much less likely than other opioids to slow breathing dangerously, but the risk rises when it's mixed with alcohol, benzodiazepines or other opioids, and with concentrated 7-OH products. After a break, start lower than before.";

/** The spacing notes for a substance's families, in family order, at most one each. */
export function spacingNotes(families: string[], substance = ""): string[] {
  const kratom = substance.trim().toLowerCase() === "kratom";
  return [...new Set(families)]
    .map((f) => (kratom && f === "opioids" ? KRATOM_SPACING_NOTE : SPACING_NOTE[f]))
    .filter((n): n is string => !!n);
}

// ---------- bedtime ----------

/** "23:00" → minutes after midnight, or null for "varies", "skip" or anything else. */
export function bedMinutes(bedtime: string | null | undefined): number | null {
  const m = /^(\d{1,2}):(\d{2})$/.exec(bedtime ?? "");
  if (!m) return null;
  const h = Number(m[1]);
  const min = Number(m[2]);
  return h < 24 && min < 60 ? h * 60 + min : null;
}

/** How long before bedtime `t` falls, in minutes on the viewer's clock (0 at
 *  bedtime, up to a day before). A dose after bedtime counts as nearly a day
 *  before the next one: this doesn't know when anyone actually slept. */
export function minutesBeforeBed(t: number, bed: number): number {
  const d = new Date(t);
  const at = d.getHours() * 60 + d.getMinutes();
  return (bed - at + 1440) % 1440;
}

/** Of these dose times, how many fell within `hours` before bedtime, counting
 *  one taken exactly that long before: half of it is still there at bedtime. */
export function closeToBed(times: number[], bed: number, hours: number): number {
  const window = Math.min(hours, 24) * 60;
  return times.filter((t) => minutesBeforeBed(t, bed) <= window).length;
}

/** Is this hour of the day (0–23) inside the `hours` before bedtime? For shading. */
export function hourBeforeBed(hour: number, bed: number, hours: number): boolean {
  const before = (bed - hour * 60 + 1440) % 1440;
  return before > 0 && before <= Math.min(hours, 24) * 60;
}

/** "11:30 pm", or "23:30" on the 24-hour clock. */
export function fmtBedtime(bed: number): string {
  const d = new Date(2026, 0, 1, Math.floor(bed / 60), bed % 60);
  return clockTime(d);
}

// ---------- sleep (step 6) ----------

/** The night a morning asks about: the local date of the evening before. */
export function lastNight(now = Date.now()): string {
  const d = new Date(now);
  d.setDate(d.getDate() - 1);
  return dayKey(d.getTime());
}

/** When bedtime fell on a night ("2026-10-05"): that evening at bedtime, or
 *  the next morning's clock for a bedtime between midnight and noon. */
export function bedOf(night: string, bed: number): number {
  const [y, m, d] = night.split("-").map(Number);
  return new Date(y, m - 1, d + (bed < 720 ? 1 : 0), Math.floor(bed / 60), bed % 60).getTime();
}

export interface SleepGroup {
  nights: number;
  average: number | null;
}

/**
 * Sleep after nights with a dose near bedtime against the other nights. A dose
 * is near bedtime when it was taken within its substance's half-life (`hours`,
 * the low end) before that night's bedtime. Counts and averages only: few
 * nights and many causes, so nothing here is a finding.
 */
export function sleepCompare(
  nights: { night: string; rating: number }[],
  doses: { t: number; hours: number }[],
  bed: number,
): { near: SleepGroup; other: SleepGroup } {
  const near: number[] = [];
  const other: number[] = [];
  for (const n of nights) {
    const b = bedOf(n.night, bed);
    const late = doses.some((d) => d.t <= b && b - d.t <= Math.min(d.hours, 24) * 3_600_000);
    (late ? near : other).push(n.rating);
  }
  const avg = (xs: number[]) => (xs.length ? Math.round((xs.reduce((a, b) => a + b, 0) / xs.length) * 10) / 10 : null);
  return { near: { nights: near.length, average: avg(near) }, other: { nights: other.length, average: avg(other) } };
}
