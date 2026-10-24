// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// Keeps the phone page (/m) on the phone, so it opens without a connection.
// Registered from /m only, never in the desktop app (svelte.config.js turns off
// SvelteKit's automatic registration).
//
// - The app's own files (this build's scripts, styles, icons, fonts and the
//   offline checker) are saved on install and served from the phone.
// - Opening the page asks the computer first, so a new version arrives as soon
//   as it's reachable, and falls back to the saved page when it doesn't answer
//   within a few seconds.
// - Nothing from the journal passes through here: `/api/` is never cached.
/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />
/// <reference lib="webworker" />
/// <reference types="@sveltejs/kit" />
import { build, files, version } from "$service-worker";

const sw = self as unknown as ServiceWorkerGlobalScope;
const CACHE = `fieldnotes-app-${version}`;
const PAGE = "/m";
// Not the iOS launch images (static/splash/): iOS reads only the one for its own
// screen, once, when the app is added to the Home Screen.
const ASSETS = new Set([...build, ...files.filter((f) => !f.startsWith("/splash/"))]);
// How long opening the app waits on the computer before using the saved page.
// The same as offline.ts waits on "are you there?".
const PAGE_WAIT_MS = 4_000;

sw.addEventListener("install", (event) => {
  event.waitUntil(
    caches
      .open(CACHE)
      .then((c) => c.addAll([...ASSETS, PAGE]))
      .then(() => sw.skipWaiting()),
  );
});

sw.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      // Older versions of the app go; the reference files (offline.ts) stay.
      .then((keys) => Promise.all(keys.filter((k) => k.startsWith("fieldnotes-app-") && k !== CACHE).map((k) => caches.delete(k))))
      .then(() => sw.clients.claim()),
  );
});

sw.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.method !== "GET") return;
  const url = new URL(req.url);
  if (url.origin !== sw.location.origin || url.pathname.startsWith("/api/")) return;

  if (req.mode === "navigate") {
    const net = fetch(req).then((res) => {
      if (res.ok && url.pathname === PAGE) {
        const copy = res.clone();
        caches.open(CACHE).then((c) => c.put(PAGE, copy));
      }
      return res;
    });
    // Let a slow answer still refresh the saved page after the phone stopped waiting.
    event.waitUntil(net.catch(() => {}));
    event.respondWith(page(net));
    return;
  }
  if (ASSETS.has(url.pathname)) {
    event.respondWith(caches.match(url.pathname).then((hit) => hit ?? fetch(req)));
  }
});

/** The page: the computer's copy when it answers in time, the saved one when it
 *  doesn't. A computer that's off or asleep doesn't refuse the connection, it
 *  never answers, so without the wait the phone sat on its launch screen and then
 *  went white. A 5xx (the tailnet helper's "Field Notes isn't answering") also
 *  gets the saved page. */
async function page(net: Promise<Response>): Promise<Response> {
  const saved = await caches.match(PAGE);
  if (!saved) return net;
  const late = new Promise<undefined>((done) => setTimeout(done, PAGE_WAIT_MS));
  const res = await Promise.race([net.catch(() => undefined), late]);
  return res && res.status < 500 ? res : saved;
}
