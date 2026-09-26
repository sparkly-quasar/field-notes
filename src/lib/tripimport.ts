// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
/**
 * Turning a trip log written somewhere else — a notes app, a message to a friend,
 * a sitter's scrawl — into an entry: doses and moments, correctly timed, so the
 * journal shows them on the usual T+ timeline.
 *
 * **Deterministic on purpose.** The desktop's text import asks a local model; this
 * is for the phone, mid-morning-after, where there may be no model and there must
 * be no guessing you can't see. Every line becomes exactly one visible row in a
 * preview, which the person corrects before anything is saved.
 *
 * Pure: no API calls, no DOM. The caller passes the substance catalogue in.
 *
 * Handles, per line:
 * - clock times: `8:43am`, `8:43 am`, `8.43pm`, `11am`, `20:15`, `[20:15]`, `(08:43)`
 * - offsets: `T+1:30`, `t+90m`, `+1h30`, `+45m`, `1h30 in`, `90 min in`
 * - separators after the time: `-`, `–`, `—`, `:`, `|`, `>`, or just a space
 * - lines with no time: continue the row above (or start the log, untimed)
 * - `(6/10)` / `6/10` anywhere: the moment's intensity
 */

export type Unit = "mg" | "µg" | "g" | "ml" | "tab";

export interface ParsedRow {
  kind: "dose" | "moment";
  /** Minutes after the first timed line — the T+ the journal will show. */
  offsetMin: number;
  /** Local wall-clock time as written (for the preview), when the line had one. */
  clock: string | null;
  text: string;
  substance: string;
  amount: number | null;
  unit: Unit;
  route: string;
  intensity: number | null;
}

export interface ParsedLog {
  rows: ParsedRow[];
  /** "clock" when lines carried times of day — the date is still needed. "offset"
   *  when they were T+ offsets — the start time is needed. "none" when nothing was
   *  timed, so rows fall at the start. */
  timing: "clock" | "offset" | "none";
  /** Minutes after midnight of the first timed line, for clock logs. */
  startClockMin: number | null;
}

export interface CatalogueEntry {
  name: string;
  aliases?: string[];
}

const UNIT_RE = /(\d+(?:[.,]\d+)?)\s*(mg|mgs|milligrams?|µg|ug|mcg|micrograms?|g|grams?|gr|ml|mls|tabs?|tablets?|pills?|caps?|capsules?)\b/i;

function normUnit(u: string): Unit {
  const s = u.toLowerCase();
  if (/^(µg|ug|mcg|micrograms?)$/.test(s)) return "µg";
  if (/^(mg|mgs|milligrams?)$/.test(s)) return "mg";
  if (/^(g|grams?|gr)$/.test(s)) return "g";
  if (/^(ml|mls)$/.test(s)) return "ml";
  return "tab";
}

function routeOf(text: string): string {
  const t = text.toLowerCase();
  if (/\b(snort|snorted|insufflat|nasal|bump|line)/.test(t)) return "insufflated";
  if (/\b(smok|vap|vape|vaped|joint|bong|dab)/.test(t)) return "vaporized";
  if (/\b(sublingual|under (my|the) tongue|sl\b|buccal)/.test(t)) return "sublingual";
  if (/\b(rectal|boof|plug)/.test(t)) return "rectal";
  if (/\b(inject|iv\b|intravenous)/.test(t)) return "IV";
  if (/\b(im\b|intramuscular)/.test(t)) return "IM";
  return "oral";
}

/** Words that sit around a dose without naming the substance. */
const FILLER = new Set([
  "took", "take", "taking", "dropped", "drop", "ate", "had", "have", "dosed", "dose", "redose", "redosed",
  "re-dose", "re-dosed", "another", "more", "of", "a", "an", "the", "some", "my", "first", "second", "third",
  "booster", "boost", "top", "up", "topped", "around", "about", "approx", "approximately", "~", "roughly", "and",
  "then", "i", "we", "orally", "oral", "snorted", "smoked", "vaped", "sublingual", "insufflated", "plus", "+",
]);

/** A catalogue name for what was written: exact name or alias first, then a unique
 *  prefix ("mesc" → Mescaline). Falls back to the words as written, capitalised. */
export function resolveSubstance(words: string, catalogue: CatalogueEntry[]): string {
  const w = words.trim().toLowerCase();
  if (!w) return "";
  for (const c of catalogue) {
    if (c.name.toLowerCase() === w || (c.aliases ?? []).some((a) => a.toLowerCase() === w)) return c.name;
  }
  if (w.length >= 3) {
    // A unique main name first ("mesc" → Mescaline), street names only after —
    // otherwise an obscure alias ("Mescalamphetamine", for TMA) makes it ambiguous.
    const byName = [...new Set(catalogue.filter((c) => c.name.toLowerCase().startsWith(w)).map((c) => c.name))];
    if (byName.length === 1) return byName[0];
    if (byName.length === 0) {
      const byAlias = [
        ...new Set(catalogue.filter((c) => (c.aliases ?? []).some((a) => a.toLowerCase().startsWith(w))).map((c) => c.name)),
      ];
      if (byAlias.length === 1) return byAlias[0];
    }
  }
  return words.trim().replace(/^./, (ch) => ch.toUpperCase());
}

/** Street names that are also ordinary words. In a sentence they're far more likely
 *  to mean the word ("pot of tea", "a few beans"), so they only count when they're
 *  the whole substance field ("2 tabs", "1g pot"), never when found mid-sentence. */
const AMBIGUOUS = new Set([
  "pot", "herb", "bud", "purple", "jet", "adam", "emma", "beans", "rolls", "lucy", "cactus", "buttons",
  "kitty", "tabs", "blotter", "rock", "ice", "snow", "crystal", "glass", "candy", "dust", "speed", "tina",
  "grass", "hash", "cat", "mandy", "boomers", "cid", "special k", "vitamin k", "kit kat",
]);

/** A known substance mentioned anywhere in a line ("took another 20mg of the ketamine"
 *  finds Ketamine). Word n-grams against a name index; the longest match wins. */
function findKnown(text: string, index: Map<string, string>): string | null {
  const words = text.toLowerCase().replace(/[^a-z0-9µ\-' ]+/g, " ").split(/\s+/).filter(Boolean);
  let best: { name: string; len: number } | null = null;
  for (let n = Math.min(4, words.length); n >= 1; n--) {
    for (let i = 0; i + n <= words.length; i++) {
      const phrase = words.slice(i, i + n).join(" ");
      if (phrase.length < 3 || AMBIGUOUS.has(phrase)) continue;
      const hit = index.get(phrase);
      if (hit && (!best || phrase.length > best.len)) best = { name: hit, len: phrase.length };
    }
    if (best) return best.name;
  }
  return null;
}

/** Lowercase name / alias → canonical name. */
export function nameIndex(catalogue: CatalogueEntry[]): Map<string, string> {
  const idx = new Map<string, string>();
  for (const c of catalogue) {
    for (const n of [c.name, ...(c.aliases ?? [])]) {
      const k = n.trim().toLowerCase();
      if (k && !idx.has(k)) idx.set(k, c.name);
    }
  }
  return idx;
}

type Stamp = { kind: "clock"; min: number; label: string } | { kind: "offset"; min: number };

/** Pull a leading time off a line. Returns the stamp and the rest of the line. */
export function takeStamp(line: string): { stamp: Stamp | null; rest: string } {
  let s = line.trim().replace(/^[-*•·>]\s*/, "");
  const strip = (rest: string) => rest.replace(/^\s*[-–—:|>)\]]*\s*/, "").trim();

  // T+1:30, t+90m, +1h30, +45m, T+2h
  let m = s.match(/^[[(]?\s*t?\s*\+\s*(\d+)\s*(?::|h|hr|hrs)\s*(\d{1,2})?\s*(?:m|min|mins)?\s*[\])]?/i);
  if (m && (/[:h]/i.test(m[0]) || m[2])) {
    return { stamp: { kind: "offset", min: Number(m[1]) * 60 + Number(m[2] ?? 0) }, rest: strip(s.slice(m[0].length)) };
  }
  m = s.match(/^[[(]?\s*t?\s*\+\s*(\d+)\s*(m|min|mins|minutes)\b\s*[\])]?/i);
  if (m) return { stamp: { kind: "offset", min: Number(m[1]) }, rest: strip(s.slice(m[0].length)) };
  // "1h30 in", "90 min in", "2 hours in"
  m = s.match(/^(\d+)\s*(h|hr|hrs|hours?)\s*(\d{1,2})?\s*(m|min|mins)?\s+in\b/i);
  if (m) return { stamp: { kind: "offset", min: Number(m[1]) * 60 + Number(m[3] ?? 0) }, rest: strip(s.slice(m[0].length)) };
  m = s.match(/^(\d+)\s*(m|min|mins|minutes)\s+in\b/i);
  if (m) return { stamp: { kind: "offset", min: Number(m[1]) }, rest: strip(s.slice(m[0].length)) };

  // Clock: 8:43am, 8.43 pm, 20:15, [20:15], 11am, 11 pm
  m = s.match(/^[[(]?\s*(\d{1,2})(?:[:.](\d{2}))?\s*(am|pm|a\.m\.|p\.m\.)?\s*[\])]?/i);
  if (m && (m[2] !== undefined || m[3])) {
    let h = Number(m[1]);
    const min = Number(m[2] ?? 0);
    const ap = m[3]?.toLowerCase().replace(/\./g, "");
    if (h > 23 || min > 59) return { stamp: null, rest: s };
    if (ap === "pm" && h < 12) h += 12;
    if (ap === "am" && h === 12) h = 0;
    // A bare "8" isn't a time; "8:43" is. And "35mg" must never read as 35 o'clock.
    const after = s.slice(m[0].length);
    if (!ap && /^\s*(mg|µg|ug|mcg|g|ml|tab|x\b)/i.test(after)) return { stamp: null, rest: s };
    const label = `${String(h).padStart(2, "0")}:${String(min).padStart(2, "0")}`;
    return { stamp: { kind: "clock", min: h * 60 + min, label }, rest: strip(after) };
  }
  return { stamp: null, rest: s };
}

/** Is this line a dose? If so, what, how much, how. */
function doseOf(text: string, catalogue: CatalogueEntry[], index: Map<string, string>) {
  const m = text.match(UNIT_RE);
  if (!m) return null;
  const amount = Number(m[1].replace(",", "."));
  const unit = normUnit(m[2]);
  const around = (text.slice(0, m.index) + " " + text.slice(m.index! + m[0].length))
    .replace(/[(),.;!]/g, " ")
    .split(/\s+/)
    .filter((w) => w && !FILLER.has(w.toLowerCase()));
  // A substance we know, named anywhere in the line, wins: "took 20mg of the
  // ketamine in the bathroom" is Ketamine, not "Ketamine in bathroom".
  // (The amount and unit are taken out first, so "2 tabs" can't read as a name.)
  const known = findKnown(text.slice(0, m.index) + " " + text.slice(m.index! + m[0].length), index);
  if (known) return { substance: known, amount, unit, route: routeOf(text) };
  // Otherwise a short line names it: "35mg mesc", "2C-B 15mg", "2 tabs of acid".
  if (around.length >= 1 && around.length <= 3) {
    return { substance: resolveSubstance(around.join(" "), catalogue), amount, unit, route: routeOf(text) };
  }
  // Just an amount ("2 tabs", "another 50mg"): a dose, with the substance left for
  // the person to name in the preview — saving refuses a nameless dose.
  if (around.length === 0) return { substance: "", amount, unit, route: routeOf(text) };
  // A sentence with an amount in it and nothing we recognise: leave it as a moment
  // for the person to promote in the preview, rather than invent a substance.
  return null;
}

function intensityOf(text: string): { intensity: number | null; text: string } {
  const m = text.match(/\(?\b(10|[0-9])\s*\/\s*10\b\)?/);
  if (!m) return { intensity: null, text };
  return { intensity: Number(m[1]), text: text.replace(m[0], "").replace(/\s{2,}/g, " ").trim() };
}

export function parseTripLog(raw: string, catalogue: CatalogueEntry[]): ParsedLog {
  const index = nameIndex(catalogue);
  const rows: ParsedRow[] = [];
  let timing: ParsedLog["timing"] = "none";
  let firstClock: number | null = null;
  let lastClockAbs: number | null = null; // minutes, rolled across midnight
  let lastOffset = 0;

  for (const line of raw.split(/\r?\n/)) {
    if (!line.trim()) continue;
    const { stamp, rest } = takeStamp(line);

    if (!stamp) {
      // No time: it continues the line above, however it was written.
      const prev = rows.at(-1);
      if (prev) {
        prev.text = `${prev.text}${prev.text ? " " : ""}${rest}`.trim();
        continue;
      }
    }

    let offsetMin = lastOffset;
    let clock: string | null = null;
    if (stamp?.kind === "clock") {
      if (timing === "none") timing = "clock";
      let abs = stamp.min;
      // Earlier than the line before? It's after midnight.
      while (lastClockAbs != null && abs < lastClockAbs) abs += 1440;
      if (firstClock == null) firstClock = abs;
      lastClockAbs = abs;
      offsetMin = abs - firstClock;
      clock = stamp.label;
    } else if (stamp?.kind === "offset") {
      if (timing === "none") timing = "offset";
      offsetMin = stamp.min;
    }
    lastOffset = offsetMin;

    const { intensity, text } = intensityOf(rest);
    const dose = doseOf(text, catalogue, index);
    rows.push({
      kind: dose ? "dose" : "moment",
      offsetMin,
      clock,
      text,
      substance: dose?.substance ?? "",
      amount: dose?.amount ?? null,
      unit: dose?.unit ?? "mg",
      route: dose?.route ?? "oral",
      intensity,
    });
  }

  // T+ in the journal counts from the first *dose*. If the log opened with a note
  // ("feeling nervous"), offsets above are from that line, which is the start of the
  // session — the journal recomputes T+ from the first dose itself.
  return { rows, timing, startClockMin: firstClock == null ? null : firstClock % 1440 };
}
