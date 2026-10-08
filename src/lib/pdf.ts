// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// One experience as a PDF, to share with someone: the trip report people send.
//
// Written by hand rather than with a library, because it has to work everywhere
// Field Notes runs (the desktop app, a phone's browser, a phone that's offline)
// and the whole job is a few pages of text. It uses the PDF standard fonts
// Helvetica and Helvetica-Bold, which every PDF reader has, so nothing is
// embedded. They cover Western European text (WinAnsi); anything outside that,
// such as other scripts, is written as "?", and emoji are left out. Font widths are the standard
// Adobe metrics, as published in @pdf-lib/standard-fonts (MIT).
//
// Pure: data in, bytes out. The pages save or share the result.

import type { ExperienceDetail } from "./api";
import { describeAmount } from "./dosedetail.ts";

/** What goes in, beyond the title, times and doses (always included). */
export interface PdfParts {
  intention: boolean;
  moments: boolean;
  writeup: boolean;
  rating: boolean;
}

export const ALL_PARTS: PdfParts = { intention: true, moments: true, writeup: true, rating: true };

// Widths (thousandths of the font size) of WinAnsi codes 32–255.
const W_REG = [278,278,355,556,556,889,667,191,333,333,389,584,278,333,278,278,556,556,556,556,556,556,556,556,556,556,278,278,584,584,584,556,1015,667,667,722,722,667,611,778,722,278,500,667,556,833,722,778,667,778,722,667,611,722,667,944,667,667,611,278,278,278,469,556,333,556,556,500,556,556,278,556,556,222,222,500,222,833,556,556,556,556,333,500,278,556,500,722,500,500,500,334,260,334,584,0,556,0,222,556,333,1000,556,556,333,1000,667,333,1000,0,611,0,0,222,222,333,333,350,556,1000,333,1000,500,333,944,0,500,500,278,333,556,556,556,556,260,556,333,737,370,556,584,333,737,333,400,584,333,333,333,556,537,278,333,333,365,556,834,834,834,611,667,667,667,667,667,667,1000,722,667,667,667,667,278,278,278,278,722,722,778,778,778,778,778,584,778,722,722,722,722,667,667,611,556,556,556,556,556,556,889,500,556,556,556,556,278,278,278,278,556,556,556,556,556,556,556,584,611,556,556,556,556,500,556,500];
const W_BOLD = [278,333,474,556,556,889,722,238,333,333,389,584,278,333,278,278,556,556,556,556,556,556,556,556,556,556,333,333,584,584,584,611,975,722,722,722,722,667,611,778,722,278,556,722,611,833,722,778,667,778,722,667,611,722,667,944,667,667,611,333,278,333,584,556,333,556,611,556,611,556,333,611,611,278,278,556,278,889,611,611,611,611,389,556,333,611,556,778,556,556,500,389,280,389,584,0,556,0,278,556,500,1000,556,556,333,1000,667,333,1000,0,611,0,0,278,278,500,500,350,556,1000,333,1000,556,333,944,0,500,556,278,333,556,556,556,556,280,556,333,737,370,556,584,333,737,333,400,584,333,333,333,611,556,278,333,333,365,556,834,834,834,611,722,722,722,722,722,722,1000,722,667,667,667,667,278,278,278,278,722,722,778,778,778,778,778,584,778,722,722,722,722,667,667,611,556,556,556,556,556,556,889,556,556,556,556,556,278,278,278,278,611,611,611,611,611,611,611,584,611,611,611,611,611,556,611,556];
/** Unicode code points WinAnsi puts somewhere other than their own number. */
const WINANSI: Record<number, number> = {338: 140, 339: 156, 352: 138, 353: 154, 376: 159, 381: 142, 382: 158, 402: 131, 710: 136, 732: 152, 8211: 150, 8212: 151, 8216: 145, 8217: 146, 8218: 130, 8220: 147, 8221: 148, 8222: 132, 8224: 134, 8225: 135, 8226: 149, 8230: 133, 8240: 137, 8249: 139, 8250: 155, 8364: 128, 8482: 153};
/** Common characters WinAnsi lacks, written as something close. */
const NEAR: Record<string, string> = { "→": "->", "←": "<-", "≈": "~", "≤": "<=", "≥": ">=", "−": "-", "×": "x", "\u00a0": " ", "\u2009": " ", "\u202f": " " };

/** A character as a WinAnsi byte, or "?" (63). */
function code(ch: string): number {
  const cp = ch.codePointAt(0) ?? 63;
  if (cp >= 32 && cp < 127) return cp;
  if (cp >= 160 && cp <= 255) return cp;
  return WINANSI[cp] ?? 63;
}

function encode(text: string): number[] {
  const out: number[] = [];
  // Emoji (and the joiners and variation selectors that build them) are left out
  // rather than shown as "?", which would read as a question.
  const plain = text.normalize("NFC").replace(/[\p{Extended_Pictographic}\u200d\ufe0e\ufe0f]/gu, "").replace(/ +$/, "");
  for (const ch of plain) {
    const near = NEAR[ch];
    if (near) for (const c of near) out.push(code(c));
    else if (ch === "\t") out.push(32);
    else out.push(code(ch));
  }
  return out;
}

function width(bytes: number[], size: number, bold: boolean): number {
  const w = bold ? W_BOLD : W_REG;
  let sum = 0;
  for (const b of bytes) sum += w[b - 32] ?? 556;
  return (sum * size) / 1000;
}

// ---------- layout ----------

const PAGE_W = 612; // US Letter
const PAGE_H = 792;
const MARGIN = 56;
const TEXT_W = PAGE_W - 2 * MARGIN;
const GRAY = "0.38 0.36 0.33";
const INK = "0.1 0.09 0.08";

interface Run {
  x: number;
  y: number;
  size: number;
  bold: boolean;
  gray: boolean;
  bytes: number[];
}

class Layout {
  pages: Run[][] = [[]];
  y = PAGE_H - MARGIN;

  private get page() {
    return this.pages[this.pages.length - 1];
  }

  space(pt: number) {
    this.y -= pt;
  }

  /** Make room for `pt` more, starting a page if it doesn't fit. */
  need(pt: number) {
    if (this.y - pt < MARGIN + 24) {
      this.pages.push([]);
      this.y = PAGE_H - MARGIN;
    }
  }

  /** Wrapped text from `indent`, `width` wide. */
  text(s: string, o: { size?: number; bold?: boolean; gray?: boolean; indent?: number; width?: number; lead?: number } = {}) {
    const size = o.size ?? 10.5;
    const bold = o.bold ?? false;
    const indent = o.indent ?? 0;
    const max = o.width ?? TEXT_W - indent;
    const lead = o.lead ?? size * 1.45;
    for (const para of s.split("\n")) {
      const lines = wrap(encode(para), size, bold, max);
      for (const line of lines) {
        this.need(lead);
        this.y -= lead;
        this.page.push({ x: MARGIN + indent, y: this.y, size, bold, gray: !!o.gray, bytes: line });
      }
    }
  }

  /** A label in a left column and wrapped text beside it, on one baseline. */
  row(label: string, s: string, o: { bold?: boolean; labelW?: number; gray?: boolean } = {}) {
    const size = 10.5;
    const lead = size * 1.45;
    const labelW = o.labelW ?? 104;
    const lines = wrap(encode(s), size, o.bold ?? false, TEXT_W - labelW);
    this.need(lead * Math.min(lines.length, 3));
    lines.forEach((line, i) => {
      this.need(lead);
      this.y -= lead;
      if (i === 0) this.page.push({ x: MARGIN, y: this.y, size: 9.5, bold: false, gray: true, bytes: encode(label) });
      this.page.push({ x: MARGIN + labelW, y: this.y, size, bold: o.bold ?? false, gray: !!o.gray, bytes: line });
    });
  }

  heading(s: string) {
    this.need(44);
    this.space(14);
    this.text(s.toUpperCase(), { size: 9, bold: true, gray: true, lead: 14 });
    this.space(2);
  }
}

/** Greedy word wrap on measured widths; a word too long for a line is broken. */
function wrap(bytes: number[], size: number, bold: boolean, max: number): number[][] {
  if (!bytes.length) return [[]];
  const words: number[][] = [];
  let cur: number[] = [];
  for (const b of bytes) {
    if (b === 32) {
      words.push(cur);
      cur = [];
    } else cur.push(b);
  }
  words.push(cur);
  const lines: number[][] = [];
  let line: number[] = [];
  for (let word of words) {
    const tryLine = line.length ? [...line, 32, ...word] : word;
    if (width(tryLine, size, bold) <= max) {
      line = tryLine;
      continue;
    }
    if (line.length) lines.push(line);
    while (width(word, size, bold) > max) {
      let n = word.length - 1;
      while (n > 1 && width(word.slice(0, n), size, bold) > max) n--;
      lines.push(word.slice(0, n));
      word = word.slice(n);
    }
    line = word;
  }
  lines.push(line);
  return lines;
}

// ---------- the report ----------

const two = (n: number) => String(n).padStart(2, "0");

function clock(iso: string): string {
  const d = new Date(iso);
  return isNaN(d.getTime()) ? "" : d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit", hour12: true });
}

function day(iso: string): string {
  const d = new Date(iso);
  return isNaN(d.getTime())
    ? ""
    : d.toLocaleDateString([], { weekday: "short", day: "numeric", month: "short", year: "numeric" });
}

function span(min: number): string {
  const m = Math.max(0, Math.round(min));
  const h = Math.floor(m / 60);
  return h ? `${h}h ${two(m % 60)}m` : `${m}m`;
}

/** "T+1:35" from the first dose (or the start), or "T-0:20" before it. */
function offset(iso: string, from: number): string {
  const t = Date.parse(iso);
  if (isNaN(t) || isNaN(from)) return "";
  const m = Math.round((t - from) / 60000);
  const a = Math.abs(m);
  return `T${m < 0 ? "-" : "+"}${Math.floor(a / 60)}:${two(a % 60)}`;
}

function doseLine(d: ExperienceDetail["doses"][number]): string {
  // "3 g fresh (about 0.3 g dried)", "2 × 00 caps, 0.45 g each (0.9 g)".
  const amount = d.amount != null ? describeAmount(d.substance_name, d.amount, d.unit, d).trim() : "";
  return [d.substance_name, amount, d.route].filter(Boolean).join(" · ");
}

export function pdfFilename(e: ExperienceDetail): string {
  const d = new Date(e.started_at);
  const date = isNaN(d.getTime()) ? "" : `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())} `;
  const title = (e.title || "Experience").replace(/[\\/:*?"<>|]+/g, " ").replace(/\s+/g, " ").trim().slice(0, 60);
  return `${date}${title}.pdf`;
}

export function experiencePdf(e: ExperienceDetail, parts: PdfParts = ALL_PARTS, now = new Date()): Uint8Array {
  const L = new Layout();
  const isSession = e.kind === "session";
  L.text(e.title || (isSession ? "Experience" : "Journal note"), { size: 20, bold: true, lead: 26 });

  const start = e.started_at;
  const end = e.ended_at;
  let when = `${day(start)}, ${clock(start)}`;
  if (end) {
    const sameDay = day(end) === day(start);
    when += ` to ${sameDay ? "" : `${day(end)}, `}${clock(end)}`;
    const len = (Date.parse(end) - Date.parse(start)) / 60000;
    if (len > 0) when += ` (${span(len)})`;
  }
  L.space(2);
  L.text(when, { gray: true });
  if (parts.rating && e.rating != null) L.text(`Rating: ${e.rating}/10`, { gray: true });

  if (parts.intention && (e.intention.trim() || e.setting.trim())) {
    L.heading("Set and setting");
    if (e.intention.trim()) L.row("Intention", e.intention.trim());
    if (e.setting.trim()) L.row("Setting", e.setting.trim());
  }

  if (isSession && (e.doses.length || (parts.moments && e.timeline.length))) {
    const firstDose = e.doses.map((d) => Date.parse(d.taken_at)).filter((t) => !isNaN(t)).sort((a, b) => a - b)[0];
    const from = firstDose ?? Date.parse(start);
    type Item = { at: string; dose?: ExperienceDetail["doses"][number]; moment?: ExperienceDetail["timeline"][number] };
    const items: Item[] = [
      ...e.doses.map((d) => ({ at: d.taken_at, dose: d })),
      ...(parts.moments ? e.timeline.map((m) => ({ at: m.at, moment: m })) : []),
    ].sort((a, b) => Date.parse(a.at) - Date.parse(b.at));
    L.heading(parts.moments && e.timeline.length ? "Timeline" : "Doses");
    // A trip that runs past midnight gets a line for each new day.
    const multiDay = new Set(items.map((i) => day(i.at))).size > 1;
    let lastDay = "";
    for (const it of items) {
      if (multiDay && day(it.at) !== lastDay) {
        lastDay = day(it.at);
        L.space(4);
        L.text(lastDay, { size: 9, bold: true, gray: true, lead: 13 });
      }
      const label = `${clock(it.at)}  ${offset(it.at, from)}`;
      if (it.dose) {
        L.row(label, doseLine(it.dose), { bold: true });
        if (it.dose.note.trim()) L.row("", it.dose.note.trim(), { gray: true });
      } else if (it.moment) {
        const m = it.moment;
        const extra = [m.mood, m.intensity != null ? `intensity ${m.intensity}/10` : ""].filter(Boolean).join(", ");
        L.row(label, [m.note.trim(), extra ? `(${extra})` : ""].filter(Boolean).join(" "));
      }
    }
  }

  if (parts.writeup && e.notes.trim()) {
    L.heading(isSession ? "Write-up" : "Note");
    for (const para of e.notes.trim().split(/\n\s*\n/)) {
      L.text(para.trim(), { lead: 15.5 });
      L.space(6);
    }
  }

  return render(L.pages, e.title || "Experience", now);
}

// ---------- the file ----------

function pdfString(bytes: number[]): string {
  let s = "(";
  for (const b of bytes) {
    if (b === 40 || b === 41 || b === 92) s += "\\" + String.fromCharCode(b);
    else if (b < 32 || b > 126) s += "\\" + b.toString(8).padStart(3, "0");
    else s += String.fromCharCode(b);
  }
  return s + ")";
}

function render(pages: Run[][], title: string, now: Date): Uint8Array {
  const objs: string[] = [];
  const add = (body: string) => {
    objs.push(body);
    return objs.length;
  };
  const catalog = add("");
  const pagesId = add("");
  const reg = add("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>");
  const bold = add("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>");
  const kids: number[] = [];
  const stamp = `Field Notes · ${now.toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" })}`;
  pages.forEach((runs, i) => {
    const footer: Run[] = [
      { x: MARGIN, y: MARGIN - 18, size: 8.5, bold: false, gray: true, bytes: encode(stamp) },
    ];
    const pageNo = encode(`${i + 1} of ${pages.length}`);
    footer.push({ x: PAGE_W - MARGIN - width(pageNo, 8.5, false), y: MARGIN - 18, size: 8.5, bold: false, gray: true, bytes: pageNo });
    let ops = "";
    for (const r of [...runs, ...footer]) {
      if (!r.bytes.length) continue;
      ops += `BT /${r.bold ? "F2" : "F1"} ${r.size} Tf ${r.gray ? GRAY : INK} rg ${r.x.toFixed(2)} ${r.y.toFixed(2)} Td ${pdfString(r.bytes)} Tj ET\n`;
    }
    const content = add(`<< /Length ${ops.length} >>\nstream\n${ops}endstream`);
    kids.push(
      add(
        `<< /Type /Page /Parent ${pagesId} 0 R /MediaBox [0 0 ${PAGE_W} ${PAGE_H}] ` +
          `/Resources << /Font << /F1 ${reg} 0 R /F2 ${bold} 0 R >> >> /Contents ${content} 0 R >>`,
      ),
    );
  });
  objs[catalog - 1] = `<< /Type /Catalog /Pages ${pagesId} 0 R >>`;
  objs[pagesId - 1] = `<< /Type /Pages /Kids [${kids.map((k) => `${k} 0 R`).join(" ")}] /Count ${kids.length} >>`;
  const info = add(`<< /Title ${pdfString(encode(title))} /Producer (Field Notes) >>`);

  // Every string above is one byte per character (WinAnsi), so length = bytes.
  let out = "%PDF-1.4\n%\xe2\xe3\xcf\xd3\n";
  const offsets: number[] = [];
  objs.forEach((body, i) => {
    offsets.push(out.length);
    out += `${i + 1} 0 obj\n${body}\nendobj\n`;
  });
  const xref = out.length;
  out += `xref\n0 ${objs.length + 1}\n0000000000 65535 f \n`;
  for (const o of offsets) out += `${String(o).padStart(10, "0")} 00000 n \n`;
  out += `trailer\n<< /Size ${objs.length + 1} /Root ${catalog} 0 R /Info ${info} 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  const bytes = new Uint8Array(out.length);
  for (let i = 0; i < out.length; i++) bytes[i] = out.charCodeAt(i) & 0xff;
  return bytes;
}
