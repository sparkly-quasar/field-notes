<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  Usage stats (roadmap #2). One component for the desktop and the phone: a single
  column of cards that widens into a grid on a desk. Read-only, descriptive only:
  no streaks, scores, goals, or good/bad colouring, and it never raises an alert.
  The grouping rules live in stats.rs; time bucketing in $lib/stats.ts.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { usageStats, type UsageStats, type StatsDosePoint } from "$lib/api";
  import {
    RANGES, type RangeKey, sinceFor, ts, frequency, perDay, byHour, daysSince, median,
    niceScale, fmtNum, startOfDay, startOfWeek, dayKey,
  } from "$lib/stats";

  let {
    onOpen,
    onCheck,
  }: {
    /** Open a journal entry by id. */
    onOpen?: (experienceId: number) => void;
    /** Open the combo checker with these substances. */
    onCheck?: (names: string[]) => void;
  } = $props();

  // Per-device conveniences only. Storage can be missing or throw; the page works
  // the same without it.
  const PREF = "fieldnotes.stats";
  function loadPref(): { range?: RangeKey; hide?: boolean; pick?: string } {
    try { return JSON.parse(localStorage.getItem(PREF) ?? "{}"); } catch { return {}; }
  }
  const saved = loadPref();

  let range = $state<RangeKey>(saved.range ?? "90d");
  let hideNames = $state(saved.hide ?? false);
  /** `key|unit` of the series on the dose chart. */
  let pick = $state<string>(saved.pick ?? "");
  let data = $state<UsageStats | null>(null);
  let err = $state<string | null>(null);
  let loading = $state(false);
  let asTable = $state(false);
  let selected = $state<StatsDosePoint | null>(null);
  let chartW = $state(600);
  let freqW = $state(600);
  let heatW = $state(600);
  let heatPage = $state(0);

  $effect(() => {
    try { localStorage.setItem(PREF, JSON.stringify({ range, hide: hideNames, pick })); } catch {}
  });

  async function load() {
    loading = true;
    err = null;
    try {
      data = await usageStats(sinceFor(range));
    } catch (e) {
      err = typeof e === "string" ? e : String(e);
    } finally {
      loading = false;
    }
  }
  onMount(load);

  function setRange(r: RangeKey) {
    range = r;
    selected = null;
    heatPage = 0;
    load();
  }

  // ---- naming (with the shoulder-surfing toggle) ----
  const alias = $derived.by(() => {
    const keys = (data?.substances ?? []).map((s) => s.key).sort();
    return new Map(keys.map((k, i) => [k, `Substance ${String.fromCharCode(65 + (i % 26))}${i >= 26 ? Math.floor(i / 26) : ""}`]));
  });
  const realName = $derived(new Map((data?.substances ?? []).map((s) => [s.key, s.name])));
  const label = (key: string) => (hideNames ? alias.get(key) : realName.get(key)) ?? key;

  // ---- series choice ----
  const choices = $derived(
    (data?.substances ?? []).flatMap((s) =>
      s.series.map((u) => ({ id: `${s.key}|${u.unit}`, sub: s, series: u, split: s.series.length > 1 })),
    ),
  );
  const current = $derived(choices.find((c) => c.id === pick) ?? choices[0] ?? null);

  // ---- time window ----
  const now = Date.now();
  const sessionTimes = $derived(
    (data?.sessions ?? []).map((s) => ts(s.started_at)).filter((t): t is number => t != null),
  );
  const windowFrom = $derived.by(() => {
    const since = sinceFor(range);
    if (since) return Date.parse(since);
    return sessionTimes.length ? startOfDay(Math.min(...sessionTimes)) : startOfDay(now);
  });

  // ---- dose chart geometry ----
  const H = 220;
  const PAD = { l: 44, r: 12, t: 12, b: 28 };
  const plotted = $derived(
    (current?.series.points ?? [])
      .map((p) => ({ p, t: ts(p.taken_at) }))
      .filter((x): x is { p: StatsDosePoint; t: number } => x.t != null && x.p.amount != null),
  );
  const bands = $derived(current?.series.bands ?? null);
  const yScale = $derived.by(() => {
    const top = Math.max(
      0,
      ...plotted.map((x) => x.p.amount ?? 0),
      bands?.common.max ?? 0,
      bands?.light.max ?? 0,
    );
    return niceScale(top * 1.1);
  });
  const x = (t: number) => {
    const span = Math.max(now - windowFrom, 86_400_000);
    return PAD.l + ((t - windowFrom) / span) * (chartW - PAD.l - PAD.r);
  };
  const y = (v: number) => PAD.t + (1 - v / yScale.max) * (H - PAD.t - PAD.b);
  const yTicks = $derived(
    Array.from({ length: Math.round(yScale.max / yScale.step) + 1 }, (_, i) => i * yScale.step),
  );
  const xTicks = $derived.by(() => {
    const n = Math.max(2, Math.min(6, Math.floor((chartW - PAD.l) / 90)));
    return Array.from({ length: n }, (_, i) => windowFrom + ((now - windowFrom) * i) / (n - 1));
  });
  const bandRects = $derived.by(() => {
    if (!bands) return [];
    const out: { name: string; lo: number; hi: number; o: number }[] = [];
    const add = (name: string, r: { min: number | null; max: number | null }, o: number) => {
      if (r.min != null && r.max != null && r.max > r.min) out.push({ name, lo: r.min, hi: Math.min(r.max, yScale.max), o });
    };
    add("light", bands.light, 0.06);
    add("common", bands.common, 0.12);
    add("strong", bands.strong, 0.2);
    return out.filter((b) => b.lo < yScale.max);
  });

  // ---- spacing, for the picked substance ----
  const sub = $derived(current?.sub ?? null);
  const lastT = $derived(sub ? ts(sub.last_used) : null);
  const subRatings = $derived(
    sub
      ? (data?.sessions ?? []).filter((s) => s.substances.includes(sub.key) && s.rating != null).map((s) => s.rating as number)
      : [],
  );

  // ---- frequency ----
  const freq = $derived(frequency(sessionTimes, windowFrom, now));
  const freqScale = $derived(niceScale(Math.max(1, ...freq.map((f) => f.count)), 3));
  const FH = 140;

  // ---- heatmap ----
  const days = $derived(perDay(sessionTimes));
  const CELL = 14;
  const GAP = 3;
  const heatCols = $derived.by(() => {
    const totalWeeks = Math.ceil((startOfWeek(now) - startOfWeek(windowFrom)) / (7 * 86_400_000)) + 1;
    const fit = Math.max(4, Math.floor((heatW - 24) / (CELL + GAP)));
    return { total: Math.min(totalWeeks, 260), fit: Math.min(fit, totalWeeks) };
  });
  const heatPages = $derived(Math.max(1, Math.ceil(heatCols.total / heatCols.fit)));
  const heatWeeks = $derived.by(() => {
    const lastWeek = startOfWeek(now);
    const endOffset = heatPage * heatCols.fit;
    return Array.from({ length: heatCols.fit }, (_, i) => {
      const d = new Date(lastWeek);
      d.setDate(d.getDate() - 7 * (endOffset + heatCols.fit - 1 - i));
      return d.getTime();
    });
  });
  /** Columns that start a month and have room for its name (no "JanFeb"). */
  const monthLabels = $derived.by(() => {
    const out = new Set<number>();
    let last = -10;
    heatWeeks.forEach((w, i) => {
      const starts = i === 0 || new Date(w).getMonth() !== new Date(heatWeeks[i - 1]).getMonth();
      if (starts && i - last >= 3) {
        out.add(i);
        last = i;
      } else if (starts && out.has(last) && last === 0) {
        // A month that starts right after the first column wins the label.
        out.delete(last);
        out.add(i);
        last = i;
      }
    });
    return out;
  });
  const heatMax = $derived(Math.max(1, ...days.values()));
  function cellDay(week: number, dow: number) {
    const d = new Date(week);
    d.setDate(d.getDate() + dow);
    return d.getTime();
  }

  // ---- time of day ----
  const hours = $derived(
    byHour((current?.series.points ?? sub?.series.flatMap((s) => s.points) ?? [])
      .map((p) => ts(p.taken_at))
      .filter((t): t is number => t != null)),
  );
  const hourMax = $derived(Math.max(1, ...hours));

  const fmtDay = (t: number) => new Date(t).toLocaleDateString(undefined, { month: "short", day: "numeric" });
  const fmtBucket = (t: number, unit: "week" | "month") =>
    unit === "month"
      ? new Date(t).toLocaleDateString(undefined, { month: "short", year: "numeric" })
      : fmtDay(t);
  const fmtDayYear = (t: number) =>
    new Date(t).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
  const fmtWhen = (s: string) => {
    const t = ts(s);
    return t == null ? s : new Date(t).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
  };
  const plural = (n: number, w: string) => `${n} ${w}${n === 1 ? "" : "s"}`;
</script>

<div class="stats">
  <div class="controls">
    <div class="seg" role="group" aria-label="Time range">
      {#each RANGES as r}
        <button class:on={range === r.key} aria-pressed={range === r.key} onclick={() => setRange(r.key)}>{r.label}</button>
      {/each}
    </div>
    <label class="hide">
      <input type="checkbox" bind:checked={hideNames} /> Hide substance names
    </label>
  </div>

  {#if err}
    <p class="msg">{err}</p>
  {:else if !data}
    <p class="msg">{loading ? "Reading your journal…" : ""}</p>
  {:else if !data.total_sessions}
    <p class="msg">No doses logged {range === "all" ? "yet" : "in this range"}.</p>
  {:else}
    <div class="tiles">
      <div class="tile"><span class="big">{data.total_sessions}</span><span class="cap">{data.total_sessions === 1 ? "session" : "sessions"}</span></div>
      <div class="tile"><span class="big">{data.total_doses}</span><span class="cap">{data.total_doses === 1 ? "dose" : "doses"}</span></div>
      <div class="tile"><span class="big">{data.substances.length}</span><span class="cap">{data.substances.length === 1 ? "substance" : "substances"}</span></div>
    </div>

    <div class="chips" role="group" aria-label="Substance">
      {#each choices as c}
        <button class:on={current?.id === c.id} aria-pressed={current?.id === c.id} onclick={() => { pick = c.id; selected = null; }}>
          {label(c.sub.key)}{c.split ? ` · ${c.series.unit}` : ""}
        </button>
      {/each}
    </div>

    <div class="grid">
      <!-- dose over time -->
      <section class="card wide">
        <div class="head">
          <h3>Dose over time{current ? ` · ${label(current.sub.key)}` : ""}</h3>
          <button class="link" onclick={() => (asTable = !asTable)}>{asTable ? "Show chart" : "Show as table"}</button>
        </div>
        {#if current}
          {#if asTable}
            <div class="tablewrap">
              <table>
                <thead><tr><th>When</th><th>Amount</th><th>Route</th></tr></thead>
                <tbody>
                  {#each [...current.series.points].reverse() as p}
                    <tr>
                      <td>{fmtWhen(p.taken_at)}</td>
                      <td>{p.amount == null ? "not recorded" : `${fmtNum(p.amount)} ${current.series.unit}`}</td>
                      <td>{p.route || "—"}</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {:else}
            <div class="chart" bind:clientWidth={chartW}>
              <svg width={chartW} height={H} role="img" aria-label="Dose amount for each dose over time">
                {#each bandRects as b}
                  <rect x={PAD.l} width={chartW - PAD.l - PAD.r} y={y(b.hi)} height={Math.max(0, y(b.lo) - y(b.hi))} class="band" fill-opacity={b.o} />
                  <text x={chartW - PAD.r - 4} y={y(b.hi) + 12} text-anchor="end" class="bandlbl">{b.name}</text>
                {/each}
                {#each yTicks as t}
                  <line x1={PAD.l} x2={chartW - PAD.r} y1={y(t)} y2={y(t)} class="grid-l" />
                  <text x={PAD.l - 6} y={y(t) + 4} text-anchor="end" class="axis">{fmtNum(t)}</text>
                {/each}
                {#each xTicks as t, i}
                  <text x={x(t)} y={H - 8} text-anchor={i === 0 ? "start" : i === xTicks.length - 1 ? "end" : "middle"} class="axis">{fmtDay(t)}</text>
                {/each}
                {#each plotted as d (d.p.dose_id)}
                  <!-- the hit target is bigger than the dot, for a thumb -->
                  <circle
                    cx={x(d.t)} cy={y(d.p.amount ?? 0)} r="14" class="hit"
                    role="button" tabindex="0"
                    aria-label={`${fmtWhen(d.p.taken_at)}: ${fmtNum(d.p.amount ?? 0)} ${current.series.unit}`}
                    onclick={() => (selected = d.p)}
                    onmouseenter={() => (selected = d.p)}
                    onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); selected = d.p; } }}
                  />
                  <circle cx={x(d.t)} cy={y(d.p.amount ?? 0)} r={selected?.dose_id === d.p.dose_id ? 6 : 4.5} class="dot" class:sel={selected?.dose_id === d.p.dose_id} />
                {/each}
              </svg>
            </div>
            <p class="axisnote">{current.series.unit}{bands ? ` · shaded: dose reference ranges (${bands.route})` : ""}</p>
            {#if selected}
              <div class="detail">
                <span><strong>{selected.amount == null ? "?" : fmtNum(selected.amount)} {current.series.unit}</strong>
                  {selected.route ? ` · ${selected.route}` : ""} · {fmtWhen(selected.taken_at)}</span>
                {#if onOpen}<button class="link" onclick={() => onOpen?.(selected!.experience_id)}>Open entry</button>{/if}
              </div>
            {/if}
          {/if}
          {#if current.series.without_amount}
            <p class="note">{plural(current.series.without_amount, "dose")} without an amount {current.series.without_amount === 1 ? "isn't" : "aren't"} shown on the chart, but {current.series.without_amount === 1 ? "is" : "are"} counted everywhere else.</p>
          {/if}
        {/if}
      </section>

      <!-- spacing -->
      {#if sub}
        <section class="card">
          <h3>Spacing · {label(sub.key)}</h3>
          <div class="facts">
            {#if lastT != null}
              <div><span class="big">{daysSince(lastT)}</span><span class="cap">days since last use</span></div>
            {/if}
            <div><span class="big">{sub.sessions}</span><span class="cap">{sub.sessions === 1 ? "session" : "sessions"} in range</span></div>
            {#if sub.gaps_days.length}
              <div><span class="big">{fmtNum(median(sub.gaps_days) ?? 0)}</span><span class="cap">median days between</span></div>
              <div><span class="big">{fmtNum(Math.min(...sub.gaps_days))}</span><span class="cap">shortest gap (days)</span></div>
            {/if}
          </div>
          {#if subRatings.length}
            <p class="note">Average rating of these sessions: {fmtNum(subRatings.reduce((a, b) => a + b, 0) / subRatings.length)} ({plural(subRatings.length, "rated session")}).</p>
          {/if}
          {#if sub.routes.length > 1 || (sub.routes[0] && sub.routes[0][0])}
            <h4>Routes</h4>
            <ul class="bars">
              {#each sub.routes as [r, n]}
                <li><span class="lbl">{r || "not recorded"}</span><span class="track"><span class="fill" style:width={`${(n / sub.doses) * 100}%`}></span></span><span class="n">{n}</span></li>
              {/each}
            </ul>
          {/if}
        </section>
      {/if}

      <!-- frequency -->
      <section class="card">
        <h3>Sessions per {freq[0]?.unit ?? "week"}</h3>
        <div class="chart" bind:clientWidth={freqW}>
          <svg width={freqW} height={FH} role="img" aria-label="Number of sessions in each period">
            {#each [0, freqScale.max] as t}
              <line x1="28" x2={freqW} y1={8 + (1 - t / freqScale.max) * (FH - 32)} y2={8 + (1 - t / freqScale.max) * (FH - 32)} class="grid-l" />
              <text x="22" y={12 + (1 - t / freqScale.max) * (FH - 32)} text-anchor="end" class="axis">{t}</text>
            {/each}
            {#each freq as f, i}
              {@const bw = Math.max(2, (freqW - 30) / freq.length - 2)}
              {@const bh = (f.count / freqScale.max) * (FH - 32)}
              {@const bx = 30 + i * ((freqW - 30) / freq.length)}
              {#if f.count}
                <path class="bar" d={`M${bx},${FH - 24} v${-(bh - Math.min(4, bw / 2))} q0,${-Math.min(4, bw / 2)} ${Math.min(4, bw / 2)},${-Math.min(4, bw / 2)} h${bw - 2 * Math.min(4, bw / 2)} q${Math.min(4, bw / 2)},0 ${Math.min(4, bw / 2)},${Math.min(4, bw / 2)} v${bh - Math.min(4, bw / 2)} z`}>
                  <title>{f.unit === "week" ? `Week of ${fmtDay(f.start)}` : new Date(f.start).toLocaleDateString(undefined, { month: "long", year: "numeric" })}: {plural(f.count, "session")}</title>
                </path>
              {/if}
            {/each}
            {#if freq.length}
              <text x="30" y={FH - 6} class="axis">{fmtBucket(freq[0].start, freq[0].unit)}</text>
              <text x={freqW} y={FH - 6} text-anchor="end" class="axis">{fmtBucket(freq[freq.length - 1].start, freq[0].unit)}</text>
            {/if}
          </svg>
        </div>
      </section>

      <!-- calendar -->
      <section class="card wide">
        <div class="head">
          <h3>Calendar</h3>
          {#if heatPages > 1}
            <div class="pager">
              <button class="link" disabled={heatPage >= heatPages - 1} onclick={() => heatPage++} aria-label="Earlier">‹ Earlier</button>
              <button class="link" disabled={heatPage === 0} onclick={() => heatPage--} aria-label="Later">Later ›</button>
            </div>
          {/if}
        </div>
        <div class="chart" bind:clientWidth={heatW}>
          <svg width={heatW} height={7 * (CELL + GAP) + 18} role="img" aria-label="Sessions per day">
            {#each ["M", "", "W", "", "F", "", ""] as d, i}
              <text x="0" y={18 + i * (CELL + GAP) + CELL - 3} class="axis">{d}</text>
            {/each}
            {#each heatWeeks as w, wi}
              {#if monthLabels.has(wi)}
                <text x={20 + wi * (CELL + GAP)} y="10" class="axis">{new Date(w).toLocaleDateString(undefined, { month: "short" })}</text>
              {/if}
              {#each Array(7) as _, dow}
                {@const t = cellDay(w, dow)}
                {@const n = days.get(dayKey(t)) ?? 0}
                {#if t <= now}
                  <rect x={20 + wi * (CELL + GAP)} y={18 + dow * (CELL + GAP)} width={CELL} height={CELL} rx="3"
                    class={n ? "cell on" : "cell"} fill-opacity={n ? 0.35 + 0.65 * (n / heatMax) : 1}>
                    <title>{fmtDayYear(t)}: {n ? plural(n, "session") : "none"}</title>
                  </rect>
                {/if}
              {/each}
            {/each}
          </svg>
        </div>
      </section>

      <!-- combinations -->
      <section class="card">
        <h3>Taken together</h3>
        {#if data.pairs.length}
          <ul class="pairs">
            {#each data.pairs.slice(0, 8) as p}
              <li>
                <span>{label(p.a)} + {label(p.b)}</span>
                <span class="n">{plural(p.sessions, "session")}</span>
                {#if onCheck && !hideNames}
                  <button class="link" onclick={() => onCheck?.([realName.get(p.a) ?? p.a, realName.get(p.b) ?? p.b])}>Check</button>
                {/if}
              </li>
            {/each}
          </ul>
        {:else}
          <p class="note">No two substances in the same session in this range.</p>
        {/if}
      </section>

      <!-- time of day -->
      <section class="card">
        <h3>Time of day{current ? ` · ${label(current.sub.key)}` : ""}</h3>
        <div class="hours" role="img" aria-label="Doses by hour of day">
          {#each hours as n, h}
            <span class="hcol" title={`${h}:00: ${plural(n, "dose")}`}>
              <span class="hbar" style:height={`${(n / hourMax) * 100}%`}></span>
            </span>
          {/each}
        </div>
        <div class="hlabels"><span>12am</span><span>6am</span><span>12pm</span><span>6pm</span><span></span></div>
      </section>
    </div>
  {/if}
</div>

<style>
  /* Works on both pages' tokens: the desktop's (--card/--ink/--muted/--line) and
     the phone's (--surface/--text/--text-2/--divider). */
  .stats {
    --st-surface: var(--surface, var(--card, #1e2127));
    --st-text: var(--text, var(--ink, #e7e9ee));
    --st-muted: var(--text-2, var(--muted, #9aa0ab));
    --st-line: var(--divider, var(--line, #2e323b));
    --st-accent: var(--accent, #6d8fb0);
    color: var(--st-text);
    min-width: 0;
  }
  .controls { display: flex; flex-wrap: wrap; gap: 0.6rem 1rem; align-items: center; justify-content: space-between; margin-bottom: 0.8rem; }
  .seg { display: inline-flex; border: 1px solid var(--st-line); border-radius: 10px; overflow: hidden; }
  .seg button { background: none; border: 0; color: var(--st-muted); padding: 0.55rem 0.8rem; font: inherit; font-size: 0.9rem; min-height: 40px; cursor: pointer; }
  .seg button + button { border-left: 1px solid var(--st-line); }
  .seg button.on { background: var(--st-accent); color: var(--on-accent, var(--accent-ink, #0c0e12)); }
  .hide { color: var(--st-muted); font-size: 0.9rem; display: inline-flex; gap: 0.4rem; align-items: center; min-height: 40px; }
  .msg { color: var(--st-muted); padding: 1rem 0; }
  .tiles { display: grid; grid-template-columns: repeat(3, 1fr); gap: 0.6rem; margin-bottom: 0.8rem; }
  .tile, .facts > div { display: flex; flex-direction: column; }
  .tile { background: var(--st-surface); border: 1px solid var(--st-line); border-radius: 12px; padding: 0.7rem 0.8rem; }
  .big { font-size: 1.5rem; font-weight: 600; font-variant-numeric: tabular-nums; }
  .cap { color: var(--st-muted); font-size: 0.8rem; }
  .chips { display: flex; flex-wrap: wrap; gap: 0.4rem; margin-bottom: 0.8rem; }
  .chips button { background: var(--st-surface); border: 1px solid var(--st-line); color: var(--st-text); border-radius: 999px; padding: 0.4rem 0.85rem; min-height: 40px; font: inherit; font-size: 0.9rem; cursor: pointer; }
  .chips button.on { border-color: var(--st-accent); box-shadow: inset 0 0 0 1px var(--st-accent); }
  .grid { display: grid; grid-template-columns: 1fr; gap: 0.8rem; }
  @media (min-width: 860px) {
    .grid { grid-template-columns: 1fr 1fr; }
    .wide { grid-column: 1 / -1; }
  }
  .card { background: var(--st-surface); border: 1px solid var(--st-line); border-radius: 14px; padding: 1rem; min-width: 0; }
  .head { display: flex; justify-content: space-between; align-items: baseline; gap: 0.5rem; flex-wrap: wrap; }
  h3 { margin: 0 0 0.6rem; font-size: 1rem; }
  h4 { margin: 0.9rem 0 0.4rem; font-size: 0.85rem; color: var(--st-muted); font-weight: 500; }
  .link { background: none; border: 0; color: var(--st-accent); font: inherit; font-size: 0.9rem; padding: 0.3rem 0; min-height: 36px; cursor: pointer; }
  .link:disabled { color: var(--st-muted); opacity: 0.5; cursor: default; }
  .chart { width: 100%; overflow: hidden; }
  svg { display: block; }
  .axis { fill: var(--st-muted); font-size: 11px; }
  .grid-l { stroke: var(--st-line); stroke-width: 1; }
  .band { fill: var(--st-accent); }
  .bandlbl { fill: var(--st-muted); font-size: 10px; }
  .dot { fill: var(--st-accent); stroke: var(--st-surface); stroke-width: 2; pointer-events: none; }
  .dot.sel { stroke: var(--st-text); }
  .hit { fill: transparent; cursor: pointer; outline: none; }
  .hit:focus-visible { outline: none; stroke: var(--focus, var(--st-text)); stroke-width: 2; }
  .bar { fill: var(--st-accent); }
  .cell { fill: var(--st-line); }
  .cell.on { fill: var(--st-accent); }
  .axisnote, .note { color: var(--st-muted); font-size: 0.85rem; margin: 0.4rem 0 0; }
  .detail { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 0.5rem; margin-top: 0.5rem; padding: 0.5rem 0.7rem; border: 1px solid var(--st-line); border-radius: 10px; font-size: 0.92rem; }
  .tablewrap { max-height: 320px; overflow: auto; }
  table { width: 100%; border-collapse: collapse; font-size: 0.9rem; }
  th, td { text-align: left; padding: 0.35rem 0.4rem; border-bottom: 1px solid var(--st-line); }
  th { color: var(--st-muted); font-weight: 500; }
  .facts { display: grid; grid-template-columns: repeat(2, 1fr); gap: 0.7rem; }
  .bars, .pairs { list-style: none; margin: 0; padding: 0; }
  .bars li { display: grid; grid-template-columns: 7rem 1fr 2rem; gap: 0.5rem; align-items: center; font-size: 0.9rem; padding: 0.2rem 0; }
  .lbl { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .track { height: 8px; background: var(--st-line); border-radius: 4px; overflow: hidden; }
  .fill { display: block; height: 100%; background: var(--st-accent); border-radius: 4px; }
  .n { color: var(--st-muted); font-variant-numeric: tabular-nums; text-align: right; font-size: 0.85rem; }
  .pairs li { display: flex; gap: 0.6rem; align-items: center; justify-content: space-between; padding: 0.35rem 0; border-bottom: 1px solid var(--st-line); font-size: 0.92rem; }
  .pairs li > span:first-child { flex: 1; min-width: 0; }
  .hours { display: grid; grid-template-columns: repeat(24, 1fr); gap: 2px; height: 90px; align-items: end; }
  .hcol { height: 100%; display: flex; align-items: flex-end; }
  .hbar { display: block; width: 100%; background: var(--st-accent); border-radius: 3px 3px 0 0; min-height: 0; }
  .hlabels { display: grid; grid-template-columns: repeat(4, 1fr) 0; color: var(--st-muted); font-size: 11px; margin-top: 4px; }
</style>
