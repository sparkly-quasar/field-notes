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
  import { clockTime } from "$lib/clock";
  import DateTimeField from "./DateTimeField.svelte";
  import { listSubstances, pwNames, type Warning } from "$lib/api";
  import { parseTripLog, decodePasted, type ParsedRow, type CatalogueEntry } from "$lib/tripimport";
  import { saveTripLog, recallDoseShape, UNITS, type TripLine } from "$lib/quicklog";
  import { formsFor } from "$lib/dosedetail";

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
  let datedFromLog = $state<string | null>(null); // the date the log itself gave
  let startAt = $state(""); // datetime-local, for offset/untimed logs
  let title = $state("");
  /** A 12-hour log with no am/pm: which half of the day its first time was in. */
  let halfAsked = $state(false);
  let half = $state<"am" | "pm" | null>(null);
  let datedGuess = $state<"am" | "pm" | null>(null); // from a time on the log's date line
  /** People the log tags doses with, and which of them wrote it ("" = none of them). */
  let people = $state<string[]>([]);
  let me = $state<string | null>(null);
  let writeup = $state("");
  let err = $state<string | null>(null);
  let tip = $state<string | null>(null);
  let busy = $state(false);
  let catalogue: CatalogueEntry[] = [];
  let clipboardOk = $state(typeof navigator !== "undefined" && !!navigator.clipboard?.readText);

  const pad2 = (n: number) => String(n).padStart(2, "0");
  const ymd = (d: Date) => `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
  const localInput = (d: Date) => `${ymd(d)}T${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
  /** Does this browser write dates day-first (28/02/2026)? Settles "03/04/2026". */
  const dayFirst = (() => {
    try {
      const parts = new Intl.DateTimeFormat(undefined).formatToParts(new Date(2026, 1, 28));
      return parts.findIndex((p) => p.type === "day") < parts.findIndex((p) => p.type === "month");
    } catch {
      return false;
    }
  })();
  const daysAgo = (n: number) => {
    const d = new Date();
    d.setDate(d.getDate() - n);
    return ymd(d);
  };

  async function pasteFromClipboard() {
    err = null;
    try {
      raw = decodePasted(await navigator.clipboard.readText());
      if (raw.trim()) await parse();
    } catch {
      // Some browsers only allow it from a secure page, or the person said no.
      // Not an error worth red: the box below works, so just point there.
      clipboardOk = false;
      tip = "This browser didn't let Field Notes read the clipboard. Paste into the box instead: press and hold, then Paste.";
    }
  }

  /** A paste into the box gets the same clean-up as "Paste from clipboard". */
  function onPaste(e: ClipboardEvent) {
    const text = e.clipboardData?.getData("text/plain");
    if (!text) return;
    const clean = decodePasted(text);
    if (clean === text) return;
    e.preventDefault();
    const box = e.currentTarget as HTMLTextAreaElement;
    raw = raw.slice(0, box.selectionStart) + clean + raw.slice(box.selectionEnd);
  }

  async function parse() {
    err = null;
    tip = null;
    raw = decodePasted(raw);
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
    const out = parseTripLog(raw, catalogue, { dayFirst });
    if (!out.rows.length) {
      err = "Nothing to import in that.";
      return;
    }
    // No route written and none earlier in the log: the route this person
    // usually logs this substance with, from quick log, beats a general default.
    for (const r of out.rows) {
      if (r.kind !== "dose" || !r.substance || (r.routeFrom !== null && r.routeFrom !== "typical")) continue;
      const mine = recallDoseShape(r.substance)?.route;
      if (mine && ROUTES.includes(mine)) [r.route, r.routeFrom] = [mine, "mine"];
    }
    rows = out.rows;
    timing = out.timing;
    startClock = out.startClockMin;
    datedFromLog = out.date;
    halfAsked = !!out.half;
    half = datedGuess = out.half?.guess ?? null;
    people = out.people;
    me = null;
    writeup = out.reflection;
    if (timing === "clock" && startClock != null) {
      day = out.date ?? likelyDay();
    } else {
      let d = new Date();
      d.setDate(d.getDate() - 1);
      if (out.date) {
        const [y, m, dd] = out.date.split("-").map(Number);
        d = new Date(y, m - 1, dd);
      }
      const at = out.dateClockMin ?? 21 * 60;
      d.setHours(Math.floor(at / 60), at % 60, 0, 0);
      startAt = localInput(d);
    }
  }

  /** The first line's time of day, once the half of the day is settled. */
  const firstClock = $derived(startClock == null ? null : startClock + (halfAsked && half === "pm" ? 720 : 0));
  /** The log's own date if it gave one. Otherwise: today if that time has already
   *  passed, else last night's. */
  function likelyDay() {
    const now = new Date();
    return firstClock != null && firstClock <= now.getHours() * 60 + now.getMinutes() ? daysAgo(0) : daysAgo(1);
  }
  function pickHalf(h: "am" | "pm") {
    half = h;
    if (!datedFromLog) day = likelyDay();
  }
  const ROUTE_FROM: Record<string, string> = {
    log: "Route not written: taken from an earlier dose of this in the log.",
    typical: "Route not written: the way this is usually taken.",
    mine: "Route not written: the one you usually log this with.",
  };
  const clockText = (min: number, h: "am" | "pm") => `${Math.floor(min / 60) % 12 || 12}:${pad2(min % 60)}${h}`;

  /** Someone else's dose: kept, as a line on the timeline rather than a dose of yours. */
  const theirs = (r: ParsedRow) => r.kind === "dose" && !!r.who && r.who !== me;
  const theirNote = (r: ParsedRow) =>
    `${r.who}: ${r.amount != null ? `${r.amount}${r.unit} ` : ""}${r.substance}${r.route !== "oral" ? ` (${r.route})` : ""}`;
  const missing = $derived(halfAsked && !half ? "am or pm" : people.length && me == null ? "who you are" : null);

  /** Absolute time of the first line. */
  const start = $derived.by((): Date | null => {
    if (timing === "clock" && firstClock != null && day) {
      if (halfAsked && !half) return null;
      const [y, m, d] = day.split("-").map(Number);
      return new Date(y, m - 1, d, Math.floor(firstClock / 60), firstClock % 60);
    }
    return startAt ? new Date(startAt) : null;
  });

  const at = (r: ParsedRow) => (start ? new Date(start.getTime() + r.offsetMin * 60000) : null);
  const hhmm = (d: Date | null) => (d ? clockTime(d) : "—");
  /** T+ as the journal will show it: from the first dose (of yours). */
  const t0 = $derived(rows?.find((r) => r.kind === "dose" && !theirs(r))?.offsetMin ?? null);
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
    if (!rows || !start || missing) return;
    err = null;
    if (inFuture) {
      err = "Some of these times are in the future — check the date.";
      return;
    }
    busy = true;
    try {
      const lines: TripLine[] = rows.map((r) => ({
        kind: theirs(r) ? "moment" : r.kind,
        at: at(r)!.toISOString(),
        text: theirs(r) ? theirNote(r) : r.text,
        substance: r.substance,
        amount: r.amount,
        unit: r.unit,
        route: r.route,
        intensity: r.intensity,
        form: theirs(r) ? "" : r.form,
      }));
      const res = await saveTripLog(title, lines, writeup);
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
    <textarea id="ti-raw" rows="8" bind:value={raw} onpaste={onPaste} placeholder={"8:43am - 35mg mesc\n10:43am - some nausea\n11am - feeling much better"}></textarea>
    {#if tip}<p class="tip" role="status">{tip}</p>{/if}
    {#if err}<p class="bad" role="alert">{err}</p>{/if}
    <div class="pair">
      <button type="button" onclick={oncancel}>Cancel</button>
      <button type="button" class="primary" disabled={!raw.trim()} onclick={parse}>Read it</button>
    </div>
  {:else}
    {#if timing === "clock" && halfAsked && startClock != null}
      <p class="ask">The log doesn't say am or pm. When was the first line?</p>
      <div class="chips" role="group" aria-label="Morning or evening">
        <button type="button" class="chip" class:on={half === "am"} aria-pressed={half === "am"} onclick={() => pickHalf("am")}>{clockText(startClock, "am")}</button>
        <button type="button" class="chip" class:on={half === "pm"} aria-pressed={half === "pm"} onclick={() => pickHalf("pm")}>{clockText(startClock, "pm")}</button>
      </div>
      {#if half && half === datedGuess}<p class="muted small dated">Guessed from the time at the top of the log.</p>{/if}
    {/if}
    {#if people.length}
      <p class="ask">Doses in this log are marked as {people.join(" and ")}'s. Which one is you?</p>
      <div class="chips" role="group" aria-label="Which one is you">
        {#each people as p}
          <button type="button" class="chip" class:on={me === p} aria-pressed={me === p} onclick={() => (me = p)}>{p}</button>
        {/each}
        <button type="button" class="chip" class:on={me === ""} aria-pressed={me === ""} onclick={() => (me = "")}>None of them</button>
      </div>
      <p class="muted small dated">Your doses are logged as doses. Everyone else's stay on the timeline as notes.</p>
    {/if}
    {#if timing === "clock"}
      <label for="ti-day">Which day did it start?</label>
      <div class="chips">
        <button type="button" class="chip" class:on={day === daysAgo(0)} onclick={() => (day = daysAgo(0))}>Today</button>
        <button type="button" class="chip" class:on={day === daysAgo(1)} onclick={() => (day = daysAgo(1))}>Yesterday</button>
        <button type="button" class="chip" class:on={day === daysAgo(2)} onclick={() => (day = daysAgo(2))}>2 days ago</button>
      </div>
      <input id="ti-day" type="date" bind:value={day} />
      {#if datedFromLog && day === datedFromLog}<p class="muted small dated">Taken from the date at the top of the log.</p>{/if}
    {:else}
      <label for="ti-start">{timing === "offset" ? "When was T+0 — the first line?" : "When did it start? (No times in the log, so every line is placed here.)"}</label>
      <DateTimeField id="ti-start" bind:value={startAt} variant="phone" />
      {#if datedFromLog && startAt.startsWith(datedFromLog)}<p class="muted small dated">Taken from the date at the top of the log.</p>{/if}
    {/if}

    <label for="ti-title">Title (optional)</label>
    <input id="ti-title" bind:value={title} placeholder="Blank takes the first substance's name" />

    <p class="muted small">{rows.length} line{rows.length === 1 ? "" : "s"}. Tap Dose / Moment to switch one, × to drop it.</p>
    <ol class="rows">
      {#each rows as r, i (i)}
        <li class:dose={r.kind === "dose" && !theirs(r)}>
          <div class="head">
            <span class="time">{hhmm(at(r))}<span class="rel">{rel(r)}</span></span>
            {#if r.kind === "dose" && r.who}<span class="who" class:mine={!theirs(r)}>{r.who}</span>{/if}
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
              <select aria-label="Route" bind:value={r.route} onchange={() => (r.routeFrom = "written")}>{#each ROUTES as rt}<option>{rt}</option>{/each}</select>
              {#if formsFor(r.substance).length}
                <!-- "2g fresh shrooms": fresh mushrooms count as about a tenth of their weight dried. -->
                <select aria-label="Form" bind:value={r.form}>
                  <option value="">Form not said</option>
                  {#each formsFor(r.substance) as f}<option value={f.value}>{f.label}</option>{/each}
                </select>
              {/if}
            </div>
            {#if r.routeFrom && ROUTE_FROM[r.routeFrom]}<p class="orig">{ROUTE_FROM[r.routeFrom]}</p>{/if}
            {#if theirs(r)}<p class="orig">Saved as a note: “{theirNote(r)}”</p>{:else if r.text}<p class="orig">“{r.text}”</p>{/if}
          {:else}
            <textarea aria-label="Moment" rows="2" bind:value={r.text}></textarea>
            {#if r.intensity != null}<p class="orig">Intensity {r.intensity}/10</p>{/if}
          {/if}
        </li>
      {/each}
    </ol>

    <label for="ti-writeup">Write-up</label>
    <textarea id="ti-writeup" rows={writeup ? 6 : 2} bind:value={writeup} placeholder="Anything written after the log lands here"></textarea>

    {#if missing}<p class="muted small">Choose {missing} above to save.</p>{/if}
    {#if inFuture}<p class="bad" role="alert">Some of these times are in the future — check the day.</p>{/if}
    {#if err}<p class="bad" role="alert">{err}</p>{/if}
    <div class="pair">
      <button type="button" onclick={() => (rows = null)}>Back</button>
      <button type="button" class="primary" disabled={busy || !start || !!missing} onclick={save}>{busy ? "Saving…" : "Save as a past experience"}</button>
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
  .dated { margin: -0.25rem 0 0.5rem; }
  .ask { margin: 0.6rem 0 0.35rem; font-weight: 600; }
  .who { font-size: 0.8em; padding: 0.1rem 0.5rem; border-radius: 999px; border: 1px solid var(--ti-border); color: var(--ti-text-2); }
  .who.mine { border-color: var(--ti-accent); color: inherit; }
  .bad { color: var(--ti-danger); font-weight: 600; }
  .tip { color: var(--text-2, var(--muted)); }

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
