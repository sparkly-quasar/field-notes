// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// Discreet mode: one switch, every screen. Substance names become stable
// stand-ins ("Substance K7") wherever a list or summary shows them, so the app
// can be open on a train or shared on a call. It is a per-device preference, kept
// in this browser only, and it never changes anything stored in the journal.

const KEY = "fieldnotes.discreet";

function load(): boolean {
  try {
    return localStorage.getItem(KEY) === "1";
  } catch {
    return false;
  }
}

/** `available`: offered at all (Settings, on the computer that holds the
 *  journal). `on`: hiding names right now, on this device. */
export const discreet = $state({ on: load(), available: false });

export function setDiscreet(on: boolean) {
  discreet.on = on;
  try {
    localStorage.setItem(KEY, on ? "1" : "0");
  } catch {
    // Not remembered on this device; it still applies until the page closes.
  }
}

/** Hiding names right now: switched on here, and offered at all. */
export function hiding(): boolean {
  return discreet.available && discreet.on;
}

/** A stand-in that is the same for a name everywhere and every day, so "K7" on
 *  the journal is "K7" in Stats. Derived from the name, never stored. */
export function aliasOf(name: string): string {
  const s = name.trim().toLowerCase();
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  h >>>= 0;
  const letter = "ABCDEFGHJKLMNPQRSTUVWXYZ"[h % 24];
  return `Substance ${letter}${Math.floor(h / 24) % 10}`;
}

/** The name to show: real, or its stand-in when discreet mode is on. */
export function shown(name: string): string {
  return discreet.available && discreet.on && name.trim() ? aliasOf(name) : name;
}
