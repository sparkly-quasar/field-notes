<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  Trends, at the top of Stats: what changed between the chosen period and the one
  before it, each with a small chart of the thing it describes. Same voice as the
  rest of Stats: it describes what was logged, it doesn't judge it, and it never
  uses warning colours. A trend shows only when each period has at least
  MIN_EACH experiences of what it's about, so one unusual week isn't a pattern.

  It loads its own data (twice the chosen range), so the rest of Stats is untouched.
  Psychedelic doses that rise are shown with a note that working up gradually is a
  common way to work with them; nothing is ever marked as a concern.
-->
<script lang="ts">
  import { hiding, shown as nameShown } from "$lib/discreet.svelte";
  import { usageStats, type UsageStats } from "$lib/api";
  import { ts, median, niceScale, fmtNum, startOfDay, startOfWeek } from "$lib/stats";

  let {
    days,
    focus = null,
    onOpen,
  }: {
    /** Length of each period, in days. */
    days: number;
    /** Only trends about these substance keys (a substance or family is picked). */
    focus?: Set<string> | null;
    onOpen?: (experienceId: number) => void;
  } = $props();

  const DAY = 86_400_000;
  const MIN_EACH = 3;
  const SHOW_FIRST = 4;

  let data = $state<UsageStats | null>(null);
  let showAll = $state(false);
  let failed = $state(false);

  const now = Date.now();
  const mid = $derived(startOfDay(now) - (days - 1) * DAY);
  const start = $derived(mid - days * DAY);

  $effect(() => {
    const since = new Date(start).toISOString();
    failed = false;
    usageStats(since).then((d) => (data = d)).catch(() => (failed = true));
  });

  const realName = $derived(new Map((data?.substances ?? []).map((s) => [s.key, s.name])));
  const label = (key: string) => nameShown(realName.get(key) ?? key);
  const periodWord = $derived(days === 30 ? "30 days" : days === 90 ? "90 days" : days === 365 ? "year" : `${days} days`);

  type Exp = { id: number; t: number; subs: string[]; rating: number | null };
  const exps = $derived(
    (data?.sessions ?? [])
      .map((s) => ({ id: s.experience_id, t: ts(s.started_at), subs: s.substances, rating: s.rating }))
      .filter((e): e is Exp => e.t != null && e.t >= start)
      .sort((a, b) => a.t - b.t),
  );
  const recent = (t: number) => t >= mid;

  // Routine and as-needed substances (kinds.rs) aren't experiences: the overall
  // "how often" and combinations count only entries with something else in them.
  // A routine one has no "how often" or "time between" of its own (daily is the
  // point), but keeps its amount trends; an as-needed one keeps everything, since
  // taking it more often is exactly what's worth seeing.
  const kindOf = (key: string) => data?.substances.find((s) => s.key === key)?.kind ?? "";
  const experiences = $derived(exps.filter((e) => e.subs.some((k) => !kindOf(k))));

  // ---- the trends ----
  type Bucket = { start: number; count: number; recent: boolean };
  type Trend =
    | { kind: "often"; key: string | null; title: string; text: string; up: boolean; buckets: Bucket[]; before: number; lately: number; unit: string }
    | { kind: "gaps"; key: string; title: string; text: string; up: boolean; times: number[]; note: string }
    | { kind: "dose"; key: string; title: string; text: string; up: boolean; unit: string; points: { t: number; v: number; id: number }[]; note: string | null }
    | { kind: "chips"; key: string | null; title: string; text: string; up: boolean; cells: { id: number; on: boolean; n: number | null; recent: boolean }[]; legend: string };

  /** Months for a long window, weeks for a short one, empty buckets included. */
  function buckets(times: number[]): { list: Bucket[]; unit: string } {
    const monthly = days > 31;
    const begin = (t: number) => {
      if (!monthly) return startOfWeek(t);
      const d = new Date(t);
      return new Date(d.getFullYear(), d.getMonth(), 1).getTime();
    };
    const next = (t: number) => {
      const d = new Date(t);
      return monthly ? new Date(d.getFullYear(), d.getMonth() + 1, 1).getTime() : t + 7 * DAY;
    };
    const list: Bucket[] = [];
    for (let b = begin(start); b <= now; b = next(b)) list.push({ start: b, count: 0, recent: b + (next(b) - b) / 2 >= mid });
    for (const t of times) {
      const slot = list.find((x, i) => t >= x.start && (i === list.length - 1 || t < list[i + 1].start));
      if (slot) slot.count++;
    }
    return { list, unit: monthly ? "month" : "week" };
  }

  const changed = (before: number, lately: number, by = 1.5) => lately >= before * by || lately * by <= before;

  const trends = $derived.by((): Trend[] => {
    if (!data) return [];
    const out: Trend[] = [];
    const keys = (data.substances ?? []).map((s) => s.key).filter((k) => !focus || focus.has(k));

    // How often: overall (when nothing is picked), then per substance.
    const often = (key: string | null) => {
      const mine = key == null ? experiences : exps.filter((e) => e.subs.includes(key));
      const before = mine.filter((e) => !recent(e.t)).length;
      const lately = mine.filter((e) => recent(e.t)).length;
      if (before < MIN_EACH || lately < MIN_EACH || !changed(before, lately)) return;
      const { list, unit } = buckets(mine.map((e) => e.t));
      const up = lately > before;
      const what = key == null ? "experiences" : `${label(key)} experiences`;
      out.push({
        kind: "often", key, up, buckets: list, before, lately, unit,
        title: key == null ? (up ? "More experiences lately" : "Fewer experiences lately") : `${label(key)}: ${up ? "more often" : "less often"} lately`,
        text: `${lately} ${what} in the last ${periodWord}, ${up ? "up" : "down"} from ${before} in the ${periodWord} before.`,
      });
    };
    if (!focus) often(null);
    for (const k of keys) if (kindOf(k) !== "routine") often(k);

    for (const k of keys) {
      const sub = data.substances.find((s) => s.key === k)!;
      const isPsychedelic = (sub.families ?? []).includes("psychedelics") && !(sub.families ?? []).includes("entactogens");
      const mine = exps.filter((e) => e.subs.includes(k));
      const before = mine.filter((e) => !recent(e.t));
      const lately = mine.filter((e) => recent(e.t));
      if (before.length < MIN_EACH || lately.length < MIN_EACH) continue;

      // Time between: the typical gap in each period.
      const gaps = (xs: Exp[]) => xs.slice(1).map((e, i) => (e.t - xs[i].t) / DAY);
      const gB = median(gaps(before)), gL = median(gaps(lately));
      if (kindOf(k) !== "routine" && gB != null && gL != null && gB > 0 && changed(gB, gL)) {
        const closer = gL < gB;
        out.push({
          kind: "gaps", key: k, up: !closer, times: mine.map((e) => e.t),
          title: `${label(k)} experiences are ${closer ? "closer together" : "further apart"}`,
          text: `Typically ${fmtNum(Math.round(gL))} days apart lately, ${fmtNum(Math.round(gB))} days before.`,
          note: closer ? "Closer together, more of the tolerance from one carries into the next, so the same amount does less." : "",
        });
      }

      // Amount over time, per unit: the total taken in each experience (a redose
      // adds to the night rather than counting as a smaller "typical dose").
      for (const series of sub.series) {
        const byExp = new Map<number, { t: number; v: number; id: number }>();
        for (const p of series.points) {
          const t = ts(p.taken_at);
          if (t == null || p.amount == null || t < start) continue;
          const cur = byExp.get(p.experience_id);
          if (cur) { cur.v += p.amount; cur.t = Math.min(cur.t, t); }
          else byExp.set(p.experience_id, { t, v: p.amount, id: p.experience_id });
        }
        const pts = [...byExp.values()];
        const dB = pts.filter((p) => !recent(p.t)), dL = pts.filter((p) => recent(p.t));
        const idsB = new Set(dB.map((p) => p.id)), idsL = new Set(dL.map((p) => p.id));
        if (idsB.size < MIN_EACH || idsL.size < MIN_EACH) continue;
        const mB = median(dB.map((p) => p.v))!, mL = median(dL.map((p) => p.v))!;
        if (!changed(mB, mL, 1.25)) continue;
        const up = mL > mB;
        out.push({
          kind: "dose", key: k, up, unit: series.unit, points: pts,
          title: isPsychedelic && up ? `${label(k)}: working up from a lower start` : `${label(k)}: ${up ? "more" : "less"} per experience lately`,
          text: `Typically ${fmtNum(mL)} ${series.unit} in an experience lately, ${fmtNum(mB)} ${series.unit} before.`,
          note: isPsychedelic && up ? "Starting low and working up gradually is a common way to work with psychedelics." : null,
        });
      }

      // Redosing: the share of experiences with more than one dose of it.
      const dosesIn = (id: number) => sub.series.reduce((n, u) => n + u.points.filter((p) => p.experience_id === id).length, 0);
      const share = (xs: Exp[]) => xs.filter((e) => dosesIn(e.id) > 1).length / xs.length;
      const sB = share(before), sL = share(lately);
      if (Math.abs(sL - sB) >= 0.25) {
        const take = (xs: Exp[]) => xs.slice(-6);
        const count = (xs: Exp[]) => xs.filter((e) => dosesIn(e.id) > 1).length;
        out.push({
          kind: "chips", key: k, up: sL > sB,
          cells: [...take(before), ...take(lately)].map((e) => ({ id: e.id, on: dosesIn(e.id) > 1, n: dosesIn(e.id), recent: recent(e.t) })),
          title: `${sL > sB ? "More" : "Fewer"} ${label(k)} redoses`,
          text: `Redosed in ${count(lately)} of ${lately.length} ${label(k)} experiences lately, ${count(before)} of ${before.length} before.`,
          legend: "Filled: redosed (number = doses that day)",
        });
      }
    }

    // Combinations: the share of experiences with more than one substance.
    if (!focus) {
      const before = experiences.filter((e) => !recent(e.t)), lately = experiences.filter((e) => recent(e.t));
      if (before.length >= MIN_EACH && lately.length >= MIN_EACH) {
        const share = (xs: Exp[]) => xs.filter((e) => e.subs.length > 1).length / xs.length;
        const sB = share(before), sL = share(lately);
        if (Math.abs(sL - sB) >= 0.25) {
          const take = (xs: Exp[]) => xs.slice(-6);
          const count = (xs: Exp[]) => xs.filter((e) => e.subs.length > 1).length;
          out.push({
            kind: "chips", key: null, up: sL > sB,
            cells: [...take(before), ...take(lately)].map((e) => ({ id: e.id, on: e.subs.length > 1, n: e.subs.length, recent: recent(e.t) })),
            title: `${sL > sB ? "More" : "Fewer"} combinations lately`,
            text: `${count(lately)} of ${lately.length} experiences lately had more than one substance, ${count(before)} of ${before.length} before.`,
            legend: "Filled: more than one substance (number = how many)",
          });
        }
      }
    }
    return out;
  });

  const shown = $derived(showAll ? trends : trends.slice(0, SHOW_FIRST));
  const enough = $derived(
    exps.filter((e) => recent(e.t)).length >= MIN_EACH && exps.filter((e) => !recent(e.t)).length >= MIN_EACH,
  );

  const KIND: Record<Trend["kind"], string> = { often: "How often", gaps: "Time between", dose: "Amount per experience", chips: "Pattern" };
  const fmtMonth = (t: number, unit: string) =>
    unit === "month" ? new Date(t).toLocaleDateString(undefined, { month: "short" }) : new Date(t).toLocaleDateString(undefined, { month: "short", day: "numeric" });

  // ---- chart geometry (viewBox units; the SVG scales to the card) ----
  const W = 340;
</script>

{#if data && !failed}
  <section class="trends" aria-labelledby="trends-h">
    <h3 id="trends-h">Trends</h3>
    <p class="lede">
      Differences between the last {periodWord} and the {periodWord} before.
    </p>

    {#if !enough}
      <p class="note">Trends appear once each period has at least {MIN_EACH} experiences{focus ? " of this" : ""}.</p>
    {:else if !trends.length}
      <p class="note">No clear changes between the two periods.</p>
    {/if}

    {#each shown as tr}
      <article class="card">
        <span class="kind">{tr.kind === "dose" && tr.note ? "" : tr.up ? "↑ " : "↓ "}{tr.kind === "chips" ? (tr.key ? "Redosing" : "Combinations") : KIND[tr.kind]}</span>
        <h4>{tr.title}</h4>
        <p>{tr.text}</p>

        {#if tr.kind === "often"}
          {@const H = 130}{@const L = 26}{@const R = W - 4}{@const T = 8}{@const B = H - 22}
          {@const sc = niceScale(Math.max(1, ...tr.buckets.map((b) => b.count)), 2)}
          {@const slot = (R - L) / tr.buckets.length}
          {@const bw = Math.max(3, Math.min(26, slot - 4))}
          {@const y = (v: number) => B - (v / sc.max) * (B - T)}
          {@const per = days / (tr.unit === "month" ? 30.44 : 7)}
          {@const nB = per}
          {@const nL = per}
          {@const split = tr.buckets.findIndex((b) => b.recent)}
          <svg viewBox="0 0 {W} {H}" role="img" aria-label="{tr.title}: experiences per {tr.unit}">
            {#each [0, sc.max] as v}
              <line class="grid-l" x1={L} x2={R} y1={y(v)} y2={y(v)} />
              <text class="axis" x={L - 6} y={y(v) + 4} text-anchor="end">{v}</text>
            {/each}
            {#each tr.buckets as b, i}
              {@const x = L + i * slot + (slot - bw) / 2}
              <rect class="bar" class:before={!b.recent} x={x} y={y(b.count)} width={bw} height={Math.max(0, B - y(b.count))} rx="3">
                <title>{fmtMonth(b.start, tr.unit)}: {b.count}</title>
              </rect>
            {/each}
            {#if split > 0}
              <line class="avg" x1={L + 2} x2={L + split * slot - 4} y1={y(tr.before / nB)} y2={y(tr.before / nB)} />
              <line class="avg" x1={L + split * slot + 4} x2={R - 2} y1={y(tr.lately / nL)} y2={y(tr.lately / nL)} />
            {/if}
            <text class="axis" x={L} y={H - 4}>{fmtMonth(tr.buckets[0].start, tr.unit)}</text>
            <text class="axis" x={R} y={H - 4} text-anchor="end">{fmtMonth(tr.buckets[tr.buckets.length - 1].start, tr.unit)}</text>
          </svg>
          <div class="legend">
            <span><i class="sw before"></i>Before: {fmtNum(Math.round((tr.before / nB) * 10) / 10)} a {tr.unit}</span>
            <span><i class="sw"></i>Lately: {fmtNum(Math.round((tr.lately / nL) * 10) / 10)} a {tr.unit}</span>
          </div>
        {:else if tr.kind === "gaps"}
          {@const H = 70}{@const L = 8}{@const R = W - 8}{@const cy = 34}
          {@const x = (t: number) => L + ((t - start) / (now - start)) * (R - L)}
          <svg viewBox="0 0 {W} {H}" role="img" aria-label="{tr.title}: experiences on a timeline">
            <line class="grid-l" x1={L} x2={R} y1={cy} y2={cy} />
            <line class="tick" x1={x(mid)} x2={x(mid)} y1={cy + 8} y2={cy + 16} />
            <text class="axis" x={x(mid)} y={cy + 28} text-anchor="middle">{periodWord} ago</text>
            {#each tr.times as t, i}
              {#if i > 0 && x(t) - x(tr.times[i - 1]) > 18}
                <text class="axis" class:hi={recent(t)} x={(x(t) + x(tr.times[i - 1])) / 2} y={cy - 12} text-anchor="middle">{Math.round((t - tr.times[i - 1]) / DAY)}d</text>
              {/if}
              <circle class="pt" class:before={!recent(t)} cx={x(t)} cy={cy} r="5"><title>{new Date(t).toLocaleDateString()}</title></circle>
            {/each}
          </svg>
          {#if tr.note}<p class="note">{tr.note}</p>{/if}
        {:else if tr.kind === "dose"}
          {@const H = 140}{@const L = 30}{@const R = W - 6}{@const T = 8}{@const B = H - 20}
          {@const top = Math.max(...tr.points.map((p) => p.v)) * 1.15}
          {@const sc = niceScale(top, 2)}
          {@const y = (v: number) => B - (Math.min(v, sc.max) / sc.max) * (B - T)}
          {@const x = (t: number) => L + ((t - start) / (now - start)) * (R - L)}
          {@const pts = [...tr.points].sort((a, b) => a.t - b.t)}
          <svg viewBox="0 0 {W} {H}" role="img" aria-label="{tr.title}: doses over time in {tr.unit}">
            <line class="grid-l" x1={L} x2={R} y1={B} y2={B} />
            {#each [0, sc.max] as v}<text class="axis" x={L - 5} y={y(v) + 4} text-anchor="end">{v}</text>{/each}
            <polyline class="line" points={pts.map((p) => `${x(p.t)},${y(p.v)}`).join(" ")} />
            {#each pts as p}
              <circle class="pt" cx={x(p.t)} cy={y(p.v)} r="4.5" role="button" tabindex="0"
                onclick={() => onOpen?.(p.id)} onkeydown={(e) => e.key === "Enter" && onOpen?.(p.id)}>
                <title>{new Date(p.t).toLocaleDateString()}: {fmtNum(p.v)} {tr.unit} in all</title>
              </circle>
            {/each}
          </svg>
          {#if tr.note}<p class="note">{tr.note}</p>{/if}
        {:else}
          <div class="cells">
            {#each tr.cells as c, i}
              {#if i > 0 && c.recent && !tr.cells[i - 1].recent}<span class="sep" aria-hidden="true"></span>{/if}
              <button class="cell" class:on={c.on} onclick={() => onOpen?.(c.id)}
                aria-label="{c.on ? 'Yes' : 'No'}, {c.n}. Open this experience">{c.n}</button>
            {/each}
          </div>
          <p class="note">{tr.legend}. Older on the left; tap one to open it.</p>
        {/if}
      </article>
    {/each}

    {#if trends.length > SHOW_FIRST}
      <button class="link" onclick={() => (showAll = !showAll)}>{showAll ? "Show fewer" : `Show all trends (${trends.length})`}</button>
    {/if}
    {#if hiding()}<p class="note">Names are hidden by discreet mode.</p>{/if}
  </section>
{/if}

<style>
  .trends {
    --tr-surface: var(--surface, var(--card, #1e2127));
    --tr-surface-2: var(--surface-2, color-mix(in srgb, var(--tr-surface) 80%, #fff 6%));
    --tr-muted: var(--text-2, var(--muted, #9aa0ab));
    --tr-line: var(--divider, var(--line, #2e323b));
    --tr-edge: var(--edge, var(--tr-muted));
    --tr-accent: var(--accent, #6d8fb0);
    --tr-text: var(--text, var(--ink, #e7e9ee));
    margin-bottom: 1rem;
  }
  h3 { margin: 0.2rem 0 0.2rem; font-size: 1rem; }
  .lede, .note { color: var(--tr-muted); font-size: 0.88rem; margin: 0 0 0.7rem; line-height: 1.45; }
  .card .note { margin: 0.45rem 0 0; }
  .card { background: var(--tr-surface); border: 1px solid var(--tr-line); border-radius: 14px; padding: 0.85rem 1rem 0.8rem; margin-bottom: 0.7rem; }
  .kind { display: inline-block; font-size: 0.8rem; color: var(--tr-muted); border: 1px solid var(--tr-line); border-radius: 999px; padding: 0.1rem 0.55rem; margin-bottom: 0.4rem; }
  h4 { margin: 0 0 0.2rem; font-size: 1.02rem; }
  .card > p { margin: 0 0 0.5rem; font-size: 0.93rem; line-height: 1.45; }
  svg { display: block; width: 100%; height: auto; overflow: visible; }
  .axis { fill: var(--tr-muted); font-size: 11px; }
  .axis.hi { fill: var(--tr-text); }
  .grid-l { stroke: var(--tr-line); stroke-width: 1; }
  .tick { stroke: var(--tr-edge); }
  .bar { fill: var(--tr-accent); }
  .bar.before { fill: var(--tr-surface-2); stroke: var(--tr-edge); stroke-width: 1.5; }
  .avg { stroke: var(--tr-text); stroke-width: 1.5; stroke-dasharray: 4 3; }
  .pt { fill: var(--tr-accent); stroke: var(--tr-surface); stroke-width: 2; cursor: pointer; }
  .pt.before { fill: var(--tr-surface-2); stroke: var(--tr-edge); stroke-width: 1.5; }
  .line { fill: none; stroke: var(--tr-accent); stroke-width: 2; }
  .legend { display: flex; gap: 0.9rem; flex-wrap: wrap; font-size: 0.82rem; color: var(--tr-muted); margin-top: 0.3rem; }
  .sw { display: inline-block; width: 10px; height: 10px; border-radius: 3px; margin-right: 0.3rem; vertical-align: -1px; background: var(--tr-accent); }
  .sw.before { background: var(--tr-surface-2); border: 1.5px solid var(--tr-edge); }
  .cells { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .cell { width: 34px; height: 34px; min-height: 34px; padding: 0; border-radius: 8px; border: 1.5px solid var(--tr-edge); background: transparent; color: var(--tr-muted); font: inherit; font-size: 0.8rem; cursor: pointer; }
  .cell.on { background: var(--tr-accent); border-color: var(--tr-accent); color: var(--on-accent, var(--accent-ink, #0c0e12)); font-weight: 700; }
  .sep { width: 1px; align-self: stretch; background: var(--tr-line); margin: 0 4px; }
  .link { background: none; border: 0; color: var(--tr-accent); font: inherit; font-size: 0.92rem; font-weight: 600; padding: 0.3rem 0; min-height: 40px; cursor: pointer; }
</style>
