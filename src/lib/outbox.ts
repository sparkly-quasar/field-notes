// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// The phone's outbox: what it does with new entries while the computer can't be
// reached. The same rules as a laptop's (src-tauri/src/remote.rs), in the same
// shape, so the two behave alike:
//
// - **New entries queue; nothing else does.** Starting an experience, logging a
//   dose, adding a moment and ending an experience wait here and are sent, in
//   order, when the computer answers again. Edits and deletes are refused offline:
//   queueing them re-opens conflict resolution (ROADMAP.md, Phase 3b).
// - **Temporary IDs are negative.** When an entry is sent, the computer's real ID
//   is recorded against it, and later calls carrying the temporary one are
//   translated on the way out.
// - **The phone keeps as little as it can.** Queued entries until the computer
//   confirms it has them; the experiences in progress (their doses, so a dose
//   logged offline is checked against them); and the last day's entries by
//   substance and time, for the same check. Nothing else from the journal.
//
// Pure logic over a plain object, so it can be tested without a browser
// (outbox.test.mjs); `offline.ts` stores it and talks to the network.

/** Writes that may wait. Only ever *new* things, plus ending an experience. */
export const QUEUEABLE = ["create_experience", "log_dose", "add_timeline_event", "end_experience"];

/** How far back the phone keeps entries for offline checks (by start time). */
export const KEEP_RECENT_MS = 24 * 3600_000;

export interface Item {
  seq: number;
  cmd: string;
  args: Record<string, any>;
  /** The temporary ID this created, if it created something. */
  tempId: number | null;
  /** What the computer would have answered, as shown while it waits. */
  preview: any;
  /** The computer refused it: kept, visibly, until discarded. */
  failed?: string;
}

export interface State {
  items: Item[];
  seq: number;
  /** Lowest temporary ID handed out, never reused. */
  lowest: number;
  /** Temporary ID → the computer's real one. */
  idmap: Record<string, number>;
  /** The last list of entries, trimmed to [`KEEP_RECENT_MS`] and those in progress. */
  list: any[] | null;
  /** Experiences in progress, by ID, as last seen. */
  details: Record<string, any>;
}

export function emptyState(): State {
  return { items: [], seq: 0, lowest: 0, idmap: {}, list: null, details: {} };
}

/** Fill in anything an older saved state is missing. */
export function normalise(s: Partial<State> | null | undefined): State {
  return { ...emptyState(), ...(s ?? {}) };
}

export const pending = (s: State) => s.items.filter((i) => !i.failed);
export const failed = (s: State) => s.items.filter((i) => i.failed);

function nextTemp(s: State): number {
  const lowest = Math.min(0, s.lowest, ...s.items.map((i) => i.tempId ?? 0), ...Object.keys(s.idmap).map(Number));
  s.lowest = lowest - 1;
  return s.lowest;
}

/** Every ID an entry has gone by: its real one and any temporary one it started as. */
export function aliases(s: State, id: number): number[] {
  const ids = [id];
  if (id < 0) {
    if (s.idmap[id] != null) ids.push(s.idmap[id]);
  } else {
    for (const [temp, real] of Object.entries(s.idmap)) if (real === id) ids.push(Number(temp));
  }
  return ids;
}

const ID_PATHS: [string, string?][] = [["id"], ["input", "experience_id"], ["experienceId"]];

/** Replace temporary IDs in `args` with real ones where the computer has assigned
 *  them. False if one is still waiting for its entry to be sent. */
export function translate(s: State, args: Record<string, any>): boolean {
  let resolved = true;
  for (const [a, b] of ID_PATHS) {
    const holder = b ? args[a] : args;
    const key = b ?? a;
    if (!holder || typeof holder !== "object" || typeof holder[key] !== "number" || holder[key] >= 0) continue;
    const real = s.idmap[holder[key]];
    if (real != null) holder[key] = real;
    else resolved = false;
  }
  return resolved;
}

/** Keep what offline checks need from a fresh list: entries started in the last
 *  day, and any still in progress. */
export function keepList(s: State, list: any[], now = Date.now()) {
  s.list = list.filter((e) => (e.kind === "session" && !e.ended_at) || now - Date.parse(e.started_at) < KEEP_RECENT_MS);
  // An experience no longer in progress (or no longer there) isn't kept.
  const open = new Set(s.list.filter((e) => e.kind === "session" && !e.ended_at).map((e) => String(e.id)));
  for (const id of Object.keys(s.details)) if (!open.has(id)) delete s.details[id];
}

/** Keep an experience as last seen, if it's in progress. */
export function keepDetail(s: State, detail: any) {
  if (detail && detail.kind === "session" && !detail.ended_at) s.details[String(detail.id)] = detail;
  else if (detail) delete s.details[String(detail.id)];
}

/** An experience as this phone knows it: as last seen (or as queued, for one
 *  started offline), with everything queued since applied on top. */
export function overlayDetail(s: State, id: number): any | null {
  const ids = aliases(s, id);
  const queue = pending(s);
  let detail: any = null;
  for (const i of ids) if (i > 0 && s.details[String(i)]) detail = structuredClone(s.details[String(i)]);
  if (!detail) {
    const created = queue.find((q) => q.cmd === "create_experience" && ids.includes(q.preview.id));
    if (!created) return null;
    detail = { ...structuredClone(created.preview), doses: [], timeline: [] };
  }
  const belongs = (v: unknown) => typeof v === "number" && ids.includes(v);
  for (const q of queue) {
    if (q.cmd === "log_dose" && belongs(q.args.input?.experience_id)) {
      detail.doses.push(q.preview);
      // The computer names an untitled experience after its first dose.
      if (!detail.title?.trim() && detail.doses.length === 1) detail.title = q.preview.substance_name;
    } else if (q.cmd === "add_timeline_event" && belongs(q.args.input?.experience_id)) {
      detail.timeline.push(q.preview);
    } else if (q.cmd === "end_experience" && belongs(q.args.id)) {
      detail.ended_at = q.args.endedAt;
      detail.rating = q.args.rating;
      detail.notes = q.args.notes;
    }
  }
  return detail;
}

/** The journal list as this phone knows it: what it kept, with entries started
 *  since on top. */
export function overlayList(s: State): any[] {
  const list: any[] = structuredClone(s.list ?? []);
  const queue = pending(s);
  for (const q of queue.filter((q) => q.cmd === "create_experience")) {
    const d = overlayDetail(s, q.preview.id) ?? q.preview;
    const names: string[] = (d.doses ?? []).map((x: any) => x.substance_name);
    list.unshift({
      ...q.preview,
      title: d.title,
      substances: [...new Set(names)].sort(),
      dose_count: names.length,
      ended_at: d.ended_at ?? null,
    });
  }
  // Queued doses into entries the computer already has: keep the summary honest.
  for (const item of list) {
    if (item.id < 0) continue;
    const ids = aliases(s, item.id);
    const doses = queue.filter((q) => q.cmd === "log_dose" && ids.includes(q.args.input?.experience_id));
    if (doses.length) {
      item.dose_count = (item.dose_count ?? 0) + doses.length;
      item.substances = [...new Set([...(item.substances ?? []), ...doses.map((q) => q.preview.substance_name)])].sort();
    }
    const end = [...queue].reverse().find((q) => q.cmd === "end_experience" && ids.includes(q.args.id));
    if (end) item.ended_at = end.args.endedAt;
  }
  return list;
}

/** The checker the outbox asks about a queued dose: the same `session_warnings`
 *  the computer runs on `log_dose`, given the experience's doses. */
export type SessionCheck = (doses: { substance_name: string; route: string; taken_at: string }[]) => any[];

/** Put a new entry in the outbox and return what the computer would have: the
 *  same shape, a temporary ID, and, for a dose, the interaction warnings. */
export function queue(s: State, cmd: string, args: Record<string, any>, check: SessionCheck, now = new Date()): any {
  const nowIso = now.toISOString();
  const input = args.input ?? {};
  let tempId: number | null = null;
  let preview: any;
  let result: any;
  switch (cmd) {
    case "create_experience": {
      tempId = nextTemp(s);
      preview = {
        id: tempId,
        kind: input.kind === "note" ? "note" : "session",
        title: input.title ?? "",
        intention: input.intention ?? "",
        setting: input.setting ?? "",
        notes: "",
        rating: null,
        started_at: input.started_at ?? nowIso,
        ended_at: null,
        created_at: nowIso,
      };
      result = preview;
      break;
    }
    case "log_dose": {
      const detail = overlayDetail(s, input.experience_id);
      if (detail?.kind === "note") throw new Error("This entry is a plain note, not an experience — it can't have doses.");
      tempId = nextTemp(s);
      preview = {
        id: tempId,
        experience_id: input.experience_id,
        substance_id: null,
        substance_name: input.substance_name ?? "",
        amount: input.amount ?? null,
        unit: input.unit ?? "mg",
        route: input.route ?? "",
        taken_at: input.taken_at ?? nowIso,
        note: input.note ?? "",
      };
      // The same question the computer answers on `log_dose`, over what this
      // phone knows: the experience as last seen, everything queued since, and
      // this dose.
      result = { dose: preview, warnings: check([...(detail?.doses ?? []), preview]) };
      break;
    }
    case "add_timeline_event": {
      tempId = nextTemp(s);
      preview = {
        id: tempId,
        experience_id: input.experience_id,
        at: input.at ?? nowIso,
        note: input.note ?? "",
        mood: input.mood ?? "",
        intensity: input.intensity ?? null,
      };
      result = preview;
      break;
    }
    case "end_experience": {
      const detail = overlayDetail(s, args.id);
      if (!detail) throw new Error("This experience isn't saved on this phone, so it can't be ended until your computer is back.");
      const { doses: _d, timeline: _t, ...exp } = detail;
      preview = { ...exp, ended_at: args.endedAt, rating: args.rating, notes: args.notes };
      result = preview;
      break;
    }
    case "stretch_to_cover": {
      // Not a command the computer knows: a note to widen an entry's times to
      // cover a dose, worked out against the computer's copy when it's sent.
      preview = { id: args.id, at: args.at };
      result = null;
      break;
    }
    default:
      throw new Error(`\`${cmd}\` can't wait for your computer.`);
  }
  s.seq += 1;
  s.items.push({ seq: s.seq, cmd, args: structuredClone(args), tempId, preview });
  return result;
}

/** The computer has the oldest pending item: forget it, remembering its real ID. */
export function sent(s: State, seq: number, answer: any) {
  const item = s.items.find((i) => i.seq === seq);
  if (!item) return;
  const real = typeof answer?.id === "number" ? answer.id : typeof answer?.dose?.id === "number" ? answer.dose.id : null;
  if (item.tempId != null && real != null) s.idmap[item.tempId] = real;
  s.items = s.items.filter((i) => i.seq !== seq);
  // The map only has to outlive what still refers to it; keep it small.
  const keys = Object.keys(s.idmap);
  if (keys.length > 200) for (const k of keys.slice(0, keys.length - 200)) delete s.idmap[k];
}

export function markFailed(s: State, seq: number, why: string) {
  const item = s.items.find((i) => i.seq === seq);
  if (item) item.failed = why;
}

export function discard(s: State, seq: number) {
  s.items = s.items.filter((i) => !(i.seq === seq && i.failed));
}

/** One line for the "waiting to send" list. */
export function describe(item: Item): string {
  const p = item.preview ?? {};
  switch (item.cmd) {
    case "create_experience":
      return p.kind === "note" ? `New note${p.title ? `: ${p.title}` : ""}` : `New experience${p.title ? `: ${p.title}` : ""}`;
    case "log_dose":
      return `Dose: ${p.substance_name}${p.amount != null ? ` ${p.amount} ${p.unit}` : ""}`;
    case "add_timeline_event":
      return `Moment${p.note ? `: ${String(p.note).slice(0, 40)}` : ""}`;
    case "end_experience":
      return "End of an experience";
    case "stretch_to_cover":
      return "Widen an entry's times to cover a dose";
    default:
      return item.cmd;
  }
}
