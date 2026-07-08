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
    ollamaUp,
    ollamaModels,
    companionChat,
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
  let showNewExp = $state(false);

  // dose form
  let dSubstance = $state("");
  let dAmount = $state("");
  let dUnit = $state("mg");
  let dRoute = $state("oral");

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
    selected = await getExperience(id);
  }

  async function submitNewExperience() {
    const e = await createExperience({
      title: neTitle || "Untitled experience",
      intention: neIntention,
      setting: neSetting,
      started_at: nowIso(),
    });
    neTitle = neIntention = neSetting = "";
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
      taken_at: nowIso(),
    });
    lastWarnings = res.warnings;
    dSubstance = dAmount = "";
    await openExperienceKeepWarnings(selected.id);
    await loadJournal();
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
            <span class="muted small">{fmtDate(selected.started_at)} · {fmtTime(selected.started_at)}{selected.ended_at ? " → " + fmtTime(selected.ended_at) : " · ongoing"}</span>
          </div>
          {#if selected.intention}<p><strong>Intention:</strong> {selected.intention}</p>{/if}
          {#if selected.setting}<p><strong>Setting:</strong> {selected.setting}</p>{/if}

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
                  <span class="dtime">{fmtTime(d.taken_at)}</span>
                  <span class="dname">{d.substance_name}</span>
                  <span class="damt">{d.amount ?? "?"} {d.unit}{d.route ? " · " + d.route : ""}</span>
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
            <button class="primary small-btn" onclick={submitDose}>Log dose</button>
          </div>
          <datalist id="subnames">
            {#each substances as s}<option value={s.name}></option>{/each}
          </datalist>

          <h3>Timeline</h3>
          {#if selected.timeline.length}
            <ul class="timeline">
              {#each selected.timeline as t}
                <li><span class="dtime">{fmtTime(t.at)}</span> {t.note}{t.intensity != null ? ` (${t.intensity}/10)` : ""}{t.mood ? ` · ${t.mood}` : ""}</li>
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
            <button class="primary small-btn" onclick={() => (showNewExp = !showNewExp)}>+ New</button>
          </div>

          {#if showNewExp}
            <div class="new-exp">
              <input placeholder="Title" bind:value={neTitle} />
              <input placeholder="Intention (optional)" bind:value={neIntention} />
              <input placeholder="Set & setting (optional)" bind:value={neSetting} />
              <button class="primary small-btn" onclick={submitNewExperience}>Start</button>
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
                <strong>{s.name}</strong>
                {#if s.category}<span class="muted small"> · {s.category}</span>{/if}
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

  .notice { border: 1px solid var(--caution); background: color-mix(in srgb, var(--caution) 12%, transparent); border-radius: 10px; padding: 0.8rem 1rem; line-height: 1.5; }
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
