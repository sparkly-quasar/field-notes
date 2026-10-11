// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
/**
 * The warnings worth showing the moment a dose is saved: the ones about *that
 * dose*. Both checks behind a save (the entry's own, and the wider one across
 * nearby entries) answer for every pair they can see, so logging a benzo at
 * midnight would otherwise repeat "Caffeine + LSD" from the afternoon. That pair
 * was shown when the second of them was logged; showing it again under the benzo
 * buries the one warning that's new, and teaches people to stop reading them.
 *
 * Kept: any pair with the dose in it, and a dose profile's note about the dose's
 * own amount (no second name). Names are compared as logged, which is what the
 * checker reports them as.
 */

export interface PairWarning {
  a: string;
  b: string;
}

export function aboutDose<W extends PairWarning>(warnings: W[], substance: string): W[] {
  const s = substance.trim().toLowerCase();
  if (!s) return warnings;
  const is = (n: string) => n.trim().toLowerCase() === s;
  return warnings.filter((w) => is(w.a) || (w.b ? is(w.b) : false));
}
