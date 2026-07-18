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
    listSubstances,
    addSubstance,
    usageBySubstance,
    checkCombo,
    crisisScan,
    companionChat,
    aiStatus,
    aiStart,
    pwLookup,
    knowledgeSearch,
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

  type View = "now" | "journal" | "combo" | "reference" | "companion";

  let paired = $state(false);
  let view = $state<View>("now");
  let err = $state<string | null>(null);
  let busy = $state(false);

  let session = $state<ExperienceDetail | null>(null);
  let recent = $state<ExperienceSummary[]>([]);

  // dose entry
  let dSub = $state("");
  let dAmt = $state("");
  let dUnit = $state("mg");
  let dRoute = $state("oral");
  let doseWarnings = $state<Warning[]>([]);

  // editing a dose already logged
  let editing = $state<Dose | null>(null);
  let eAmt = $state("");
  let eUnit = $state("mg");
  let eRoute = $state("oral");
  let eNote = $state("");

  // timeline note
  let note = $state("");
  let intensity = $state("");

  // editing a timeline note already written
  let editingEvent = $state<TimelineEvent | null>(null);
  let evNote = $state("");
  let evIntensity = $state("");

  // renaming an experience (live session or an opened journal entry)
  let renamingId = $state<number | null>(null);
  let renameText = $state("");

  // plain note (kind: "note") — no session required
  let plainTitle = $state("");
  let plainBody = $state("");
  let noteSaved = $state(false);

  // journal browsing
  let open = $state<ExperienceDetail | null>(null);
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
  let ai = $state<AiStatus | null>(null);
  let models = $state<string[]>([]);
  let model = $state("");
  let chat = $state<ChatMsg[]>([]);
  let ask = $state("");
  let thinking = $state(false);
  let waking = $state(false);
  let chatCrisis = $state<CrisisResult | null>(null);

  onMount(async () => {
    captureToken();
    paired = inTauri() || hasToken();
    if (!paired) return;
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
  const startSession = () =>
    run(async () => {
      await createExperience({ title: "", started_at: new Date().toISOString() });
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
      // update_experience replaces the whole row, so everything except the
      // title is passed through from the loaded entry unchanged.
      const target =
        session?.id === renamingId ? session : open?.id === renamingId ? open : null;
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

  const submitDose = () =>
    run(async () => {
      if (!session || !dSub.trim()) return;
      const res = await logDose({
        experience_id: session.id,
        substance_name: dSub.trim(),
        amount: dAmt.trim() ? Number(dAmt) : null,
        unit: dUnit,
        route: dRoute,
        taken_at: new Date().toISOString(),
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
        taken_at: editing.taken_at,
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
  }

  const saveEventEdit = () =>
    run(async () => {
      if (!editingEvent) return;
      await updateTimelineEvent(editingEvent.id, {
        at: editingEvent.at,
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

  const submitNote = () =>
    run(async () => {
      if (!session || !note.trim()) return;
      // The note is saved as written, and nothing reads it over the user's
      // shoulder: the journal is private. Crisis guardrails live where the user
      // is *talking to* something — the Companion — and in the combo checker.
      await addTimelineEvent({
        experience_id: session.id,
        at: new Date().toISOString(),
        note: note.trim(),
        intensity: intensity.trim() ? Number(intensity) : null,
      });
      note = intensity = "";
      await refresh();
    });

  // ---- journal ----
  const openExperience = (id: number) => run(async () => (open = await getExperience(id)));

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
      crisisScan(text, session?.id ?? null)
        .then((r) => { if (r.level !== "none") chatCrisis = r; })
        .catch(() => {});
      try {
        const reply = await companionChat(model, next, session?.id ?? null, null);
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
  const hhmm = (iso: string) => {
    const d = new Date(iso);
    return `${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
  };
  const day = (iso: string) => {
    const d = new Date(iso);
    return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
  };
  const range = (r: { min: number | null; max: number | null }) =>
    r.min == null && r.max == null ? "—" : `${r.min ?? "?"}–${r.max ?? "?"}`;
</script>

<svelte:head><title>Field Notes</title></svelte:head>

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
        <section class="pane">
          <h2>No session running</h2>
          <p class="muted">Start one here, or carry on with one you started at the desk.</p>
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
          <button class="primary" disabled={busy || !dSub.trim()} onclick={submitDose}>
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
          <button class="primary" disabled={busy || !note.trim()} onclick={submitNote}>Add note</button>
        </section>

        <section class="pane">
          <h2>Timeline</h2>
          <ul class="tl">
            {#each session.doses as d}
              <li>
                <button class="line" onclick={() => startEdit(d)}>
                  <span class="t">{hhmm(d.taken_at)}</span>
                  <strong>{d.substance_name}</strong>
                  {d.amount ?? ""}{d.unit}
                  <span class="muted">{d.route}</span>
                </button>
              </li>
            {/each}
            {#each session.timeline as t}
              <li>
                <button class="line" onclick={() => startEditEvent(t)}>
                  <span class="t">{hhmm(t.at)}</span>
                  {t.note}
                  {#if t.intensity != null}<span class="muted">· {t.intensity}/10</span>{/if}
                </button>
              </li>
            {/each}
          </ul>

          {#if editingEvent}
            <div class="edit">
              <h3>Edit note</h3>
              <textarea rows="3" bind:value={evNote}></textarea>
              <input placeholder="Intensity 0–10 (optional)" inputmode="numeric" bind:value={evIntensity} />
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
              <input placeholder="Note" bind:value={eNote} />
              <button class="primary" disabled={busy} onclick={saveEdit}>Save</button>
              <button disabled={busy} onclick={() => (editing = null)}>Cancel</button>
              <button class="danger-btn" disabled={busy} onclick={() => removeDose(editing!)}>Delete dose</button>
            </div>
          {/if}

          <button disabled={busy} onclick={endSession}>End session</button>
        </section>
      {/if}
    {/if}

    <!-- ---------------- JOURNAL ---------------- -->
    {#if view === "journal"}
      {#if open && open.kind === "note"}
        <section class="pane">
          <button class="link" onclick={() => (open = null)}>‹ Back</button>
          <h2>{open.title || "Untitled note"} <button class="link" onclick={() => startRename(open!)}>rename</button></h2>
          <p class="muted">{day(open.started_at)}</p>
          {#if open.notes}<p class="notes">{open.notes}</p>{/if}
        </section>
      {:else if open}
        <section class="pane">
          <button class="link" onclick={() => (open = null)}>‹ Back</button>
          <h2>{open.title || "Untitled"} <button class="link" onclick={() => startRename(open!)}>rename</button></h2>
          <p class="muted">
            {day(open.started_at)}
            {#if open.ended_at}→ {day(open.ended_at)}{:else}· still open{/if}
            {#if open.rating != null}· {open.rating}/10{/if}
          </p>
          <ul class="tl">
            {#each open.doses as d}
              <li>
                <span class="t">{hhmm(d.taken_at)}</span>
                <strong>{d.substance_name}</strong> {d.amount ?? ""}{d.unit}
                <span class="muted">{d.route}</span>
              </li>
            {/each}
            {#each open.timeline as t}
              <li><span class="t">{hhmm(t.at)}</span> {t.note}</li>
            {/each}
          </ul>
          {#if open.notes}<p class="notes">{open.notes}</p>{/if}
        </section>
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
                      {#if !e.ended_at}· live{/if}
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
        {#if !ai}
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
              <ul>
                {#each chatCrisis.resources as r}
                  <li>{r.label}{r.contact ? " — " : ""}{#if r.contact}<a href="tel:{r.contact}">{r.contact}</a>{/if}</li>
                {/each}
              </ul>
              <button onclick={() => (chatCrisis = null)}>Dismiss</button>
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
      <button class:on={view === "companion"} onclick={() => goTo("companion")}>Talk</button>
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
  .t { color: #9aa2ad; margin-right: 0.5rem; font-variant-numeric: tabular-nums; }

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
