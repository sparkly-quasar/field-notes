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

  The drug-family card says which families most experiences lately had in them
  (stimulants, dissociatives...) and which of those moved, counted in
  experiences rather than doses so a night of redosing reads as one time.
-->
<script lang="ts">
  import { hiding, shown as nameShown } from "$lib/discreet.svelte";
  import { usageStats, type UsageStats } from "$lib/api";
  import { ts, median, niceScale, fmtNum, startOfDay, startOfWeek, fisherP, familyMix, FAMILY_LABEL, type FamilyRow } from "$lib/stats";
  import StatsPick, { pickItem, type PickExp } from "$lib/StatsPick.svelte";

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
  /** A share (redosing, combinations) changes only when a split this uneven
   *  would be unlikely by chance (Fisher's exact test). With a handful of
   *  entries, one either way moves the share a lot and means nothing. */
  const P_MAX = 0.1;
  /** Experiences drawn per period on a share card; the most recent are kept. */
  const MAX_CELLS = 60;

  let data = $state<UsageStats | null>(null);
  let showAll = $state(false);
  let failed = $state(false);
  /** What a tap opened: a bar or a dot (by index), or a square (by experience
   *  id), on one card (by `tid`). */
  let pick = $state<{ card: string; i: number } | null>(null);

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

  type Exp = PickExp;
  const exps = $derived(
    (data?.sessions ?? [])
      .map((s) => ({ id: s.experience_id, t: ts(s.started_at), title: s.title, subs: s.substances, rating: s.rating }))
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
    | { kind: "gaps"; key: string; title: string; text: string; up: boolean; times: number[]; ids: number[]; note: string }
    | { kind: "dose"; key: string; title: string; text: string; up: boolean; unit: string; points: { t: number; v: number; id: number }[]; note: string | null }
    | { kind: "chips"; key: string | null; title: string; text: string; up: boolean; rows: ChipRow[]; legend: string; what: (n: number) => string }
    | { kind: "mix"; key: null; title: string; text: string; up: boolean; rows: FamilyRow[]; before: number; lately: number; note: string };
  type ChipRow = { label: string; total: number; cells: { id: number; on: boolean; n: number }[] };

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

  /** The two periods as rows of every experience, the most recent kept if
   *  there are more than fit. `n`: what the filled ones count. */
  function chipRows(before: Exp[], lately: Exp[], on: (e: Exp) => boolean, n: (e: Exp) => number): ChipRow[] {
    const row = (label: string, xs: Exp[]): ChipRow => ({
      label, total: xs.length, cells: xs.slice(-MAX_CELLS).map((e) => ({ id: e.id, on: on(e), n: n(e) })),
    });
    return [row(`The ${periodWord} before`, before), row(`Last ${periodWord}`, lately)];
  }

  // ---- drug families ----
  /** "were stimulants", "were in no family" */
  const FAMILY_MANY: Record<string, string> = {
    psychedelics: "psychedelics", entactogens: "entactogens", dissociatives: "dissociatives", stimulants: "stimulants",
    depressants: "depressants", opioids: "opioids", cannabinoids: "cannabinoids", other: "substances with no family",
  };
  const many = (f: string) => FAMILY_MANY[f] ?? f;
  const famLabel = (f: string) => FAMILY_LABEL[f] ?? f;
  /** Each experience with the families of what was in it. Routine doses (the
   *  morning coffee, a daily prescription) would outweigh everything else, so
   *  they're left out, as they are from "how often". */
  const famOf = (e: Exp) =>
    [...new Set(e.subs.filter((k) => kindOf(k) !== "routine").flatMap((k) => data?.substances.find((s) => s.key === k)?.families ?? []))];
  const famExps = $derived(exps.map((e) => ({ ...e, families: famOf(e) })).filter((e) => e.families.length));
  const MOVED: FamilyRow["trend"][] = ["up", "down", "new", "gone"];
  const TREND_WORD: Record<FamilyRow["trend"], string> = {
    up: "↑ more often", down: "↓ less often", same: "about the same", new: "↑ new lately", gone: "↓ none lately", few: "too few to tell",
  };

  function mix(out: Trend[]) {
    const before = famExps.filter((e) => !recent(e.t)).length;
    const lately = famExps.filter((e) => recent(e.t)).length;
    if (before < MIN_EACH || lately < MIN_EACH) return;
    const rows = familyMix(famExps, mid, MIN_EACH, 1.5, P_MAX);
    if (rows.length < 2) return;
    // The lead: the family in the most experiences lately, or two that tie.
    const lead = rows.filter((r) => r.lately === rows[0].lately).slice(0, 2);
    const top = lead[0];
    const most = top.lately * 2 > lately;
    const leadWords = lead.map((r) => many(r.family)).join(" and ");
    const title = lead.length > 1
      ? `${cap(leadWords)} led your experiences lately`
      : most ? `Most experiences lately had ${many(top.family)}` : `${cap(leadWords)} led your experiences lately`;
    const was = top.trend === "up" || top.trend === "new" ? "up from" : top.trend === "down" ? "down from" : "against";
    const first = `${cap(many(top.family))} were in ${top.lately} of ${lately} experiences in the last ${periodWord}, ${was} ${top.before} of ${before} in the ${periodWord} before.`;
    const ups = rows.filter((r) => r !== top && (r.trend === "up" || r.trend === "new")).map((r) => many(r.family));
    const downs = rows.filter((r) => r !== top && (r.trend === "down" || r.trend === "gone")).map((r) => many(r.family));
    const moves = [ups.length ? `${list(ups)} more often` : "", downs.length ? `${list(downs)} less often` : ""].filter(Boolean).join(", ");
    const multi = famExps.some((e) => e.families.length > 1);
    const routine = (data?.substances ?? []).some((s) => s.kind === "routine");
    out.push({
      kind: "mix", key: null, rows, before, lately, title,
      up: MOVED.includes(top.trend) ? top.trend === "up" || top.trend === "new" : lately > before,
      text: moves ? `${first} ${cap(moves)}.` : first,
      note: [
        "Each experience counts once for every family in it, however many doses it had.",
        multi ? "One with more than one family counts toward each, so the rows can add up to more than the total." : "",
        routine ? "Routine doses aren't counted here." : "",
      ].filter(Boolean).join(" "),
    });
  }
  const cap = (x: string) => x.charAt(0).toUpperCase() + x.slice(1);
  const list = (xs: string[]) => (xs.length < 3 ? xs.join(" and ") : `${xs.slice(0, -1).join(", ")} and ${xs[xs.length - 1]}`);

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
    if (!focus) mix(out);
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
          kind: "gaps", key: k, up: !closer, times: mine.map((e) => e.t), ids: mine.map((e) => e.id),
          title: `${label(k)} experiences are ${closer ? "closer together" : "further apart"}`,
          text: `Typically ${plural(Math.round(gL), "day")} apart lately, ${plural(Math.round(gB), "day")} before.`,
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
      const redosed = (e: Exp) => dosesIn(e.id) > 1;
      const cB = before.filter(redosed).length, cL = lately.filter(redosed).length;
      const sB = cB / before.length, sL = cL / lately.length;
      if (Math.abs(sL - sB) >= 0.25 && fisherP(cL, lately.length, cB, before.length) < P_MAX) {
        out.push({
          kind: "chips", key: k, up: sL > sB,
          rows: chipRows(before, lately, redosed, (e) => dosesIn(e.id)),
          title: `${sL > sB ? "More" : "Fewer"} ${label(k)} redoses`,
          text: `Redosed in ${cL} of ${lately.length} ${label(k)} experiences in the last ${periodWord}, against ${cB} of ${before.length} in the ${periodWord} before.`,
          legend: "Each square is one experience. Filled: redosed, with how many doses.",
          what: (n) => plural(n, "dose"),
        });
      }
    }

    // Combinations: the share of experiences with more than one substance.
    if (!focus) {
      const before = experiences.filter((e) => !recent(e.t)), lately = experiences.filter((e) => recent(e.t));
      if (before.length >= MIN_EACH && lately.length >= MIN_EACH) {
        const combined = (e: Exp) => e.subs.length > 1;
        const cB = before.filter(combined).length, cL = lately.filter(combined).length;
        const sB = cB / before.length, sL = cL / lately.length;
        if (Math.abs(sL - sB) >= 0.25 && fisherP(cL, lately.length, cB, before.length) < P_MAX) {
          out.push({
            kind: "chips", key: null, up: sL > sB,
            rows: chipRows(before, lately, combined, (e) => e.subs.length),
            title: `${sL > sB ? "More" : "Fewer"} combinations lately`,
            text: `${cL} of ${lately.length} experiences in the last ${periodWord} had more than one substance, against ${cB} of ${before.length} in the ${periodWord} before.`,
            legend: "Each square is one experience. Filled: more than one substance, with how many.",
            what: (n) => plural(n, "substance"),
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

  /** A card's identity, so a tap stays on its card when the list changes. */
  const tid = (tr: Trend) => `${tr.kind}|${tr.key}|${tr.kind === "dose" ? tr.unit : ""}`;
  const picked = (tr: Trend, i: number) => pick?.card === tid(tr) && pick.i === i;
  const toggle = (tr: Trend, i: number) => (pick = picked(tr, i) ? null : { card: tid(tr), i });
  const keyTap = (e: KeyboardEvent, fn: () => void) => {
    if (e.key === "Enter" || e.key === " ") { e.preventDefault(); fn(); }
  };
  const byId = $derived(new Map(exps.map((e) => [e.id, e])));
  const items = (ids: number[]) => ids.map((id) => byId.get(id)).filter((e): e is Exp => !!e).map((e) => pickItem(e, label));
  const plural = (n: number, w: string) => `${n} ${w}${n === 1 ? "" : "s"}`;
  const fmtLong = (t: number, unit: string) =>
    unit === "month" ? new Date(t).toLocaleDateString(undefined, { month: "long", year: "numeric" })
      : `Week of ${new Date(t).toLocaleDateString(undefined, { month: "short", day: "numeric" })}`;

  const KIND: Record<Trend["kind"], string> = { often: "How often", gaps: "Time between", dose: "Amount per experience", chips: "Pattern", mix: "Drug families" };
  const fmtMonth = (t: number, unit: string) =>
    unit === "month" ? new Date(t).toLocaleDateString(undefined, { month: "short" }) : new Date(t).toLocaleDateString(undefined, { month: "short", day: "numeric" });

  // ---- chart geometry (viewBox units; the SVG scales to the card) ----
  const W = 340;
</script>

{#if data && !failed}
  <section class="trends" aria-labelledby="trends-h">
    <h3 id="trends-h">Trends</h3>
    <p class="lede">
      Differences between the last {periodWord} and the {periodWord} before. Tap a bar or a dot for its experiences.
    </p>

    {#if !enough}
      <p class="note">Trends appear once each period has at least {MIN_EACH} experiences{focus ? " of this" : ""}.</p>
    {:else if !trends.length}
      <p class="note">No clear changes between the two periods.</p>
    {/if}

    {#each shown as tr}
      <article class="card">
        <span class="kind">{(tr.kind === "dose" && tr.note) || tr.kind === "mix" ? "" : tr.up ? "↑ " : "↓ "}{tr.kind === "chips" ? (tr.key ? "Redosing" : "Combinations") : KIND[tr.kind]}</span>
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
              <rect class="bar" class:before={!b.recent} class:sel={picked(tr, i)} x={x} y={y(b.count)} width={bw} height={Math.max(0, B - y(b.count))} rx="3" />
              <rect class="hit" x={L + i * slot} y={T} width={slot} height={B - T}
                role="button" tabindex={b.count ? 0 : -1} aria-label="{fmtLong(b.start, tr.unit)}: {plural(b.count, 'experience')}"
                onclick={() => toggle(tr, i)} onkeydown={(e) => keyTap(e, () => toggle(tr, i))}>
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
          {#if pick?.card === tid(tr) && tr.buckets[pick.i]}
            {@const b = tr.buckets[pick.i]}
            {@const end = tr.buckets[pick.i + 1]?.start ?? Infinity}
            {@const ids = exps.filter((e) => e.t >= b.start && e.t < end && (tr.key == null || e.subs.includes(tr.key))).map((e) => e.id)}
            <StatsPick heading={fmtLong(b.start, tr.unit)} note={plural(ids.length, "experience")} items={items(ids)}
              empty="No experiences in this {tr.unit}." {onOpen} onClose={() => (pick = null)} />
          {/if}
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
              <circle class="pt" class:before={!recent(t)} class:sel={picked(tr, i)} cx={x(t)} cy={cy} r="5" />
              <circle class="hit" cx={x(t)} cy={cy} r="12" role="button" tabindex="0"
                aria-label={new Date(t).toLocaleDateString()} onclick={() => toggle(tr, i)} onkeydown={(e) => keyTap(e, () => toggle(tr, i))}>
                <title>{new Date(t).toLocaleDateString()}</title>
              </circle>
            {/each}
          </svg>
          {#if pick?.card === tid(tr) && tr.times[pick.i] != null}
            {@const i = pick.i}
            {@const gap = (a: number, b: number) => plural(Math.round((tr.times[b] - tr.times[a]) / DAY), "day")}
            <StatsPick heading={new Date(tr.times[i]).toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric" })}
              note={[i > 0 ? `${gap(i - 1, i)} after the one before` : "", i < tr.times.length - 1 ? `${gap(i, i + 1)} before the next` : ""].filter(Boolean).join(", ")}
              items={items([tr.ids[i]])} {onOpen} onClose={() => (pick = null)} />
          {/if}
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
            {#each pts as p, i}
              <circle class="pt" class:sel={picked(tr, i)} cx={x(p.t)} cy={y(p.v)} r="4.5" />
              <circle class="hit" cx={x(p.t)} cy={y(p.v)} r="12" role="button" tabindex="0"
                aria-label="{new Date(p.t).toLocaleDateString()}: {fmtNum(p.v)} {tr.unit}"
                onclick={() => toggle(tr, i)} onkeydown={(e) => keyTap(e, () => toggle(tr, i))}>
                <title>{new Date(p.t).toLocaleDateString()}: {fmtNum(p.v)} {tr.unit} in all</title>
              </circle>
            {/each}
          </svg>
          {#if pick?.card === tid(tr) && pts[pick.i]}
            {@const p = pts[pick.i]}
            <StatsPick heading="{fmtNum(p.v)} {tr.unit} in all" note={new Date(p.t).toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric" })}
              items={items([p.id])} {onOpen} onClose={() => (pick = null)} />
          {/if}
          {#if tr.note}<p class="note">{tr.note}</p>{/if}
        {:else if tr.kind === "mix"}
          {@const RH = 46}{@const T = 2}{@const BW = W - 34}
          {@const H = T + tr.rows.length * RH}
          {@const top = Math.max(1, ...tr.rows.flatMap((r) => [r.before, r.lately]))}
          {@const w = (v: number) => (v / top) * BW}
          <svg viewBox="0 0 {W} {H}" role="img" aria-label="{tr.title}: experiences per drug family, before and lately">
            {#each tr.rows as r, i}
              {@const y0 = T + i * RH}
              <text class="axis hi fam-l" x="0" y={y0 + 12}>{famLabel(r.family)}</text>
              <text class="axis" class:hi={MOVED.includes(r.trend)} x={W} y={y0 + 12} text-anchor="end">{TREND_WORD[r.trend]}</text>
              <rect class="bar before" class:sel={picked(tr, i)} x="0" y={y0 + 18} width={Math.max(2, w(r.before))} height="8" rx="2" />
              <text class="axis" x={Math.max(2, w(r.before)) + 5} y={y0 + 26}>{r.before}</text>
              <rect class="bar" class:sel={picked(tr, i)} x="0" y={y0 + 30} width={Math.max(2, w(r.lately))} height="8" rx="2" />
              <text class="axis hi" x={Math.max(2, w(r.lately)) + 5} y={y0 + 38}>{r.lately}</text>
              <rect class="hit" x="0" y={y0} width={W} height={RH - 4} role="button" tabindex="0"
                aria-label="{famLabel(r.family)}: {plural(r.lately, 'experience')} lately, {r.before} before, {TREND_WORD[r.trend].replace(/^[↑↓] /, '')}"
                onclick={() => toggle(tr, i)} onkeydown={(e) => keyTap(e, () => toggle(tr, i))}>
                <title>{famLabel(r.family)}: {r.before} before, {r.lately} lately</title>
              </rect>
            {/each}
          </svg>
          <div class="legend">
            <span><i class="sw before"></i>The {periodWord} before: {plural(tr.before, "experience")}</span>
            <span><i class="sw"></i>Last {periodWord}: {plural(tr.lately, "experience")}</span>
          </div>
          {#if pick?.card === tid(tr) && tr.rows[pick.i]}
            {@const r = tr.rows[pick.i]}
            {@const lately = r.lately > 0}
            {@const ids = famExps.filter((e) => e.families.includes(r.family) && recent(e.t) === lately).map((e) => e.id).reverse()}
            <StatsPick heading="{famLabel(r.family)}, {lately ? `last ${periodWord}` : `the ${periodWord} before`}" note={plural(ids.length, "experience")}
              items={items(ids)} {onOpen} onClose={() => (pick = null)} />
          {/if}
          <p class="note">{tr.note} Tap a family to see its experiences.</p>
        {:else}
          {#each tr.rows as row}
            <div class="row-h">
              <span>{row.label}</span>
              <span>{row.total > row.cells.length ? `last ${row.cells.length} of ` : ""}{plural(row.total, "experience")}</span>
            </div>
            <div class="cells">
              {#each row.cells as c (c.id)}
                {@const e = byId.get(c.id)}
                <button class="cell" class:on={c.on} class:sel={picked(tr, c.id)} onclick={() => toggle(tr, c.id)}
                  aria-label="{e ? new Date(e.t).toLocaleDateString() : 'Experience'}: {tr.what(c.n)}">{c.on ? c.n : ""}</button>
              {/each}
            </div>
          {/each}
          {#if pick?.card === tid(tr) && byId.get(pick.i)}
            {@const e = byId.get(pick.i)!}
            {@const n = tr.rows.flatMap((r) => r.cells).find((c) => c.id === e.id)?.n ?? 0}
            <StatsPick heading={new Date(e.t).toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric" })}
              note={tr.what(n)} items={items([e.id])} {onOpen} onClose={() => (pick = null)} />
          {/if}
          <p class="note">{tr.legend} Oldest first; tap one to see it.</p>
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
  .hit { fill: transparent; cursor: pointer; outline: none; }
  .hit:focus-visible { stroke: var(--focus, var(--tr-text)); stroke-width: 2; }
  .avg { stroke: var(--tr-text); stroke-width: 1.5; stroke-dasharray: 4 3; }
  .pt { fill: var(--tr-accent); stroke: var(--tr-surface); stroke-width: 2; pointer-events: none; }
  .pt.before { fill: var(--tr-surface-2); stroke: var(--tr-edge); stroke-width: 1.5; }
  .bar.sel, .pt.sel { stroke: var(--tr-text); stroke-width: 2; }
  .fam-l { font-weight: 600; }
  .line { fill: none; stroke: var(--tr-accent); stroke-width: 2; }
  .legend { display: flex; gap: 0.9rem; flex-wrap: wrap; font-size: 0.82rem; color: var(--tr-muted); margin-top: 0.3rem; }
  .sw { display: inline-block; width: 10px; height: 10px; border-radius: 3px; margin-right: 0.3rem; vertical-align: -1px; background: var(--tr-accent); }
  .sw.before { background: var(--tr-surface-2); border: 1.5px solid var(--tr-edge); }
  .row-h { display: flex; justify-content: space-between; gap: 0.6rem; font-size: 0.82rem; color: var(--tr-muted); margin: 0.55rem 0 0.3rem; }
  .row-h span:first-child { color: var(--tr-text); font-weight: 600; }
  .cells { display: flex; flex-wrap: wrap; gap: 5px; align-items: center; }
  .cell { width: 26px; height: 26px; min-height: 26px; padding: 0; border-radius: 6px; border: 1.5px solid var(--tr-edge); background: transparent; color: var(--tr-muted); font: inherit; font-size: 0.75rem; cursor: pointer; }
  .cell.on { background: var(--tr-accent); border-color: var(--tr-accent); color: var(--on-accent, var(--accent-ink, #0c0e12)); font-weight: 700; }
  .cell.sel { outline: 2px solid var(--tr-text); outline-offset: 1px; }
  .link { background: none; border: 0; color: var(--tr-accent); font: inherit; font-size: 0.92rem; font-weight: 600; padding: 0.3rem 0; min-height: 40px; cursor: pointer; }
</style>
