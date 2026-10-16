// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
/**
 * Turning a trip log written somewhere else — a notes app, a message to a friend,
 * a sitter's scrawl — into an entry: doses and moments, correctly timed, so the
 * journal shows them on the usual T+ timeline.
 *
 * **Deterministic on purpose.** The desktop's text import asks a local model; this
 * is for the phone, mid-morning-after, where there may be no model and there must
 * be no guessing you can't see. Every line becomes at least one visible row in a
 * preview (a list of doses becomes one row each), which the person corrects
 * before anything is saved.
 *
 * Pure: no API calls, no DOM. The caller passes the substance catalogue in.
 *
 * Handles, per line:
 * - clock times: `8:43am`, `8:43 am`, `8.43pm`, `11am`, `20:15`, `[20:15]`, `(08:43)`,
 *   `11:30ish`, `~11:30`. A log written in 12-hour times without am/pm can't say
 *   whether it started in the morning or the evening: `ParsedLog.half` says so and
 *   the preview asks.
 * - offsets: `T+0`, `T+1:30`, `t+90m`, `+1h30`, `+45m`, `1h30 in`, `90 min in`
 * - separators after the time: `-`, `–`, `—`, `:`, `|`, `>`, or just a space
 * - lines with no time: continue the row above (or start the log, untimed). After
 *   the last timed line and a blank line, they're the write-up instead.
 * - lists: `150mg MDMA, zofran`, `150mg 4mmc + 2C-D` are a dose per item
 * - other people: `40mg DMT - Emily`, `Elle 40mg dmt`, `2C-D (11 Elle 6 Emily)`
 *   tag the dose with whose it was; the preview asks which of them is you
 * - `(6/10)` / `6/10` anywhere: the moment's intensity
 *
 * And, before the first timed line, a line that's just a date (`Date: 02-28-2026
 * 12:30pm`, `Saturday, Feb 28 2026`, `2026-02-28`): it becomes the log's date
 * instead of a row.
 */

import { withForm } from "./dosedetail.ts";

export type Unit = "mg" | "µg" | "g" | "ml" | "tab" | "capsule" | "pill" | "drink" | "hit";

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
  /** Whose dose it was, when the log names someone (`- Emily`). Null: the writer's. */
  who: string | null;
  /** Where the route came from: the line itself, an earlier dose of the same
   *  substance in this log, the substance's usual route (`RARELY_ORAL`), or
   *  nowhere (null: "oral", by default). The preview puts the person's own usual
   *  route ("mine", from quick log) in place of the last two. */
  routeFrom: "written" | "log" | "typical" | "mine" | null;
  /** The form the line names, for a substance that has forms ("2g fresh
   *  shrooms"): `withForm` in dosedetail.ts. "" when it names none. */
  form: string;
}

export interface ParsedLog {
  rows: ParsedRow[];
  /** "clock" when lines carried times of day — the date is still needed. "offset"
   *  when they were T+ offsets — the start time is needed. "none" when nothing was
   *  timed, so rows fall at the start. */
  timing: "clock" | "offset" | "none";
  /** Minutes after midnight of the first timed line, for clock logs. When `half`
   *  is set this is the morning reading: add 720 for the evening. */
  startClockMin: number | null;
  /** Set when every time was 12-hour with no am/pm, so the log can't say whether it
   *  started in the morning or the evening. `guess` comes from a time on the date
   *  line, when there was one; otherwise null and the person has to say. */
  half: { guess: "am" | "pm" | null } | null;
  /** The people the log names as taking doses (see `ParsedRow.who`), in order. */
  people: string[];
  /** Untimed paragraphs after the last timed line: the write-up, not the timeline. */
  reflection: string;
  /** The day the log says it's from (yyyy-mm-dd), from a date line at the top. */
  date: string | null;
  /** A time of day written with that date (minutes after midnight), if any. */
  dateClockMin: number | null;
}

export interface CatalogueEntry {
  name: string;
  aliases?: string[];
}

const UNIT_RE = /(\d+(?:[.,]\d+)?|\ban?|\bone)\s*(mg|mgs|milligrams?|µg|ug|mcg|micrograms?|g|grams?|gr|ml|mls|tabs?|tablets?|pills?|caps?|capsules?|drinks?|beers?|shots?|glass(?:es)? of wine|wines?|hits?|puffs?|tokes?)\b/i;

/** Drinks counted by what they were: "2 beers" and "a shot of tequila" are alcohol. */
const DRINK_WORD = /^(beers?|shots?|glass(?:es)? of wine|wines?)$/i;
const ALCOHOLS = new Set(["alcohol", "ethanol", "booze", "beer", "wine", "cider", "vodka", "whiskey", "whisky", "rum", "gin", "tequila", "mezcal", "liquor", "spirits", "sake", "champagne"]);

function normUnit(u: string): Unit {
  const s = u.toLowerCase();
  if (/^(µg|ug|mcg|micrograms?)$/.test(s)) return "µg";
  if (/^(mg|mgs|milligrams?)$/.test(s)) return "mg";
  if (/^(g|grams?|gr)$/.test(s)) return "g";
  if (/^(ml|mls)$/.test(s)) return "ml";
  if (/^(caps?|capsules?)$/.test(s)) return "capsule";
  if (/^pills?$/.test(s)) return "pill";
  if (/^(drinks?|beers?|shots?|glass(es)? of wine|wines?)$/.test(s)) return "drink";
  if (/^(hits?|puffs?|tokes?)$/.test(s)) return "hit";
  return "tab";
}

function routeOf(text: string): string {
  return writtenRoute(text) ?? "oral";
}

/** The route a line actually says, if it says one. */
function writtenRoute(text: string): string | null {
  const t = text.toLowerCase();
  if (/\b(snort|snorted|insufflat|nasal|bump|line)/.test(t)) return "insufflated";
  if (/\b(smok|vap|vape|vaped|joint|bong|dab)/.test(t)) return "vaporized";
  if (/\b(sublingual|under (my|the) tongue|sl\b|buccal)/.test(t)) return "sublingual";
  if (/\b(rectal|boof|plug)/.test(t)) return "rectal";
  if (/\b(inject|iv\b|intravenous)/.test(t)) return "IV";
  if (/\b(im\b|intramuscular)/.test(t)) return "IM";
  if (/\b(oral|orally|swallow|swallowed|drank|drink|ate|eaten)\b/.test(t)) return "oral";
  return null;
}

/** Rarely swallowed, so a dose of one with no route written isn't read as oral.
 *  `maoi`: swallowed with one (ayahuasca, pharmahuasca), so a log that mentions an
 *  MAOI leaves it oral. */
const RARELY_ORAL: Record<string, { route: string; maoi?: boolean }> = {
  dmt: { route: "vaporized", maoi: true },
  "5-meo-dmt": { route: "vaporized", maoi: true },
  salvia: { route: "vaporized" },
  ketamine: { route: "insufflated" },
};
const MAOI_RE =
  /\b(maoi|rima|harmal\w*|harmine|harmaline|syrian rue|rue|peganum|caapi|banisteriopsis|ayahuasca|pharmahuasca|yage|yaj[eé]|moclobemide|phenelzine|nardil|tranylcypromine|parnate|selegiline|isocarboxazid|marplan)\b/i;

/** Words that sit around a dose without naming the substance. */
const FILLER = new Set([
  "took", "take", "taking", "dropped", "drop", "ate", "had", "have", "dosed", "dose", "redose", "redosed",
  "re-dose", "re-dosed", "another", "more", "of", "a", "an", "the", "some", "my", "first", "second", "third",
  "booster", "boost", "top", "up", "topped", "around", "about", "approx", "approximately", "~", "roughly", "and",
  "then", "i", "we", "orally", "oral", "snorted", "smoked", "vaped", "sublingual", "insufflated", "plus", "+",
  // How it was taken goes in the route, never the name ("75mg 4mmc rectal").
  "rectal", "rectally", "boof", "boofed", "plug", "plugged", "im", "intramuscular", "intramuscularly", "iv",
  "intravenous", "intravenously", "inject", "injected", "sublingually", "buccal", "nasal", "nasally", "snort",
  "smoke", "vape", "vaporized", "vaporised", "insufflate",
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
  const words = text
    .toLowerCase()
    .replace(/(?<!\d),|,(?!\d)/g, " ") // a comma inside "1,4b" is part of the name
    .replace(/[^a-z0-9µ\-', ]+/g, " ")
    .split(/\s+/)
    .filter(Boolean);
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

/** `ap`: written with am/pm. `bare12`: a 12-hour time without ("5:30"), so `min`
 *  is its morning reading. */
type Stamp = { kind: "clock"; min: number; label: string; ap: boolean; bare12: boolean } | { kind: "offset"; min: number };

/** Pull a leading time off a line. Returns the stamp and the rest of the line. */
export function takeStamp(line: string): { stamp: Stamp | null; rest: string } {
  let s = line.trim().replace(/^[-*•·>]\s*/, "");
  const strip = (rest: string) => rest.replace(/^\s*[-–—:|>)\]]*\s*/, "").trim();

  // T+1:30, t+90m, +1h30, +45m, T+2h
  let m = s.match(/^[[(]?\s*t?\s*\+\s*(\d+)\s*(?::|h|hr|hrs)\s*(\d{1,2})?\s*(?:m|min|mins)?\s*[\])]?/i);
  if (m && (/[:h]/i.test(m[0]) || m[2])) {
    return { stamp: { kind: "offset", min: Number(m[1]) * 60 + Number(m[2] ?? 0) }, rest: strip(s.slice(m[0].length)) };
  }
  m = s.match(/^[[(]?\s*t?\s*\+\s*(\d+)\s*(m|min|mins|minutes)\b\s*[\])]?/i) ?? s.match(/^[[(]?\s*t\s*\+\s*(0)\b\s*[\])]?/i);
  if (m) return { stamp: { kind: "offset", min: Number(m[1]) }, rest: strip(s.slice(m[0].length)) };
  // "1h30 in", "90 min in", "2 hours in"
  m = s.match(/^(\d+)\s*(h|hr|hrs|hours?)\s*(\d{1,2})?\s*(m|min|mins)?\s+in\b/i);
  if (m) return { stamp: { kind: "offset", min: Number(m[1]) * 60 + Number(m[3] ?? 0) }, rest: strip(s.slice(m[0].length)) };
  m = s.match(/^(\d+)\s*(m|min|mins|minutes)\s+in\b/i);
  if (m) return { stamp: { kind: "offset", min: Number(m[1]) }, rest: strip(s.slice(m[0].length)) };

  // Clock: 8:43am, 8.43 pm, 20:15, [20:15], 11am, 11 pm, 11:30ish, ~11:30, 11ish
  m = s.match(/^[[(]?\s*~?\s*(\d{1,2})(?:[:.](\d{2}))?\s*(am|pm|a\.m\.|p\.m\.)?(\s*ish\b)?\s*(am|pm|a\.m\.|p\.m\.)?\s*[\])]?/i);
  if (m && (m[2] !== undefined || m[3] || m[4] || m[5])) {
    const written = Number(m[1]);
    let h = written;
    const min = Number(m[2] ?? 0);
    const ap = (m[3] ?? m[5])?.toLowerCase().replace(/\./g, "");
    if (h > 23 || min > 59) return { stamp: null, rest: s };
    if (ap && (h < 1 || h > 12)) return { stamp: null, rest: s };
    if (ap === "pm" && h < 12) h += 12;
    if (ap === "am" && h === 12) h = 0;
    // A bare "8" isn't a time; "8:43" is. And "35mg" must never read as 35 o'clock.
    const after = s.slice(m[0].length);
    if (!ap && /^\s*(mg|µg|ug|mcg|g|ml|tab|cap|pill|x\b)/i.test(after)) return { stamp: null, rest: s };
    const label = `${String(h).padStart(2, "0")}:${String(min).padStart(2, "0")}`;
    // "5:30" could be either half of the day; "05:30", "17:30" and "5:30pm" can't.
    const bare12 = !ap && written >= 1 && written <= 12 && !m[1].startsWith("0");
    return { stamp: { kind: "clock", min: h * 60 + min, label, ap: !!ap, bare12 }, rest: strip(after) };
  }
  return { stamp: null, rest: s };
}

/** Is this line a dose? If so, what, how much, how. */
function doseOf(text: string, catalogue: CatalogueEntry[], index: Map<string, string>) {
  const m = text.match(UNIT_RE);
  if (!m) return null;
  // "a"/"one" only counts for things counted whole: "a beer", "one hit", not "a mg".
  const word = /^(an?|one)$/i.test(m[1]);
  if (word && !/^(drinks?|beers?|shots?|glass|wines?|hits?|puffs?|tokes?)/i.test(m[2])) return null;
  const amount = word ? 1 : Number(m[1].replace(",", "."));
  const unit = normUnit(m[2]);
  const drink = DRINK_WORD.test(m[2]) || unit === "drink";
  const around = (text.slice(0, m.index) + " " + text.slice(m.index! + m[0].length))
    .replace(/[().;!]|(?<!\d),|,(?!\d)/g, " ")
    .split(/\s+/)
    .filter((w) => w && !FILLER.has(w.toLowerCase()));
  // A substance we know, named anywhere in the line, wins: "took 20mg of the
  // ketamine in the bathroom" is Ketamine, not "Ketamine in bathroom".
  // (The amount and unit are taken out first, so "2 tabs" can't read as a name.)
  const known = findKnown(text.slice(0, m.index) + " " + text.slice(m.index! + m[0].length), index);
  // "a shot of tequila", "2 beers": the drink is alcohol, whatever it was poured as.
  if (drink && (!known || ALCOHOLS.has(known.toLowerCase())) && around.every((w) => ALCOHOLS.has(w.toLowerCase()))) {
    return { substance: resolveSubstance("alcohol", catalogue), amount, unit, route: "oral" };
  }
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

/** Decode the runs of %XX escapes in a string, leaving anything invalid as it was. */
function decodeRuns(text: string): string {
  return text.replace(/(%[0-9A-Fa-f]{2})+/g, (run) => {
    try {
      return decodeURIComponent(run);
    } catch {
      return run;
    }
  });
}

/**
 * Undo percent-encoding that came along with a paste. Some apps and share sheets
 * copy text as it appeared in a link ("I%20don%E2%80%99t%20want"), which no one can
 * read and nothing below can parse. Often only part of a paste is like that: a log
 * written normally with one encoded paragraph stuck on the end. So this works a
 * word at a time (a "word" being a run with no real whitespace): a word is decoded
 * when it holds an encoded space or line break, or two or more escapes. A log that
 * merely mentions "50%" or "100%!" is left alone.
 */
export function decodePasted(text: string): string {
  return text.replace(/\S+/g, (word) => {
    const escapes = word.match(/%[0-9A-Fa-f]{2}/g)?.length ?? 0;
    if (escapes === 0) return word;
    if (escapes < 2 && !/%(20|0A|0D|09)/i.test(word)) return word;
    return decodeRuns(word);
  });
}

const MONTHS = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
const MONTH_RE = "(jan|feb|mar|apr|may|jun|jul|aug|sep|sept|oct|nov|dec)[a-z]*\\.?";
const WEEKDAY_RE = /^(mon|tue|tues|wed|thu|thur|thurs|fri|sat|sun)[a-z]*\.?,?\s+/i;

function validYmd(y: number, m: number, d: number): string | null {
  if (y < 100) y += 2000;
  if (m < 1 || m > 12 || d < 1 || d > 31) return null;
  const dt = new Date(y, m - 1, d);
  if (dt.getMonth() !== m - 1) return null; // 31 Feb
  return `${y}-${String(m).padStart(2, "0")}-${String(d).padStart(2, "0")}`;
}

/**
 * A line that is only a date, perhaps labelled and perhaps with a time: `Date:
 * 02-28-2026 12:30pm`, `Sat 28 Feb 2026`, `February 28th, 2026`, `2026-02-28`.
 * Anything else on the line and it isn't a date line, so a sentence that happens to
 * mention a date stays a sentence. `dayFirst` settles `03/04/2026` (4 March or
 * 3 April); when one number is over 12 the order is plain either way.
 */
export function takeDate(line: string, dayFirst = false): { date: string; clockMin: number | null } | null {
  let s = line
    .trim()
    .replace(/^[-*•·>#\s]+/, "")
    .replace(/^(trip\s+)?(date|day|when|dosed on|dosed|on)\s*[:\-–—]?\s*/i, "")
    .replace(WEEKDAY_RE, "");
  let date: string | null = null;
  let m: RegExpMatchArray | null;
  if ((m = s.match(/^(\d{4})[-/.](\d{1,2})[-/.](\d{1,2})\b/))) {
    date = validYmd(+m[1], +m[2], +m[3]);
  } else if ((m = s.match(/^(\d{1,2})[-/.](\d{1,2})[-/.](\d{4}|\d{2})\b/))) {
    const [a, b] = [+m[1], +m[2]];
    const dFirst = a > 12 ? true : b > 12 ? false : dayFirst;
    date = dFirst ? validYmd(+m[3], b, a) : validYmd(+m[3], a, b);
  } else if ((m = s.match(new RegExp(`^${MONTH_RE}\\s+(\\d{1,2})(?:st|nd|rd|th)?,?\\s+(\\d{4})\\b`, "i")))) {
    date = validYmd(+m[3], MONTHS.indexOf(m[1].slice(0, 3).toLowerCase()) + 1, +m[2]);
  } else if ((m = s.match(new RegExp(`^(\\d{1,2})(?:st|nd|rd|th)?\\s+(?:of\\s+)?${MONTH_RE},?\\s+(\\d{4})\\b`, "i")))) {
    date = validYmd(+m[3], MONTHS.indexOf(m[2].slice(0, 3).toLowerCase()) + 1, +m[1]);
  }
  if (!date || !m) return null;
  s = s.slice(m[0].length).replace(/^\s*(,|at|@|-|–|—)?\s*/i, "");
  if (!s) return { date, clockMin: null };
  const { stamp, rest } = takeStamp(s);
  if (stamp?.kind !== "clock" || rest.replace(/[\s.,;:!)\]-]/g, "")) return null;
  return { date, clockMin: stamp.min };
}


/** Words that make a list item a sentence ("150mg MDMA, feeling nervous"), so it's
 *  a note on the dose before it rather than a substance of its own. */
const SENTENCE = new Set([
  "i", "i'm", "im", "i've", "ive", "we", "we're", "me", "my", "us", "our", "you", "it", "it's", "its", "is", "was",
  "were", "are", "be", "been", "feel", "feeling", "feels", "felt", "so", "very", "really", "just", "still", "not",
  "no", "now", "like", "bit", "starting", "started", "kicking", "coming", "came", "getting", "got", "went", "going",
  "nothing", "yet", "much", "too", "but", "think", "maybe", "with", "for", "at", "in", "on", "to", "from",
]);

/** Capitalised words that sit where a name could but aren't one. */
const NOT_NAMES = new Set([
  "me", "myself", "us", "both", "everyone", "all", "them", "him", "her", "peak", "comeup", "come", "morning",
  "night", "evening", "afternoon", "total", "booster", "redose", "later", "again", "home", "bed", "sleep",
]);

const AMOUNT_UNITS = "mg|mgs|µg|ug|mcg|g|ml|mls";
/** "11 Elle", "6mg Emily" — a person's share of a dose, inside brackets. */
const PAIR_RE = new RegExp(`(\\d+(?:[.,]\\d+)?)\\s*(${AMOUNT_UNITS})?\\s+([A-Z][a-zA-Z'’]+)`, "g");

/** Could this capitalised word be a person, not a substance or a filler word? */
function nameLike(word: string, index: Map<string, string>): boolean {
  const w = word.toLowerCase();
  if (!/^[A-Z][a-zA-Z'’]{1,}$/.test(word)) return false;
  if (FILLER.has(w) || NOT_NAMES.has(w) || SENTENCE.has(w)) return false;
  return !index.has(w) || AMBIGUOUS.has(w);
}

/** A bracket of "amount Name" pairs, e.g. "(11 Elle 6 Emily)": the pairs, or null. */
function sharesIn(bracket: string, index: Map<string, string>): { amount: number; unit: Unit | null; who: string }[] | null {
  const inner = bracket.slice(1, -1);
  const pairs = [...inner.matchAll(PAIR_RE)];
  if (!pairs.length) return null;
  // Nothing in the bracket but the pairs and the odd separator.
  if (inner.replace(PAIR_RE, "").replace(/[\s,;&/+]|and/gi, "")) return null;
  if (!pairs.every((p) => nameLike(p[3], index))) return null;
  return pairs.map((p) => ({ amount: Number(p[1].replace(",", ".")), unit: p[2] ? normUnit(p[2]) : null, who: p[3] }));
}

/** Names the log tags doses with, in the order they first appear. Only from
 *  patterns that can't be much else: a trailing "- Name" on a dose line, and a
 *  bracket of shares. A leading "Elle 40mg dmt" counts only for a name found so. */
function peopleIn(lines: string[], index: Map<string, string>): string[] {
  const out: string[] = [];
  const add = (n: string) => out.includes(n) || out.push(n);
  for (const line of lines) {
    const dash = line.match(/\s[-–—]\s*([A-Z][a-zA-Z'’]+)\s*$/);
    if (dash && UNIT_RE.test(line) && nameLike(dash[1], index)) add(dash[1]);
    for (const b of line.match(/\([^()]*\)/g) ?? []) for (const s of sharesIn(b, index) ?? []) add(s.who);
  }
  return out;
}

/** Split on "+" and on commas, but not inside brackets or between digits ("1,4b"). */
function splitList(text: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let cur = "";
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (ch === "(" || ch === "[") depth++;
    if ((ch === ")" || ch === "]") && depth > 0) depth--;
    const digitComma = ch === "," && /\d/.test(text[i - 1] ?? "") && /\d/.test(text[i + 1] ?? "");
    const timePlus = ch === "+" && (!cur.trim() || /\bt\s*$/i.test(cur)); // "T+0", "+45m"
    if (depth === 0 && ((ch === "+" && !timePlus) || (ch === "," && !digitComma))) {
      parts.push(cur);
      cur = "";
    } else cur += ch;
  }
  parts.push(cur);
  return parts.map((p) => p.trim()).filter(Boolean);
}

type Body = Omit<ParsedRow, "offsetMin" | "clock">;

/** What a line says, apart from when: one row, or one per item of a list of doses. */
function rowsOf(line: string, catalogue: CatalogueEntry[], index: Map<string, string>, people: string[]): Body[] {
  const { intensity, text: full } = intensityOf(line);
  let text = full;
  let who: string | null = null;
  // "40mg IM DMT - Emily", "Elle 40mg dmt", "Emily: 40mg"
  const dash = text.match(/\s[-–—]\s*([A-Z][a-zA-Z'’]+)\s*$/);
  const lead = text.match(/^([A-Z][a-zA-Z'’]+)\s*[:,-]?\s+(?=[~\d])/);
  if (dash && people.includes(dash[1])) {
    who = dash[1];
    text = text.slice(0, dash.index).trim();
  } else if (lead && people.includes(lead[1])) {
    who = lead[1];
    text = text.slice(lead[0].length).trim();
  }
  const lineRoute = writtenRoute(full);
  const base = { intensity, who, routeFrom: null as ParsedRow["routeFrom"], form: "" };

  const one = (part: string, rowText: string): Body[] => {
    const said = writtenRoute(part) ?? lineRoute;
    const route = said ?? "oral";
    const routeFrom = said ? ("written" as const) : null;
    // "2C-D (11 Elle 6 Emily)": a dose each, for each of them.
    for (const b of part.match(/\([^()]*\)/g) ?? []) {
      const shares = sharesIn(b, index);
      if (!shares) continue;
      const rest = part.replace(b, " ");
      const named = doseOf(rest, catalogue, index)?.substance || resolveSubstance(words(rest).join(" "), catalogue);
      const lineUnit = full.match(UNIT_RE);
      const unit = lineUnit ? normUnit(lineUnit[2]) : "mg";
      const f = withForm(named, part);
      return shares.map((s) => ({ ...base, kind: "dose", text: rowText, substance: f.substance, amount: s.amount, unit: s.unit ?? unit, route, routeFrom, who: s.who, form: f.form }));
    }
    const dose = doseOf(part, catalogue, index);
    if (dose) return [{ ...base, kind: "dose", text: rowText, ...dose, ...withForm(dose.substance, part), route, routeFrom }];
    return [];
  };

  // A list of doses ("150mg MDMA, zofran", "150mg 4mmc + 2C-D") is a row per item,
  // but only when every item reads like a substance, not a sentence, and at least
  // one is plainly a dose (an amount, or a name we know).
  const items = splitList(text);
  if (items.length >= 2) {
    const listy = items.every((it) => {
      const ws = words(it.replace(/\([^()]*\)/g, " ").replace(UNIT_RE, " "));
      return ws.length >= 1 && ws.length <= 3 && !ws.some((w) => SENTENCE.has(w.toLowerCase()));
    });
    // Known means the whole item is the name: "alpha lipoic acid" is not LSD.
    const known = (it: string) => index.has(words(it.replace(/\([^()]*\)/g, " ")).join(" ").toLowerCase());
    const anchored = items.some((it) => UNIT_RE.test(it) || known(it));
    if (listy && anchored) {
      return items.flatMap((it) => {
        const got = one(it, it);
        if (got.length) return got;
        const name = resolveSubstance(words(it).join(" "), catalogue);
        return [{ ...base, kind: "dose" as const, text: it, substance: name, amount: null, unit: "mg" as Unit, route: lineRoute ?? "oral", routeFrom: lineRoute ? ("written" as const) : null }];
      });
    }
  }
  const got = one(text, text);
  if (got.length) return got;
  return [{ ...base, kind: "moment", text, substance: "", amount: null, unit: "mg", route: "oral" }];
}

/** The words of a list item that could be a name: no amounts, filler or brackets. */
function words(text: string): string[] {
  return text
    .replace(/[().;!]|(?<!\d),|,(?!\d)/g, " ")
    .split(/\s+/)
    .filter((w) => w && !FILLER.has(w.toLowerCase()));
}

/**
 * Where each clock time falls, in minutes from the midnight before the first one,
 * rolled forward across midnight. 24-hour logs roll a day when a time goes
 * backwards. A log in 12-hour times rolls half a day instead, so "11:30, 12:30,
 * 1:15" reads as an hour and three quarters, not thirteen hours; its bare times
 * are read as morning, and `pmStart` moves the first one to the evening.
 */
function placeClocks(stamps: Extract<Stamp, { kind: "clock" }>[], pmStart: boolean): number[] {
  const twelve = stamps.every((s) => s.bare12 || s.ap);
  const out: number[] = [];
  let last: number | null = null;
  for (const s of stamps) {
    let abs: number;
    if (twelve && s.bare12) {
      abs = (s.min % 720) + (last == null && pmStart ? 720 : 0);
      while (last != null && abs < last) abs += 720;
    } else {
      abs = s.min;
      while (last != null && abs < last) abs += 1440;
    }
    out.push(abs);
    last = abs;
  }
  return out;
}

export function parseTripLog(raw: string, catalogue: CatalogueEntry[], opts: { dayFirst?: boolean } = {}): ParsedLog {
  const index = nameIndex(catalogue);
  const lines = raw.split(/\r?\n/);
  const people = peopleIn(lines, index);
  let date: string | null = null;
  let dateClockMin: number | null = null;

  // First, what each line says and its stamp; the times are worked out after,
  // because whether "5:30" is morning or evening depends on the whole log.
  const entries: { stamp: Stamp | null; bodies: Body[] }[] = [];
  let blankSinceTimed = false;
  let tail: string[][] = []; // paragraphs after a blank line, not yet claimed
  const flushTail = () => {
    const last = entries.at(-1)?.bodies.at(-1);
    if (last && tail.length) last.text = [last.text, ...tail.flat()].join(" ").trim();
    tail = [];
  };

  for (const line of lines) {
    if (!line.trim()) {
      if (entries.length) blankSinceTimed = true;
      if (tail.at(-1)?.length) tail.push([]);
      continue;
    }
    // Before the first timed line, a date on its own line dates the log. (Checked
    // before takeStamp, which would read "12.02.2026" as 12:02.)
    if (date == null && !entries.some((e) => e.stamp)) {
      const d = takeDate(line, opts.dayFirst);
      if (d) {
        date = d.date;
        dateClockMin = d.clockMin;
        continue;
      }
    }
    const { stamp, rest } = takeStamp(line);

    if (!stamp) {
      const prev = entries.at(-1);
      if (prev) {
        const last = prev.bodies.at(-1)!;
        if (!last.text) {
          // The time was alone on its line ("12:30pm", then the words below it):
          // this is the line's real start, so read it as one.
          prev.bodies = rowsOf(rest, catalogue, index, people);
        } else if (blankSinceTimed && prev.stamp) {
          // Past a blank line after a timed one: maybe the write-up, maybe more
          // log to come. The next timed line decides.
          if (!tail.length) tail.push([]);
          tail.at(-1)!.push(rest);
        } else {
          last.text = `${last.text} ${rest}`.trim();
        }
        continue;
      }
    }
    flushTail();
    blankSinceTimed = false;
    entries.push({ stamp, bodies: stamp && !rest ? [{ kind: "moment", text: "", substance: "", amount: null, unit: "mg", route: "oral", intensity: null, who: null, routeFrom: null, form: "" }] : rowsOf(rest, catalogue, index, people) });
  }
  const reflection = tail
    .filter((p) => p.length)
    .map((p) => p.join("\n"))
    .join("\n\n");

  // Then the times.
  const first = entries.find((e) => e.stamp)?.stamp ?? null;
  const timing: ParsedLog["timing"] = first ? first.kind : "none";
  const clocks = entries.flatMap((e) => (e.stamp?.kind === "clock" ? [e.stamp] : []));
  let pmStart = false;
  let half: ParsedLog["half"] = null;
  if (clocks.length && clocks[0].bare12 && clocks.every((c) => c.bare12)) {
    // No am/pm anywhere: the times between lines are clear, the half of the day isn't.
    let guess: "am" | "pm" | null = null;
    if (dateClockMin != null) {
      const gap = (t: number) => Math.min(Math.abs(t - dateClockMin!), 1440 - Math.abs(t - dateClockMin!));
      guess = gap(clocks[0].min % 720 + 720) < gap(clocks[0].min % 720) ? "pm" : "am";
    }
    half = { guess };
  } else if (clocks.length && clocks[0].bare12 && clocks.some((c) => c.ap)) {
    // An am/pm further down settles it: whichever start makes the shorter night.
    const span = (pm: boolean) => {
      const p = placeClocks(clocks, pm);
      return p[p.length - 1] - p[0];
    };
    pmStart = span(true) <= span(false);
  }
  const placed = placeClocks(clocks, pmStart);
  const firstClock = placed[0] ?? null;

  const rows: ParsedRow[] = [];
  let lastOffset = 0;
  let ci = 0;
  for (const e of entries) {
    let offsetMin = lastOffset;
    let clock: string | null = null;
    if (e.stamp?.kind === "clock") {
      const abs = placed[ci++];
      offsetMin = abs - firstClock!;
      clock = `${String(Math.floor(abs / 60) % 24).padStart(2, "0")}:${String(abs % 60).padStart(2, "0")}`;
    } else if (e.stamp?.kind === "offset") {
      offsetMin = e.stamp.min;
    }
    lastOffset = offsetMin;
    for (const b of e.bodies) rows.push({ offsetMin, clock, ...b });
  }

  // A dose with no route written: the route of the same substance earlier in the
  // log ("12:30 40mg IM DMT", then "1:30 40mg dmt"), else its usual one.
  const maoi = MAOI_RE.test(raw);
  const seen = new Map<string, string>();
  for (const r of rows) {
    if (r.kind !== "dose" || !r.substance) continue;
    const key = r.substance.toLowerCase();
    if (r.routeFrom === "written") seen.set(key, r.route);
    else if (seen.has(key)) [r.route, r.routeFrom] = [seen.get(key)!, "log"];
    else if (RARELY_ORAL[key] && !(maoi && RARELY_ORAL[key].maoi)) [r.route, r.routeFrom] = [RARELY_ORAL[key].route, "typical"];
  }

  // T+ in the journal counts from the first *dose*. If the log opened with a note
  // ("feeling nervous"), offsets above are from that line, which is the start of the
  // session — the journal recomputes T+ from the first dose itself.
  return {
    rows,
    timing,
    startClockMin: firstClock == null ? null : firstClock % 1440,
    date,
    dateClockMin,
    half,
    people: people.filter((p) => rows.some((r) => r.who === p)),
    reflection,
  };
}
