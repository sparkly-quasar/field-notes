// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// The 12- or 24-hour clock, chosen per device in Settings and kept in this browser
// only, like discreet mode. 12-hour unless someone switches it.

import { readClock24From } from "./clock.ts";

const KEY = "fieldnotes.clock24";

function load(): boolean {
  try {
    return localStorage.getItem(KEY) === "1";
  } catch {
    return false;
  }
}

export const clockPref = $state({ h24: load() });
readClock24From(() => clockPref.h24);

export function setClock24(on: boolean) {
  clockPref.h24 = on;
  try {
    localStorage.setItem(KEY, on ? "1" : "0");
  } catch {
    // Not remembered on this device; it still applies until the page closes.
  }
}
