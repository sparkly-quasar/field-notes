// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// Time bucketing for the usage stats page. The backend (stats.rs) owns the grouping
// rules; this file only places its timestamps on the *viewer's* calendar, because the
// phone and the computer serving it need not share a time zone. Desktop and phone
// both use this, so there is one bucketing rule on the frontend too.

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
 */
export const SPACING_NOTE: Record<string, string> = {
  psychedelics:
    "Tolerance builds after one session and carries across psychedelics: LSD one day dulls mushrooms the next. It mostly fades over 3 to 7 days, so a common guide is at least 3 days apart, and a week or more for the full effect. Many people leave longer still for integration.",
  entactogens:
    "A common harm-reduction guide is at least a month between MDMA sessions, and three months is widely recommended. The three-month rule is a community convention rather than a tested limit. It allows time for serotonin to recover and for the experience not to dull. Clinical MDMA trials gave sessions about a month apart.",
  dissociatives:
    "Tolerance to ketamine builds quickly, and frequent use, more than any single dose, is what's tied to bladder damage and dependence. A common guide is to keep sessions days apart rather than back to back, and to notice if the gaps are shrinking.",
  cannabinoids:
    "With regular use, the brain's cannabinoid receptors turn down. They begin recovering within about 2 days of stopping and are close to normal after about 4 weeks, so a few days off eases tolerance and a month resets it.",
  opioids:
    "Tolerance falls during a break, faster than many expect. After days or weeks without, a dose that used to be usual can be an overdose. Start lower than before, don't use alone, and have naloxone nearby.",
  depressants:
    "With GHB/GBL and benzodiazepines, use on most days can bring dependence within weeks, and stopping suddenly after that can be dangerous. Spacing uses apart, not on consecutive days, keeps that from building. Tolerance also falls during a break, so start lower after one.",
};

/** The spacing notes for a substance's families, in family order, at most one each. */
export function spacingNotes(families: string[]): string[] {
  return [...new Set(families)].map((f) => SPACING_NOTE[f]).filter((n): n is string => !!n);
}
