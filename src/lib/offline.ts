// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// The phone without its computer.
//
// Every call the phone makes comes through `phoneInvoke` (api.ts sends them
// here). When the computer answers, nothing changes. When it can't be reached:
//
// - **The combination checker, the dose lookup, passage search and street names**
//   answer on the phone, from `field_notes_core` compiled to WebAssembly
//   (src-tauri/wasm) over the bundled reference files the phone keeps. Same code
//   as the computer's, so the same answers, except that the phone knows only the
//   reference and the built-in classes, not the person's own catalogue.
// - **New entries wait in the outbox** (outbox.ts) and go to the computer, in
//   order, when it's back. A dose logged offline is still checked.
// - **Everything else says plainly that it needs the computer.** Talk runs on the
//   computer's model and greys out; edits and deletes wait for the connection.
//
// The reference files are public CC0 data and are kept in the browser's cache,
// one copy per app version. The outbox lives in local storage and is cleared as
// the computer confirms each entry (owner's decision, 2026-10-03).

import { version } from "$app/environment";
import { LockedError, portalInvoke, TimeoutError, UnreachableError } from "./portal";
import * as ob from "./outbox";

const STATE_KEY = "fieldnotes.outbox";
const FILES_CACHE = "fieldnotes-reference";
/** After the computer didn't answer, answer on the phone for this long rather
 *  than making every tap wait; the background loop keeps checking. */
const BACKOFF_MS = 15_000;
/** How long to wait on "are you there?" before treating the computer as away. */
const PROBE_TIMEOUT_MS = 4_000;
/** How many experiences in progress to keep for offline checks. */
const KEEP_OPEN = 5;

const TALK = ["companion_chat", "companion_chat_start", "companion_chat_poll", "companion_warm", "ai_status", "ai_start", "compute_status"];

// ---------- status, for the page ----------

export interface OfflineStatus {
  /** The computer isn't answering right now. */
  offline: boolean;
  /** Entries waiting to be sent. */
  pending: number;
  /** Entries the computer refused (or that may not have landed), kept until discarded. */
  failed: { seq: number; what: string; why: string }[];
  /** The phone has the dose reference, so checks and lookups work offline. */
  ready: boolean;
  /** Journal entries kept for offline checks: in progress, and from the last day. */
  kept: { open: number; recent: number };
}

let offline = false;
let ready = false;
const listeners = new Set<(s: OfflineStatus) => void>();

export function offlineStatus(): OfflineStatus {
  const s = load();
  return {
    offline,
    pending: ob.pending(s).length,
    failed: ob.failed(s).map((i) => ({ seq: i.seq, what: ob.describe(i), why: i.failed ?? "" })),
    ready,
    kept: { open: Object.keys(s.details).length, recent: (s.list ?? []).length },
  };
}

export function onOfflineStatus(f: (s: OfflineStatus) => void): () => void {
  listeners.add(f);
  f(offlineStatus());
  return () => listeners.delete(f);
}

function notify() {
  const s = offlineStatus();
  for (const f of listeners) f(s);
}

// ---------- the outbox's storage ----------

function load(): ob.State {
  try {
    return ob.normalise(JSON.parse(localStorage.getItem(STATE_KEY) ?? "null"));
  } catch {
    return ob.emptyState();
  }
}

/**
 * Ask the browser not to clear this site's storage when the phone runs low on
 * space, the moment there's something queued worth keeping. Chrome grants it
 * without asking for an app on the home screen; Firefox may ask once, right after
 * the tap that queued it; Safari decides for itself. Refused or unsupported, the
 * queue is kept exactly as before, so nothing waits on the answer.
 */
let askedToKeep = false;
function keepStorage() {
  if (askedToKeep) return;
  askedToKeep = true;
  try {
    navigator.storage
      ?.persisted?.()
      .then((kept) => (kept ? true : navigator.storage.persist?.()))
      .catch(() => {});
  } catch {
    // No storage manager here: nothing to ask.
  }
}

function save(s: ob.State) {
  try {
    localStorage.setItem(STATE_KEY, JSON.stringify(s));
  } catch {
    // Storage full or blocked: the queue can't be kept, so say so rather than lose it quietly.
    throw new Error("This phone couldn't save that for later (its storage is full or blocked).");
  }
}

/** Change the stored state in one step, with nothing awaited in between. */
function update<T>(f: (s: ob.State) => T): T {
  const s = load();
  const out = f(s);
  save(s);
  return out;
}

export function discardFailed(seq: number) {
  update((s) => ob.discard(s, seq));
  notify();
}

/** Send a refused or timed-out entry again. */
export function retryFailed(seq: number) {
  update((s) => {
    const item = s.items.find((i) => i.seq === seq);
    if (item) delete item.failed;
  });
  notify();
  void flush();
}

/**
 * Forget what this phone keeps for offline use: the entries kept for checks and
 * the saved reference. Entries still waiting to be sent stay (clearing them would
 * lose them); discard those one by one if they were refused. While connected, the
 * phone saves fresh copies again as it's used.
 */
export async function clearKept() {
  update((s) => {
    s.list = null;
    s.details = {};
  });
  try {
    if (typeof caches !== "undefined") await caches.delete(FILES_CACHE);
  } catch {
    // Nothing saved, or storage blocked: nothing to clear.
  }
  ready = false;
  enginePromise = null;
  fetching = null;
  notify();
}

// ---------- is the computer there? ----------

let lastOk = 0;
let downUntil = 0;

function markOk() {
  lastOk = Date.now();
  downUntil = 0;
  if (offline) {
    offline = false;
    notify();
  }
}

function markDown() {
  downUntil = Date.now() + BACKOFF_MS;
  if (!offline) {
    offline = true;
    notify();
  }
}

async function reachable(): Promise<boolean> {
  if (Date.now() < downUntil) return false;
  if (Date.now() - lastOk < BACKOFF_MS) return true;
  try {
    await portalInvoke("db_status", {}, { timeoutMs: PROBE_TIMEOUT_MS });
    markOk();
    return true;
  } catch (e) {
    if (e instanceof UnreachableError || e instanceof TimeoutError) {
      markDown();
      return false;
    }
    // Anything else came from the computer itself (a person's journal locked,
    // this phone unpaired): it's there, and the real call says what's wrong.
    return true;
  }
}

// ---------- the reference files and the checker ----------

async function filesCache(): Promise<Cache | null> {
  try {
    return typeof caches === "undefined" ? null : await caches.open(FILES_CACHE);
  } catch {
    return null;
  }
}

const fileKey = (name: string) => `/offline-files/${version}/${name}`;

async function cachedText(name: string): Promise<string | null> {
  const c = await filesCache();
  const r = await c?.match(fileKey(name));
  return r ? r.text() : null;
}

let fetching: Promise<void> | null = null;

/** While connected, keep this version's reference files on the phone. Once per version. */
export function keepReferenceFiles(): Promise<void> {
  fetching ??= (async () => {
    const c = await filesCache();
    if (!c) return;
    for (const [name, cmd] of [
      ["reference", "offline_reference"],
      ["corpus", "offline_corpus"],
    ]) {
      if (await c.match(fileKey(name))) continue;
      const text = await portalInvoke<string>(cmd, {}, { raw: true, timeoutMs: 120_000 });
      await c.put(fileKey(name), new Response(text, { headers: { "Content-Type": "application/json" } }));
    }
    // An older version's files are no use to this one.
    for (const req of await c.keys()) {
      if (!new URL(req.url).pathname.startsWith(`/offline-files/${version}/`)) await c.delete(req);
    }
    ready = true;
    notify();
  })().catch(() => {
    fetching = null;
  });
  return fetching;
}

interface Engine {
  call<T>(cmd: string, args: Record<string, unknown>): T;
  /** Load the passages, the first time they're asked for. */
  withCorpus(): Promise<boolean>;
}

let enginePromise: Promise<Engine | null> | null = null;

/** The shared checker, running on the phone. Null until the reference is saved here. */
function engine(): Promise<Engine | null> {
  enginePromise ??= (async (): Promise<Engine | null> => {
    const reference = await cachedText("reference");
    if (!reference) return null;
    const bytes = await (await fetch("/offline/field-notes.wasm")).arrayBuffer();
    const { instance } = await WebAssembly.instantiate(bytes, {});
    const ex = instance.exports as unknown as {
      memory: WebAssembly.Memory;
      alloc(len: number): number;
      dealloc(ptr: number, len: number): void;
      load_reference(ptr: number, len: number): number;
      load_corpus(ptr: number, len: number): number;
      call(ptr: number, len: number): number;
      result_len(): number;
    };
    const withBuffer = <T>(text: string, f: (ptr: number, len: number) => T): T => {
      const b = new TextEncoder().encode(text);
      const p = ex.alloc(b.length);
      new Uint8Array(ex.memory.buffer, p, b.length).set(b);
      try {
        return f(p, b.length);
      } finally {
        ex.dealloc(p, b.length);
      }
    };
    if (withBuffer(reference, ex.load_reference) < 0) return null;
    ready = true;
    notify();
    let corpus = false;
    return {
      call<T>(cmd: string, args: Record<string, unknown>): T {
        const at = withBuffer(JSON.stringify({ cmd, args }), ex.call);
        const len = ex.result_len();
        const out = JSON.parse(new TextDecoder().decode(new Uint8Array(ex.memory.buffer, at, len)));
        ex.dealloc(at, len);
        if ("err" in out) throw new Error(out.err);
        return out.ok as T;
      },
      async withCorpus() {
        if (!corpus) {
          const text = await cachedText("corpus");
          corpus = !!text && withBuffer(text, ex.load_corpus) >= 0;
        }
        return corpus;
      },
    };
  })().then((e) => {
    if (!e) enginePromise = null;
    return e;
  }, () => (enginePromise = null));
  return enginePromise;
}

async function requireEngine(): Promise<Engine> {
  const e = await engine();
  if (!e) {
    throw new Error(
      "Your computer can't be reached, and this phone hasn't saved the dose reference yet. It does that the first time it connects.",
    );
  }
  return e;
}

/** Names the phone's checker knows nothing about (not in the reference, no
 *  built-in class), so the page can say they weren't checked. */
export async function uncheckedOffline(names: string[]): Promise<string[]> {
  const e = await engine();
  return e ? e.call<string[]>("unknown_names", { names }) : names;
}

const sessionCheck = (e: Engine): ob.SessionCheck => (doses) =>
  e.call("session_warnings", {
    doses: doses.map((d) => ({
      substance_name: d.substance_name,
      route: d.route,
      at_min: Number.isFinite(Date.parse(d.taken_at)) ? Date.parse(d.taken_at) / 60_000 : null,
    })),
  });

// ---------- sending what's queued ----------

let flushing: Promise<boolean> | null = null;

/** Send queued entries, oldest first. True when nothing is left waiting. */
export function flush(): Promise<boolean> {
  flushing ??= (async () => {
    try {
      for (;;) {
        const s = load();
        const next = ob.pending(s)[0];
        if (!next) return true;
        const args = structuredClone(next.args);
        if (!ob.translate(s, args)) {
          update((t) => ob.markFailed(t, next.seq, "Its experience couldn't be saved on your computer, so this couldn't either."));
          continue;
        }
        try {
          const answer = next.cmd === "stretch_to_cover" ? await stretch(args.id, args.at) : await portalInvoke(next.cmd, args);
          update((t) => ob.sent(t, next.seq, answer));
          markOk();
        } catch (e) {
          if (e instanceof UnreachableError || e instanceof LockedError) {
            markDown();
            return false;
          }
          const why =
            e instanceof TimeoutError
              ? "Your computer didn't answer in time, so this may or may not have saved. Check the entry, then send it again or discard it."
              : e instanceof Error
                ? e.message
                : String(e);
          update((t) => ob.markFailed(t, next.seq, why));
        }
      }
    } finally {
      flushing = null;
      notify();
    }
  })();
  return flushing;
}

/** Widen an entry's times to cover a dose logged offline, against the computer's
 *  copy as it is now (so it can't undo a change made there meanwhile). */
async function stretch(id: number, at: string) {
  const e = await portalInvoke<any>("get_experience", { id });
  const before = Date.parse(at) < Date.parse(e.started_at);
  const after = e.ended_at != null && Date.parse(at) > Date.parse(e.ended_at);
  if (!before && !after) return null;
  return portalInvoke("update_experience", {
    id,
    update: {
      title: e.title,
      intention: e.intention,
      setting: e.setting,
      notes: e.notes,
      rating: e.rating,
      started_at: before ? at : e.started_at,
      ended_at: after ? at : e.ended_at,
    },
  });
}

/** Queue widening an entry's times, from `stretchToCover` when the edit itself
 *  can't be sent. */
export function deferStretch(id: number, at: string) {
  update((s) => ob.queue(s, "stretch_to_cover", { id, at }, () => []));
  notify();
}

/** Error thrown for things that need the computer, so callers can tell. */
export class NeedsComputerError extends Error {
  readonly needsComputer = true;
}

// ---------- the one entry point ----------

export async function phoneInvoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  args = structuredClone(args);
  let online = await reachable();
  if (online && ob.pending(load()).length) online = await flush();
  const resolved = ob.translate(load(), args);
  if (online && resolved) {
    try {
      const v = await portalInvoke<T>(cmd, args);
      markOk();
      remember(cmd, v);
      return v;
    } catch (e) {
      if (!(e instanceof UnreachableError)) throw e;
      markDown();
    }
  }
  const v = await answerHere<T>(cmd, args);
  notify();
  return v;
}

/** Keep what offline checks need from a fresh answer. */
function remember(cmd: string, v: unknown) {
  if (cmd === "list_experiences" && Array.isArray(v)) {
    update((s) => ob.keepList(s, v));
    const open = v.filter((e) => e.kind === "session" && !e.ended_at).slice(0, KEEP_OPEN);
    for (const e of open) {
      portalInvoke("get_experience", { id: e.id })
        .then((d) => update((s) => ob.keepDetail(s, d)))
        .catch(() => {});
    }
    void keepReferenceFiles();
  } else if (cmd === "get_experience") {
    update((s) => ob.keepDetail(s, v));
  }
}

/** Answer without the computer. */
async function answerHere<T>(cmd: string, args: Record<string, any>): Promise<T> {
  const why = "Your computer can't be reached right now.";
  switch (cmd) {
    case "list_experiences":
      return ob.overlayList(load()) as T;
    case "get_experience": {
      const d = ob.overlayDetail(load(), args.id);
      if (!d) throw new NeedsComputerError(`${why} This entry isn't saved on this phone, so it opens when your computer is back.`);
      return d as T;
    }
    case "check_combo":
      return (await requireEngine()).call("check_combo", { names: args.names, doses: args.doses });
    case "pw_lookup":
      return (await requireEngine()).call("pw_lookup", { name: args.name });
    case "pw_names":
      return (await requireEngine()).call("pw_names", {});
    case "knowledge_search": {
      const e = await requireEngine();
      if (!(await e.withCorpus())) return [] as T;
      return e.call("knowledge_search", { query: args.query, limit: args.limit ?? 5 });
    }
    case "log_dose": {
      const e = await engine();
      const check: ob.SessionCheck = e
        ? sessionCheck(e)
        : (doses) => [
            {
              severity: "note",
              a: doses[doses.length - 1]?.substance_name ?? "",
              b: "anything else",
              message:
                "Saved on this phone. It couldn't be checked against anything else yet: this phone saves the dose reference the first time it connects.",
              advice: [],
            },
          ];
      keepStorage();
      return update((s) => ob.queue(s, cmd, args, check)) as T;
    }
    case "list_unit_kinds":
      // Saved capsule kinds live on the computer. Offline, the form still takes
      // a typed amount per capsule, which the dose keeps.
      return [] as T;
    case "create_experience":
    case "add_timeline_event":
    case "end_experience":
      keepStorage();
      return update((s) => ob.queue(s, cmd, args, () => [])) as T;
    default:
      if (TALK.includes(cmd)) throw new NeedsComputerError(`${why} Talk runs on your computer, so it's back when your computer is.`);
      throw new NeedsComputerError(
        `${why} Editing and deleting need the connection. New experiences, doses and moments still save on this phone and are sent when it's back.`,
      );
  }
}

// ---------- the background loop ----------

let started = false;

/** Keep trying to send what's waiting, and notice when the computer is back. */
export function startOffline() {
  if (started || typeof window === "undefined") return;
  started = true;
  const tick = async () => {
    const waiting = ob.pending(load()).length > 0;
    if (!waiting && !offline) return;
    downUntil = 0;
    if (await reachable()) {
      if (waiting) await flush();
      void keepReferenceFiles();
    }
  };
  setInterval(tick, BACKOFF_MS);
  window.addEventListener("online", () => void tick());
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") void tick();
  });
  void engine();
}
