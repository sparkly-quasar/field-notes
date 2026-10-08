// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// Active arcs: what each substance in a live session is likely doing now, drawn
// from the reference's own stage ranges (onset, come-up, peak, offset,
// after-effects). One line per substance, every dose of it added in, so a redose
// shows as the dose the body is carrying rather than a fresh small arc. The
// height is the reference's dose tier for what is still counting; the shape and
// timing are the reference's. Nothing here invents a number the reference lacks:
// a stage it doesn't give isn't drawn, a dose with no comparable amount gets no
// tier, and a substance with no timings stays listed instead of vanishing.
//
// One exception, owner-approved: drinks clear at a fixed rate (see
// `DRINKS_PER_HOUR`), because alcohol is cleared at a near-constant rate rather
// than tapering, and DoseWiki has no figure for it.

import type { Dose, Experience, PwInfo, PwRoa } from "./api";
import { detailOf, inUnit, measure } from "./dosedetail.ts";

const HOUR = 3_600_000;

/** [fastest, slowest] in hours. */
export type Span = [number, number];

export interface Stages {
  onset: Span | null;
  comeUp: Span | null;
  peak: Span | null;
  offset: Span | null;
  after: Span | null;
  total: Span | null;
}

/** A reference duration such as "20–70 minutes", "3 hours" or "10–60 seconds",
 *  in hours. Null for anything it can't read. */
export function parseSpan(s: string | null | undefined): Span | null {
  if (!s) return null;
  const nums = (s.match(/\d+(?:\.\d+)?/g) ?? []).map(Number).filter((n) => Number.isFinite(n));
  if (!nums.length) return null;
  const t = s.toLowerCase();
  const per = t.includes("sec") ? 1 / 3600 : t.includes("min") ? 1 / 60 : t.includes("day") ? 24 : t.includes("hour") ? 1 : null;
  if (per == null) return null;
  const a = nums[0] * per;
  const b = nums[nums.length - 1] * per;
  return [Math.min(a, b), Math.max(a, b)];
}

export function stagesOf(roa: PwRoa | null): Stages {
  return {
    onset: parseSpan(roa?.onset),
    comeUp: parseSpan(roa?.come_up),
    peak: parseSpan(roa?.peak),
    offset: parseSpan(roa?.offset),
    after: parseSpan(roa?.after_effects),
    total: parseSpan(roa?.total),
  };
}

/** The reference's entry for the route a dose was logged by: the same route by
 *  name, IM read as IV as the combination check does, or the first route listed
 *  when no route was logged. */
export function routeFor(info: PwInfo | null, route: string): PwRoa | null {
  if (!info || !info.roas.length) return null;
  const r = route.trim().toLowerCase();
  if (!r) return info.roas[0];
  const want = r === "im" || r === "iv" ? "intravenous" : r;
  return info.roas.find((x) => x.name.trim().toLowerCase() === want) ?? null;
}

// ---- how a second dose behaves

/** How a redose shows up in the felt effect. The line always shows the combined
 *  dose; this only decides what the row and key say about it. */
export type Redose = "additive" | "blunted" | "partial" | "delayed";

export const REDOSE_NOTE: Record<Redose, string> = {
  additive: "Repeat doses add up in both the body and the effect, so the line climbs with each one.",
  blunted:
    "A second dose raises levels in your body more than it raises how it feels. The line shows the combined dose, so it can sit higher than the high does. Strain on the heart and body temperature follows the line, not the feeling.",
  partial:
    "Redosing early adds some intensity, with diminishing returns. The line shows the combined dose, so it can overstate how much stronger it feels. Tolerance builds over the next day or two.",
  delayed:
    "Effects arrive late, so a redose taken because it didn't seem to work stacks on top of a dose that was still coming up.",
};

const ADDITIVE = ["depressant", "sedative", "opioid", "gabaergic", "anxiolytic", "hypnotic", "dissociative"];
const BLUNTED = ["entactogen", "stimulant"];
const PARTIAL = ["psychedelic", "hallucinogen", "entheogen"];
const GHB_LIKE = /\b(ghb|gbl|1,4-b|14b)\b|butanediol/i;

/** Which way a redose goes for a substance, from the reference's classes (and
 *  the name for the GHB family and drinks, which DoseWiki leaves unclassed). */
export function redoseOf(info: PwInfo | null, name: string, dose?: Pick<Dose, "unit" | "route" | "form">): Redose | null {
  if (dose && dose.unit.trim().toLowerCase() === "drink") return "additive";
  const label = `${name} ${info?.name ?? ""}`;
  if (/\b(alcohol|ethanol)\b/i.test(label) || GHB_LIKE.test(label)) return "additive";
  const classes = (info?.psychoactive ?? []).map((c) => c.toLowerCase());
  const has = (list: string[]) => classes.some((c) => list.some((l) => c.startsWith(l)));
  if (classes.some((c) => c.startsWith("cannabinoid"))) {
    const eaten = (dose?.form ?? "").toLowerCase() === "edible" || /^oral$/i.test(dose?.route ?? "");
    return eaten ? "delayed" : "additive";
  }
  if (has(ADDITIVE)) return "additive";
  if (has(BLUNTED)) return "blunted";
  if (has(PARTIAL)) return "partial";
  return null;
}

// ---- series

/**
 * Drinks clear at about one standard drink an hour: the common rule of thumb,
 * owner-approved 2026-10-08 over the slower average the forensic literature gives
 * (about 0.015 g/100 mL an hour, closer to half a US drink an hour for a 70 kg
 * person). The key says so, and that many people clear more slowly.
 */
export const DRINKS_PER_HOUR = 1;
/** The top of the chart, for drinks. Drinks get their own scale: the app never
 *  compares a number of drinks with DoseWiki's alcohol ranges (quicklog.ts). */
export const DRINKS_TOP = 8;

/** shape: the reference gives come-up, peak and offset. drinks: counted in
 *  standard drinks. block: only a total duration. marker: no timings at all. */
export type ArcKind = "shape" | "drinks" | "block" | "marker";

export interface ArcDose {
  at: number;
  /** In the reference's unit (or drinks), or null when it can't be compared. */
  amount: number | null;
  dose: Dose;
}

export interface Series {
  key: string;
  /** What to call it: the reference's name, or what was written. */
  name: string;
  kind: ArcKind;
  doses: ArcDose[];
  roa: PwRoa | null;
  stages: Stages;
  unit: string;
  /** Height anchors, amount → height, ascending; null when there's no tier. */
  tiers: [number, number][] | null;
  redose: Redose | null;
}

/** Where each tier's line sits, 0 to 1. */
export const TIER_HEIGHT = { threshold: 0.18, light: 0.4, common: 0.62, strong: 0.82, heavy: 1 } as const;
export type Tier = keyof typeof TIER_HEIGHT;
export const TIERS = Object.keys(TIER_HEIGHT) as Tier[];

/** The bottom of each tier the reference gives, as (amount, height) anchors. */
export function tierAnchors(roa: PwRoa | null): [number, number][] | null {
  if (!roa) return null;
  const raw: [number | null, number][] = [
    [roa.threshold, TIER_HEIGHT.threshold],
    [roa.light.min, TIER_HEIGHT.light],
    [roa.common.min, TIER_HEIGHT.common],
    [roa.strong.min, TIER_HEIGHT.strong],
    [roa.heavy, TIER_HEIGHT.heavy],
  ];
  const pts: [number, number][] = [[0, 0]];
  for (const [a, h] of raw) {
    // A reference that skips or reverses a tier just loses that anchor.
    if (a != null && a > pts[pts.length - 1][0]) pts.push([a, h]);
  }
  return pts.length > 1 ? pts : null;
}

/** Which tier an amount falls in: at or over the bottom of a tier is that tier. */
export function tierOf(roa: PwRoa | null, amount: number): Tier | null {
  if (!roa) return null;
  if (roa.heavy != null && amount >= roa.heavy) return "heavy";
  if (roa.strong.min != null && amount >= roa.strong.min) return "strong";
  if (roa.common.min != null && amount >= roa.common.min) return "common";
  if (roa.light.min != null && amount >= roa.light.min) return "light";
  if (roa.threshold != null || roa.light.min != null) return "threshold";
  return null;
}

/** A dose's amount in the reference's unit, or null when there's nothing honest
 *  to compare (a hit, a count with no amount each, an edible with no estimate). */
export function comparable(d: Dose, roa: PwRoa | null): number | null {
  if (!roa || (d.amount == null && d.estimate == null)) return null;
  const m = measure(d.substance_name, d.amount, d.unit, detailOf(d));
  if (!m) return null;
  const u = roa.units ?? "";
  return u ? inUnit(m.amount, m.unit, u) : m.amount;
}

/** The reference entry a dose means, by the name it was logged under. */
export type InfoFor = (name: string) => PwInfo | null | undefined;

/** One series per substance, in the order each was first taken. Drinks of any
 *  name ("Beer", "Wine") are one series, counted in drinks. */
export function buildSeries(doses: Dose[], infoFor: InfoFor): Series[] {
  const byKey = new Map<string, Series>();
  const sorted = [...doses].filter((d) => Number.isFinite(Date.parse(d.taken_at))).sort((a, b) => Date.parse(a.taken_at) - Date.parse(b.taken_at));
  for (const d of sorted) {
    const isDrink = d.unit.trim().toLowerCase() === "drink";
    const info = infoFor(d.substance_name) ?? null;
    const key = isDrink ? "drinks" : (info?.name ?? d.substance_name).trim().toLowerCase();
    let s = byKey.get(key);
    if (!s) {
      const roa = isDrink ? routeFor(info, "oral") ?? info?.roas[0] ?? null : routeFor(info, d.route);
      const stages = stagesOf(roa);
      const kind: ArcKind = isDrink
        ? "drinks"
        : stages.comeUp && stages.peak && stages.offset
          ? "shape"
          : stages.total
            ? "block"
            : "marker";
      s = {
        key,
        name: isDrink ? "Alcohol" : info?.name ?? d.substance_name.trim(),
        kind,
        doses: [],
        roa,
        stages,
        unit: isDrink ? "drinks" : roa?.units ?? d.unit,
        tiers: isDrink ? null : tierAnchors(roa),
        redose: redoseOf(info, d.substance_name, d),
      };
      byKey.set(key, s);
    }
    const amount = isDrink ? (d.amount ?? null) : comparable(d, s.roa);
    s.doses.push({ at: Date.parse(d.taken_at), amount, dose: d });
  }
  // A dose with no comparable amount makes the whole line untiered: a sum that
  // left it out would understate what's there.
  for (const s of byKey.values()) if (s.doses.some((x) => x.amount == null)) s.tiers = null;
  return [...byKey.values()];
}

// ---- the curve

/** Stage boundaries, hours after a dose, at one end of each range: 0 fastest,
 *  1 slowest, 0.5 the middle. */
export interface Bounds {
  on: number;
  up: number;
  peak: number;
  off: number;
  after: number;
}
export function bounds(st: Stages, k: number): Bounds {
  const v = (r: Span | null) => (r ? r[0] + (r[1] - r[0]) * k : 0);
  const on = v(st.onset);
  const up = on + v(st.comeUp);
  const peak = up + v(st.peak);
  const off = peak + v(st.offset);
  return { on, up, peak, off, after: off + v(st.after) };
}

const smooth = (x: number) => {
  const c = Math.max(0, Math.min(1, x));
  return c * c * (3 - 2 * c);
};

/** How much of one dose is still counting, hours after it: rising over the
 *  come-up, whole through the peak, falling to nothing over the offset.
 *  After-effects aren't dose; they're drawn as their own tail. */
export function share(b: Bounds, h: number): number {
  if (h < b.on) return 0;
  if (h < b.up) return smooth((h - b.on) / (b.up - b.on));
  if (h < b.peak) return 1;
  if (h < b.off) return 1 - smooth((h - b.peak) / (b.off - b.peak));
  return 0;
}

const taken = (s: Series, upTo: number) => s.doses.filter((d) => d.at <= upTo);

/** The combined dose still counting at `t`, from doses taken by `upTo`. An
 *  untiered line counts each dose as 1. */
export function combined(s: Series, t: number, k: number, upTo: number): number {
  const b = bounds(s.stages, k);
  let sum = 0;
  for (const d of taken(s, upTo)) sum += (s.tiers ? d.amount ?? 0 : 1) * share(b, (t - d.at) / HOUR);
  return sum;
}

/** Drinks still being processed, minute by minute from the first drink to `to`.
 *  Each drink is absorbed over the middle of the reference's onset range (or
 *  20 minutes without one) and the body clears `DRINKS_PER_HOUR`. */
export function drinksTrack(s: Series, to: number, upTo: number, stepMs = 60_000): [number, number][] {
  const absorb = (s.stages.onset ? (s.stages.onset[0] + s.stages.onset[1]) / 2 : 1 / 3) * HOUR;
  const doses = taken(s, upTo);
  const out: [number, number][] = [];
  if (!s.doses.length) return out;
  let load = 0;
  for (let t = s.doses[0].at; t <= to; t += stepMs) {
    for (const d of doses) {
      const a = d.amount ?? 0;
      if (t >= d.at && t < d.at + absorb) load += (a * Math.min(stepMs, d.at + absorb - t)) / absorb;
    }
    load = Math.max(0, load - (DRINKS_PER_HOUR * stepMs) / HOUR);
    out.push([t, load]);
  }
  return out;
}

export function drinksAt(s: Series, t: number, upTo: number): number {
  const tr = drinksTrack(s, t, upTo);
  return tr.length ? tr[tr.length - 1][1] : 0;
}

/** When the drinks taken by `upTo` are all processed, or null if never logged. */
export function drinksClearAt(s: Series, upTo: number): number | null {
  const last = taken(s, upTo).at(-1);
  if (!last) return null;
  for (const [t, v] of drinksTrack(s, last.at + 48 * HOUR, upTo)) if (t > last.at && v <= 0) return t;
  return null;
}

/** An amount to a height, 0 to 1, through the tier anchors. */
export function heightFor(s: Series, amount: number): number {
  if (s.kind === "drinks") return Math.min(1, amount / DRINKS_TOP);
  if (!s.tiers) return Math.min(0.9, amount * 0.45);
  const pts = s.tiers;
  if (amount >= pts[pts.length - 1][0]) return pts[pts.length - 1][1];
  for (let i = 1; i < pts.length; i++) {
    if (amount <= pts[i][0]) {
      const [a0, h0] = pts[i - 1];
      const [a1, h1] = pts[i];
      return h0 + ((h1 - h0) * (amount - a0)) / (a1 - a0);
    }
  }
  return 1;
}

/** The amount on the line at `t` (middle timings), with doses taken by `upTo`. */
export function amountAt(s: Series, t: number, upTo: number): number {
  if (s.kind === "drinks") return drinksAt(s, t, upTo);
  if (s.kind === "shape") return combined(s, t, 0.5, upTo);
  return taken(s, upTo).reduce((a, d) => a + (s.tiers ? d.amount ?? 0 : 1), 0);
}

// ---- words

export interface Phase {
  label: string;
  /** For styling: coming, peak, down, after, past, unknown, wait. */
  tone: string;
  detail: string;
}

const PHASES = [
  { label: "Not felt yet", tone: "wait" },
  { label: "Coming up", tone: "coming" },
  { label: "Peaking", tone: "peak" },
  { label: "Winding down", tone: "down" },
  { label: "After-effects", tone: "after" },
  { label: "Likely past", tone: "past" },
];
function phaseIdx(b: Bounds, h: number): number {
  if (h < b.on) return 0;
  if (h < b.up) return 1;
  if (h < b.peak) return 2;
  if (h < b.off) return 3;
  if (h < b.after) return 4;
  return 5;
}

/** "9:30 pm": the 12-hour clock is the default everywhere times are shown. */
export function clock(t: number): string {
  return new Date(t).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit", hour12: true });
}
/** "9 pm", for the hour ticks on a chart's time axis. */
export function clockHour(t: number): string {
  return new Date(t).toLocaleTimeString(undefined, { hour: "numeric", hour12: true });
}
const spanText = (r: Span, unit = "h") => `${+r[0].toFixed(2)}–${+r[1].toFixed(2)} ${unit}`;

/** The amounts taken, as "110 + 40 mg". */
export function amountsText(s: Series, upTo = Infinity): string {
  const t = taken(s, upTo);
  const u = t[0]?.dose.unit ?? "";
  const total = t.reduce((a, d) => a + (d.dose.amount ?? 0), 0);
  // Counted units read as words: "2 hits", "1 pill".
  const unit = s.kind === "drinks" ? "" : ` ${/^(hit|pill|tab|capsule|piece)$/i.test(u) && total !== 1 ? `${u}s` : u}`;
  const parts = t.map((d) => (d.dose.amount == null ? "?" : `${+d.dose.amount.toFixed(3)}`));
  return s.kind === "drinks" ? `${parts.join(" + ")} ${total === 1 ? "drink" : "drinks"}` : `${parts.join(" + ")}${unit}`;
}

/** Where the line is now, in words: a phase (from the dose contributing most),
 *  when the next stage usually arrives, and what a redose means for it. Phases,
 *  never countdowns, and never a "clear by" time. */
export function phaseOf(s: Series, now: number): Phase | null {
  const t = taken(s, now);
  if (!t.length) return null;
  const last = t[t.length - 1];
  const multi = t.length > 1;

  if (s.kind === "drinks") {
    const load = drinksAt(s, now, now);
    const before = drinksAt(s, now - 10 * 60_000, now);
    if (load > 0.05) {
      const rising = load > before + 0.01;
      const r = Math.round(load);
      const amt = load < 1 ? "under one drink's" : r === 1 ? "about one drink's" : `about ${r} drinks'`;
      return { label: rising ? "Rising" : "Coming down", tone: rising ? "coming" : "down", detail: `Roughly ${amt} worth still being processed.` };
    }
    const cleared = drinksClearAt(s, now) ?? last.at;
    const after = s.stages.after;
    if (after && now - cleared < after[1] * HOUR) {
      return { label: "After-effects", tone: "after", detail: `After-effects such as poor sleep and dehydration can last ${spanText(after)}.` };
    }
    return { label: "Likely past", tone: "past", detail: "" };
  }

  if (s.kind === "marker") {
    return { label: "No duration data", tone: "unknown", detail: "The reference gives no timings for this, so it stays listed until the trip report ends." };
  }

  if (s.kind === "block") {
    const total = s.stages.total!;
    const h = (now - last.at) / HOUR;
    if (h < total[0]) return { label: "Active", tone: "coming", detail: `Total duration ${spanText(total)}; the reference gives no shape within it.` };
    if (h < total[1]) return { label: "Active or after-effects", tone: "after", detail: `Inside the ${spanText(total)} total duration.` };
    return {
      label: s.stages.after && h < total[1] + s.stages.after[1] ? "After-effects" : "Likely past",
      tone: s.stages.after && h < total[1] + s.stages.after[1] ? "after" : "past",
      detail: s.stages.after ? `After-effects can last ${spanText(s.stages.after)}.` : "",
    };
  }

  const mid = bounds(s.stages, 0.5);
  let lead = last;
  let best = 0;
  for (const d of t) {
    const c = (s.tiers ? d.amount ?? 0 : 1) * share(mid, (now - d.at) / HOUR);
    if (c > best) {
      best = c;
      lead = d;
    }
  }
  const fast = bounds(s.stages, 0);
  const slow = bounds(s.stages, 1);
  const h = (now - lead.at) / HOUR;
  const a = phaseIdx(fast, h);
  const b = phaseIdx(slow, h);
  const lo = Math.min(a, b);
  const hi = Math.max(a, b);
  const label = lo === hi ? PHASES[lo].label : `${PHASES[lo].label}${hi - lo > 1 ? " to " : " or "}${PHASES[hi].label.toLowerCase()}`;
  const ends: (keyof Bounds)[] = ["on", "up", "peak", "off", "after"];
  const what = ["Effects usually begin", "The peak usually arrives", "The peak usually ends", "Felt effects usually fade", "After-effects can last until"];
  let detail = "";
  if (lo < 5) {
    const key = ends[lo];
    detail = `${what[lo]} ${clock(lead.at + fast[key] * HOUR)}–${clock(lead.at + slow[key] * HOUR)}${multi ? `, going by the ${clock(lead.at)} dose.` : "."}`;
  }
  if (lo >= 4 && s.roa?.half_life) detail += ` Half-life ${s.roa.half_life}, so it still counts for combinations.`;
  if (multi && (s.redose === "blunted" || s.redose === "partial")) detail += ` ${REDOSE_NOTE[s.redose].split(". ")[0]}.`;
  return { label, tone: PHASES[lo].tone, detail: detail.trim() };
}

/** The tier the line reaches, in words for the row: "strong range combined". */
export function tierText(s: Series, now: number): string {
  if (s.kind === "drinks") return "drinks scale";
  const t = taken(s, now);
  if (!s.tiers || !s.roa) return t.some((d) => d.dose.unit.trim().toLowerCase() === "hit") ? "hits: no tier" : "no tier";
  const sum = t.reduce((a, d) => a + (d.amount ?? 0), 0);
  const tier = tierOf(s.roa, sum);
  return tier ? `${tier} range${t.length > 1 ? " combined" : ""}` : "no tier";
}

/** What the key's tooltip says about a line. */
export function aboutLines(s: Series): string[] {
  const out: string[] = [];
  const list = s.doses.map((d) => `${d.dose.amount ?? "?"} ${d.dose.unit} at ${clock(d.at)}`).join(", ");
  out.push(s.doses.length > 1 ? `Logged ${list}. Each dose is a tick on the time axis, and they add into one line.` : `Logged ${list}.`);
  if (s.kind === "drinks") {
    out.push("The height is standard drinks still being processed, on its own scale at the right. Drinks aren't compared with the reference's alcohol ranges, which mix UK units and drinks.");
    out.push(
      "Clearing is drawn at about one standard drink an hour, a common rule of thumb. Many people clear more slowly: the average rate measured in blood works out nearer half a drink an hour for a 70 kg person, and less for smaller bodies. Food and liver health change it too.",
    );
  } else if (s.tiers && s.roa) {
    const sum = s.doses.reduce((a, d) => a + (d.amount ?? 0), 0);
    const tier = tierOf(s.roa, sum);
    out.push(`${s.doses.length > 1 ? "Combined, that's" : "That's"} ${+sum.toFixed(3)} ${s.unit}${tier ? `, in the ${tier} range` : ""} for ${s.roa.name.toLowerCase()}. The height follows the dose still counting against the reference's tiers.`);
  } else {
    out.push("The amount can't be placed on the reference's tiers (a hit, a count with no amount each, or no ranges for this route), so this line shows timing only, dashed.");
  }
  if (s.redose) out.push(REDOSE_NOTE[s.redose]);
  if (s.kind === "shape" && s.stages.total) out.push(`Tapping this shows the timing band, from the fastest to the slowest timings the reference gives. Felt effects usually last ${spanText(s.stages.total)} a dose.`);
  if (s.kind === "block") out.push(`The reference gives only a total duration (${spanText(s.stages.total!)}), so no curve is drawn.`);
  if (s.kind === "marker") out.push("The reference gives no timings for this. It stays on the chart until the trip report ends.");
  if (s.stages.after) out.push(`After-effects can last ${spanText(s.stages.after)}.`);
  if (s.roa?.half_life) out.push(`Half-life ${s.roa.half_life}.`);
  return out;
}

/** The window to draw: from just before the first dose to an hour past the
 *  slowest end of felt effects (at least an hour past now), at most 36 hours. */
export function windowOf(series: Series[], now: number): [number, number] {
  const firsts = series.flatMap((s) => s.doses.map((d) => d.at));
  if (!firsts.length) return [now - HOUR, now + HOUR];
  const start = Math.min(...firsts) - 15 * 60_000;
  let end = now + HOUR;
  for (const s of series) {
    const last = taken(s, now).at(-1) ?? s.doses[0];
    let felt = 0;
    if (s.kind === "shape") felt = bounds(s.stages, 1).off;
    else if (s.kind === "block") felt = s.stages.total![1];
    else if (s.kind === "drinks") felt = ((drinksClearAt(s, now) ?? last.at) - last.at) / HOUR;
    end = Math.max(end, last.at + (felt + 1) * HOUR);
  }
  return [start, Math.min(end, start + 36 * HOUR)];
}

// ---- what's recent enough to still be active

/** How far back to look for doses that may still be active: the longest
 *  after-effects the reference gives for common substances (LSD, alcohol). */
export const RECENT_HOURS = 48;

/** The trip reports whose doses may still be active at `now`: live, or started or
 *  ended within `RECENT_HOURS`. Plain notes have no doses. Newest first, at most
 *  `limit`, so a busy weekend doesn't mean dozens of lookups. */
export function recentEntries<E extends Pick<Experience, "id" | "kind" | "started_at" | "ended_at">>(entries: E[], now: number, limit = 10): E[] {
  const since = now - RECENT_HOURS * HOUR;
  const t = (iso: string | null) => (iso ? Date.parse(iso) : NaN);
  return entries
    .filter((e) => e.kind === "session" && (e.ended_at == null || t(e.started_at) >= since || t(e.ended_at) >= since))
    .sort((a, b) => t(b.started_at) - t(a.started_at))
    .slice(0, limit);
}
