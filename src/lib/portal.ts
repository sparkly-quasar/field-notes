// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// Phone-side half of the portal (see src-tauri/src/portal.rs).
//
// On the desktop this file does nothing: the app talks to Rust over Tauri's IPC.
// It only comes into play when the same frontend is served over HTTP to a phone,
// where there is no `invoke` and every call becomes a `fetch` carrying a token.

const TOKEN_KEY = "fieldnotes.portalToken";
/** Every phone request is short — the Companion runs as a polled job — so this is generous. */
const REQUEST_TIMEOUT_MS = 20_000;

/** True when we're running inside the Tauri desktop shell rather than a browser. */
export function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * Take the token out of the pairing URL (`/m#t=<token>`) and keep it.
 *
 * It arrives in the **fragment**, not the query string, because a fragment is never
 * sent to a server and never lands in a server log. We strip it from the address bar
 * immediately afterwards so it doesn't linger in the phone's history or in a
 * screenshot of the tab.
 */
export function captureToken(): void {
  if (typeof window === "undefined") return;
  const m = window.location.hash.match(/[#&]t=([a-f0-9]{64})/i);
  if (!m) return;
  localStorage.setItem(TOKEN_KEY, m[1]);
  history.replaceState(null, "", window.location.pathname);
}

/**
 * Pair from a pasted link (`…/m#t=<token>`) or a bare token. Returns false if the
 * text doesn't contain one.
 *
 * This exists for the iPhone Home Screen app: iOS gives a saved web app its own
 * storage, separate from Safari's, so a phone paired in Safari opens the saved app
 * unpaired — and the saved app has no address bar to scan a link into.
 */
export function acceptPairing(text: string): boolean {
  const m = text.trim().match(/(?:^|[#&]t=)([a-f0-9]{64})$/i);
  if (!m) return false;
  localStorage.setItem(TOKEN_KEY, m[1].toLowerCase());
  return true;
}

/** This phone's pairing link, to carry it from Safari into the Home Screen app. */
export function pairingLink(): string | null {
  const token = typeof localStorage !== "undefined" ? localStorage.getItem(TOKEN_KEY) : null;
  return token ? `${window.location.origin}/m#t=${token}` : null;
}

/** Running as a saved Home Screen app rather than in a browser tab. */
export function isStandalone(): boolean {
  if (typeof window === "undefined") return false;
  return (
    ("standalone" in navigator && (navigator as { standalone?: boolean }).standalone === true) ||
    window.matchMedia?.("(display-mode: standalone)").matches
  );
}

/** iPhone/iPad Safari (iPadOS reports itself as a Mac, but with touch). */
export function isIos(): boolean {
  if (typeof navigator === "undefined") return false;
  return /iPhone|iPad|iPod/.test(navigator.userAgent) || (/Macintosh/.test(navigator.userAgent) && navigator.maxTouchPoints > 1);
}

/** Fired on `window` when the server says this person's journal is locked. */
export const LOCKED_EVENT = "fieldnotes:locked";
export class LockedError extends Error {
  readonly locked = true;
}

export function hasToken(): boolean {
  return typeof localStorage !== "undefined" && !!localStorage.getItem(TOKEN_KEY);
}

export function forgetToken(): void {
  localStorage.removeItem(TOKEN_KEY);
}

/**
 * The phone's transport. Same command names, same argument shapes, same errors as
 * `invoke` — so `api.ts` above it doesn't know which one it's talking to.
 */
export async function portalInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const token = localStorage.getItem(TOKEN_KEY);
  if (!token) throw new Error("This phone isn't paired. On your server, open Settings → Devices & server → Pair a device.");

  let res: Response;
  // A sleeping server or a dropped tailnet can leave a request hanging with the
  // button saying "Saving…" forever. Give up after a while — and say honestly that
  // a write may still have landed, so nobody re-logs a dose that's already there.
  const ctl = new AbortController();
  const timer = setTimeout(() => ctl.abort(), REQUEST_TIMEOUT_MS);
  try {
    res = await fetch(`/api/${cmd}`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Authorization: `Bearer ${token}` },
      body: JSON.stringify(args ?? {}),
      signal: ctl.signal,
    });
  } catch {
    if (ctl.signal.aborted) {
      throw new Error("That took too long to answer. It may or may not have saved — check the entry before trying again.");
    }
    // The server has to be awake and on the tailnet to answer. Say so plainly —
    // a silent failure while someone is logging a dose is the worst outcome here.
    throw new Error("Can't reach your Field Notes server. Is it awake and on your tailnet?");
  } finally {
    clearTimeout(timer);
  }

  if (res.status === 401) {
    forgetToken();
    throw new Error("This phone is no longer paired. Pair it again on your server: Settings → Devices & server.");
  }
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    if (res.status === 503 && body.locked) {
      // Another person's journal locked itself (the server restarted). Tell the
      // page, which swaps in the password screen; Help stays reachable there.
      window.dispatchEvent(new CustomEvent(LOCKED_EVENT));
      throw new LockedError(body.error ?? "Your journal is locked.");
    }
    throw new Error(body.error ?? `Request failed (${res.status}).`);
  }
  return (await res.json()) as T;
}
