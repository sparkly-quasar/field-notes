// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// Keeps the phone page (/m) on the phone, so it opens without a connection.
// Registered from /m only, never in the desktop app (svelte.config.js turns off
// SvelteKit's automatic registration).
//
// - The app's own files (this build's scripts, styles, icons, fonts and the
//   offline checker) are saved on install and served from the phone.
// - Opening the page asks the computer first, so a new version arrives as soon
//   as it's reachable, and falls back to the saved page when it isn't.
// - Nothing from the journal passes through here: `/api/` is never cached.
/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />
/// <reference lib="webworker" />
/// <reference types="@sveltejs/kit" />
import { build, files, version } from "$service-worker";

const sw = self as unknown as ServiceWorkerGlobalScope;
const CACHE = `fieldnotes-app-${version}`;
const PAGE = "/m";
const ASSETS = new Set([...build, ...files]);

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
    // The page: the computer's copy when it answers, the saved one when it doesn't.
    event.respondWith(
      fetch(req)
        .then((res) => {
          if (res.ok && url.pathname === PAGE) {
            const copy = res.clone();
            caches.open(CACHE).then((c) => c.put(PAGE, copy));
          }
          return res;
        })
        .catch(async () => (await caches.match(PAGE)) ?? Response.error()),
    );
    return;
  }
  if (ASSETS.has(url.pathname)) {
    event.respondWith(caches.match(url.pathname).then((hit) => hit ?? fetch(req)));
  }
});
