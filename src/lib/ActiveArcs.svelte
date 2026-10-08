<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  Active arcs: a chart of what each substance in a live trip report is likely
  doing, and a row each saying where it is now (arcs.ts has the model). Names sit
  in a key under the chart, not on it; the details for a line sit behind a tap on
  its key entry, which also shows that line's timing band and fades the others.
  Combinations stay with the combination notes, not here.

  Used by the desktop and the phone, so the two can't drift. Like RiskNotes, it
  styles itself from whichever page hosts it, falling back between the two pages'
  variable names.
-->
<script lang="ts">
  import { pwLookup, type Dose, type PwInfo, type TimelineEvent } from "$lib/api";
  import {
    DRINKS_TOP,
    TIERS,
    TIER_HEIGHT,
    aboutLines,
    amountAt,
    amountsText,
    bounds,
    buildSeries,
    clock,
    combined,
    drinksClearAt,
    drinksTrack,
    heightFor,
    phaseOf,
    tierText,
    windowOf,
    type Series,
  } from "$lib/arcs";
  import { shown as nameShown } from "$lib/discreet.svelte";

  let {
    doses,
    moments = [],
    now,
    lookup = pwLookup,
  }: {
    doses: Dose[];
    moments?: TimelineEvent[];
    now: number;
    /** Where reference entries come from; the app's lookup unless a test swaps it. */
    lookup?: (name: string) => Promise<PwInfo | null>;
  } = $props();

  // The reference for each name logged, looked up once. A name that can't be
  // looked up (offline, unknown) is drawn as a marker rather than left out.
  let infos = $state<Record<string, PwInfo | null>>({});
  $effect(() => {
    for (const d of doses) {
      const n = d.substance_name.trim();
      if (!n || n.toLowerCase() in infos) continue;
      infos[n.toLowerCase()] = null;
      lookup(n)
        .then((i) => (infos[n.toLowerCase()] = i))
        .catch(() => {});
    }
  });

  // Redrawn once a minute, not on every second the live clock ticks.
  const minute = $derived(Math.floor(now / 60_000) * 60_000);
  const series = $derived(buildSeries(doses, (n) => infos[n.trim().toLowerCase()]));

  let width = $state(600);
  let focus = $state<string | null>(null);
  let open = $state<string | null>(null);

  const COLORS = 6;
  const colorOf = (i: number) => `var(--arc-${(i % COLORS) + 1})`;

  const chart = $derived.by(() => {
    const W = Math.max(280, width);
    const narrow = W < 520;
    const H = narrow ? 210 : 260;
    const tiered = series.some((s) => s.tiers);
    const drinks = series.some((s) => s.kind === "drinks");
    const m = { l: tiered ? (narrow ? 62 : 72) : 10, r: drinks ? (narrow ? 44 : 56) : 10, t: 12, b: 28 };
    const pw = W - m.l - m.r;
    const ph = H - m.t - m.b;
    const [t0, t1] = windowOf(series, minute);
    const x = (t: number) => m.l + ((t - t0) / (t1 - t0)) * pw;
    const y = (v: number) => m.t + (1 - v) * ph;
    const step = Math.max(60_000, (t1 - t0) / (pw / 2));
    const pts = (list: [number, number][]) => "M" + list.map(([a, b]) => `${a.toFixed(1)},${b.toFixed(1)}`).join("L");

    const hours = (t1 - t0) / 3_600_000;
    const every = [1, 2, 3, 6].find((n) => pw / (hours / n) >= 46) ?? 6;
    const ticks: { x: number; label: string }[] = [];
    const first = new Date(t0);
    first.setMinutes(0, 0, 0);
    for (let t = first.getTime() + 3_600_000; t <= t1; t += 3_600_000) {
      if (new Date(t).getHours() % every === 0) ticks.push({ x: x(t), label: clock(t) });
    }

    const lines = series.map((s, i) => {
      const lastNow = s.doses.filter((d) => d.at <= minute).at(-1);
      const base = { s, color: colorOf(i), line: "", band: "", tail: "", preview: "", marker: null as null | { x: number; y: number; to: number } };
      const tick = s.doses.map((d) => ({ x: x(d.at), future: d.at > minute }));
      const hasFuture = s.doses.some((d) => d.at > minute);
      if (s.kind === "drinks") {
        if (hasFuture) base.preview = pts(drinksTrack(s, t1, Infinity, step).map(([t, v]) => [x(t), y(heightFor(s, v))]));
        if (lastNow) {
          const clear = drinksClearAt(s, minute);
          base.line = pts(drinksTrack(s, clear ? Math.min(t1, clear) : t1, minute, step).map(([t, v]) => [x(t), y(heightFor(s, v))]));
          const after = s.stages.after;
          if (clear && after) base.tail = tailPath(x, y, clear, clear + ((after[0] + after[1]) / 2) * 3_600_000, t1);
        }
      } else if (s.kind === "shape") {
        const start = s.doses[0].at;
        const slow = bounds(s.stages, 1);
        const mid = bounds(s.stages, 0.5);
        if (hasFuture) {
          const end = Math.min(t1, s.doses.at(-1)!.at + slow.off * 3_600_000);
          const p: [number, number][] = [];
          for (let t = start; t <= end; t += step) p.push([x(t), y(heightFor(s, combined(s, t, 0.5, Infinity)))]);
          base.preview = pts(p);
        }
        if (lastNow) {
          const solidEnd = lastNow.at + mid.off * 3_600_000;
          const end = Math.min(t1, lastNow.at + slow.off * 3_600_000);
          const line: [number, number][] = [];
          const up: [number, number][] = [];
          const dn: [number, number][] = [];
          for (let t = start; t <= end; t += step) {
            const a = combined(s, t, 0, minute);
            const b = combined(s, t, 1, minute);
            up.push([x(t), y(heightFor(s, Math.max(a, b)))]);
            dn.push([x(t), y(heightFor(s, Math.min(a, b)))]);
            if (t <= solidEnd) line.push([x(t), y(heightFor(s, combined(s, t, 0.5, minute)))]);
          }
          base.line = line.length ? pts(line) : "";
          base.band = up.length ? pts(up) + "L" + dn.reverse().map(([a, b]) => `${a.toFixed(1)},${b.toFixed(1)}`).join("L") + "Z" : "";
          if (s.stages.after) base.tail = tailPath(x, y, Math.min(solidEnd, t1), lastNow.at + mid.after * 3_600_000, t1);
        }
      } else if (s.kind === "block" && lastNow) {
        const on = s.stages.onset ? ((s.stages.onset[0] + s.stages.onset[1]) / 2) * 3_600_000 : 0;
        const h = heightFor(s, amountAt(s, minute, minute));
        const a = x(lastNow.at + on);
        const b = x(lastNow.at + s.stages.total![0] * 3_600_000);
        const c = x(lastNow.at + s.stages.total![1] * 3_600_000);
        base.band = `M${a},${y(0)}L${a},${y(h)}L${c},${y(h)}L${c},${y(0)}Z`;
        base.line = `M${a},${y(0)}L${a},${y(h)}L${b},${y(h)}`;
      } else if (s.kind === "marker" && lastNow) {
        base.marker = { x: x(lastNow.at), y: y(0.5), to: x(Math.max(minute, lastNow.at + 1_800_000)) };
      }
      return { ...base, ticks: tick };
    });

    // Moments sit on the first line, at the time they were added.
    const firstLine = series[0];
    const dots = firstLine
      ? moments
          .map((mo) => ({ t: Date.parse(mo.at), mo }))
          .filter(({ t }) => Number.isFinite(t) && t >= t0 && t <= t1)
          .map(({ t, mo }) => ({
            x: x(t),
            y: y(firstLine.kind === "marker" ? 0.5 : heightFor(firstLine, amountAt(firstLine, t, t))),
            title: `${clock(t)} ${[mo.mood, mo.note].filter(Boolean).join(" · ")}`,
          }))
      : [];

    return {
      W, H, m, narrow, tiered, drinks, ticks, lines, dots,
      nowX: x(minute),
      tierY: TIERS.map((t) => ({ t, y: y(TIER_HEIGHT[t]) })),
      drinkY: [2, 4, 6, 8].filter((n) => n <= DRINKS_TOP).map((n) => ({ n, y: y(n / DRINKS_TOP) })),
      base: y(0),
    };
  });

  function tailPath(x: (t: number) => number, y: (v: number) => number, from: number, until: number, t1: number): string {
    const lift = 0.06;
    const rise = from + 15 * 60_000;
    const end = Math.min(t1, until);
    if (end <= rise) return "";
    const frac = Math.max(0, 1 - (end - rise) / Math.max(1, until - rise));
    return `M${x(from)},${y(0)}L${x(rise)},${y(lift)}L${x(end)},${y(lift * frac)}`;
  }

  const rows = $derived(
    series
      .map((s, i) => ({ s, color: colorOf(i), phase: phaseOf(s, minute) }))
      .filter((r) => r.phase),
  );

  function toggle(key: string) {
    open = open === key ? null : key;
    focus = open && key !== "moments" ? key : null;
  }
  const keyId = (k: string) => `arc-key-${k.replace(/[^a-z0-9]+/gi, "-")}`;
  const titleOf = (s: Series) => nameShown(s.name);
</script>

{#if series.length}
  <section class="arcs" aria-label="Active arcs">
    <div class="chart" bind:clientWidth={width}>
      <svg viewBox="0 0 {chart.W} {chart.H}" width={chart.W} height={chart.H} role="img" aria-label="Each substance's likely effect over time, with a line for now">
        <defs>
          <clipPath id="arcs-plot"><rect x={chart.m.l} y="0" width={chart.W - chart.m.l - chart.m.r} height={chart.H} /></clipPath>
        </defs>
        {#if chart.tiered}
          {#each chart.tierY as g}
            <line class="grid" x1={chart.m.l} x2={chart.W - chart.m.r} y1={g.y} y2={g.y} />
            <text class="axis" x={chart.m.l - 6} y={g.y + 4} text-anchor="end">{g.t}</text>
          {/each}
        {/if}
        {#if chart.drinks}
          {#each chart.drinkY as g}
            {#if !chart.tiered}<line class="grid" x1={chart.m.l} x2={chart.W - chart.m.r} y1={g.y} y2={g.y} />{/if}
            <text class="axis" x={chart.W - chart.m.r + 6} y={g.y + 4}>{g.n}{g.n === DRINKS_TOP ? (chart.narrow ? "" : " drinks") : ""}</text>
          {/each}
        {/if}
        <line class="base" x1={chart.m.l} x2={chart.W - chart.m.r} y1={chart.base} y2={chart.base} />
        {#each chart.ticks as t}
          <line class="tick" x1={t.x} x2={t.x} y1={chart.base} y2={chart.base + 4} />
          <text class="axis" x={t.x} y={chart.H - 8} text-anchor="middle">{t.label}</text>
        {/each}

        <g clip-path="url(#arcs-plot)">
          {#each chart.lines as l (l.s.key)}
            {@const dim = focus !== null && focus !== l.s.key}
            <g style="color: {l.color}" opacity={dim ? 0.2 : 1}>
              {#if l.s.kind === "block" && l.band}
                <path d={l.band} class="block" />
              {:else if l.band && focus === l.s.key}
                <path d={l.band} class="band" />
              {/if}
              {#if l.preview}<path d={l.preview} class="preview" />{/if}
              {#if l.line}<path d={l.line} class="line" class:untiered={!l.s.tiers && l.s.kind === "shape"} />{/if}
              {#if l.tail}<path d={l.tail} class="tail" />{/if}
              {#if l.marker}
                <circle cx={l.marker.x} cy={l.marker.y} r="5" class="dot" />
                <path d="M{l.marker.x + 8},{l.marker.y}H{l.marker.to}" class="tail" />
              {/if}
              {#each l.ticks as t}
                <path d="M{t.x},{chart.base + 1}l-4,7h8z" class="dot" opacity={t.future ? 0.35 : 1} />
              {/each}
            </g>
          {/each}
          {#each chart.dots as d}
            <circle cx={d.x} cy={d.y} r="5" class="moment"><title>{d.title}</title></circle>
          {/each}
        </g>
        <line class="now" x1={chart.nowX} x2={chart.nowX} y1={chart.m.t} y2={chart.base} />
        <text class="now-label" x={chart.nowX + (chart.nowX > chart.W - 70 ? -5 : 5)} y={chart.m.t + 10} text-anchor={chart.nowX > chart.W - 70 ? "end" : "start"}>now</text>
      </svg>
    </div>

    <ul class="key" aria-label="Chart key">
      {#each chart.lines as l (l.s.key)}
        <li>
          <button
            type="button"
            id={keyId(l.s.key)}
            aria-expanded={open === l.s.key}
            aria-controls="arcs-about"
            style="color: {l.color}"
            onclick={() => toggle(l.s.key)}
            onmouseenter={() => { if (!open) focus = l.s.key; }}
            onmouseleave={() => { if (!open) focus = null; }}
          >
            <svg viewBox="0 0 30 14" aria-hidden="true">
              {#if l.s.kind === "drinks"}<path d="M1 13 L6 8 L10 10 L15 4 L19 6 L29 13" class="line" />
              {:else if l.s.kind === "block"}<rect x="2" y="2" width="26" height="11" class="block" />
              {:else if l.s.kind === "marker"}<circle cx="5" cy="7" r="4" class="dot" /><path d="M11 7 H29" class="tail" />
              {:else}<path d="M1 13 C7 13 8 3 15 3 S23 3 29 13" class="line" class:untiered={!l.s.tiers} />{/if}
            </svg>
            <span class="nm">{titleOf(l.s)}</span>
            <span class="sub">{amountsText(l.s)}</span>
            <span class="i" aria-hidden="true">i</span>
          </button>
        </li>
      {/each}
      {#if chart.dots.length}
        <li>
          <button type="button" id={keyId("moments")} aria-expanded={open === "moments"} aria-controls="arcs-about" onclick={() => toggle("moments")}>
            <svg viewBox="0 0 30 14" aria-hidden="true"><circle cx="15" cy="7" r="4.5" class="moment" /></svg>
            <span class="nm">Moments</span>
            <span class="i" aria-hidden="true">i</span>
          </button>
        </li>
      {/if}
    </ul>

    {#if open}
      {@const l = chart.lines.find((x) => x.s.key === open)}
      <div class="about" id="arcs-about" role="status">
        {#if open === "moments"}
          <strong>Moments</strong>
          <p>Each dot is a moment you added, on the first line at the time you added it. Point at one to read it.</p>
        {:else if l}
          <strong>{titleOf(l.s)}{l.s.roa ? `, ${l.s.roa.name.toLowerCase()}` : ""}</strong>
          {#each aboutLines(l.s) as line}<p>{line}</p>{/each}
        {/if}
      </div>
    {/if}

    <h4 class="rows-h">Likely still active</h4>
    <ul class="rows">
      {#each rows as r (r.s.key)}
        <li>
          <span class="swatch" style="background: {r.color}" aria-hidden="true"></span>
          <div class="what">
            <div><strong>{titleOf(r.s)}</strong> <span class="muted">{amountsText(r.s, minute)}</span> <span class="tier">{tierText(r.s, minute)}</span></div>
            {#if r.phase!.detail}<div class="detail">{r.phase!.detail}</div>{/if}
          </div>
          <span class="pill {r.phase!.tone}">{r.phase!.label}</span>
        </li>
      {/each}
    </ul>
    <p class="fine">From the dose reference's timings and ranges: a guide, not a measurement. A line ending isn't an all-clear.</p>
  </section>
{/if}

<style>
  .arcs {
    --a-ink: var(--text, var(--ink, #eee));
    --a-muted: var(--text-2, var(--muted, #aaa));
    --a-line: var(--divider, var(--line, #444));
    --a-edge: var(--edge, var(--a-muted));
    --a-raised: var(--surface-2, #2a2a2a);
    --a-accent: var(--accent, #b9a3c9);
    --a-on-accent: var(--on-accent, var(--accent-ink, #111));
    --a-bg: var(--bg, #111);
    /* Line colours: mid tones pulled toward the page's ink, so they read on the
       dark pages and the phone's light one. */
    --arc-1: #b9a3c9;
    --arc-2: #7fc4bd;
    --arc-3: #e8a0ae;
    --arc-4: #a9c98b;
    --arc-5: #9fb7e6;
    --arc-6: #e3b98f;
    --arc-1: color-mix(in oklab, #a58cc0 78%, var(--a-ink));
    --arc-2: color-mix(in oklab, #4fa89f 78%, var(--a-ink));
    --arc-3: color-mix(in oklab, #d97f92 78%, var(--a-ink));
    --arc-4: color-mix(in oklab, #86ad5f 78%, var(--a-ink));
    --arc-5: color-mix(in oklab, #7397d6 78%, var(--a-ink));
    --arc-6: color-mix(in oklab, #cf955a 78%, var(--a-ink));
    display: grid;
    gap: 0.6rem;
    margin: 0.75rem 0;
    min-width: 0;
  }
  /* Night (red) mode: every line red, told apart by the key and by tapping. */
  :global(:root[data-theme="night"]) .arcs {
    --arc-1: #ff7a5c;
    --arc-2: #ffb199;
    --arc-3: #d9533a;
    --arc-4: #ff9a6b;
    --arc-5: #ffc4b0;
    --arc-6: #c2452f;
  }
  .chart { width: 100%; min-width: 0; }
  svg { display: block; overflow: visible; }
  .chart svg { width: 100%; height: auto; }
  .grid { stroke: var(--a-line); stroke-width: 1; }
  .base { stroke: var(--a-line); stroke-width: 1; }
  .tick { stroke: var(--a-muted); stroke-width: 1; }
  .axis { fill: var(--a-muted); font-size: 11px; font-variant-numeric: tabular-nums; }
  .now { stroke: var(--a-ink); stroke-width: 1.5; }
  .now-label { fill: var(--a-ink); font-size: 11px; font-weight: 700; }
  .line { fill: none; stroke: currentColor; stroke-width: 2.25; stroke-linejoin: round; }
  .line.untiered { stroke-dasharray: 6 4; }
  .band { fill: currentColor; fill-opacity: 0.18; stroke: none; }
  .block { fill: currentColor; fill-opacity: 0.14; stroke: currentColor; stroke-dasharray: 3 4; }
  .preview { fill: none; stroke: currentColor; stroke-width: 1.5; stroke-opacity: 0.3; stroke-dasharray: 4 4; }
  .tail { fill: none; stroke: currentColor; stroke-width: 2; stroke-dasharray: 2 5; stroke-linecap: round; }
  .dot { fill: currentColor; }
  .moment { fill: var(--a-bg); stroke: var(--a-ink); stroke-width: 1.5; }

  .key { list-style: none; margin: 0; padding: 0; display: flex; flex-wrap: wrap; gap: 0.4rem; }
  .key button {
    display: inline-flex; align-items: center; gap: 0.45rem;
    font: inherit; font-size: 0.88rem; color: inherit;
    background: transparent; border: 1px solid var(--a-line); border-radius: 8px;
    padding: 0.3rem 0.6rem 0.3rem 0.45rem; min-height: 40px; cursor: pointer; text-align: left;
  }
  .key button:hover, .key button[aria-expanded="true"] { border-color: var(--a-edge); background: var(--a-raised); }
  .key button:focus-visible { outline: 2px solid var(--focus, var(--a-accent)); outline-offset: 2px; }
  .key svg { width: 30px; height: 14px; flex: none; }
  .key .nm { color: var(--a-ink); font-weight: 600; }
  .key .sub { color: var(--a-muted); font-variant-numeric: tabular-nums; }
  .key .i {
    width: 16px; height: 16px; border-radius: 50%; border: 1px solid var(--a-edge);
    display: inline-grid; place-items: center; font-size: 10px; font-weight: 700; color: var(--a-muted); flex: none;
  }
  .about {
    background: var(--a-raised); border: 1px solid var(--a-edge); border-radius: 10px;
    padding: 0.6rem 0.75rem; font-size: 0.9rem; line-height: 1.45; max-width: 66ch;
  }
  .about p { margin: 0.3rem 0 0; color: var(--a-muted); }

  .rows-h { margin: 0.4rem 0 0; font-size: 0.95rem; }
  .rows { list-style: none; margin: 0; padding: 0; }
  .rows li {
    display: grid; grid-template-columns: 10px minmax(0, 1fr) auto; gap: 0.25rem 0.6rem; align-items: start;
    padding: 0.5rem 0; border-top: 1px solid var(--a-line); font-size: 0.92rem;
  }
  .rows li:first-child { border-top: 0; }
  .swatch { width: 10px; height: 10px; border-radius: 50%; margin-top: 0.4rem; }
  .what { min-width: 0; }
  .muted { color: var(--a-muted); font-variant-numeric: tabular-nums; }
  .tier { font-size: 0.78rem; color: var(--a-muted); border: 1px solid var(--a-line); border-radius: 6px; padding: 0 0.35rem; white-space: nowrap; }
  .detail { color: var(--a-muted); font-size: 0.86rem; margin-top: 0.15rem; }
  .pill {
    font-size: 0.78rem; font-weight: 700; padding: 0.1rem 0.55rem; border-radius: 999px; white-space: nowrap;
    border: 1px solid var(--a-edge); color: var(--a-ink);
  }
  .pill.peak { background: var(--a-accent); color: var(--a-on-accent); border-color: var(--a-accent); }
  .pill.coming, .pill.down { background: var(--a-raised); }
  .pill.after { border-style: dashed; }
  .pill.unknown { border-style: dotted; }
  .pill.past, .pill.wait { color: var(--a-muted); border-color: var(--a-line); }
  .fine { margin: 0; font-size: 0.8rem; color: var(--a-muted); }
  @media (max-width: 420px) {
    .rows li { grid-template-columns: 10px minmax(0, 1fr); }
    .pill { grid-column: 2; justify-self: start; }
  }
</style>
