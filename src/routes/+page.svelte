<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<script lang="ts">
  import { onMount } from "svelte";
  import {
    listExperiences,
    listSubstances,
    interactionClasses,
    usageBySubstance,
    getExperience,
    createExperience,
    endExperience,
    logDose,
    addTimelineEvent,
    addSubstance,
    updateExperience,
    updateDose,
    deleteExperience,
    deleteDose,
    deleteTimelineEvent,
    deleteSubstance,
    type Dose,
    ollamaUp,
    ollamaModels,
    companionChat,
    parseExperience,
    importExperience,
    type ParsedExperience,
    type ExperienceSummary,
    type ExperienceDetail,
    type Substance,
    type SubstanceUsage,
    type Warning,
    type ChatMsg,
  } from "$lib/api";

  type Tab = "journal" | "companion" | "substances" | "bysub";

  let acknowledged = $state(false);
  let tab = $state<Tab>("journal");

  let experiences = $state<ExperienceSummary[]>([]);
  let substances = $state<Substance[]>([]);
  let classesVocab = $state<string[]>([]);
  let usage = $state<SubstanceUsage[]>([]);
  let selected = $state<ExperienceDetail | null>(null);
  let lastWarnings = $state<Warning[]>([]);

  // new-experience form
  let neTitle = $state("");
  let neIntention = $state("");
  let neSetting = $state("");
  let neStart = $state("");
  let showNewExp = $state(false);

  // dose form
  let dSubstance = $state("");
  let dAmount = $state("");
  let dUnit = $state("mg");
  let dRoute = $state("oral");
  let dTime = $state("");

  // import-from-text state
  let showImport = $state(false);
  let importText = $state("");
  let importBusy = $state(false);
  let importErr = $state<string | null>(null);
  let importParsed = $state<ParsedExperience | null>(null);
  let importTitle = $state("");
  let importStart = $state("");

  // edit state
  let editExp = $state(false);
  let eTitle = $state("");
  let eIntention = $state("");
  let eSetting = $state("");
  let eNotes = $state("");
  let eRating = $state("");
  let eStart = $state("");
  let editingDoseId = $state<number | null>(null);
  let edSub = $state("");
  let edAmt = $state("");
  let edUnit = $state("mg");
  let edRoute = $state("");
  let edTime = $state("");

  // timeline form
  let tNote = $state("");
  let tMood = $state("");
  let tIntensity = $state("");

  // new-substance form
  let nsName = $state("");
  let nsCategory = $state("");
  let nsClasses = $state<string[]>([]);
  let nsDose = $state("");
  let nsNotes = $state("");

  // companion (local LLM)
  let cReady = $state<boolean | null>(null);
  let cModels = $state<string[]>([]);
  let cModel = $state("");
  let cMessages = $state<ChatMsg[]>([]);
  let cInput = $state("");
  let cSending = $state(false);
  let cShareSession = $state(true);

  const nowIso = () => new Date().toISOString();
  const fmtDate = (iso: string) => new Date(iso).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
  const fmtTime = (iso: string) => new Date(iso).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });

  // <input type="datetime-local"> <-> ISO helpers (local time)
  const localOffset = (d: Date) => new Date(d.getTime() - d.getTimezoneOffset() * 60000);
  const nowLocalInput = () => localOffset(new Date()).toISOString().slice(0, 16);
  const isoToLocalInput = (iso: string) => localOffset(new Date(iso)).toISOString().slice(0, 16);
  const localInputToIso = (local: string) => (local ? new Date(local).toISOString() : nowIso());

  onMount(async () => {
    classesVocab = await interactionClasses();
  });

  async function enter() {
    acknowledged = true;
    await Promise.all([loadJournal(), loadSubstances()]);
  }

  async function loadJournal() {
    experiences = await listExperiences();
  }
  async function loadSubstances() {
    substances = await listSubstances();
  }
  async function loadUsage() {
    usage = await usageBySubstance();
  }

  async function openExperience(id: number) {
    lastWarnings = [];
    editExp = false;
    editingDoseId = null;
    selected = await getExperience(id);
    dTime = nowLocalInput();
  }

  function openNewExp() {
    showNewExp = !showNewExp;
    if (showNewExp && !neStart) neStart = nowLocalInput();
  }

  // ---- import a past experience from pasted text ----
  async function openImport() {
    showImport = !showImport;
    if (showImport) {
      importParsed = null;
      importErr = null;
      await loadCompanion(); // populates cReady / cModels / cModel
    }
  }

  async function runParse() {
    if (!importText.trim() || !cModel || importBusy) return;
    importBusy = true;
    importErr = null;
    importParsed = null;
    try {
      const p = await parseExperience(cModel, importText);
      importParsed = p;
      importTitle = p.title || "Imported experience";
      importStart = p.started_at ? isoToLocalInput(p.started_at) : nowLocalInput();
    } catch (e) {
      importErr = typeof e === "string" ? e : String(e);
    } finally {
      importBusy = false;
    }
  }

  async function confirmImport() {
    if (!importParsed) return;
    const exp = await importExperience({
      ...importParsed,
      title: importTitle,
      started_at: localInputToIso(importStart),
    });
    importParsed = null;
    importText = "";
    showImport = false;
    await loadJournal();
    await openExperience(exp.id);
  }

  async function submitNewExperience() {
    const e = await createExperience({
      title: neTitle || "Untitled experience",
      intention: neIntention,
      setting: neSetting,
      started_at: localInputToIso(neStart),
    });
    neTitle = neIntention = neSetting = neStart = "";
    showNewExp = false;
    await loadJournal();
    await openExperience(e.id);
  }

  async function submitDose() {
    if (!selected || !dSubstance.trim()) return;
    const res = await logDose({
      experience_id: selected.id,
      substance_name: dSubstance.trim(),
      amount: dAmount ? parseFloat(dAmount) : null,
      unit: dUnit,
      route: dRoute,
      taken_at: localInputToIso(dTime),
    });
    lastWarnings = res.warnings;
    dSubstance = dAmount = "";
    dTime = nowLocalInput();
    await openExperienceKeepWarnings(selected.id);
    await loadJournal();
  }

  // ---- edit / delete ----
  function startEditExp() {
    if (!selected) return;
    eTitle = selected.title;
    eIntention = selected.intention;
    eSetting = selected.setting;
    eNotes = selected.notes;
    eRating = selected.rating != null ? String(selected.rating) : "";
    eStart = isoToLocalInput(selected.started_at);
    editExp = true;
  }

  async function saveExp() {
    if (!selected) return;
    await updateExperience(selected.id, {
      title: eTitle,
      intention: eIntention,
      setting: eSetting,
      notes: eNotes,
      rating: eRating ? parseInt(eRating) : null,
      started_at: localInputToIso(eStart),
      ended_at: selected.ended_at,
    });
    editExp = false;
    await openExperienceKeepWarnings(selected.id);
    await loadJournal();
  }

  async function delExp() {
    if (!selected || !confirm("Delete this experience and all its doses? This cannot be undone.")) return;
    await deleteExperience(selected.id);
    selected = null;
    await loadJournal();
  }

  async function delDose(id: number) {
    if (!selected) return;
    await deleteDose(id);
    await openExperienceKeepWarnings(selected.id);
    await loadJournal();
  }

  async function delTimeline(id: number) {
    if (!selected) return;
    await deleteTimelineEvent(id);
    await openExperienceKeepWarnings(selected.id);
  }

  function startEditDose(d: Dose) {
    editingDoseId = d.id;
    edSub = d.substance_name;
    edAmt = d.amount != null ? String(d.amount) : "";
    edUnit = d.unit;
    edRoute = d.route;
    edTime = isoToLocalInput(d.taken_at);
  }

  async function saveDose() {
    if (!selected || editingDoseId == null) return;
    await updateDose(editingDoseId, {
      substance_name: edSub.trim(),
      amount: edAmt ? parseFloat(edAmt) : null,
      unit: edUnit,
      route: edRoute,
      taken_at: localInputToIso(edTime),
    });
    editingDoseId = null;
    await openExperienceKeepWarnings(selected.id);
    await loadJournal();
  }

  async function delSubstance(id: number) {
    if (!confirm("Delete this substance? Logged doses keep their name but lose the link.")) return;
    await deleteSubstance(id);
    await loadSubstances();
  }

  // reload the detail but preserve the warning banner we just set
  async function openExperienceKeepWarnings(id: number) {
    selected = await getExperience(id);
  }

  async function submitTimeline() {
    if (!selected || !tNote.trim()) return;
    await addTimelineEvent({
      experience_id: selected.id,
      at: nowIso(),
      note: tNote.trim(),
      mood: tMood,
      intensity: tIntensity ? parseInt(tIntensity) : null,
    });
    tNote = tMood = tIntensity = "";
    await openExperienceKeepWarnings(selected.id);
  }

  async function finishExperience() {
    if (!selected) return;
    await endExperience(selected.id, nowIso(), null, selected.notes);
    await loadJournal();
    await openExperienceKeepWarnings(selected.id);
  }

  async function submitSubstance() {
    if (!nsName.trim()) return;
    await addSubstance({
      name: nsName.trim(),
      category: nsCategory,
      classes: nsClasses,
      dose_note: nsDose,
      notes: nsNotes,
    });
    nsName = nsCategory = nsDose = nsNotes = "";
    nsClasses = [];
    await loadSubstances();
  }

  function toggleClass(c: string) {
    nsClasses = nsClasses.includes(c) ? nsClasses.filter((x) => x !== c) : [...nsClasses, c];
  }

  async function loadCompanion() {
    cReady = await ollamaUp();
    if (cReady) {
      cModels = await ollamaModels();
      if (!cModel && cModels.length) cModel = cModels[0];
      if (!experiences.length) await loadJournal();
    }
  }

  // the experience the companion is aware of (most recent), if sharing is on
  const attachedExp = $derived(cShareSession && experiences.length ? experiences[0] : null);

  async function sendCompanion() {
    if (!cInput.trim() || !cModel || cSending) return;
    const history: ChatMsg[] = [...cMessages, { role: "user", content: cInput.trim() }];
    cMessages = history;
    cInput = "";
    cSending = true;
    try {
      const reply = await companionChat(cModel, history, attachedExp ? attachedExp.id : null);
      cMessages = [...cMessages, { role: "assistant", content: reply }];
    } catch (e) {
      cMessages = [...cMessages, { role: "assistant", content: `⚠️ ${typeof e === "string" ? e : String(e)}` }];
    } finally {
      cSending = false;
    }
  }

  async function goTab(t: Tab) {
    tab = t;
    selected = null;
    if (t === "bysub") await loadUsage();
    if (t === "substances") await loadSubstances();
    if (t === "journal") await loadJournal();
    if (t === "companion") await loadCompanion();
  }

  const sevClass = (s: string) => (s === "danger" ? "danger" : s === "caution" ? "caution" : "note");
</script>

{#if !acknowledged}
  <div class="gate">
    <div class="gate-card">
      <h1>Field Notes</h1>
      <p class="lead">A private, offline journal for tracking experiences — kept entirely on this device.</p>
      <div class="ack">
        <h2>Before you continue</h2>
        <ul>
          <li>This is a <strong>harm-reduction and journaling tool</strong>, not medical advice and not encouragement to use anything.</li>
          <li>Dose and interaction information here is a <strong>reference and safety backstop, not a prescription</strong> — it is incomplete and may be wrong. Always cross-check trusted sources.</li>
          <li>The interaction checker only flags some well-known dangerous combinations. <strong>Absence of a warning does not mean a combination is safe.</strong></li>
          <li>In an emergency, contact local emergency services or poison control immediately.</li>
          <li>Your data stays on this computer. Keep it secure.</li>
        </ul>
      </div>
      <button class="primary" onclick={enter}>I understand — continue</button>
    </div>
  </div>
{:else}
  <main>
    <header>
      <h1>Field Notes</h1>
      <nav>
        <button class:active={tab === "journal"} onclick={() => goTab("journal")}>Journal</button>
        <button class:active={tab === "companion"} onclick={() => goTab("companion")}>Companion</button>
        <button class:active={tab === "substances"} onclick={() => goTab("substances")}>Substances</button>
        <button class:active={tab === "bysub"} onclick={() => goTab("bysub")}>By substance</button>
      </nav>
    </header>

    <!-- ============ JOURNAL ============ -->
    {#if tab === "journal"}
      {#if selected}
        <section class="card">
          <button class="link" onclick={() => (selected = null)}>← All experiences</button>
          <div class="exp-head">
            <h2>{selected.title || "Untitled experience"}</h2>
            <span class="row-actions">
              {#if !editExp}<button class="link" onclick={startEditExp}>Edit</button>{/if}
              <button class="link danger-link" onclick={delExp}>Delete</button>
            </span>
          </div>
          <span class="muted small">{fmtDate(selected.started_at)} · {fmtTime(selected.started_at)}{selected.ended_at ? " → " + fmtTime(selected.ended_at) : " · ongoing"}</span>

          {#if editExp}
            <div class="edit-form">
              <label>Title<input bind:value={eTitle} /></label>
              <label>Started<input type="datetime-local" bind:value={eStart} /></label>
              <label>Intention<input bind:value={eIntention} /></label>
              <label>Setting<input bind:value={eSetting} /></label>
              <label>Notes<textarea bind:value={eNotes} rows="3"></textarea></label>
              <label>Rating (0–10)<input type="number" min="0" max="10" bind:value={eRating} /></label>
              <div class="row-actions">
                <button class="primary small-btn" onclick={saveExp}>Save</button>
                <button class="ghost small-btn" onclick={() => (editExp = false)}>Cancel</button>
              </div>
            </div>
          {:else}
            {#if selected.intention}<p><strong>Intention:</strong> {selected.intention}</p>{/if}
            {#if selected.setting}<p><strong>Setting:</strong> {selected.setting}</p>{/if}
            {#if selected.notes}<p><strong>Notes:</strong> {selected.notes}</p>{/if}
            {#if selected.rating != null}<p class="muted small">Rating: {selected.rating}/10</p>{/if}
          {/if}

          {#if lastWarnings.length}
            <div class="warnings">
              {#each lastWarnings as w}
                <div class="warn {sevClass(w.severity)}">
                  <strong>{w.severity.toUpperCase()}</strong> · {w.a} + {w.b}
                  <div>{w.message}</div>
                </div>
              {/each}
            </div>
          {/if}

          <h3>Doses</h3>
          {#if selected.doses.length}
            <ul class="doses">
              {#each selected.doses as d}
                <li>
                  {#if editingDoseId === d.id}
                    <div class="dose-form inline">
                      <input list="subnames" bind:value={edSub} />
                      <input type="number" step="any" bind:value={edAmt} class="narrow" />
                      <input bind:value={edUnit} class="narrow" />
                      <input bind:value={edRoute} class="narrow" />
                      <input type="datetime-local" bind:value={edTime} />
                      <button class="primary small-btn" onclick={saveDose}>Save</button>
                      <button class="ghost small-btn" onclick={() => (editingDoseId = null)}>Cancel</button>
                    </div>
                  {:else}
                    <span class="dtime">{fmtTime(d.taken_at)}</span>
                    <span class="dname">{d.substance_name}</span>
                    <span class="damt">{d.amount ?? "?"} {d.unit}{d.route ? " · " + d.route : ""}</span>
                    <span class="row-actions">
                      <button class="icon-btn" title="Edit dose" onclick={() => startEditDose(d)}>✎</button>
                      <button class="icon-btn" title="Delete dose" onclick={() => delDose(d.id)}>✕</button>
                    </span>
                  {/if}
                </li>
              {/each}
            </ul>
          {:else}
            <p class="muted small">No doses logged yet.</p>
          {/if}

          <div class="dose-form">
            <input list="subnames" placeholder="Substance" bind:value={dSubstance} />
            <input type="number" step="any" placeholder="Amount" bind:value={dAmount} />
            <input placeholder="unit" bind:value={dUnit} class="narrow" />
            <input placeholder="route" bind:value={dRoute} class="narrow" />
            <input type="datetime-local" bind:value={dTime} title="Time taken" />
            <button class="primary small-btn" onclick={submitDose}>Log dose</button>
          </div>
          <datalist id="subnames">
            {#each substances as s}<option value={s.name}></option>{/each}
          </datalist>

          <h3>Timeline</h3>
          {#if selected.timeline.length}
            <ul class="timeline">
              {#each selected.timeline as t}
                <li>
                  <span class="dtime">{fmtTime(t.at)}</span>
                  <span class="tl-note">{t.note}{t.intensity != null ? ` (${t.intensity}/10)` : ""}{t.mood ? ` · ${t.mood}` : ""}</span>
                  <button class="icon-btn" title="Delete" onclick={() => delTimeline(t.id)}>✕</button>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="muted small">No timeline notes yet.</p>
          {/if}
          <div class="dose-form">
            <input placeholder="How are you feeling?" bind:value={tNote} />
            <input placeholder="mood" bind:value={tMood} class="narrow" />
            <input type="number" min="0" max="10" placeholder="0-10" bind:value={tIntensity} class="narrow" />
            <button class="ghost small-btn" onclick={submitTimeline}>Add note</button>
          </div>

          {#if !selected.ended_at}
            <button class="ghost" onclick={finishExperience}>End experience</button>
          {/if}
        </section>
      {:else}
        <section class="card">
          <div class="exp-head">
            <h2>Experiences</h2>
            <span class="row-actions">
              <button class="ghost small-btn" onclick={openImport}>Import from text</button>
              <button class="primary small-btn" onclick={openNewExp}>+ New</button>
            </span>
          </div>

          {#if showNewExp}
            <div class="new-exp">
              <input placeholder="Title" bind:value={neTitle} />
              <input type="datetime-local" bind:value={neStart} title="Start time" />
              <input placeholder="Intention (optional)" bind:value={neIntention} />
              <input placeholder="Set & setting (optional)" bind:value={neSetting} />
              <button class="primary small-btn" onclick={submitNewExperience}>Start</button>
            </div>
          {/if}

          {#if showImport}
            <div class="import-panel">
              {#if cReady === null}
                <p class="muted">Checking for a local model…</p>
              {:else if !cReady}
                <p class="notice">Import uses a local model (via Ollama) to read your text — nothing leaves this computer. Start Ollama (or install it with Cairn), then try again.</p>
              {:else if !importParsed}
                <p class="muted small">Paste a past experience in your own words. The local model extracts the substances, doses, and timeline for you to review before saving.</p>
                {#if cModels.length}
                  <select bind:value={cModel} class="model-sel">
                    {#each cModels as m}<option value={m}>{m}</option>{/each}
                  </select>
                {/if}
                <textarea class="import-text" rows="6" placeholder="e.g. Last Saturday around 9pm I took 100mg of MDMA at a friend's place. About an hour later I redosed 50mg…" bind:value={importText}></textarea>
                {#if importErr}<p class="notice bad-notice">{importErr}</p>{/if}
                <div class="row-actions">
                  <button class="primary small-btn" disabled={importBusy || !importText.trim() || !cModels.length} onclick={runParse}>
                    {importBusy ? "Reading…" : "Read & preview"}
                  </button>
                  <button class="ghost small-btn" onclick={() => (showImport = false)}>Cancel</button>
                </div>
                {#if importBusy}<p class="muted small">Reading your account — this can take up to a minute on larger models.</p>{/if}
              {:else}
                <p class="muted small">Review what was found, then import. You can edit or delete anything afterward.</p>
                <div class="new-exp">
                  <input placeholder="Title" bind:value={importTitle} />
                  <input type="datetime-local" bind:value={importStart} title="Start time" />
                </div>
                {#if importParsed.doses.length}
                  <h3>Doses found</h3>
                  <ul class="doses">
                    {#each importParsed.doses as d}
                      <li><span class="dname">{d.substance}</span><span class="damt">{d.amount ?? "?"} {d.unit}{d.route ? " · " + d.route : ""}</span></li>
                    {/each}
                  </ul>
                {:else}
                  <p class="notice">No doses were detected — you can still import and add them by hand.</p>
                {/if}
                {#if importParsed.timeline.length}
                  <h3>Timeline</h3>
                  <ul class="timeline">
                    {#each importParsed.timeline as t}<li><span class="tl-note">{t.note}{t.intensity != null ? ` (${t.intensity}/10)` : ""}</span></li>{/each}
                  </ul>
                {/if}
                <div class="row-actions">
                  <button class="primary small-btn" onclick={confirmImport}>Import</button>
                  <button class="ghost small-btn" onclick={() => (importParsed = null)}>Back</button>
                </div>
              {/if}
            </div>
          {/if}

          {#if experiences.length}
            <ul class="exp-list">
              {#each experiences as e}
                <li>
                  <button class="exp-row" onclick={() => openExperience(e.id)}>
                    <div>
                      <strong>{e.title || "Untitled"}</strong>
                      <span class="muted small">{fmtDate(e.started_at)}{e.ended_at ? "" : " · ongoing"}</span>
                    </div>
                    <div class="exp-meta">
                      {#each e.substances as s}<span class="pill">{s}</span>{/each}
                      <span class="muted small">{e.dose_count} dose{e.dose_count === 1 ? "" : "s"}</span>
                    </div>
                  </button>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="muted">No experiences yet. Start one to begin logging.</p>
          {/if}
        </section>
      {/if}
    {/if}

    <!-- ============ COMPANION ============ -->
    {#if tab === "companion"}
      <section class="card companion">
        <div class="exp-head">
          <h2>Companion</h2>
          {#if cReady && cModels.length}
            <select bind:value={cModel} class="model-sel">
              {#each cModels as m}<option value={m}>{m}</option>{/each}
            </select>
          {/if}
        </div>

        {#if cReady === null}
          <p class="muted">Checking for a local model…</p>
        {:else if !cReady}
          <p class="notice">
            No local model is running. Companion talks only to a model on <em>this</em> computer
            (via Ollama) — nothing you say leaves the device. Start Ollama (or install it with
            <strong>Cairn</strong>), then reopen this tab.
          </p>
        {:else if !cModels.length}
          <p class="notice">Ollama is running but no models are installed. Pull one (e.g. <code>ollama pull qwen3:8b</code>), then reopen this tab.</p>
        {:else}
          <p class="muted small disclaimer">
            A calm harm-reduction companion, running locally. Not medical advice. In an emergency,
            contact emergency services or poison control.
          </p>

          <label class="share">
            <input type="checkbox" bind:checked={cShareSession} />
            Share current session with the companion
            {#if attachedExp}<span class="muted small"> · aware of “{attachedExp.title || "Untitled"}”</span>{/if}
          </label>

          <div class="chat">
            {#if !cMessages.length}
              <p class="muted small chat-empty">Say hello, or ask about how you're feeling. It can see your logged doses if sharing is on.</p>
            {/if}
            {#each cMessages as m}
              <div class="bubble {m.role}">{m.content}</div>
            {/each}
            {#if cSending}<div class="bubble assistant muted">…</div>{/if}
          </div>

          <div class="chat-input">
            <input
              placeholder="Type a message…"
              bind:value={cInput}
              onkeydown={(e) => e.key === "Enter" && sendCompanion()}
            />
            <button class="primary small-btn" disabled={cSending} onclick={sendCompanion}>Send</button>
          </div>
        {/if}
      </section>
    {/if}

    <!-- ============ SUBSTANCES ============ -->
    {#if tab === "substances"}
      <section class="card">
        <h2>Substances</h2>
        <p class="muted small">Catalogue substances you track. Assign classes so the safety checker can flag interactions — or leave them blank and common ones are auto-classified.</p>

        <div class="new-sub">
          <input placeholder="Name" bind:value={nsName} />
          <input placeholder="Category (optional)" bind:value={nsCategory} />
          <input placeholder="Dose notes (optional)" bind:value={nsDose} />
          <div class="classes">
            {#each classesVocab as c}
              <button type="button" class="chip" class:on={nsClasses.includes(c)} onclick={() => toggleClass(c)}>{c}</button>
            {/each}
          </div>
          <button class="primary small-btn" onclick={submitSubstance}>Add substance</button>
        </div>

        {#if substances.length}
          <ul class="sub-list">
            {#each substances as s}
              <li>
                <div class="sub-head">
                  <span><strong>{s.name}</strong>{#if s.category}<span class="muted small"> · {s.category}</span>{/if}</span>
                  <button class="icon-btn" title="Delete substance" onclick={() => delSubstance(s.id)}>✕</button>
                </div>
                <div class="classes ro">
                  {#each s.classes as c}<span class="chip on">{c}</span>{/each}
                </div>
                {#if s.dose_note}<div class="muted small">{s.dose_note}</div>{/if}
              </li>
            {/each}
          </ul>
        {:else}
          <p class="muted">No substances catalogued yet.</p>
        {/if}
      </section>
    {/if}

    <!-- ============ BY SUBSTANCE ============ -->
    {#if tab === "bysub"}
      <section class="card">
        <h2>By substance</h2>
        {#if usage.length}
          {#each usage as u}
            <div class="usage">
              <div class="usage-head">
                <strong>{u.substance_name}</strong>
                <span class="muted small">{u.times_used} dose{u.times_used === 1 ? "" : "s"}</span>
              </div>
              <ul class="doses">
                {#each u.doses as d}
                  <li>
                    <span class="dtime">{fmtDate(d.taken_at)}</span>
                    <span class="damt">{d.amount ?? "?"} {d.unit}{d.route ? " · " + d.route : ""}</span>
                  </li>
                {/each}
              </ul>
            </div>
          {/each}
        {:else}
          <p class="muted">No doses logged yet.</p>
        {/if}
      </section>
    {/if}

    <footer>Offline · private · harm-reduction. Not medical advice.</footer>
  </main>
{/if}

<style>
  :global(:root) {
    --bg: #16181d;
    --card: #1e2127;
    --ink: #e7e9ee;
    --muted: #9aa0ab;
    --line: #2e323b;
    --accent: #6d8fb0;
    --accent-ink: #0c0e12;
    --danger: #e06b6b;
    --caution: #d6a24e;
    --note: #6fae8f;
  }
  :global(body) {
    margin: 0;
    background: var(--bg);
    color: var(--ink);
    font-family: Inter, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }

  .gate {
    min-height: 100vh; display: grid; place-items: center; padding: 1.5rem;
  }
  .gate-card {
    max-width: 540px; background: var(--card); border: 1px solid var(--line);
    border-radius: 16px; padding: 2rem;
  }
  .gate-card h1 { margin: 0 0 0.3rem; }
  .lead { color: var(--muted); margin-top: 0; }
  .ack { border: 1px solid var(--line); border-radius: 12px; padding: 1rem 1.2rem; margin: 1.2rem 0; }
  .ack h2 { margin: 0 0 0.6rem; font-size: 1rem; }
  .ack ul { margin: 0; padding-left: 1.1rem; }
  .ack li { margin: 0.45rem 0; line-height: 1.5; font-size: 0.92rem; }

  main { max-width: 720px; margin: 0 auto; padding: 1.6rem 1.4rem 2rem; }
  header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 1.2rem; flex-wrap: wrap; gap: 0.6rem; }
  header h1 { margin: 0; font-size: 1.4rem; }
  nav { display: inline-flex; gap: 4px; background: var(--card); border: 1px solid var(--line); border-radius: 999px; padding: 4px; }
  nav button { border: none; background: transparent; color: var(--muted); font: inherit; font-weight: 600; padding: 0.4rem 0.9rem; border-radius: 999px; cursor: pointer; }
  nav button.active { background: var(--accent); color: var(--accent-ink); }

  .card { background: var(--card); border: 1px solid var(--line); border-radius: 16px; padding: 1.4rem; }
  h2 { margin: 0 0 0.6rem; font-size: 1.15rem; }
  h3 { margin: 1.2rem 0 0.4rem; font-size: 0.95rem; color: var(--muted); text-transform: uppercase; letter-spacing: 0.04em; }
  p { line-height: 1.5; }
  .muted { color: var(--muted); }
  .small { font-size: 0.85rem; }

  button { font: inherit; cursor: pointer; border-radius: 9px; border: 1px solid transparent; }
  .primary { background: var(--accent); color: var(--accent-ink); font-weight: 600; padding: 0.7rem 1rem; }
  .primary:hover { filter: brightness(1.08); }
  .ghost { background: transparent; color: var(--muted); border-color: var(--line); padding: 0.7rem 1rem; margin-top: 0.8rem; }
  .small-btn { padding: 0.5rem 0.85rem; margin: 0; }
  .link { background: none; border: none; color: var(--accent); padding: 0; font-weight: 600; cursor: pointer; margin-bottom: 0.6rem; }

  input { font: inherit; background: var(--bg); color: var(--ink); border: 1px solid var(--line); border-radius: 8px; padding: 0.55rem 0.7rem; }
  input.narrow { width: 5.5rem; }

  .exp-head { display: flex; justify-content: space-between; align-items: center; gap: 0.8rem; }
  .exp-list, .sub-list, .doses, .timeline { list-style: none; padding: 0; margin: 0.6rem 0 0; }
  .exp-row { width: 100%; text-align: left; background: transparent; border: 1px solid var(--line); border-radius: 10px; padding: 0.8rem; margin-bottom: 0.5rem; display: flex; justify-content: space-between; align-items: center; gap: 0.8rem; color: var(--ink); }
  .exp-row:hover { border-color: var(--accent); }
  .exp-row strong { display: block; }
  .exp-meta { display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap; justify-content: flex-end; }
  .pill { font-size: 0.72rem; border: 1px solid var(--line); border-radius: 999px; padding: 0.1rem 0.5rem; color: var(--muted); }

  .new-exp, .dose-form, .new-sub { display: flex; flex-wrap: wrap; gap: 0.5rem; margin: 0.8rem 0; align-items: center; }
  .new-exp input, .new-sub input { flex: 1; min-width: 8rem; }
  .dose-form input:first-child { flex: 1; min-width: 8rem; }

  .doses li, .timeline li { display: flex; gap: 0.7rem; padding: 0.4rem 0; border-bottom: 1px solid var(--line); font-size: 0.92rem; align-items: baseline; }
  .doses li:last-child, .timeline li:last-child { border-bottom: none; }
  .dtime { color: var(--muted); font-variant-numeric: tabular-nums; min-width: 3.4rem; }
  .dname { font-weight: 600; }
  .damt { color: var(--muted); }

  .warnings { margin: 0.8rem 0; display: flex; flex-direction: column; gap: 0.5rem; }
  .warn { border-radius: 10px; padding: 0.7rem 0.9rem; font-size: 0.9rem; line-height: 1.45; border: 1px solid; }
  .warn.danger { border-color: var(--danger); background: color-mix(in srgb, var(--danger) 16%, transparent); }
  .warn.caution { border-color: var(--caution); background: color-mix(in srgb, var(--caution) 14%, transparent); }
  .warn.note { border-color: var(--note); background: color-mix(in srgb, var(--note) 12%, transparent); }

  .classes { display: flex; flex-wrap: wrap; gap: 0.35rem; width: 100%; }
  .classes.ro { margin-top: 0.3rem; }
  .chip { font-size: 0.75rem; border: 1px solid var(--line); border-radius: 999px; padding: 0.2rem 0.6rem; background: transparent; color: var(--muted); cursor: pointer; }
  .chip.on { background: var(--accent); color: var(--accent-ink); border-color: var(--accent); }

  .sub-list li { border: 1px solid var(--line); border-radius: 10px; padding: 0.7rem 0.9rem; margin-bottom: 0.5rem; }
  .usage { border: 1px solid var(--line); border-radius: 10px; padding: 0.8rem 1rem; margin-bottom: 0.6rem; }
  .usage-head { display: flex; justify-content: space-between; align-items: baseline; }

  .row-actions { display: inline-flex; gap: 0.5rem; align-items: center; margin-left: auto; }
  .icon-btn { background: transparent; border: 1px solid transparent; color: var(--muted); padding: 0.15rem 0.35rem; border-radius: 6px; font-size: 0.85rem; line-height: 1; }
  .icon-btn:hover { color: var(--ink); border-color: var(--line); }
  .link.danger-link { color: var(--danger); }
  .edit-form { display: flex; flex-direction: column; gap: 0.5rem; margin: 0.8rem 0; }
  .edit-form label { display: flex; flex-direction: column; gap: 0.2rem; font-size: 0.8rem; color: var(--muted); }
  .edit-form input, .edit-form textarea { font: inherit; background: var(--bg); color: var(--ink); border: 1px solid var(--line); border-radius: 8px; padding: 0.5rem 0.6rem; }
  .dose-form.inline { margin: 0; width: 100%; }
  .tl-note { flex: 1; }
  .sub-head { display: flex; justify-content: space-between; align-items: center; gap: 0.5rem; }

  .import-panel { border: 1px solid var(--line); border-radius: 12px; padding: 1rem; margin: 0.6rem 0 1rem; display: flex; flex-direction: column; gap: 0.6rem; }
  .import-text { font: inherit; background: var(--bg); color: var(--ink); border: 1px solid var(--line); border-radius: 8px; padding: 0.6rem 0.7rem; resize: vertical; width: 100%; box-sizing: border-box; }

  .notice { border: 1px solid var(--caution); background: color-mix(in srgb, var(--caution) 12%, transparent); border-radius: 10px; padding: 0.8rem 1rem; line-height: 1.5; }
  .notice.bad-notice { border-color: var(--danger); background: color-mix(in srgb, var(--danger) 12%, transparent); }
  .model-sel { font: inherit; background: var(--bg); color: var(--ink); border: 1px solid var(--line); border-radius: 8px; padding: 0.4rem 0.6rem; max-width: 55%; }
  .disclaimer { margin-top: 0; }
  .share { display: flex; align-items: center; gap: 0.4rem; font-size: 0.85rem; color: var(--muted); margin: 0.4rem 0 0.8rem; }
  .share input { width: auto; }
  .chat { display: flex; flex-direction: column; gap: 0.5rem; min-height: 220px; max-height: 46vh; overflow-y: auto; padding: 0.4rem; border: 1px solid var(--line); border-radius: 12px; background: var(--bg); }
  .chat-empty { margin: auto; text-align: center; max-width: 26ch; }
  .bubble { padding: 0.55rem 0.8rem; border-radius: 12px; max-width: 82%; line-height: 1.45; font-size: 0.92rem; white-space: pre-wrap; word-break: break-word; }
  .bubble.user { align-self: flex-end; background: var(--accent); color: var(--accent-ink); border-bottom-right-radius: 4px; }
  .bubble.assistant { align-self: flex-start; background: var(--card); border: 1px solid var(--line); border-bottom-left-radius: 4px; }
  .chat-input { display: flex; gap: 0.5rem; margin-top: 0.7rem; }
  .chat-input input { flex: 1; }

  footer { margin-top: 1.6rem; text-align: center; color: var(--muted); font-size: 0.8rem; }
</style>
