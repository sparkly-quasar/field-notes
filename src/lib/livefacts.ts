// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// The facts a live session needs at a glance. "When did I last take it?" is the
// question that matters most before a redose, so both the desktop and the phone
// answer it the same way.

import type { Dose } from "./api";

/** The most recent dose, optionally of one substance (case-insensitive). */
export function lastDose(doses: Dose[], substance?: string): Dose | null {
  const want = substance?.trim().toLowerCase();
  let best: Dose | null = null;
  for (const d of doses) {
    if (want && d.substance_name.trim().toLowerCase() !== want) continue;
    if (!best || Date.parse(d.taken_at) > Date.parse(best.taken_at)) best = d;
  }
  return best;
}

/** "8m", "1h 12m", "2d 3h": the gap between two moments, rounded down. */
export function span(fromMs: number, toMs: number): string {
  const mins = Math.max(0, Math.floor((toMs - fromMs) / 60_000));
  if (mins < 60) return `${mins}m`;
  const h = Math.floor(mins / 60);
  if (h < 48) return `${h}h ${String(mins % 60).padStart(2, "0")}m`;
  return `${Math.floor(h / 24)}d ${h % 24}h`;
}
