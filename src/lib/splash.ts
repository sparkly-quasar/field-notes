// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// The loading splash (src/app.html) stays up until the first screen has its
// data, not just its frame, so Today doesn't open empty and fill in piece by
// piece. Anything the first screen waits on holds it; when the last hold lets
// go, the splash fades. app.html also gives up on its own after a while, so a
// hold that's never let go can't leave it up for good.

let holds = 0;
let released = false;

/** Hold the splash up. Call what this returns once loaded; extra calls do nothing. */
export function holdSplash(): () => void {
  if (released || typeof window === "undefined") return () => {};
  holds++;
  let done = false;
  return () => {
    if (done) return;
    done = true;
    holds--;
    // Wait a beat: what one load just put on screen may take a hold of its own
    // (the chart, once the doses are in).
    if (holds === 0) setTimeout(release, 50);
  };
}

function release() {
  if (released || holds > 0) return;
  released = true;
  window.dispatchEvent(new Event("fieldnotes:ready"));
}
