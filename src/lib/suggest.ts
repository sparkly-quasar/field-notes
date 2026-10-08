// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// Which substance names the substance box suggests for what's typed
// (SubstanceInput.svelte). Kept apart from the component so `npm test` reaches it.

export type Name = { name: string; aliases: string[]; mine: boolean };

export type Suggestion = { name: string; via: string | null };

/** Best few matches for what's typed: the name itself, then start of a name,
 *  then start of a word or street name, then anywhere. Your own substances
 *  first within each. A name typed in full stays listed while longer names
 *  share its start ("3-MeO-PCP" beside "3-MeO-PCPr"); alone, it needs no menu. */
export function suggest(all: Name[], typed: string, limit = 6): Suggestion[] {
  const q = typed.trim().toLowerCase();
  if (!q) return [];
  const scored: { s: Suggestion; score: number }[] = [];
  for (const n of all) {
    const name = n.name.toLowerCase();
    let score = -1;
    let via: string | null = null;
    if (name === q) score = 0;
    else if (name.startsWith(q)) score = 1;
    else if (name.split(/[\s\-(/]+/).some((w) => w.startsWith(q))) score = 2;
    else {
      const a = n.aliases.find((a) => a.toLowerCase().startsWith(q));
      if (a && q.length >= 2) {
        score = 3;
        via = a;
      } else if (q.length >= 3 && name.includes(q)) score = 4;
    }
    if (score < 0) continue;
    scored.push({ s: { name: n.name, via }, score: score * 2 + (n.mine ? 0 : 1) });
  }
  scored.sort((a, b) => a.score - b.score || a.s.name.length - b.s.name.length || a.s.name.localeCompare(b.s.name));
  if (scored.length === 1 && scored[0].s.name.toLowerCase() === q) return []; // already typed in full
  return scored.slice(0, limit).map((x) => x.s);
}
