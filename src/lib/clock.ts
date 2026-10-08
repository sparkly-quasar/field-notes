// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// How times are shown: a 12-hour clock ("9:30 pm") by default, or 24-hour
// ("21:30") when someone picks that in Settings. Plain TypeScript so the tests can
// import the formatters; clock.svelte.ts keeps the per-device choice and points
// `read` at it, which makes every caller rendering a time redraw when it changes.

let read: () => boolean = () => false;

/** Where the 24-hour choice lives. Called once, by clock.svelte.ts. */
export function readClock24From(f: () => boolean) {
  read = f;
}

/** Options for toLocaleTimeString/toLocaleString: hour and minute (or just the hour). */
export function timeOpts(minutes = true): Intl.DateTimeFormatOptions {
  return minutes ? { hour: "numeric", minute: "2-digit", hour12: !read() } : { hour: "numeric", hour12: !read() };
}

/** "9:30 pm", or "21:30" on the 24-hour clock. */
export function clockTime(t: Date | number | string): string {
  return new Date(t).toLocaleTimeString(undefined, timeOpts());
}

/** "9 pm", or "21", for the hour ticks on a time axis. */
export function clockHour(t: Date | number | string): string {
  return new Date(t).toLocaleTimeString(undefined, timeOpts(false));
}

/** Whether times are on the 24-hour clock, for Intl options that set their own fields. */
export function clock24(): boolean {
  return read();
}
