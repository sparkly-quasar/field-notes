<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  Paste a trip log, see exactly what it will become, fix anything, save it as a
  past session. Used by both the desktop and the phone, so the two can't drift.

  The parsing is deterministic (tripimport.ts): every pasted line becomes one row
  here, visibly, and nothing is saved until the person says so. It styles itself
  from whichever page hosts it — the phone's tokens or the desktop's — falling
  back between the two variable names.
-->
<script lang="ts">
  import { listSubstances, pwNames, type Warning } from "$lib/api";
  import { parseTripLog, type ParsedRow, type CatalogueEntry } from "$lib/tripimport";
  import { saveTripLog, UNITS, type TripLine } from "$lib/quicklog";

  let {
    onsaved,
    oncancel,
  }: {
    onsaved: (r: { id: number; title: string; warnings: Warning[] }) => void;
    oncancel: () => void;
  } = $props();

  const ROUTES = ["oral", "insufflated", "sublingual", "vaporized", "rectal", "IM", "IV"];

  let raw = $state("");
  let rows = $state<ParsedRow[] | null>(null);
  let timing = $state<"clock" | "offset" | "none">("none");
  let startClock = $state<number | null>(null);
  let day = $state(""); // yyyy-mm-dd, for clock logs
  let startAt = $state(""); // datetime-local, for offset/untimed logs
  let title = $state("");
  let err = $state<string | null>(null);
  let busy = $state(false);
  let catalogue: CatalogueEntry[] = [];
  let clipboardOk = $state(typeof navigator !== "undefined" && !!navigator.clipboard?.readText);

  const pad2 = (n: number) => String(n).padStart(2, "0");
  const ymd = (d: Date) => `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
  const localInput = (d: Date) => `${ymd(d)}T${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
  const daysAgo = (n: number) => {
    const d = new Date();
    d.setDate(d.getDate() - n);
    return ymd(d);
  };

  async function pasteFromClipboard() {
    err = null;
    try {
      raw = await navigator.clipboard.readText();
      if (raw.trim()) await parse();
    } catch {
      // Some browsers only allow it from a secure page, or the person said no.
      clipboardOk = false;
      err = "Couldn't read the clipboard here — paste into the box instead.";
    }
  }

  async function parse() {
    err = null;
    if (!raw.trim()) return;
    if (!catalogue.length) {
      // Your own catalogue first (its spelling wins), then the dose reference's
      // 500-odd names and street names.
      const [mine, ref] = await Promise.all([
        listSubstances().catch(() => []),
        pwNames().catch(() => []),
      ]);
      catalogue = [...mine.map((s) => ({ name: s.name, aliases: s.aliases })), ...ref];
    }
    const out = parseTripLog(raw, catalogue);
    if (!out.rows.length) {
      err = "Nothing to import in that.";
      return;
    }
    rows = out.rows;
    timing = out.timing;
    startClock = out.startClockMin;
    // A log of times with no date: today if that time has already passed, else last night's.
    if (timing === "clock" && startClock != null) {
      const now = new Date();
      day = startClock <= now.getHours() * 60 + now.getMinutes() ? daysAgo(0) : daysAgo(1);
    } else {
      const d = new Date();
      d.setDate(d.getDate() - 1);
      d.setHours(21, 0, 0, 0);
      startAt = localInput(d);
    }
  }

  /** Absolute time of the first line. */
  const start = $derived.by((): Date | null => {
    if (timing === "clock" && startClock != null && day) {
      const [y, m, d] = day.split("-").map(Number);
      return new Date(y, m - 1, d, Math.floor(startClock / 60), startClock % 60);
    }
    return startAt ? new Date(startAt) : null;
  });

  const at = (r: ParsedRow) => (start ? new Date(start.getTime() + r.offsetMin * 60000) : null);
  const hhmm = (d: Date | null) => (d ? `${pad2(d.getHours())}:${pad2(d.getMinutes())}` : "—");
  /** T+ as the journal will show it: from the first dose. */
  const t0 = $derived(rows?.find((r) => r.kind === "dose")?.offsetMin ?? null);
  const rel = (r: ParsedRow) => {
    if (t0 == null) return "";
    const d = r.offsetMin - t0;
    const m = Math.abs(d);
    return `T${d < 0 ? "−" : "+"}${Math.floor(m / 60)}:${pad2(m % 60)}`;
  };
  const inFuture = $derived(!!rows && !!start && rows.some((r) => at(r)!.getTime() > Date.now()));

  function remove(i: number) {
    rows = rows!.filter((_, j) => j !== i);
  }

  async function save() {
    if (!rows || !start) return;
    err = null;
    if (inFuture) {
      err = "Some of these times are in the future — check the date.";
      return;
    }
    busy = true;
    try {
      const lines: TripLine[] = rows.map((r) => ({
        kind: r.kind,
        at: at(r)!.toISOString(),
        text: r.text,
        substance: r.substance,
        amount: r.amount,
        unit: r.unit,
        route: r.route,
        intensity: r.intensity,
      }));
      const res = await saveTripLog(title, lines);
      onsaved(res);
    } catch (e) {
      err = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  function amountInput(r: ParsedRow, v: string) {
    const n = Number(v.trim().replace(",", "."));
    r.amount = v.trim() && Number.isFinite(n) ? n : null;
  }
</script>

<div class="ti">
  {#if !rows}
    <p class="muted">
      Paste a log from anywhere — a notes app, a message. Lines like <code>8:43am - 35mg mesc</code>,
      <code>22:10 nothing yet</code> or <code>T+1:30 peak</code> each become one line of the timeline.
      Nothing is saved until you've checked it.
    </p>
    {#if clipboardOk}
      <button type="button" class="primary" onclick={pasteFromClipboard}>Paste from clipboard</button>
    {/if}
    <label for="ti-raw">{clipboardOk ? "Or paste it here" : "Paste it here"}</label>
    <textarea id="ti-raw" rows="8" bind:value={raw} placeholder={"8:43am - 35mg mesc\n10:43am - some nausea\n11am - feeling much better"}></textarea>
    {#if err}<p class="bad" role="alert">{err}</p>{/if}
    <div class="pair">
      <button type="button" onclick={oncancel}>Cancel</button>
      <button type="button" class="primary" disabled={!raw.trim()} onclick={parse}>Read it</button>
    </div>
  {:else}
    {#if timing === "clock"}
      <label for="ti-day">Which day did it start?</label>
      <div class="chips">
        <button type="button" class="chip" class:on={day === daysAgo(0)} onclick={() => (day = daysAgo(0))}>Today</button>
        <button type="button" class="chip" class:on={day === daysAgo(1)} onclick={() => (day = daysAgo(1))}>Yesterday</button>
        <button type="button" class="chip" class:on={day === daysAgo(2)} onclick={() => (day = daysAgo(2))}>2 days ago</button>
      </div>
      <input id="ti-day" type="date" bind:value={day} />
    {:else}
      <label for="ti-start">{timing === "offset" ? "When was T+0 — the first line?" : "When did it start? (No times in the log, so every line is placed here.)"}</label>
      <input id="ti-start" type="datetime-local" bind:value={startAt} />
    {/if}

    <label for="ti-title">Title (optional)</label>
    <input id="ti-title" bind:value={title} placeholder="Blank takes the first substance's name" />

    <p class="muted small">{rows.length} line{rows.length === 1 ? "" : "s"}. Tap Dose / Moment to switch one, × to drop it.</p>
    <ol class="rows">
      {#each rows as r, i (i)}
        <li class:dose={r.kind === "dose"}>
          <div class="head">
            <span class="time">{hhmm(at(r))}<span class="rel">{rel(r)}</span></span>
            <span class="seg" role="group" aria-label="Line type">
              <button type="button" class:on={r.kind === "dose"} aria-pressed={r.kind === "dose"} onclick={() => (r.kind = "dose")}>Dose</button>
              <button type="button" class:on={r.kind === "moment"} aria-pressed={r.kind === "moment"} onclick={() => (r.kind = "moment")}>Moment</button>
            </span>
            <button type="button" class="x" aria-label="Drop this line" onclick={() => remove(i)}>×</button>
          </div>
          {#if r.kind === "dose"}
            <div class="dose-fields">
              <input aria-label="Substance" placeholder="Substance" bind:value={r.substance} autocapitalize="none" />
              <input aria-label="Amount" inputmode="decimal" placeholder="Amount" value={r.amount ?? ""} oninput={(e) => amountInput(r, e.currentTarget.value)} />
              <select aria-label="Unit" bind:value={r.unit}>{#each UNITS as u}<option>{u}</option>{/each}</select>
              <select aria-label="Route" bind:value={r.route}>{#each ROUTES as rt}<option>{rt}</option>{/each}</select>
            </div>
            {#if r.text}<p class="orig">“{r.text}”</p>{/if}
          {:else}
            <textarea aria-label="Moment" rows="2" bind:value={r.text}></textarea>
            {#if r.intensity != null}<p class="orig">Intensity {r.intensity}/10</p>{/if}
          {/if}
        </li>
      {/each}
    </ol>

    {#if inFuture}<p class="bad" role="alert">Some of these times are in the future — check the day.</p>{/if}
    {#if err}<p class="bad" role="alert">{err}</p>{/if}
    <div class="pair">
      <button type="button" onclick={() => (rows = null)}>Back</button>
      <button type="button" class="primary" disabled={busy || !start} onclick={save}>{busy ? "Saving…" : "Save as a past session"}</button>
    </div>
  {/if}
</div>

<style>
  .ti {
    --ti-border: var(--field-border, var(--line, #555));
    --ti-field: var(--field, var(--bg, transparent));
    --ti-text-2: var(--text-2, var(--muted, #999));
    --ti-accent: var(--accent, #6ea8fe);
    --ti-on-accent: var(--on-accent, var(--accent-ink, #0b0e14));
    --ti-danger: var(--danger, #e06b6b);
    color: inherit;
  }
  .muted { color: var(--ti-text-2); }
  .small { font-size: 0.9em; }
  code { font-size: 0.9em; background: color-mix(in srgb, currentColor 10%, transparent); border-radius: 4px; padding: 0 0.25rem; }
  label { display: block; color: var(--ti-text-2); font-size: 0.92em; margin: 0.5rem 0 0.25rem; }
  input, select, textarea {
    width: 100%; box-sizing: border-box; font: inherit; color: inherit;
    background: var(--ti-field); border: 1px solid var(--ti-border); border-radius: 10px;
    padding: 0.6rem 0.7rem; margin-bottom: 0.5rem; min-height: 44px;
  }
  button {
    font: inherit; font-weight: 600; color: inherit; background: color-mix(in srgb, currentColor 8%, transparent);
    border: 1px solid var(--ti-border); border-radius: 10px; min-height: 44px; padding: 0 1rem; cursor: pointer;
  }
  button.primary { background: var(--ti-accent); color: var(--ti-on-accent); border-color: var(--ti-accent); }
  button:disabled { opacity: 0.5; cursor: default; }
  .pair { display: flex; gap: 0.5rem; margin-top: 0.4rem; }
  .pair > button { flex: 1; }
  .ti > button.primary { width: 100%; margin-bottom: 0.3rem; }
  .chips { display: flex; flex-wrap: wrap; gap: 0.4rem; margin-bottom: 0.5rem; }
  .chip { border-radius: 999px; font-weight: 500; font-size: 0.92em; }
  .chip.on { background: var(--ti-accent); color: var(--ti-on-accent); border-color: var(--ti-accent); }
  .bad { color: var(--ti-danger); font-weight: 600; }

  .rows { list-style: none; padding: 0; margin: 0.4rem 0; }
  .rows li { border: 1px solid var(--ti-border); border-radius: 12px; padding: 0.5rem 0.6rem; margin-bottom: 0.5rem; }
  .rows li.dose { border-color: var(--ti-accent); }
  .head { display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.4rem; }
  .time { font-variant-numeric: tabular-nums; font-weight: 600; min-width: 3.2rem; display: flex; flex-direction: column; line-height: 1.2; }
  .rel { font-size: 0.8em; font-weight: 400; color: var(--ti-text-2); }
  .seg { display: flex; flex: 1; gap: 0.25rem; }
  .seg button { flex: 1; min-height: 40px; padding: 0 0.4rem; font-size: 0.9em; }
  .seg button.on { background: var(--ti-accent); color: var(--ti-on-accent); border-color: var(--ti-accent); }
  .x { width: 44px; padding: 0; font-size: 1.3rem; font-weight: 400; }
  .dose-fields { display: grid; grid-template-columns: 1.6fr 1fr 0.9fr 1.3fr; gap: 0.35rem; }
  .dose-fields input, .dose-fields select { margin-bottom: 0; padding: 0.5rem; }
  .orig { margin: 0.35rem 0 0; font-size: 0.88em; color: var(--ti-text-2); overflow-wrap: anywhere; }
  @media (max-width: 420px) {
    .dose-fields { grid-template-columns: 1fr 1fr; }
  }
</style>
