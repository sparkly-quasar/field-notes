// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// Phone-side half of the portal (see src-tauri/src/portal.rs).
//
// On the desktop this file does nothing: the app talks to Rust over Tauri's IPC.
// It only comes into play when the same frontend is served over HTTP to a phone,
// where there is no `invoke` and every call becomes a `fetch` carrying a token.

const TOKEN_KEY = "fieldnotes.portalToken";

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
  if (!token) throw new Error("This phone isn't paired. Scan the QR code in the desktop app.");

  let res: Response;
  try {
    res = await fetch(`/api/${cmd}`, {
      method: "POST",
      headers: { "Content-Type": "application/json", Authorization: `Bearer ${token}` },
      body: JSON.stringify(args ?? {}),
    });
  } catch {
    // The desktop has to be awake and on the tailnet to answer. Say so plainly —
    // a silent failure while someone is logging a dose is the worst outcome here.
    throw new Error("Can't reach the desktop app. Is it awake and on your tailnet?");
  }

  if (res.status === 401) {
    forgetToken();
    throw new Error("This phone is no longer paired. Scan the QR code again.");
  }
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body.error ?? `Request failed (${res.status}).`);
  }
  return (await res.json()) as T;
}
