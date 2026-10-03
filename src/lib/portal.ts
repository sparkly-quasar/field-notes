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

/**
 * The computer couldn't be reached at all (or its journal is locked at the
 * desk), so nothing was sent: safe to answer on the phone or queue instead. A
 * request that *timed out* is not this — it may have landed.
 */
export class UnreachableError extends Error {
  readonly unreachable = true;
}

/** No answer in time. Unlike [`UnreachableError`], a write may still have landed. */
export class TimeoutError extends Error {
  readonly timedOut = true;
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
export async function portalInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
  opts: { timeoutMs?: number; raw?: boolean } = {},
): Promise<T> {
  const token = localStorage.getItem(TOKEN_KEY);
  if (!token) throw new Error("This phone isn't paired. On your server, open Settings → Devices & server → Pair a device.");

  let res: Response;
  // A sleeping server or a dropped tailnet can leave a request hanging with the
  // button saying "Saving…" forever. Give up after a while — and say honestly that
  // a write may still have landed, so nobody re-logs a dose that's already there.
  const ctl = new AbortController();
  const timer = setTimeout(() => ctl.abort(), opts.timeoutMs ?? REQUEST_TIMEOUT_MS);
  try {
    res = await fetch(`/api/${cmd}`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Authorization: `Bearer ${token}` },
      body: JSON.stringify(args ?? {}),
      signal: ctl.signal,
    });
  } catch {
    if (ctl.signal.aborted) {
      throw new TimeoutError("That took too long to answer. It may or may not have saved — check the entry before trying again.");
    }
    // Nothing answered at all, so the phone isn't on the tailnet (or the computer
    // isn't). Say which things to check, in the order they usually go wrong — a
    // silent failure while someone is logging a dose is the worst outcome here.
    throw new UnreachableError(
      "Can't reach your computer. Check that Tailscale is switched on on this phone and signed in " +
        "with the same account as your computer (another VPN app can switch it off), and that your " +
        "computer is on.",
    );
  } finally {
    clearTimeout(timer);
  }

  if (res.status === 401) {
    forgetToken();
    throw new Error("This phone is no longer paired. Pair it again on your server: Settings → Devices & server.");
  }
  if (res.status === 502 || res.status === 504) {
    // Tailscale reached the computer, but Field Notes isn't running there.
    throw new UnreachableError("Your computer isn't answering. Is it awake, with Field Notes open?");
  }
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    if (res.status === 503 && body.locked) {
      // Another person's journal locked itself (the server restarted). Tell the
      // page, which swaps in the password screen; Help stays reachable there.
      window.dispatchEvent(new CustomEvent(LOCKED_EVENT));
      throw new LockedError(body.error ?? "Your journal is locked.");
    }
    if (res.status === 503) {
      // The owner's journal is locked at the desk: nothing was read or written.
      throw new UnreachableError(body.error ?? "The journal is locked on your computer.");
    }
    throw new Error(body.error ?? `Request failed (${res.status}).`);
  }
  return (opts.raw ? await res.text() : await res.json()) as T;
}
