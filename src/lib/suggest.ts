// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// Which substance names the substance box suggests for what's typed
// (SubstanceInput.svelte). Kept apart from the component so `npm test` reaches it.

export type Name = { name: string; aliases: string[]; mine: boolean };

export type Suggestion = { name: string; via: string | null };

/** Only the letters and digits, lowercase, the way `squash` in names.rs reads a
 *  name: "3-MeO-PCP" and "3meopcp" are both "3meopcp". */
const squash = (s: string) => s.toLowerCase().replace(/[^\p{L}\p{N}]/gu, "");

/** Best few matches for what's typed: the name itself, then start of a name,
 *  then start of a word or street name, then anywhere. Your own substances
 *  first within each. Hyphens and spaces don't matter ("3meopcp" finds
 *  3-MeO-PCP). A name typed in full stays listed, first, even when nothing
 *  else matches, so it's plain the reference has it. */
export function suggest(all: Name[], typed: string, limit = 6): Suggestion[] {
  const q = typed.trim().toLowerCase();
  const sq = squash(q);
  if (!sq) return [];
  const scored: { s: Suggestion; score: number }[] = [];
  for (const n of all) {
    const name = n.name.toLowerCase();
    const sname = squash(name);
    let score = -1;
    let via: string | null = null;
    if (name === q || sname === sq) score = 0;
    else if (name.startsWith(q) || sname.startsWith(sq)) score = 1;
    else if (name.split(/[\s\-(/]+/).some((w) => w.startsWith(q))) score = 2;
    else {
      const a = n.aliases.find((a) => a.toLowerCase().startsWith(q) || squash(a).startsWith(sq));
      if (a && q.length >= 2) {
        score = 3;
        via = a;
      } else if (q.length >= 3 && (name.includes(q) || sname.includes(sq))) score = 4;
    }
    if (score < 0) continue;
    scored.push({ s: { name: n.name, via }, score: score * 2 + (n.mine ? 0 : 1) });
  }
  scored.sort((a, b) => a.score - b.score || a.s.name.length - b.s.name.length || a.s.name.localeCompare(b.s.name));
  return scored.slice(0, limit).map((x) => x.s);
}
