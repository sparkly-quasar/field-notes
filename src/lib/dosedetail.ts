// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
/**
 * What a dose was, beyond amount and unit: mushroom and kratom forms, how much
 * each capsule holds, and an edible's estimate (`DoseDetail` in db.rs).
 *
 * The rules here are the ones Stats uses (`measure` in stats.rs), so a dose reads
 * the same in the form, the journal and the charts:
 *
 * - Fresh mushrooms are about a tenth of their weight dried. Always "about":
 *   water content varies. Truffles hold less water, so theirs isn't converted.
 * - Capsules (pills, tabs, pieces) with an amount each are that weight in all.
 * - An edible is its estimate, in grams of dried mushroom or mg of psilocybin,
 *   never converted between the two. Without one, its weight says nothing.
 * - Kratom extract and 7-OH are far stronger per gram than leaf, so they're
 *   never read against leaf's ranges.
 *
 * Imports nothing but types, so `npm test` runs it directly.
 */
import type { DoseDetail } from "./api";

/** Units that count things, which can say how much each one holds ("00 caps, 0.45 g"). */
export const COUNTED_UNITS = ["capsule", "pill", "tab", "piece"];

export interface FormChoice {
  value: string;
  label: string;
}

/**
 * The forms a dose can take, by substance (owner's decisions, 2026-10-05). An
 * empty form means "not said": mushrooms read as dried, kratom as leaf. A Rust
 * test (stats.rs) checks these names are the ones Stats understands.
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

/**
 * The form a written dose names ("2g fresh shrooms", "15mg 7-OH"), when it's one
 * this substance has; "" when it names none. Tea isn't "edible": "2g in tea" is
 * 2 g of mushrooms, brewed. Chocolate and gummies are, since their weight isn't
 * the mushroom's.
 */
export function formIn(substance: string, text: string): string {
  const offered = formsFor(substance).map((f) => f.value);
  const t = text.toLowerCase();
  const said = [
    ["7-oh", /\b7[\s-]?oh\b|7-hydroxy/],
    ["extract", /\bextracts?\b/],
    ["leaf", /\bleaf\b/],
    ["fresh", /\bfresh\b/],
    ["dried", /\bdried\b|\bdry weight\b/],
    ["powdered", /\bpowder(ed)?\b|\bground\b/],
    ["edible", /\bedibles?\b|\bchoc(olate)?s?\b|\bgumm(y|ies)\b|\bcandy\b/],
  ] as const;
  for (const [form, re] of said) if (offered.includes(form) && re.test(t)) return form;
  return "";
}

/** Words that name a form, as they'd sit in a substance's name ("Fresh shrooms"). */
const FORM_WORDS = /\b(fresh|dried|powder(ed)?|edibles?|extracts?|leaf|7[\s-]?oh)\b/gi;

/**
 * A pasted dose's substance and form, with the form's word taken out of the
 * name: "Fresh shrooms" is shrooms, fresh; "Kratom extract" is kratom, extract.
 */
export function withForm(substance: string, text: string): { substance: string; form: string } {
  const bare = substance.replace(FORM_WORDS, " ").replace(/\s+/g, " ").trim();
  const name = bare && formsFor(bare).length ? bare : substance;
  const form = formIn(name, text);
  if (!form) return { substance, form: "" };
  return { substance: name === substance ? substance : name.charAt(0).toUpperCase() + name.slice(1), form };
}

/** Psilocybin mushrooms, which have a fresh-to-dried estimate. Matches
 *  `is_mushroom` in stats.rs. */
export function isMushroom(substance: string): boolean {
  const s = substance.trim().toLowerCase();
  return !s.includes("truffle") && /mushroom|shroom|mushies/.test(s);
}

/** Fresh psilocybin mushrooms are about 90% water: `FRESH_TO_DRIED` in stats.rs. */
export const FRESH_TO_DRIED = 10;

/** What an edible's estimate can be in. */
export const ESTIMATE_UNITS = [
  { value: "g", label: "g of dried mushroom" },
  { value: "mg psilocybin", label: "mg of psilocybin" },
];

const MICROGRAMS: Record<string, number> = { "µg": 1, mg: 1_000, g: 1_000_000 };

/** A unit's one spelling, for units of mass: `canon_unit` in stats.rs. */
function massUnit(u: string): string | null {
  const s = u.trim().toLowerCase();
  if (["ug", "mcg", "μg", "µg", "microgram", "micrograms"].includes(s)) return "µg";
  if (["mg", "milligram", "milligrams"].includes(s)) return "mg";
  if (["g", "gram", "grams"].includes(s)) return "g";
  return null;
}

export const isMassUnit = (u: string) => massUnit(u) != null;

/** `amount` of `from` in `to`, when both are units of mass. */
export function inUnit(amount: number, from: string, to: string): number | null {
  const f = massUnit(from);
  const t = massUnit(to);
  if (!f || !t) return from.trim().toLowerCase() === to.trim().toLowerCase() ? amount : null;
  return round(amount * (MICROGRAMS[f] / MICROGRAMS[t]));
}

const round = (v: number) => (v === 0 || !Number.isFinite(v) ? v : +v.toPrecision(12));
const fmt = (v: number) => String(+v.toFixed(3));

export interface Measured {
  amount: number;
  unit: string;
  /** An estimate: say "about". */
  approx: boolean;
}

/**
 * The amount to compare with dose ranges, or null when there's nothing honest
 * to compare: a count with no amount each, an edible without an estimate, mg of
 * psilocybin (the reference has no psilocybin entry), kratom extract or 7-OH,
 * fresh truffles.
 */
export function measure(
  substance: string,
  amount: number | null,
  unit: string,
  d: DoseDetail = {},
): Measured | null {
  if (amount == null || !Number.isFinite(amount)) {
    if (d.form === "edible" && d.estimate != null && isMassUnit(d.estimate_unit ?? "")) {
      return { amount: d.estimate, unit: massUnit(d.estimate_unit!)!, approx: true };
    }
    return null;
  }
  const form = (d.form ?? "").trim().toLowerCase();
  if (form === "7-oh" || form === "extract") return null;
  if (form === "edible") {
    if (d.estimate != null && isMassUnit(d.estimate_unit ?? "")) {
      return { amount: d.estimate, unit: massUnit(d.estimate_unit!)!, approx: true };
    }
    return null;
  }
  if (!isMassUnit(unit) && d.per_unit != null && isMassUnit(d.per_unit_unit ?? "")) {
    return { amount: round(amount * d.per_unit), unit: massUnit(d.per_unit_unit!)!, approx: false };
  }
  if (form === "fresh") {
    if (!isMushroom(substance) || !isMassUnit(unit)) return null;
    const g = inUnit(amount, unit, "g");
    return g == null ? null : { amount: round(g / FRESH_TO_DRIED), unit: "g", approx: true };
  }
  return isMassUnit(unit) ? { amount, unit: massUnit(unit)!, approx: false } : null;
}

/** A dose's amount as the journal shows it: what was written, then what it
 *  works out to when that differs. "3 g fresh (about 0.3 g dried)". */
export function describeAmount(
  substance: string,
  amount: number | null,
  unit: string,
  d: DoseDetail = {},
): string {
  const form = (d.form ?? "").trim().toLowerCase();
  const found = [...FORMS.mushrooms, ...FORMS.kratom].find((f) => f.value === form)?.label;
  const formLabel = found && found.charAt(0).toLowerCase() + found.slice(1);
  const n = amount == null ? "?" : fmt(amount);
  const label = (d.unit_label ?? "").trim();
  const each = d.per_unit != null && !isMassUnit(unit);
  let text = label && !isMassUnit(unit) ? `${n} × ${label}` : `${n} ${unit}`;
  if (each) text += `, ${fmt(d.per_unit!)} ${d.per_unit_unit} each`;
  // Leaf and dried are what a plain weight already means; only say the others.
  if (formLabel && form !== "dried" && form !== "leaf") text += `${each ? "," : ""} ${formLabel}`;
  if (form === "edible") {
    if (d.estimate != null && d.estimate_unit) {
      const of = d.estimate_unit === "g" ? "g dried mushroom" : d.estimate_unit;
      text += ` (about ${fmt(d.estimate)} ${of})`;
    }
    return text;
  }
  const m = measure(substance, amount, unit, d);
  if (m && (m.approx || massUnit(unit) == null)) {
    text += ` (${m.approx ? "about " : ""}${fmt(m.amount)} ${m.unit}${form === "fresh" ? " dried" : ""})`;
  }
  return text;
}

/** The detail fields of a dose (or anything carrying them), as `DoseDetail`. */
export function detailOf(d: DoseDetail): DoseDetail {
  return {
    form: d.form ?? "",
    per_unit: d.per_unit ?? null,
    per_unit_unit: d.per_unit_unit ?? "",
    unit_label: d.unit_label ?? "",
    estimate: d.estimate ?? null,
    estimate_unit: d.estimate_unit ?? "",
  };
}

/**
 * Microdose cutoffs (owner-approved 2026-10-05): at or under these, a
 * psychedelic dose is a microdose. `MICRODOSE` in stats.rs is the same list; a
 * Rust test checks. Mushrooms are compared as dried (fresh by its estimate).
 */
export const MICRODOSE: [string, number, string][] = [
  ["lsd", 20, "µg"],
  ["1p-lsd", 20, "µg"],
  ["1cp-lsd", 20, "µg"],
  ["ald-52", 20, "µg"],
  ["al-lad", 30, "µg"],
  ["psilocybin mushrooms", 0.3, "g"],
  ["psilocybin", 3, "mg"],
  ["4-aco-dmt", 3, "mg"],
  ["4-ho-met", 2, "mg"],
  ["mescaline", 30, "mg"],
  ["2c-b", 3, "mg"],
];

/** Never called a microdose: `never_micro` in stats.rs. */
const NEVER_MICRO = ["lsa", "morning glory", "hawaiian baby woodrose", "hbwr",
  "doc", "dob", "doi", "dom", "doet", "dopr", "dox"];

/**
 * The microdose cutoff for a substance, or null if it isn't microdosed: from
 * the table, else a psychedelic's threshold in the reference's unit.
 */
export function microCutoff(
  substance: string,
  psychedelic: boolean,
  threshold: number | null,
  thresholdUnit: string,
): { amount: number; unit: string } | null {
  const s = substance.trim().toLowerCase();
  if (NEVER_MICRO.includes(s) || s.includes("nbome")) return null;
  const k = isMushroom(s) ? "psilocybin mushrooms" : s;
  const row = MICRODOSE.find((r) => r[0] === k);
  if (row) return { amount: row[1], unit: row[2] };
  return psychedelic && threshold != null ? { amount: threshold, unit: thresholdUnit } : null;
}
