<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  The phone portal's UI (see src-tauri/src/portal.rs).

  A *mirror* of the desktop, not a copy of its layout: the same journal, the same
  deterministic safety checks, the same reference data — re-laid out for one hand,
  in the dark, possibly altered. Big targets, bottom nav, no dense tables.

  What is deliberately NOT here, and cannot be reached from here (portal.rs does not
  allowlist it): wiping the journal, the encryption passphrase, backups, Obsidian
  sync, installing Ollama, revealing the data directory. A phone is the device you
  lose. Everything you'd actually reach for mid-session is here.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import {
    listExperiences,
    getExperience,
    createExperience,
    endExperience,
    logDose,
    updateExperience,
    updateDose,
    deleteDose,
    addTimelineEvent,
    updateTimelineEvent,
    deleteTimelineEvent,
    deleteExperience,
    listSubstances,
    addSubstance,
    usageBySubstance,
    checkCombo,
    crisisScan,
    companionEnabled as companionEnabledPref,
    companionChat,
    companionChatStart,
    companionChatPoll,
    type CompanionPoll,
    type CompanionReply,
    aiStatus,
    aiStart,
    pwLookup,
    knowledgeSearch,
    exportExperienceMarkdown,
    type ExperienceSummary,
    type ExperienceDetail,
    type Substance,
    type SubstanceUsage,
    type Dose,
    type TimelineEvent,
    type Warning,
    type CrisisResult,
    type ChatMsg,
    type PwInfo,
    type KnowledgeHit,
    type AiStatus,
  } from "$lib/api";
  import { captureToken, hasToken, inTauri } from "$lib/portal";
  import {
    quickLog,
    recentSubstances,
    whenPresets,
    recallDoseShape,
    rememberDoseShape,
  } from "$lib/quicklog";

  type View = "now" | "journal" | "combo" | "reference" | "companion";

  let paired = $state(false);
  let view = $state<View>("now");
  let err = $state<string | null>(null);
  let busy = $state(false);

  let session = $state<ExperienceDetail | null>(null);
  let recent = $state<ExperienceSummary[]>([]);

  // Dose entry. One set of fields serves both places you can log into — the live
  // session on Now, and an entry opened in the Journal — because only one of the
  // two is ever on screen.
  let dSub = $state("");
  let dAmt = $state("");
  let dUnit = $state("mg");
  let dRoute = $state("oral");
  /** datetime-local. Only asked for when adding to an entry that already happened;
   *  in a live session the answer is always "now". */
  let dWhen = $state("");
  let doseWarnings = $state<Warning[]>([]);

  // editing a dose already logged
  let editing = $state<Dose | null>(null);
  let eAmt = $state("");
  let eUnit = $state("mg");
  let eRoute = $state("oral");
  let eWhen = $state("");
  let eNote = $state("");

  // timeline note (shared between the live session and an opened entry, as above)
  let note = $state("");
  let intensity = $state("");
  let noteWhen = $state("");

  // editing a timeline note already written
  let editingEvent = $state<TimelineEvent | null>(null);
  let evNote = $state("");
  let evIntensity = $state("");
  let evWhen = $state("");

  // starting a session: an optional title, blank by default
  let newTitle = $state("");

  // Quick log — one substance, a time, nothing else. The common case is not a
  // trip you sit through and write up; it's "I took this, record it". No session
  // to end, no write-up to fill in. Logic lives in $lib/quicklog.ts, shared with
  // the desktop.
  let qSub = $state("");
  let qAmt = $state("");
  let qUnit = $state("mg");
  let qRoute = $state("oral");
  let qWhen = $state("");
  let qWarnings = $state<Warning[]>([]);
  /** The entry the last quick log landed in — what "add notes to it" acts on. */
  let qSaved = $state<{ id: number; title: string; at: string } | null>(null);
  /** Set while adding a second substance to that same entry, rather than starting another. */
  let qInto = $state<number | null>(null);
  const qRecents = $derived(recentSubstances(recent));

  // editing an entry already in the journal: its title, dates, rating, write-up
  let editEntry = $state(false);
  let enTitle = $state("");
  let enStart = $state("");
  let enEnd = $state("");
  let enRating = $state("");
  let enNotes = $state("");

  // renaming an experience (live session or an opened journal entry)
  let renamingId = $state<number | null>(null);
  let renameText = $state("");

  // plain note (kind: "note") — no session required
  let plainTitle = $state("");
  let plainBody = $state("");
  let noteSaved = $state(false);

  // journal browsing
  let open = $state<ExperienceDetail | null>(null);

  /** Export the opened entry as a Markdown download. The phone never touches the
   *  desktop's filesystem — the desktop renders the text, the browser saves it.
   *
   *  Three details here are all working around mobile-browser download quirks;
   *  the desktop tolerates any of them being wrong, which is why they were.
   *
   *  1. **`application/octet-stream`, not `text/markdown`.** Given a MIME type it
   *     recognises, mobile Safari names the saved file from *that* rather than
   *     from `download`, and it has no extension mapped for `text/markdown` — so
   *     the file arrived correctly named but with no `.md` on the end. An opaque
   *     type leaves the `download` filename alone. (`note.filename` already ends
   *     in `.md`; see `note_filename` in obsidian.rs, pinned by a test.)
   *  2. **The anchor goes into the document before it is clicked.** A detached
   *     anchor's synthetic click is ignored by some mobile browsers.
   *  3. **The object URL is revoked on a later tick.** Revoking synchronously
   *     after `click()` can pull the blob out from under a download that hasn't
   *     started reading it yet. */
  async function exportOpen() {
    if (!open) return;
    err = null;
    try {
      const note = await exportExperienceMarkdown(open.id);
      const url = URL.createObjectURL(
        new Blob([note.markdown], { type: "application/octet-stream" }),
      );
      const a = document.createElement("a");
      a.href = url;
      a.download = note.filename;
      a.style.display = "none";
      document.body.appendChild(a);
      a.click();
      a.remove();
      setTimeout(() => URL.revokeObjectURL(url), 10_000);
    } catch (e) {
      err = typeof e === "string" ? e : String(e);
    }
  }
  let usage = $state<SubstanceUsage[]>([]);
  let showUsage = $state(false);

  // combo check
  let comboText = $state("");
  let comboWarnings = $state<Warning[] | null>(null);

  // reference: catalogue + dose table + prose search
  let substances = $state<Substance[]>([]);
  let refQuery = $state("");
  let pw = $state<PwInfo | null>(null);
  let hits = $state<KnowledgeHit[]>([]);
  let searched = $state(false);
  let newSub = $state("");

  // companion
  /** Mirrors the desktop's companion-off switch; the phone can't read its localStorage. */
  let companionEnabled = $state(true);
  let ai = $state<AiStatus | null>(null);
  let models = $state<string[]>([]);
  let model = $state("");
  let chat = $state<ChatMsg[]>([]);
  let ask = $state("");
  let thinking = $state(false);
  let waking = $state(false);
  let chatCrisis = $state<CrisisResult | null>(null);
  let chatCrisisShown = $state(false);

  onMount(async () => {
    captureToken();
    paired = inTauri() || hasToken();
    if (!paired) return;
    qWhen = nowLocalInput();
    await refresh();
    await loadAi();
  });

  /** Ask the desktop what its local model is doing *right now*.
   *
   *  This used to run once, at mount. If Ollama happened to be asleep at that
   *  moment, the Companion stayed dead for the life of the page — no retry, no
   *  explanation — which is exactly what you don't want mid-session. So: re-check
   *  whenever the Talk tab is opened, and say which of the three states we're in
   *  rather than one vague "not reachable". */
  async function loadAi() {
    try {
      companionEnabled = await companionEnabledPref();
      ai = await aiStatus();
      models = ai.models;
      if (!models.includes(model)) model = models[0] ?? "";
    } catch {
      ai = null;
      models = [];
    }
  }

  /** Wake the desktop's model server from the phone. The alternative is walking to
   *  the desk, which is the situation the portal exists to avoid. */
  const wakeAi = () =>
    run(async () => {
      waking = true;
      try {
        await aiStart();
        // `ollama serve` takes a moment to accept connections.
        await new Promise((r) => setTimeout(r, 1500));
        await loadAi();
      } finally {
        waking = false;
      }
    });

  function goTo(v: View) {
    view = v;
    if (v === "companion") loadAi();
  }

  /** Anything that fails does so out loud — a phone that silently drops a dose you
   *  thought you logged is worse than a phone that says it couldn't. */
  async function run(f: () => Promise<unknown>) {
    busy = true;
    err = null;
    try {
      await f();
    } catch (e) {
      err = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  async function refresh() {
    await run(async () => {
      recent = await listExperiences();
      // Only a *session* can be live — a plain note has no ended_at either, but
      // it isn't something you're "in".
      const live = recent.find((e) => e.kind === "session" && !e.ended_at);
      session = live ? await getExperience(live.id) : null;
      if (open) open = await getExperience(open.id);
    });
  }

  // ---- the live session ----
  // A title is offered but never demanded: typing one at the moment you're
  // starting a session is often the last thing you want to be doing. Left blank,
  // the backend names the session after the first substance logged into it
  // (`name_after_first_dose` in db.rs) rather than leaving it "Untitled" forever.
  const startSession = () =>
    run(async () => {
      await createExperience({ title: newTitle.trim(), started_at: new Date().toISOString() });
      newTitle = "";
      await refresh();
    });

  // ---- a plain note: the 3am jot that isn't a session ----
  const submitPlainNote = () =>
    run(async () => {
      if (!plainBody.trim()) return;
      const e = await createExperience({
        kind: "note",
        title: plainTitle.trim(),
        started_at: new Date().toISOString(),
      });
      await updateExperience(e.id, {
        title: e.title,
        notes: plainBody.trim(),
        rating: null,
        started_at: e.started_at,
        ended_at: null,
      });
      plainTitle = plainBody = "";
      noteSaved = true;
      setTimeout(() => (noteSaved = false), 2500);
      await refresh();
    });

  // ---- quick log: a substance and a time, and that's the whole entry ----
  const submitQuickLog = () =>
    run(async () => {
      if (!qSub.trim()) return;
      const at = localInputToIso(qWhen);
      const res = await quickLog({
        substance: qSub.trim(),
        amount: qAmt.trim() ? Number(qAmt) : null,
        unit: qUnit,
        route: qRoute,
        at,
        intoId: qInto,
      });
      rememberDoseShape(qSub, { unit: qUnit, route: qRoute });
      qWarnings = res.warnings;
      qSaved = { id: res.id, title: res.title, at };
      qInto = null;
      qSub = qAmt = "";
      qWhen = nowLocalInput();
      await refresh();
    });

  /** A second substance goes into the same entry, not a new one — that's how the
   *  evening reads back, and it's what lets the checker compare the two. */
  function addAnother() {
    if (!qSaved) return;
    qInto = qSaved.id;
    qWhen = isoToLocalInput(qSaved.at);
    qSaved = null;
  }

  /** Fill in a substance you've logged before, in the shape you logged it in. */
  function pickRecent(name: string) {
    qSub = name;
    applyRemembered();
  }

  function applyRemembered() {
    const shape = recallDoseShape(qSub);
    if (shape) {
      qUnit = shape.unit;
      qRoute = shape.route;
    }
  }

  /** Open the entry a quick log just made, with the write-up ready to type into.
   *  This is the whole "log it now, say what it was like later" path: it has to
   *  be one tap from the confirmation or it won't happen. */
  const addNotesTo = (id: number) =>
    run(async () => {
      qSaved = null;
      view = "journal";
      await openExperience(id);
      startEditEntry();
      await new Promise((r) => setTimeout(r, 0));
      const el = document.getElementById("entry-writeup");
      el?.scrollIntoView({ block: "center" });
      el?.focus();
    });

  const endSession = () =>
    run(async () => {
      if (!session || !confirm("End this session?")) return;
      await endExperience(session.id, new Date().toISOString(), null, "");
      await refresh();
    });

  // ---- renaming ----
  function startRename(e: ExperienceDetail) {
    renamingId = e.id;
    renameText = e.title;
  }

  const saveRename = () =>
    run(async () => {
      // Renaming the live session from the header. An entry in the Journal is
      // renamed through its own editor, which can change everything else too.
      // update_experience replaces the whole row, so the rest is passed through.
      const target = session?.id === renamingId ? session : null;
      if (!target) return;
      await updateExperience(target.id, {
        title: renameText.trim(),
        intention: target.intention,
        setting: target.setting,
        notes: target.notes,
        rating: target.rating,
        started_at: target.started_at,
        ended_at: target.ended_at,
      });
      renamingId = null;
      await refresh();
    });

  /** Log into whichever entry is on screen: the live session, or one opened in
   *  the Journal (where `at` comes from the time field rather than the clock). */
  const submitDose = (to: ExperienceDetail, at: string) =>
    run(async () => {
      if (!dSub.trim()) return;
      const res = await logDose({
        experience_id: to.id,
        substance_name: dSub.trim(),
        amount: dAmt.trim() ? Number(dAmt) : null,
        unit: dUnit,
        route: dRoute,
        taken_at: at,
      });
      // The same deterministic checker the desktop runs — it does not go quiet just
      // because you're on a phone.
      doseWarnings = res.warnings;
      dSub = dAmt = "";
      await refresh();
    });

  function startEdit(d: Dose) {
    editing = d;
    editingEvent = null;
    eAmt = d.amount?.toString() ?? "";
    eUnit = d.unit ?? "mg";
    eRoute = d.route ?? "oral";
    eWhen = isoToLocalInput(d.taken_at);
    eNote = d.note ?? "";
  }

  const saveEdit = () =>
    run(async () => {
      if (!editing) return;
      await updateDose(editing.id, {
        substance_name: editing.substance_name,
        amount: eAmt.trim() ? Number(eAmt) : null,
        unit: eUnit,
        route: eRoute,
        taken_at: localInputToIso(eWhen),
        note: eNote,
      });
      editing = null;
      await refresh();
    });

  const removeDose = (d: Dose) =>
    run(async () => {
      if (!confirm(`Delete the ${d.substance_name} dose?`)) return;
      await deleteDose(d.id);
      editing = null;
      await refresh();
    });

  function startEditEvent(t: TimelineEvent) {
    editingEvent = t;
    editing = null;
    evNote = t.note;
    evIntensity = t.intensity != null ? String(t.intensity) : "";
    evWhen = isoToLocalInput(t.at);
  }

  const saveEventEdit = () =>
    run(async () => {
      if (!editingEvent) return;
      await updateTimelineEvent(editingEvent.id, {
        at: localInputToIso(evWhen),
        note: evNote.trim(),
        mood: editingEvent.mood,
        intensity: evIntensity.trim() ? Number(evIntensity) : null,
      });
      editingEvent = null;
      await refresh();
    });

  const removeEvent = (id: number) =>
    run(async () => {
      if (!confirm("Delete this note?")) return;
      await deleteTimelineEvent(id);
      editingEvent = null;
      await refresh();
    });

  const submitNote = (to: ExperienceDetail, at: string) =>
    run(async () => {
      if (!note.trim()) return;
      // The note is saved as written, and nothing reads it over the user's
      // shoulder: the journal is private. Crisis guardrails live where the user
      // is *talking to* something — the Companion — and in the combo checker.
      await addTimelineEvent({
        experience_id: to.id,
        at,
        note: note.trim(),
        intensity: intensity.trim() ? Number(intensity) : null,
      });
      note = intensity = "";
      await refresh();
    });

  // ---- journal ----
  const openExperience = (id: number) =>
    run(async () => {
      open = await getExperience(id);
      editing = editingEvent = null;
      editEntry = false;
      doseWarnings = [];
      // Adding to something that already happened: default to when it happened.
      // A session that's still open is a different matter — there, "now" is right.
      dWhen = noteWhen = open.ended_at ? isoToLocalInput(open.started_at) : nowLocalInput();
    });

  // ---- editing an entry that's already in the journal ----
  function startEditEntry() {
    if (!open) return;
    editEntry = true;
    editing = editingEvent = null;
    enTitle = open.title;
    enStart = isoToLocalInput(open.started_at);
    enEnd = open.ended_at ? isoToLocalInput(open.ended_at) : "";
    enRating = open.rating != null ? String(open.rating) : "";
    enNotes = open.notes;
  }

  const saveEntry = () =>
    run(async () => {
      if (!open) return;
      // update_experience replaces the whole row; intention and setting aren't
      // editable on a phone, so they're passed through untouched.
      await updateExperience(open.id, {
        title: enTitle.trim(),
        intention: open.intention,
        setting: open.setting,
        notes: enNotes,
        rating: enRating.trim() ? Number(enRating) : null,
        started_at: localInputToIso(enStart),
        ended_at: enEnd.trim() ? localInputToIso(enEnd) : null,
      });
      editEntry = false;
      await refresh();
    });

  const removeEntry = () =>
    run(async () => {
      if (!open) return;
      if (!confirm(`Delete "${open.title || "this entry"}" and everything in it?`)) return;
      await deleteExperience(open.id);
      open = null;
      editEntry = false;
      await refresh();
    });

  const loadUsage = () =>
    run(async () => {
      usage = await usageBySubstance();
      showUsage = true;
    });

  // ---- combo ----
  const runCombo = () =>
    run(async () => {
      const names = comboText.split(/[,+\n]/).map((s) => s.trim()).filter(Boolean);
      if (names.length < 2) return;
      comboWarnings = await checkCombo(names);
    });

  // ---- reference ----
  const loadSubstances = () => run(async () => (substances = await listSubstances()));

  const lookUp = () =>
    run(async () => {
      const q = refQuery.trim();
      if (!q) return;
      // Doses come from the deterministic reference; prose comes from the corpus.
      // Never the other way round.
      pw = await pwLookup(q);
      hits = await knowledgeSearch(q, 6);
      searched = true;
    });

  const addToCatalogue = () =>
    run(async () => {
      if (!newSub.trim()) return;
      await addSubstance({ name: newSub.trim() });
      newSub = "";
      substances = await listSubstances();
    });

  // ---- companion ----
  // A Companion turn on a slow local model can take minutes. One long request
  // dies at mobile Safari's ~60s timeout — and instantly when the screen locks.
  // So the phone starts the turn as a *job* on the desktop and polls for the
  // result: every poll is a fresh, fast request, so a locked screen or a network
  // blip mid-generation costs nothing but another poll.
  async function awaitCompanionJob(job: number): Promise<Exclude<CompanionPoll, { status: "running" }>> {
    let misses = 0;
    for (;;) {
      await new Promise((r) => setTimeout(r, 2000));
      let poll: CompanionPoll;
      try {
        poll = await companionChatPoll(job);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        // Not transient — stop polling: the phone was unpaired (401), or the
        // desktop no longer knows the job (delivered elsewhere, or swept).
        if (msg.includes("paired") || msg.includes("unknown or expired")) throw e;
        // Anything else is likely a blip — Wi-Fi rejoining after the screen was
        // locked, the desktop napping. The job is still running at the desk, so
        // keep polling; give up only when the blips stop looking transient.
        if (++misses >= 5) throw e;
        continue;
      }
      misses = 0;
      if (poll.status !== "running") return poll;
    }
  }

  const send = () =>
    run(async () => {
      if (!ask.trim() || !model) return;
      const text = ask.trim();
      const next: ChatMsg[] = [...chat, { role: "user", content: text }];
      chat = next;
      ask = "";
      thinking = true;
      // The deterministic crisis layer runs on what's *said to* the Companion,
      // independent of the model's reply — same as the desktop. It never reads
      // the journal itself.
      // `next` already ends with `text`, which the backend appends itself.
      crisisScan(text, session?.id ?? null, next.slice(0, -1).filter((m) => m.role === "user").map((m) => m.content))
        .then((r) => { if (r.level !== "none") { chatCrisis = r; chatCrisisShown = false; } })
        .catch(() => {});
      try {
        let reply: CompanionReply;
        if (inTauri()) {
          // Desktop IPC has no timeout to outrun, and the job commands are
          // portal-only — call the Companion directly.
          reply = await companionChat(model, next, session?.id ?? null, null);
        } else {
          const { job } = await companionChatStart(model, next, session?.id ?? null, null);
          const done = await awaitCompanionJob(job);
          if (done.status === "error") throw new Error(done.error);
          reply = done.reply;
        }
        chat = [...next, { role: "assistant", content: reply.reply }];
        // The Companion has tools: it may have logged something. Reflect that.
        if (reply.journal_changed) await refresh();
      } finally {
        thinking = false;
      }
    });

  // Timestamps are stored as UTC ISO; convert to the phone's local time for
  // display. Slicing the raw string shows UTC — hours off from the clock on
  // the wall, and the wrong *date* for any evening session.
  const pad2 = (n: number) => String(n).padStart(2, "0");
  // <input type="datetime-local"> works in local time and has no zone; the journal
  // stores UTC. Shift across the offset in both directions rather than slicing an
  // ISO string, which silently backdates an evening entry by a day.
  const localOffset = (d: Date) => new Date(d.getTime() - d.getTimezoneOffset() * 60000);
  const nowLocalInput = () => localOffset(new Date()).toISOString().slice(0, 16);
  const isoToLocalInput = (iso: string) => localOffset(new Date(iso)).toISOString().slice(0, 16);
  const localInputToIso = (local: string) =>
    local ? new Date(local).toISOString() : new Date().toISOString();
  const hhmm = (iso: string) => {
    const d = new Date(iso);
    return `${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
  };
  /** Doses and notes live in two tables but tell one story, and the phone shows
   *  them as a single list — so merge them into time order. Without this, a dose
   *  whose time you just corrected sits wherever it was logged. */
  type Row =
    | { kind: "dose"; at: string; dose: Dose }
    | { kind: "event"; at: string; event: TimelineEvent };
  const rowsOf = (e: ExperienceDetail): Row[] =>
    [
      ...e.doses.map((d) => ({ kind: "dose" as const, at: d.taken_at, dose: d })),
      ...e.timeline.map((t) => ({ kind: "event" as const, at: t.at, event: t })),
    ].sort((a, b) => new Date(a.at).getTime() - new Date(b.at).getTime());

  /** T-zero for a session: its first dose. Same rule as the desktop timeline —
   *  t+ is counted from ingestion, not from when the entry was opened. Null
   *  until something is logged, and callers then show wall-clock only. */
  const t0Of = (s: { doses: { taken_at: string }[] } | null | undefined) => {
    const doses = s?.doses ?? [];
    if (!doses.length) return null;
    return doses.reduce(
      (earliest, d) => (new Date(d.taken_at) < new Date(earliest) ? d.taken_at : earliest),
      doses[0].taken_at,
    );
  };
  /** `t+1:20` since the first dose; minus sign for anything logged before it. */
  const rel = (iso: string, t0: string | null) => {
    if (!t0) return "";
    const ms = new Date(iso).getTime() - new Date(t0).getTime();
    const mins = Math.floor(Math.abs(ms) / 60000);
    return ` (t${ms < 0 ? "−" : "+"}${Math.floor(mins / 60)}:${pad2(mins % 60)})`;
  };
  const day = (iso: string) => {
    const d = new Date(iso);
    return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
  };
  const range = (r: { min: number | null; max: number | null }) =>
    r.min == null && r.max == null ? "—" : `${r.min ?? "?"}–${r.max ?? "?"}`;
</script>

<svelte:head><title>Field Notes</title></svelte:head>

<!-- One entry's doses and notes in time order, every line a tap-to-edit target.
     The live session and an entry opened in the Journal render the same list:
     what you got wrong at 3am is usually only fixable the next morning. -->
{#snippet timelineList(e: ExperienceDetail)}
  <ul class="tl">
    {#each rowsOf(e) as r (r.kind + (r.kind === "dose" ? r.dose.id : r.event.id))}
      <li>
        {#if r.kind === "dose"}
          <button class="line" onclick={() => startEdit(r.dose)}>
            <span class="t">{hhmm(r.at)}<span class="rel">{rel(r.at, t0Of(e))}</span></span>
            <strong>{r.dose.substance_name}</strong>
            {r.dose.amount ?? ""}{r.dose.unit}
            <span class="muted">{r.dose.route}</span>
          </button>
        {:else}
          <button class="line" onclick={() => startEditEvent(r.event)}>
            <span class="t">{hhmm(r.at)}<span class="rel">{rel(r.at, t0Of(e))}</span></span>
            {r.event.note}
            {#if r.event.intensity != null}<span class="muted">· {r.event.intensity}/10</span>{/if}
          </button>
        {/if}
      </li>
    {/each}
  </ul>
{/snippet}

<!-- Editing a dose or a note, wherever it lives: the live session's timeline or an
     entry opened in the Journal. Only one line is ever being edited, so one form
     serves both and the two screens can't drift apart. -->
{#snippet editors()}
  {#if editingEvent}
    <div class="edit">
      <h3>Edit note</h3>
      <textarea rows="3" bind:value={evNote}></textarea>
      <input placeholder="Intensity 0–10 (optional)" inputmode="numeric" bind:value={evIntensity} />
      <input type="datetime-local" bind:value={evWhen} />
      <button class="primary" disabled={busy || !evNote.trim()} onclick={saveEventEdit}>Save</button>
      <button disabled={busy} onclick={() => (editingEvent = null)}>Cancel</button>
      <button class="danger-btn" disabled={busy} onclick={() => removeEvent(editingEvent!.id)}>Delete note</button>
    </div>
  {/if}

  {#if editing}
    <div class="edit">
      <h3>Edit {editing.substance_name}</h3>
      <div class="row">
        <input placeholder="Amount" inputmode="decimal" bind:value={eAmt} />
        <select bind:value={eUnit}>
          {#each ["mg", "µg", "g", "ml", "tab"] as u}<option>{u}</option>{/each}
        </select>
        <select bind:value={eRoute}>
          {#each ["oral", "insufflated", "sublingual", "vaporized", "rectal", "IM", "IV"] as r}<option>{r}</option>{/each}
        </select>
      </div>
      <input type="datetime-local" bind:value={eWhen} />
      <input placeholder="Note" bind:value={eNote} />
      <button class="primary" disabled={busy} onclick={saveEdit}>Save</button>
      <button disabled={busy} onclick={() => (editing = null)}>Cancel</button>
      <button class="danger-btn" disabled={busy} onclick={() => removeDose(editing!)}>Delete dose</button>
    </div>
  {/if}
{/snippet}

<!-- The entry itself: what it's called, when it happened, how it went. A plain
     note has no doses and no end, so it's only asked what it is. -->
{#snippet entryEditor()}
  <div class="edit">
    <h3>Edit entry</h3>
    <input placeholder="Title" bind:value={enTitle} />
    <input type="datetime-local" bind:value={enStart} />
    <p class="muted hint">When it started.</p>
    {#if open?.kind !== "note"}
      <input type="datetime-local" bind:value={enEnd} />
      <p class="muted hint">When it ended. Leave blank and it stays open.</p>
      <input placeholder="Rating 0–10 (optional)" inputmode="numeric" bind:value={enRating} />
    {/if}
    <textarea id="entry-writeup" rows="6" placeholder="How was it? (optional)" bind:value={enNotes}></textarea>
    <button class="primary" disabled={busy} onclick={saveEntry}>Save</button>
    <button disabled={busy} onclick={() => (editEntry = false)}>Cancel</button>
    <button class="danger-btn" disabled={busy} onclick={removeEntry}>Delete entry</button>
  </div>
{/snippet}

<main>
  {#if !paired}
    <section class="pane">
      <h1>Not paired</h1>
      <p>
        Open Field Notes on your desktop, go to <strong>Settings → Phone access</strong>, and scan the
        QR code with this phone.
      </p>
    </section>
  {:else}
    <header>
      <strong>Field Notes</strong>
      {#if session}
        <button class="link live" title="Rename this session" onclick={() => startRename(session!)}>
          ● {session.title || "Live session"} ✎
        </button>
      {:else}
        <span class="muted">No live session</span>
      {/if}
    </header>

    {#if renamingId != null}
      <section class="pane">
        <h2>Rename</h2>
        <input placeholder="Title" bind:value={renameText} />
        <button class="primary" disabled={busy} onclick={saveRename}>Save</button>
        <button disabled={busy} onclick={() => (renamingId = null)}>Cancel</button>
      </section>
    {/if}

    {#if err}<p class="banner danger">{err}</p>{/if}

    <!-- ---------------- NOW ---------------- -->
    {#if view === "now"}
      {#if !session}
        <!-- First, because it's the commonest thing anyone opens this app to do:
             record that they took something. No session, no write-up. -->
        <section class="pane">
          {#if qInto}
            <h2>Add to that entry</h2>
            <p class="muted">
              A second substance in the same night belongs with the first — that way they're
              checked against each other.
              <button class="link" onclick={() => (qInto = null)}>separate entry instead</button>
            </p>
          {:else}
            <h2>Log something you took</h2>
            <p class="muted">One substance, one time. No session to end, nothing to write.</p>
          {/if}

          {#if qRecents.length}
            <div class="chips">
              {#each qRecents as s}
                <button class="chip" class:on={qSub === s} onclick={() => pickRecent(s)}>{s}</button>
              {/each}
            </div>
          {/if}
          <input
            placeholder="Substance"
            bind:value={qSub}
            onblur={applyRemembered}
            autocapitalize="none"
          />

          <div class="row">
            <input placeholder="Amount" inputmode="decimal" bind:value={qAmt} />
            <select bind:value={qUnit}>
              {#each ["mg", "µg", "g", "ml", "tab"] as u}<option>{u}</option>{/each}
            </select>
            <select bind:value={qRoute}>
              {#each ["oral", "insufflated", "sublingual", "vaporized", "rectal", "IM", "IV"] as r}<option>{r}</option>{/each}
            </select>
          </div>

          <!-- The preset fills the field below rather than replacing it, so what
               will be saved is always on screen. -->
          <div class="chips">
            {#each whenPresets as p}
              <button class="chip" onclick={() => (qWhen = isoToLocalInput(p.at().toISOString()))}>
                {p.label}
              </button>
            {/each}
          </div>
          <input type="datetime-local" bind:value={qWhen} />
          <p class="muted hint">When you took it — today, or any day you're catching up on.</p>

          <button class="primary" disabled={busy || !qSub.trim()} onclick={submitQuickLog}>
            {busy ? "Saving…" : "Log it"}
          </button>

          {#each qWarnings as w}
            <p class="banner {w.severity}">{w.message}</p>
          {/each}

          {#if qSaved}
            <!-- The entry exists and is correctly timed; everything else about it
                 can wait. These are the two things anyone wants next. -->
            <div class="saved">
              <p><strong>{qSaved.title || "Logged"}</strong> · {hhmm(qSaved.at)} {day(qSaved.at)}</p>
              <button onclick={() => addNotesTo(qSaved!.id)}>Add notes to it</button>
              <button onclick={addAnother}>Log another into it</button>
            </div>
          {/if}
        </section>

        <section class="pane">
          <h2>No session running</h2>
          <p class="muted">Start one here, or carry on with one you started at the desk.</p>
          <input placeholder="Title (optional)" bind:value={newTitle} />
          <p class="muted hint">Leave it blank and it takes the name of the first substance you log.</p>
          <button class="primary" disabled={busy} onclick={startSession}>Start a session</button>
          <button disabled={busy} onclick={refresh}>Refresh</button>
        </section>

        <section class="pane">
          <h2>Or just write</h2>
          <p class="muted">A plain journal entry — no session, no doses.</p>
          <input placeholder="Title (optional)" bind:value={plainTitle} />
          <textarea rows="4" placeholder="Write anything." bind:value={plainBody}></textarea>
          <button class="primary" disabled={busy || !plainBody.trim()} onclick={submitPlainNote}>
            Save note
          </button>
          {#if noteSaved}<p class="muted">Saved to the journal.</p>{/if}
        </section>
      {:else}
        <section class="pane">
          <h2>Log a dose</h2>
          <input placeholder="Substance" bind:value={dSub} autocapitalize="none" />
          <div class="row">
            <input placeholder="Amount" inputmode="decimal" bind:value={dAmt} />
            <select bind:value={dUnit}>
              {#each ["mg", "µg", "g", "ml", "tab"] as u}<option>{u}</option>{/each}
            </select>
            <select bind:value={dRoute}>
              {#each ["oral", "insufflated", "sublingual", "vaporized", "rectal", "IM", "IV"] as r}<option>{r}</option>{/each}
            </select>
          </div>
          <button
            class="primary"
            disabled={busy || !dSub.trim()}
            onclick={() => submitDose(session!, new Date().toISOString())}
          >
            {busy ? "Logging…" : "Log dose"}
          </button>
          {#each doseWarnings as w}
            <p class="banner {w.severity}">{w.message}</p>
          {/each}
        </section>

        <section class="pane">
          <h2>Note</h2>
          <textarea rows="3" placeholder="How's it going?" bind:value={note}></textarea>
          <input placeholder="Intensity 0–10 (optional)" inputmode="numeric" bind:value={intensity} />
          <button
            class="primary"
            disabled={busy || !note.trim()}
            onclick={() => submitNote(session!, new Date().toISOString())}
          >
            Add note
          </button>
        </section>

        <section class="pane">
          <h2>Timeline</h2>
          {@render timelineList(session)}

          {@render editors()}

          <button disabled={busy} onclick={endSession}>End session</button>
        </section>
      {/if}
    {/if}

    <!-- ---------------- JOURNAL ---------------- -->
    {#if view === "journal"}
      {#if open && open.kind === "note"}
        <section class="pane">
          <button class="link" onclick={() => (open = null)}>‹ Back</button>
          <h2>{open.title || "Untitled note"}</h2>
          <p class="muted">{day(open.started_at)}</p>
          {#if editEntry}
            {@render entryEditor()}
          {:else}
            {#if open.notes}<p class="notes">{open.notes}</p>{/if}
            <button disabled={busy} onclick={startEditEntry}>Edit</button>
            <button disabled={busy} onclick={exportOpen}>Export</button>
          {/if}
        </section>
      {:else if open}
        <section class="pane">
          <button class="link" onclick={() => (open = null)}>‹ Back</button>
          <h2>{open.title || "Untitled"}</h2>
          <p class="muted">
            {day(open.started_at)}
            {#if open.ended_at}→ {day(open.ended_at)}{:else}· still open{/if}
            {#if open.rating != null}· {open.rating}/10{/if}
          </p>
          {@render timelineList(open)}

          {@render editors()}

          {#if editEntry}
            {@render entryEditor()}
          {:else}
            {#if open.notes}<p class="notes">{open.notes}</p>{/if}
            <button disabled={busy} onclick={startEditEntry}>Edit entry</button>
            <button disabled={busy} onclick={exportOpen}>Export</button>
          {/if}
        </section>

        {#if !editEntry}
          <section class="pane">
            <h2>Add to this entry</h2>
            <p class="muted">Something you took, or something you remember, that isn't logged yet.</p>
            <input placeholder="Substance" bind:value={dSub} autocapitalize="none" />
            <div class="row">
              <input placeholder="Amount" inputmode="decimal" bind:value={dAmt} />
              <select bind:value={dUnit}>
                {#each ["mg", "µg", "g", "ml", "tab"] as u}<option>{u}</option>{/each}
              </select>
              <select bind:value={dRoute}>
                {#each ["oral", "insufflated", "sublingual", "vaporized", "rectal", "IM", "IV"] as r}<option>{r}</option>{/each}
              </select>
            </div>
            <input type="datetime-local" bind:value={dWhen} />
            <button
              class="primary"
              disabled={busy || !dSub.trim()}
              onclick={() => submitDose(open!, localInputToIso(dWhen))}
            >
              Add dose
            </button>
            {#each doseWarnings as w}
              <p class="banner {w.severity}">{w.message}</p>
            {/each}

            <h3>Or a note</h3>
            <textarea rows="3" placeholder="What happened?" bind:value={note}></textarea>
            <input placeholder="Intensity 0–10 (optional)" inputmode="numeric" bind:value={intensity} />
            <input type="datetime-local" bind:value={noteWhen} />
            <button
              class="primary"
              disabled={busy || !note.trim()}
              onclick={() => submitNote(open!, localInputToIso(noteWhen))}
            >
              Add note
            </button>
          </section>
        {/if}
      {:else}
        <section class="pane">
          <h2>Journal</h2>
          <ul class="tl">
            {#each recent as e}
              <li>
                <button class="line" onclick={() => openExperience(e.id)}>
                  {#if e.kind === "note"}
                    <strong>{e.title || "Untitled note"}</strong>
                    <span class="muted">{day(e.started_at)} · note</span>
                  {:else}
                    <strong>{e.title || "Untitled"}</strong>
                    <span class="muted">
                      {day(e.started_at)} · {e.dose_count} dose{e.dose_count === 1 ? "" : "s"}
                      {#if !e.ended_at}
                        · live
                      {:else if !e.notes.trim()}
                        <!-- Says out loud that a quick log is still waiting for its
                             story, so "I'll write it up later" has somewhere to land. -->
                        · no notes yet
                      {/if}
                    </span>
                  {/if}
                </button>
              </li>
            {:else}
              <li class="muted">Nothing logged yet.</li>
            {/each}
          </ul>
        </section>

        <section class="pane">
          <h2>By substance</h2>
          {#if !showUsage}
            <button disabled={busy} onclick={loadUsage}>Show substance log</button>
          {:else}
            <ul class="tl">
              {#each usage as u}
                <li>
                  <strong>{u.substance_name}</strong>
                  <span class="muted">· {u.times_used} time{u.times_used === 1 ? "" : "s"}</span>
                </li>
              {/each}
            </ul>
          {/if}
        </section>
      {/if}
    {/if}

    <!-- ---------------- COMBO ---------------- -->
    {#if view === "combo"}
      <section class="pane">
        <h2>Check a combo</h2>
        <p class="muted">Two or more, separated by commas.</p>
        <input placeholder="e.g. MDMA, ketamine" bind:value={comboText} autocapitalize="none" />
        <button class="primary" disabled={busy} onclick={runCombo}>Check</button>
        {#if comboWarnings}
          {#if comboWarnings.length === 0}
            <p class="banner note">Nothing flagged between those. That isn't the same as "safe".</p>
          {:else}
            {#each comboWarnings as w}<p class="banner {w.severity}">{w.message}</p>{/each}
          {/if}
        {/if}
      </section>
    {/if}

    <!-- ---------------- REFERENCE ---------------- -->
    {#if view === "reference"}
      <section class="pane">
        <h2>Look up a substance</h2>
        <input placeholder="e.g. ketamine" bind:value={refQuery} autocapitalize="none" />
        <button class="primary" disabled={busy || !refQuery.trim()} onclick={lookUp}>
          {busy ? "Looking…" : "Look up"}
        </button>

        {#if pw}
          <h3>{pw.name} — doses</h3>
          {#each pw.roas as roa}
            <div class="roa">
              <strong>{roa.name}</strong> <span class="muted">{roa.units ?? ""}</span>
              <ul class="tl">
                <li>Threshold <span class="muted">{roa.threshold ?? "—"}</span></li>
                <li>Light <span class="muted">{range(roa.light)}</span></li>
                <li>Common <span class="muted">{range(roa.common)}</span></li>
                <li>Strong <span class="muted">{range(roa.strong)}</span></li>
                <li>Heavy <span class="muted">{roa.heavy ?? "—"}</span></li>
                {#if roa.onset}<li>Onset <span class="muted">{roa.onset}</span></li>{/if}
                {#if roa.total}<li>Total <span class="muted">{roa.total}</span></li>{/if}
              </ul>
            </div>
          {/each}
          {#if pw.interactions.length}
            <h3>Interactions</h3>
            {#each pw.interactions as i}
              <p class="banner {i.severity}">{i.name}{i.reason ? ` — ${i.reason}` : ""}</p>
            {/each}
          {/if}
        {:else if searched}
          <p class="muted">No dose data for that. The prose below may still help.</p>
        {/if}

        {#if hits.length}
          <h3>From the reference</h3>
          {#each hits as h}
            <div class="hit">
              <strong>{h.title}</strong>
              <span class="muted">· {h.section}</span>
              {#if h.thin}<span class="flag">thin entry</span>{/if}
              <p>{h.text}</p>
            </div>
          {/each}
        {:else if searched}
          <p class="muted">Nothing in the reference for that.</p>
        {/if}
      </section>

      <section class="pane">
        <h2>Your substances</h2>
        {#if !substances.length}
          <button disabled={busy} onclick={loadSubstances}>Show catalogue</button>
        {:else}
          <ul class="tl">
            {#each substances as s}
              <li><strong>{s.name}</strong> <span class="muted">{s.category ?? ""}</span></li>
            {/each}
          </ul>
        {/if}
        <input placeholder="Add a substance" bind:value={newSub} autocapitalize="none" />
        <button disabled={busy || !newSub.trim()} onclick={addToCatalogue}>Add</button>
      </section>
    {/if}

    <!-- ---------------- COMPANION ---------------- -->
    {#if view === "companion"}
      <section class="pane">
        <h2>Companion</h2>
        {#if !companionEnabled}
          <p class="muted">
            Companion disabled. It's turned off in Settings on the desktop. Everything else on this
            screen still works.
          </p>
        {:else if !ai}
          <p class="muted">Couldn't ask the desktop about its local model.</p>
          <button disabled={busy} onclick={loadAi}>Try again</button>
        {:else if !ai.installed}
          <p class="muted">
            The Companion runs on a local model, and this desktop doesn't have one installed.
            Install it over there — it's a big download, so it isn't something to start from a phone.
          </p>
        {:else if !ai.running}
          <p class="muted">
            The local model isn't running on the desktop right now. Everything else on this screen
            still works.
          </p>
          <button class="primary" disabled={busy || waking} onclick={wakeAi}>
            {waking ? "Starting it…" : "Start it on the desktop"}
          </button>
        {:else if !models.length}
          <p class="muted">
            The model server is running, but no models are installed. Pull one on the desktop.
          </p>
          <button disabled={busy} onclick={loadAi}>Check again</button>
        {:else}
          {#if models.length > 1}
            <select bind:value={model}>
              {#each models as m}<option>{m}</option>{/each}
            </select>
          {/if}
          {#if chatCrisis && chatCrisis.level !== "none"}
            <div class="banner danger">
              <strong>{chatCrisis.headline}</strong>
              {#if chatCrisis.presentation === "offer" && !chatCrisisShown}
                <!-- Same reasoning as the desktop: a hard moment gets an offer. -->
                <button onclick={() => (chatCrisisShown = true)}>Show me some options</button>
                <button onclick={() => (chatCrisis = null)}>No thanks</button>
              {:else}
                <ul>
                  {#each chatCrisis.resources as r}
                    <li>{r.label}{r.contact ? " — " : ""}{#if r.contact}<a href="tel:{r.contact}">{r.contact}</a>{/if}</li>
                  {/each}
                </ul>
                <button onclick={() => (chatCrisis = null)}>Dismiss</button>
              {/if}
            </div>
          {/if}
          <div class="chat">
            {#each chat as m}
              <p class="msg {m.role}">{m.content}</p>
            {/each}
            {#if thinking}<p class="msg assistant muted">…</p>{/if}
          </div>
          <textarea rows="2" placeholder="Say anything" bind:value={ask}></textarea>
          <button class="primary" disabled={thinking || !ask.trim()} onclick={send}>Send</button>
        {/if}
      </section>
    {/if}

    <nav>
      <button class:on={view === "now"} onclick={() => goTo("now")}>Now</button>
      <button class:on={view === "journal"} onclick={() => goTo("journal")}>Journal</button>
      <button class:on={view === "combo"} onclick={() => goTo("combo")}>Combo</button>
      <button class:on={view === "reference"} onclick={() => goTo("reference")}>Look up</button>
      {#if companionEnabled}
        <button class:on={view === "companion"} onclick={() => goTo("companion")}>Talk</button>
      {/if}
    </nav>
  {/if}
</main>

<style>
  :global(body) {
    margin: 0;
    background: #14161a;
    color: #e8eaed;
    font: 16px/1.5 -apple-system, system-ui, sans-serif;
    /* Thumb-reachable: the nav is pinned to the bottom, so keep clear of it. */
    padding-bottom: calc(5rem + env(safe-area-inset-bottom));
  }
  main { max-width: 34rem; margin: 0 auto; padding: 0.8rem; }
  header { display: flex; justify-content: space-between; align-items: baseline; padding: 0.4rem 0.2rem 0.8rem; }
  .live { color: #7ee787; font-size: 0.85rem; }
  .muted { color: #9aa2ad; }
  /* Sits under the field it explains, so it reads as part of it. */
  .hint { font-size: 0.85rem; margin: -0.2rem 0 0.6rem; }

  .pane { background: #1b1e24; border: 1px solid #2a2f38; border-radius: 14px; padding: 1rem; margin-bottom: 0.8rem; }
  h1 { font-size: 1.2rem; margin: 0 0 0.5rem; }
  h2 { font-size: 1rem; margin: 0 0 0.6rem; }
  h3 { font-size: 0.95rem; margin: 1rem 0 0.4rem; }

  input, select, textarea {
    width: 100%; box-sizing: border-box; font: inherit;
    background: #14161a; color: #e8eaed; border: 1px solid #2a2f38;
    border-radius: 10px; padding: 0.7rem 0.75rem; margin-bottom: 0.5rem;
  }
  .row { display: flex; gap: 0.5rem; }
  .row input { flex: 2; }
  .row select { flex: 1; }

  button {
    font: inherit; font-weight: 600; border-radius: 10px; border: 1px solid #2a2f38;
    background: #21252c; color: #e8eaed; padding: 0.7rem 1rem; width: 100%;
    /* Big enough to hit while your hands aren't steady. */
    min-height: 2.9rem; margin-bottom: 0.4rem;
  }
  button.primary { background: #6ea8fe; color: #10131a; border-color: #6ea8fe; }
  button.danger-btn { border-color: #ff6b6b; color: #ff9d9d; }
  button:disabled { opacity: 0.5; }

  /* A whole row you can tap — a target the size of the line it sits on. */
  button.line {
    text-align: left; background: none; border: none; padding: 0.55rem 0;
    min-height: 0; font-weight: 400; margin: 0;
  }
  button.link {
    display: inline; width: auto; min-height: 0; margin: 0; padding: 0 0 0 0.4rem;
    background: none; border: none; color: #9aa2ad; font-size: 0.85rem; font-weight: 400;
  }
  button.link.live { color: #7ee787; padding-left: 0; }

  .banner { border-radius: 10px; padding: 0.7rem 0.8rem; margin: 0.6rem 0 0; border: 1px solid; font-size: 0.92rem; }
  .banner.danger { border-color: #ff6b6b; background: rgba(255, 107, 107, 0.14); }
  .banner.caution { border-color: #ffb454; background: rgba(255, 180, 84, 0.12); }
  .banner.note { border-color: #6ea8fe; background: rgba(110, 168, 254, 0.1); }
  .banner ul { margin: 0.4rem 0 0; padding-left: 1.1rem; }
  .banner a { color: inherit; }

  .tl { list-style: none; padding: 0; margin: 0; }
  .tl li { border-top: 1px solid #2a2f38; padding: 0.5rem 0; font-size: 0.92rem; }
  .t { color: #9aa2ad; margin-right: 0.5rem; font-variant-numeric: tabular-nums; white-space: nowrap; }
  /* t+ offset: readable in the dark, but subordinate to the wall-clock time */
  .rel { opacity: 0.75; font-size: 0.9em; }

  /* One tap instead of a keyboard or a date picker — the difference between a
     dose getting logged and not. They wrap; a long substance list is fine. */
  .chips { display: flex; flex-wrap: wrap; gap: 0.4rem; margin-bottom: 0.5rem; }
  .chip {
    width: auto; margin: 0; min-height: 2.2rem; padding: 0.35rem 0.7rem;
    font-size: 0.85rem; font-weight: 400; border-radius: 999px; background: #21252c;
  }
  .chip.on { background: #6ea8fe; color: #10131a; border-color: #6ea8fe; }

  .saved { border-top: 1px solid #2a2f38; margin-top: 0.8rem; padding-top: 0.8rem; }
  .saved p { margin: 0 0 0.6rem; font-size: 0.92rem; }

  .edit { border-top: 1px solid #2a2f38; margin-top: 0.8rem; padding-top: 0.8rem; }
  .notes { white-space: pre-wrap; font-size: 0.92rem; }
  .roa { margin-bottom: 0.6rem; }
  .hit { border-top: 1px solid #2a2f38; padding-top: 0.6rem; margin-top: 0.6rem; font-size: 0.92rem; }
  .hit p { margin: 0.3rem 0 0; }
  .flag {
    font-size: 0.72rem; border: 1px solid #ffb454; color: #ffb454;
    border-radius: 6px; padding: 0 0.35rem; margin-left: 0.35rem;
  }

  .chat { max-height: 45vh; overflow-y: auto; margin-bottom: 0.6rem; }
  .msg { border-radius: 12px; padding: 0.6rem 0.75rem; margin: 0 0 0.5rem; white-space: pre-wrap; }
  .msg.user { background: #21252c; }
  .msg.assistant { background: #1f2a3a; }

  nav {
    position: fixed; left: 0; right: 0; bottom: 0;
    display: flex; gap: 0.3rem; padding: 0.5rem;
    background: #14161ae6; backdrop-filter: blur(8px); border-top: 1px solid #2a2f38;
  }
  nav button { flex: 1; padding: 0.6rem 0; margin: 0; font-size: 0.8rem; }
  nav button.on { background: #6ea8fe; color: #10131a; border-color: #6ea8fe; }
</style>
