<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  Usage stats (roadmap #2). One component for the desktop and the phone: a single
  column of cards that widens into a grid on a desk. Read-only, descriptive only:
  no streaks, scores, goals, or good/bad colouring, and it never raises an alert.
  The grouping rules live in stats.rs; time bucketing in $lib/stats.ts.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { hiding, shown as nameShown } from "$lib/discreet.svelte";
  import { usageStats, setBedtime, setSleepCheckin, setSubstanceKind, type UsageStats, type StatsDosePoint, type StatsSubstance } from "$lib/api";
  import Trends from "$lib/Trends.svelte";
  import StatsPick, { pickItem, type PickExp } from "$lib/StatsPick.svelte";
  import {
    RANGES, type RangeKey, sinceFor, ts, frequency, perDay, byHour, daysSince, median,
    niceScale, fmtNum, startOfDay, startOfWeek, dayKey, SPACING_NOTE, spacingNotes,
    bedMinutes, closeToBed, hourBeforeBed, fmtBedtime, sleepCompare,
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
  function loadPref(): { range?: RangeKey; hide?: boolean; pick?: string; fam?: string } {
    try { return JSON.parse(localStorage.getItem(PREF) ?? "{}"); } catch { return {}; }
  }
  const saved = loadPref();

  let range = $state<RangeKey>(saved.range ?? "90d");
  // Names follow the app-wide discreet mode ($lib/discreet), so a stand-in here is
  // the same stand-in on the journal.
  /** `key|unit` of the series on the dose chart. */
  let pick = $state<string>(saved.pick ?? "");
  /** A drug family, or "" for none. Picking a family clears the substance, and
   *  the other way round: one filter at a time. */
  let fam = $state<string>(saved.fam ?? "");
  let data = $state<UsageStats | null>(null);
  let err = $state<string | null>(null);
  let loading = $state(false);
  let asTable = $state(false);
  let selected = $state<StatsDosePoint | null>(null);
  let chartW = $state(600);
  let freqW = $state(600);
  let heatW = $state(600);
  let heatPage = $state(0);
  /** What a tap opened: a frequency bar (its start), a calendar day (its
   *  start), an hour of the day. Each card has its own. */
  let pickBar = $state<number | null>(null);
  let pickDay = $state<number | null>(null);
  let pickHour = $state<number | null>(null);

  $effect(() => {
    try { localStorage.setItem(PREF, JSON.stringify({ range, pick, fam })); } catch {}
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
    pickBar = pickDay = pickHour = null;
    load();
  }

  // ---- naming (with the shoulder-surfing toggle) ----
  const realName = $derived(new Map((data?.substances ?? []).map((s) => [s.key, s.name])));
  const label = (key: string) => nameShown(realName.get(key) ?? key);

  // ---- series choice ----
  const choices = $derived(
    (data?.substances ?? []).flatMap((s) =>
      s.series.map((u) => ({ id: `${s.key}|${u.unit}`, sub: s, series: u, split: s.series.length > 1 })),
    ),
  );
  /** The picked series, or null for "All": then the page is an overview of every
   *  substance, and picking one turns every card to that substance alone. */
  const current = $derived(choices.find((c) => c.id === pick) ?? null);
  const sub = $derived(current?.sub ?? null);

  // ---- drug families ----
  const FAMILY_ORDER = ["psychedelics", "entactogens", "dissociatives", "stimulants", "depressants", "opioids", "cannabinoids", "other"];
  const FAMILY_LABEL: Record<string, string> = {
    psychedelics: "Psychedelics", entactogens: "Entactogens", dissociatives: "Dissociatives", stimulants: "Stimulants",
    depressants: "Depressants", opioids: "Opioids", cannabinoids: "Cannabinoids", other: "Other",
  };
  /** "days since the last psychedelic" */
  const FAMILY_ONE: Record<string, string> = {
    psychedelics: "psychedelic", entactogens: "entactogen", dissociatives: "dissociative", stimulants: "stimulant",
    depressants: "depressant", opioids: "opioid", cannabinoids: "cannabinoid", other: "of these",
  };
  const families = $derived(
    FAMILY_ORDER.filter((f) => (data?.substances ?? []).some((s) => (s.families ?? []).includes(f))),
  );
  /** The family in force: only while no single substance is picked, and only if
   *  this range has any of it. */
  const family = $derived(!sub && fam && families.includes(fam) ? fam : "");
  const famSubs = $derived(
    family ? (data?.substances ?? []).filter((s) => (s.families ?? []).includes(family)) : [],
  );
  const famKeys = $derived(new Set(famSubs.map((s) => s.key)));
  /** What the page is narrowed to, for headings: a substance, a family, or nothing. */
  const scope = $derived(sub ? label(sub.key) : family ? FAMILY_LABEL[family] : "");
  function pickFamily(f: string) {
    fam = f;
    pick = "";
    selected = null;
  }
  function pickSubstance(id: string) {
    pick = id;
    selected = null;
  }
  /** With a family picked, the substance chips narrow to its members. */
  const shownChoices = $derived(family ? choices.filter((c) => famKeys.has(c.sub.key)) : choices);

  // ---- routine and as-needed (kinds.rs) ----
  // They leave the experience views (how often, the calendar, time between,
  // "days since"), never the rest: the dose chart, time of day, combinations and
  // amount trends count every dose. Picking one shows it on its own.
  const careKeys = $derived(new Set((data?.substances ?? []).filter((x) => x.kind).map((x) => x.key)));
  const KIND_LABEL: Record<string, string> = { "": "As experiences", routine: "Routine", as_needed: "As needed" };

  async function setKind(kind: string) {
    if (!sub || !data) return;
    const key = sub.key;
    await setSubstanceKind(sub.name, kind);
    data = { ...data, substances: data.substances.map((x) => (x.key === key ? { ...x, kind } : x)) };
  }

  // ---- time window ----
  const now = Date.now();
  const sessions = $derived(
    (data?.sessions ?? []).filter((s) =>
      sub
        ? s.substances.includes(sub.key)
        : family
          ? s.substances.some((k) => famKeys.has(k) && !careKeys.has(k))
          : s.substances.some((k) => !careKeys.has(k)),
    ),
  );
  const sessionTimes = $derived(
    sessions.map((s) => ts(s.started_at)).filter((t): t is number => t != null),
  );
  /** Pairs taken together; with a substance picked, only the ones it's part of,
   *  picked substance first. */
  const pairs = $derived(
    (data?.pairs ?? [])
      .filter((p) => (sub ? p.a === sub.key || p.b === sub.key : family ? famKeys.has(p.a) || famKeys.has(p.b) : true))
      .map((p) => (sub && p.b === sub.key ? { ...p, a: p.b, b: p.a } : p)),
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
  const lastT = $derived(sub ? ts(sub.last_used) : null);
  // ---- and for a family: across sessions with anything in it, so LSD then
  // mushrooms nine days later is a nine-day gap. ----
  const famLastT = $derived.by(() => {
    const t = famSubs.filter((s) => !s.kind).map((s) => ts(s.last_used)).filter((x): x is number => x != null);
    return t.length ? Math.max(...t) : null;
  });
  const famDoses = $derived(famSubs.reduce((n, s) => n + s.doses, 0));
  const famGaps = $derived.by(() => {
    if (!family) return [];
    const t = [...sessionTimes].sort((a, b) => a - b);
    return t.slice(1).map((x, i) => Math.round(((x - t[i]) / 86_400_000) * 10) / 10);
  });
  const subRatings = $derived(
    sub ? sessions.filter((s) => s.rating != null).map((s) => s.rating as number) : [],
  );

  /** For a routine or as-needed substance: days taken in the last four weeks and
   *  the four before, its usual time, and its usual amount. Plain counts only;
   *  any note about what a pattern means waits for the dependence notes, whose
   *  wording is checked by a clinician first (ROADMAP.md). */
  const care = $derived.by(() => {
    if (!sub?.kind) return null;
    const pts = sub.series.flatMap((u) => u.points);
    const times = pts.map((p) => ts(p.taken_at)).filter((t): t is number => t != null);
    const today = startOfDay(now);
    const daysIn = (from: number, to: number) =>
      new Set(times.filter((t) => t >= from && t < to).map((t) => dayKey(t))).size;
    const lately = daysIn(today - 27 * 86_400_000, today + 86_400_000);
    const before = daysIn(today - 55 * 86_400_000, today - 27 * 86_400_000);
    const mins = times.map((t) => new Date(t).getHours() * 60 + new Date(t).getMinutes());
    const usual = median(mins);
    const main = sub.series[0];
    const amount = median(main.points.map((p) => p.amount).filter((a): a is number => a != null));
    return {
      lately,
      before,
      time: usual == null ? null : new Date(2026, 0, 1, Math.floor(usual / 60), Math.round(usual % 60)).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit", hour12: true }),
      amount: amount == null ? null : `${fmtNum(amount)} ${main.unit}`,
    };
  });
  /** The experiences in view, oldest first, for what a tap opens. */
  const exps = $derived(
    sessions
      .map((s) => ({ id: s.experience_id, t: ts(s.started_at), title: s.title, subs: s.substances, rating: s.rating }))
      .filter((e): e is PickExp => e.t != null)
      .sort((a, b) => a.t - b.t),
  );
  const keyTap = (e: KeyboardEvent, fn: () => void) => {
    if (e.key === "Enter" || e.key === " ") { e.preventDefault(); fn(); }
  };

  // ---- frequency ----
  const freq = $derived(frequency(sessionTimes, windowFrom, now));
  /** The tapped bar, if it's still on the chart (a filter can move "All time"). */
  const barIdx = $derived(pickBar == null ? -1 : freq.findIndex((f) => f.start === pickBar));
  const barExps = $derived(
    barIdx < 0 ? [] : exps.filter((e) => e.t >= freq[barIdx].start && (barIdx === freq.length - 1 || e.t < freq[barIdx + 1].start)),
  );
  const freqScale = $derived(niceScale(Math.max(1, ...freq.map((f) => f.count)), 3));
  const FH = 140;

  // ---- tiers (dose-aware Stats, step 5) ----
  // The doses in view: the picked substance, or everything not routine or as
  // needed (which have their own view).
  const scopedPoints = $derived(
    (sub ? [sub] : family ? famSubs.filter((x) => !x.kind) : (data?.substances ?? []).filter((x) => !x.kind)).flatMap((x) =>
      x.series.flatMap((u) => u.points),
    ),
  );
  const RANK: Record<string, number> = { micro: 0, below: 1, threshold: 1, light: 1, common: 2, strong: 3, heavy: 4 };
  /** Experiences where every dose in view was a microdose. */
  const microExps = $derived.by(() => {
    const by = new Map<number, boolean>();
    for (const p of scopedPoints) by.set(p.experience_id, (by.get(p.experience_id) ?? true) && p.tier === "micro");
    return new Set([...by].filter(([, all]) => all).map(([id]) => id));
  });
  const anyMicro = $derived(microExps.size > 0);
  /** Per day: the strongest tier, and whether every dose was a microdose. */
  const dayTier = $derived.by(() => {
    const m = new Map<string, { rank: number | null; micro: boolean; label: string }>();
    for (const p of scopedPoints) {
      const t = ts(p.taken_at);
      if (t == null) continue;
      const k = dayKey(t);
      const cur = m.get(k) ?? { rank: null, micro: true, label: "" };
      const r = p.tier ? RANK[p.tier] ?? null : null;
      if (r != null && (cur.rank == null || r > cur.rank)) {
        cur.rank = r;
        cur.label = p.tier_label ?? "";
      }
      cur.micro = cur.micro && p.tier === "micro";
      m.set(k, cur);
    }
    return m;
  });
  const anyTier = $derived([...dayTier.values()].some((d) => d.rank != null && d.rank > 0));
  /** Runs of microdose days no more than 4 days apart, 3 or more long: how a
   *  schedule (every third day, four on and three off) reads. */
  const microPeriods = $derived.by(() => {
    const ds = [...new Set(scopedPoints.filter((p) => p.tier === "micro").map((p) => ts(p.taken_at)).filter((t): t is number => t != null).map((t) => startOfDay(t)))].sort((a, b) => a - b);
    const out: { from: number; to: number; n: number }[] = [];
    for (const d of ds) {
      const last = out[out.length - 1];
      if (last && d - last.to <= 4 * 86_400_000) {
        last.to = d;
        last.n++;
      } else out.push({ from: d, to: d, n: 1 });
    }
    return out.filter((x) => x.n >= 3);
  });
  /** Days since the last dose that wasn't a microdose, when microdoses are mixed in. */
  const lastFullT = $derived.by(() => {
    if (!anyMicro) return null;
    const t = scopedPoints.filter((p) => p.tier !== "micro").map((p) => ts(p.taken_at)).filter((x): x is number => x != null);
    return t.length ? Math.max(...t) : null;
  });
  const freqMicro = $derived(
    frequency(sessions.filter((s) => microExps.has(s.experience_id)).map((s) => ts(s.started_at)).filter((t): t is number => t != null), windowFrom, now),
  );

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
  const dayExps = $derived(pickDay == null ? [] : exps.filter((e) => dayKey(e.t) === dayKey(pickDay!)));
  function cellDay(week: number, dow: number) {
    const d = new Date(week);
    d.setDate(d.getDate() + dow);
    return d.getTime();
  }

  // ---- time of day ----
  const hourDoses = $derived(
    (sub ? sub.series : (family ? famSubs : data?.substances ?? []).flatMap((x) => x.series))
      .flatMap((u) => u.points)
      .map((p) => ({ id: p.experience_id, t: ts(p.taken_at) }))
      .filter((d): d is { id: number; t: number } => d.t != null),
  );
  const hours = $derived(byHour(hourDoses.map((d) => d.t)));
  const hourMax = $derived(Math.max(1, ...hours));
  /** Experiences with a dose in the tapped hour (listed newest first). */
  const hourExps = $derived.by(() => {
    if (pickHour == null) return [];
    const ids = new Set(hourDoses.filter((d) => new Date(d.t).getHours() === pickHour).map((d) => d.id));
    return exps.filter((e) => ids.has(e.id));
  });
  const fmtHour = (h: number) => new Date(2000, 0, 1, h % 24).toLocaleTimeString(undefined, { hour: "numeric", hour12: true });

  // ---- bedtime (dose-aware Stats, step 3 in ROADMAP.md) ----
  // Asked once, here, the first time this card is seen; kept in the journal so
  // the phone and the computer agree. Each dose is counted against its own
  // substance's half-life from the dose reference, using the low end of the
  // range, so "close to bedtime" never overstates how much is left.
  /** Older servers don't send a bedtime and can't store one: say nothing. */
  const bedSupported = $derived(!!data && "bedtime" in data);
  const bedtime = $derived(data?.bedtime ?? null);
  const bed = $derived(bedMinutes(bedtime));
  let bedEditing = $state(false);
  let bedInput = $state("23:00");
  let bedErr = $state("");
  const asking = $derived(bedSupported && (bedtime == null || bedEditing));

  async function answerBedtime(value: string) {
    bedErr = "";
    try {
      await setBedtime(value);
      if (data) data = { ...data, bedtime: value };
      bedEditing = false;
    } catch (e) {
      bedErr = `Couldn't save that (${String(e).replace(/^Error: /, "")}).`;
    }
  }
  function changeBedtime() {
    if (bed != null) bedInput = bedtime!;
    bedEditing = true;
  }

  const timesOf = (x: StatsSubstance) =>
    x.series.flatMap((u) => u.points).map((p) => ts(p.taken_at)).filter((t): t is number => t != null);
  const fmtHours = (h: number) =>
    h < 1 ? `${Math.round(h * 60)} minutes` : `${fmtNum(Math.round(h * 10) / 10)} hour${h === 1 ? "" : "s"}`;
  /** With one substance picked: the hours before bed that are within its half-life. */
  const lateHours = $derived(
    sub?.half_life && bed != null
      ? Array.from({ length: 24 }, (_, h) => hourBeforeBed(h, bed, sub!.half_life!.low_hours))
      : Array(24).fill(false),
  );
  const bedHour = $derived(bed == null ? -1 : Math.floor(bed / 60));
  // ---- sleep (step 6): only for someone who asked to be asked ----
  async function toggleSleep(on: boolean) {
    await setSleepCheckin(on);
    if (data) data = { ...data, sleep_checkin: on };
  }
  /** Rated nights in this range, after a dose near bedtime against the rest.
   *  Every substance with a half-life counts, routine ones too: coffee is
   *  coffee. Nights outside the range are left out, since their doses aren't. */
  const sleepCmp = $derived.by(() => {
    if (bed == null || !data?.sleep?.length) return null;
    const from = dayKey(windowFrom);
    const nights = data.sleep.filter((n) => n.night >= from);
    if (!nights.length) return null;
    const doses = data.substances
      .filter((x) => x.half_life)
      .flatMap((x) => timesOf(x).map((t) => ({ t, hours: x.half_life!.low_hours })));
    return { total: nights.length, ...sleepCompare(nights, doses, bed) };
  });

  /** Without one picked: each substance with doses inside its half-life before bed. */
  const nearBed = $derived.by(() => {
    if (bed == null || sub) return [];
    const list = family ? famSubs : (data?.substances ?? []);
    return list
      .filter((x) => x.half_life)
      .map((x) => {
        const times = timesOf(x);
        return { key: x.key, n: closeToBed(times, bed, x.half_life!.low_hours), total: times.length, hours: x.half_life!.low_hours };
      })
      .filter((x) => x.n > 0)
      .sort((a, b) => b.n - a.n || a.key.localeCompare(b.key))
      .slice(0, 5);
  });

  const fmtDay = (t: number) => new Date(t).toLocaleDateString(undefined, { month: "short", day: "numeric" });
  /** Axis dates carry the year whenever the window spans more than one, or
   *  "Oct 2 … Oct 1" reads as a single day. */
  const fmtTick = (t: number) =>
    new Date(windowFrom).getFullYear() !== new Date(now).getFullYear()
      ? new Date(t).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "2-digit" })
      : fmtDay(t);
  const fmtBucket = (t: number, unit: "week" | "month") =>
    unit === "month"
      ? new Date(t).toLocaleDateString(undefined, { month: "short", year: "numeric" })
      : fmtDay(t);
  const fmtDayYear = (t: number) =>
    new Date(t).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
  const fmtWhen = (s: string) => {
    const t = ts(s);
    return t == null ? s : new Date(t).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short", hour12: true });
  };
  const plural = (n: number, w: string) => `${n} ${w}${n === 1 ? "" : "s"}`;
  /** " (logged as 600 mg)" when the amount shown isn't what was written. */
  const asLogged = (p: StatsDosePoint) =>
    p.logged_unit && p.logged_amount != null ? ` (logged as ${fmtNum(p.logged_amount)} ${p.logged_unit})` : "";
  /** "about " for an estimate: fresh mushrooms as dried, an edible's guess. */
  const about = (p: StatsDosePoint) => (p.approx ? "about " : "");
  /** " · logged as a strong dose": what was written, never what was received
   *  (potency is unknown). With no ranges to compare with, against the person's
   *  own usual amount instead, once there are enough doses to have one. */
  function tierText(p: StatsDosePoint): string {
    const l = p.tier_label;
    if (l === "microdose") return " · logged as a microdose";
    if (l === "below threshold") return " · logged below threshold";
    if (l) return ` · logged as a ${l}${l.endsWith("dose") ? "" : " dose"}`;
    if (!current || sub?.kind || p.amount == null) return "";
    const amts = current.series.points.map((x) => x.amount).filter((a): a is number => a != null);
    const usual = amts.length >= 5 ? median(amts) : null;
    if (!usual) return "";
    if (p.amount >= usual * 1.5) return " · above your usual";
    if (p.amount <= usual / 1.5) return " · below your usual";
    return "";
  }
  const fmtBucketLong = (f: { start: number; unit: "week" | "month" }) =>
    f.unit === "week" ? `Week of ${fmtDay(f.start)}` : new Date(f.start).toLocaleDateString(undefined, { month: "long", year: "numeric" });
</script>

<div class="stats">
  <div class="controls">
    <div class="seg" role="group" aria-label="Time range">
      {#each RANGES as r}
        <button class:on={range === r.key} aria-pressed={range === r.key} onclick={() => setRange(r.key)}>{r.label}</button>
      {/each}
    </div>
  </div>

  {#if err}
    <p class="msg">{err}</p>
  {:else if !data}
    <p class="msg">{loading ? "Reading your journal…" : ""}</p>
  {:else if !data.total_sessions}
    <p class="msg">No doses logged {range === "all" ? "yet" : "in this range"}.</p>
  {:else}
    {#if families.length > 1 || families[0] === "other"}
      <div class="chips fams" role="group" aria-label="Drug family">
        <button class:on={!family && !current} aria-pressed={!family && !current} onclick={() => pickFamily("")}>All</button>
        {#each families as f}
          <button class:on={family === f} aria-pressed={family === f} onclick={() => pickFamily(f)}>{FAMILY_LABEL[f]}</button>
        {/each}
      </div>
    {/if}
    <div class="chips" role="group" aria-label="Substance">
      {#if !family}
        <button class:on={!current} aria-pressed={!current} onclick={() => { pick = ""; fam = ""; selected = null; }}>All</button>
      {/if}
      {#each shownChoices as c}
        <button class:on={current?.id === c.id} aria-pressed={current?.id === c.id} onclick={() => pickSubstance(c.id)}>
          {label(c.sub.key)}{c.split ? ` · ${c.series.unit}` : ""}
        </button>
      {/each}
    </div>

    {#if sub}
      <div class="tiles">
        {#if !sub.kind}
          <div class="tile"><span class="big">{sub.sessions}</span><span class="cap">{sub.sessions === 1 ? "experience" : "experiences"}</span></div>
        {/if}
        <div class="tile"><span class="big">{sub.doses}</span><span class="cap">{sub.doses === 1 ? "dose" : "doses"}</span></div>
        {#if lastT != null}
          <div class="tile"><span class="big">{daysSince(lastT)}</span><span class="cap">{daysSince(lastT) === 1 ? "day" : "days"} since the last {sub.kind ? "dose" : "experience"}</span></div>
        {/if}
        {#if lastFullT != null && lastFullT !== lastT}
          <div class="tile"><span class="big">{daysSince(lastFullT)}</span><span class="cap">{daysSince(lastFullT) === 1 ? "day" : "days"} since the last full dose</span></div>
        {/if}
      </div>
    {:else if family}
      <div class="tiles">
        {#if famLastT != null}
          <div class="tile"><span class="big">{daysSince(famLastT)}</span><span class="cap">{daysSince(famLastT) === 1 ? "day" : "days"} since the last {FAMILY_ONE[family]}</span></div>
        {/if}
        {#if lastFullT != null && lastFullT !== famLastT}
          <div class="tile"><span class="big">{daysSince(lastFullT)}</span><span class="cap">{daysSince(lastFullT) === 1 ? "day" : "days"} since the last full dose</span></div>
        {/if}
        <div class="tile"><span class="big">{sessions.length}</span><span class="cap">{sessions.length === 1 ? "experience" : "experiences"}</span></div>
        <div class="tile"><span class="big">{famDoses}</span><span class="cap">{famDoses === 1 ? "dose" : "doses"}</span></div>
      </div>
      {#if family === "other"}
        <p class="note pickhint">Substances Field Notes couldn't place in a family. Give one classes in your catalogue, under Check, to sort it.</p>
      {/if}
    {:else}
      <div class="tiles">
        <div class="tile"><span class="big">{sessions.length}</span><span class="cap">{sessions.length === 1 ? "experience" : "experiences"}</span></div>
        <div class="tile"><span class="big">{data.total_doses}</span><span class="cap">{data.total_doses === 1 ? "dose" : "doses"}</span></div>
        <div class="tile"><span class="big">{data.substances.length}</span><span class="cap">{data.substances.length === 1 ? "substance" : "substances"}</span></div>
      </div>
      <p class="note pickhint">Pick a family or a substance to narrow everything below to it.</p>
    {/if}
    {#if sub}
      <div class="kindrow" role="group" aria-label="How you take {label(sub.key)}">
        <span class="note">How you take it</span>
        <div class="seg small">
          {#each ["", "routine", "as_needed"] as k}
            <button class:on={(sub.kind ?? "") === k} aria-pressed={(sub.kind ?? "") === k} onclick={() => setKind(k)}>{KIND_LABEL[k]}</button>
          {/each}
        </div>
      </div>
      {#if sub.notes?.length && !hiding()}
        <section class="card care" aria-label="Worth knowing about {label(sub.key)}">
          <h3>Worth knowing · {label(sub.key)}</h3>
          {#each sub.notes as n}<p class="note">{n.text}</p>{/each}
        </section>
      {/if}
      {#if care}
        <section class="card care">
          <h3>{sub.kind === "routine" ? "Routine" : "As needed"} · {label(sub.key)}</h3>
          <div class="facts">
            <div><span class="big">{care.lately}</span><span class="cap">of the last 28 days</span></div>
            <div><span class="big">{care.before}</span><span class="cap">of the 28 before</span></div>
            {#if care.time}<div><span class="big">{care.time}</span><span class="cap">usual time</span></div>{/if}
            {#if care.amount}<div><span class="big">{care.amount}</span><span class="cap">usual amount</span></div>{/if}
          </div>
          <p class="note">Days it was taken. These doses aren't counted as experiences elsewhere in Stats, but every chart for {label(sub.key)} below includes them, and combination checks always do.</p>
        </section>
      {/if}
    {/if}

    {#key range}
      <Trends
        days={RANGES.find((r) => r.key === range)?.days ?? 90}
        focus={sub ? new Set([sub.key]) : family ? famKeys : null}
        {onOpen}
      />
    {/key}

    <div class="grid">
      <!-- dose over time -->
      {#if current}
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
                      <td>{p.amount == null ? "not recorded" : `${about(p)}${fmtNum(p.amount)} ${current.series.unit}${asLogged(p)}${tierText(p)}`}</td>
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
                  <text x={x(t)} y={H - 8} text-anchor={i === 0 ? "start" : i === xTicks.length - 1 ? "end" : "middle"} class="axis">{fmtTick(t)}</text>
                {/each}
                {#each plotted as d (d.p.dose_id)}
                  <!-- the hit target is bigger than the dot, for a thumb -->
                  <circle
                    cx={x(d.t)} cy={y(d.p.amount ?? 0)} r="14" class="hit"
                    role="button" tabindex="0"
                    aria-label={`${fmtWhen(d.p.taken_at)}: ${about(d.p)}${fmtNum(d.p.amount ?? 0)} ${current.series.unit}${asLogged(d.p)}`}
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
                <span><strong>{about(selected)}{selected.amount == null ? "?" : fmtNum(selected.amount)} {current.series.unit}</strong>{asLogged(selected)}{tierText(selected)}
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
      {/if}

      <!-- spacing -->
      {#if sub}
        <section class="card">
          <h3>Spacing · {label(sub.key)}</h3>
          <div class="facts">
            {#if sub.gaps_days.length === 1}
              <!-- One gap is one number: showing it as both "median" and "shortest"
                   reads like a bug. -->
              <p class="note">One gap so far: {fmtNum(sub.gaps_days[0])} {sub.gaps_days[0] === 1 ? "day" : "days"}.</p>
            {:else if sub.gaps_days.length}
              <div><span class="big">{fmtNum(median(sub.gaps_days) ?? 0)}</span><span class="cap">median days between</span></div>
              <div><span class="big">{fmtNum(Math.min(...sub.gaps_days))}</span><span class="cap">shortest gap (days)</span></div>
            {:else}
              <p class="note">Only one experience in this range, so there's no gap to measure yet.</p>
            {/if}
          </div>
          <!-- The guidance names substances, so it would give a stand-in away. -->
          {#each hiding() ? [] : spacingNotes(sub.families ?? [], sub.key) as n}
            <p class="note guide">{n}</p>
          {/each}
          {#if subRatings.length}
            <p class="note">Average rating of these experiences: {fmtNum(subRatings.reduce((a, b) => a + b, 0) / subRatings.length)} ({plural(subRatings.length, "rated experience")}).</p>
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

      <!-- a family: spacing across it, and what's in it -->
      {#if family}
        <section class="card">
          <h3>Spacing · {FAMILY_LABEL[family]}</h3>
          <div class="facts">
            {#if famGaps.length === 1}
              <p class="note">One gap so far: {fmtNum(famGaps[0])} {famGaps[0] === 1 ? "day" : "days"}.</p>
            {:else if famGaps.length}
              <div><span class="big">{fmtNum(median(famGaps) ?? 0)}</span><span class="cap">median days between</span></div>
              <div><span class="big">{fmtNum(Math.min(...famGaps))}</span><span class="cap">shortest gap (days)</span></div>
            {:else}
              <p class="note">Only one experience in this range, so there's no gap to measure yet.</p>
            {/if}
          </div>
          <p class="note">Counted across every experience with anything in this family, whichever substance it was.</p>
          {#if SPACING_NOTE[family] && !hiding()}<p class="note guide">{SPACING_NOTE[family]}</p>{/if}
          <h4>In this family (experiences)</h4>
          <ul class="bars">
            {#each famSubs.filter((x) => !x.kind) as s}
              <li><span class="lbl">{label(s.key)}</span><span class="track"><span class="fill" style:width={`${(s.sessions / Math.max(1, sessions.length)) * 100}%`}></span></span><span class="n" title={plural(s.sessions, "session")}>{s.sessions}</span></li>
            {/each}
          </ul>
        </section>
      {/if}

      <!-- frequency -->
      <section class="card">
        <h3>Experiences per {freq[0]?.unit ?? "week"}{scope ? ` · ${scope}` : ""}</h3>
        <div class="chart" bind:clientWidth={freqW}>
          <svg width={freqW} height={FH} role="img" aria-label="Number of experiences in each period">
            {#each [0, freqScale.max] as t}
              <line x1="28" x2={freqW} y1={8 + (1 - t / freqScale.max) * (FH - 32)} y2={8 + (1 - t / freqScale.max) * (FH - 32)} class="grid-l" />
              <text x="22" y={12 + (1 - t / freqScale.max) * (FH - 32)} text-anchor="end" class="axis">{t}</text>
            {/each}
            {#each freq as f, i}
              {@const slot = (freqW - 30) / freq.length}
              {@const bw = Math.max(2, slot - 2)}
              {@const bh = (f.count / freqScale.max) * (FH - 32)}
              {@const bx = 30 + i * slot}
              {#if f.count}
                {@const nm = freqMicro[i]?.count ?? 0}
                <path class="bar" class:sel={barIdx === i} d={`M${bx},${FH - 24} v${-(bh - Math.min(4, bw / 2))} q0,${-Math.min(4, bw / 2)} ${Math.min(4, bw / 2)},${-Math.min(4, bw / 2)} h${bw - 2 * Math.min(4, bw / 2)} q${Math.min(4, bw / 2)},0 ${Math.min(4, bw / 2)},${Math.min(4, bw / 2)} v${bh - Math.min(4, bw / 2)} z`} />
                {#if nm}
                  <!-- the microdoses, from the bottom of the bar -->
                  <rect class="bar micro" x={bx} y={FH - 24 - (nm / freqScale.max) * (FH - 32)} width={bw} height={(nm / freqScale.max) * (FH - 32)} />
                {/if}
              {/if}
              <!-- the whole column is the target, so a thin bar is still easy to tap -->
              <rect class="hit" x={bx} y="0" width={slot} height={FH - 24}
                role="button" tabindex={f.count ? 0 : -1} aria-label="{fmtBucketLong(f)}: {plural(f.count, 'experience')}"
                onclick={() => (pickBar = barIdx === i ? null : f.start)}
                onkeydown={(e) => keyTap(e, () => (pickBar = barIdx === i ? null : f.start))}>
                <title>{fmtBucketLong(f)}: {plural(f.count, "experience")}{freqMicro[i]?.count ? `, ${freqMicro[i].count} of them microdoses` : ""}</title>
              </rect>
            {/each}
            {#if freq.length}
              <text x="30" y={FH - 6} class="axis">{fmtBucket(freq[0].start, freq[0].unit)}</text>
              <text x={freqW} y={FH - 6} text-anchor="end" class="axis">{fmtBucket(freq[freq.length - 1].start, freq[0].unit)}</text>
            {/if}
          </svg>
        </div>
        {#if anyMicro}<p class="axisnote"><i class="sw micro"></i> Lighter: experiences that were only microdoses.</p>{/if}
        {#if barIdx >= 0}
          <StatsPick heading={fmtBucketLong(freq[barIdx])} note={plural(barExps.length, "experience")}
            items={barExps.map((e) => pickItem(e, label))} empty="No experiences in this {freq[barIdx].unit}."
            {onOpen} onClose={() => (pickBar = null)} />
        {:else if freq.some((f) => f.count)}
          <p class="axisnote">Tap a bar to see its experiences.</p>
        {/if}
      </section>

      <!-- calendar -->
      <section class="card wide">
        <div class="head">
          <h3>Days with an experience{scope ? ` · ${scope}` : ""}</h3>
          {#if heatPages > 1}
            <div class="pager">
              <button class="link" disabled={heatPage >= heatPages - 1} onclick={() => heatPage++} aria-label="Earlier">‹ Earlier</button>
              <button class="link" disabled={heatPage === 0} onclick={() => heatPage--} aria-label="Later">Later ›</button>
            </div>
          {/if}
        </div>
        <div class="chart" bind:clientWidth={heatW}>
          <svg width={heatW} height={7 * (CELL + GAP) + 18} role="img" aria-label="Experiences per day">
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
                {@const dt = n ? dayTier.get(dayKey(t)) : undefined}
                {#if t <= now}
                  <!-- Shaded by the strongest dose that day when the reference has
                       ranges for it, else by how many experiences; a day of only
                       microdoses is an outline. -->
                  <rect x={20 + wi * (CELL + GAP)} y={18 + dow * (CELL + GAP)} width={CELL} height={CELL} rx="3"
                    class={n ? (dt?.micro ? "cell micro" : "cell on") : "cell"} class:sel={pickDay === t}
                    fill-opacity={!n || dt?.micro ? 1 : anyTier && dt?.rank != null ? 0.3 + 0.175 * dt.rank : 0.35 + 0.65 * (n / heatMax)}
                    role="button" tabindex={n ? 0 : -1} aria-label="{fmtDayYear(t)}: {n ? plural(n, 'experience') : 'none'}"
                    onclick={() => (pickDay = pickDay === t ? null : t)}
                    onkeydown={(e) => keyTap(e, () => (pickDay = pickDay === t ? null : t))}>
                    <title>{fmtDayYear(t)}: {n ? plural(n, "experience") : "none"}{dt?.micro ? " · microdoses only" : dt?.label ? ` · strongest logged as ${dt.label}` : ""}</title>
                  </rect>
                {/if}
              {/each}
            {/each}
          </svg>
        </div>
        {#if anyTier || anyMicro}
          <p class="axisnote">
            {anyTier ? "Darker: a stronger dose that day, as logged, against the dose reference's ranges." : ""}
            {anyMicro ? " Outlined: microdoses only." : ""}
          </p>
        {/if}
        {#each microPeriods as mp}
          <p class="note">Microdosing: {fmtDayYear(mp.from)} to {fmtDayYear(mp.to)}, {plural(mp.n, "day")} with microdoses.</p>
        {/each}
        {#if pickDay != null}
          <StatsPick heading={new Date(pickDay).toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric", year: "numeric" })}
            note={dayExps.length ? plural(dayExps.length, "experience") : ""}
            items={dayExps.map((e) => pickItem(e, label, true))} empty="No experiences on this day."
            {onOpen} onClose={() => (pickDay = null)} />
        {:else}
          <p class="axisnote">Tap a day to see it.</p>
        {/if}
      </section>

      <!-- combinations -->
      <section class="card">
        <h3>Taken together{scope ? ` · ${scope}` : ""}</h3>
        {#if pairs.length}
          <ul class="pairs">
            {#each pairs.slice(0, 8) as p}
              <li>
                <span>{label(p.a)} + {label(p.b)}</span>
                <span class="n">{plural(p.sessions, "experience")}</span>
                {#if onCheck && !hiding()}
                  <button class="link" onclick={() => onCheck?.([realName.get(p.a) ?? p.a, realName.get(p.b) ?? p.b])}>Check</button>
                {/if}
              </li>
            {/each}
          </ul>
        {:else}
          <p class="note">{scope ? `${scope} ${family ? "weren't" : "wasn't"} taken with anything else in this range.` : "No two substances in the same experience in this range."}</p>
        {/if}
      </section>

      <!-- time of day -->
      <section class="card">
        <h3>Time of day{scope ? ` · ${scope}` : ""}</h3>
        <div class="hours" class:picking={pickHour != null} role="group" aria-label="Doses by hour of day">
          {#each hours as n, h}
            <button class="hcol" class:sel={pickHour === h} class:late={lateHours[h]} class:bed={bedHour === h}
              title={`${h}:00: ${plural(n, "dose")}${bedHour === h ? " · bedtime" : ""}`}
              aria-label="{fmtHour(h)}: {plural(n, 'dose')}{bedHour === h ? ', bedtime' : ''}" aria-pressed={pickHour === h} tabindex={n ? 0 : -1}
              onclick={() => (pickHour = pickHour === h ? null : h)}>
              <span class="hbar" style:height={`${(n / hourMax) * 100}%`}></span>
            </button>
          {/each}
        </div>
        <div class="hlabels"><span>12am</span><span>6am</span><span>12pm</span><span>6pm</span><span></span></div>
        {#if pickHour != null}
          <StatsPick heading="{fmtHour(pickHour)} to {fmtHour(pickHour + 1)}" note="{plural(hours[pickHour], 'dose')} in this hour"
            items={hourExps.map((e) => pickItem(e, label)).reverse()} empty="No doses in this hour."
            {onOpen} onClose={() => (pickHour = null)} />
        {:else if hours.some(Boolean)}
          <p class="axisnote">Tap an hour to see its experiences.</p>
        {/if}

        {#if asking}
          <div class="bedq">
            <p class="note">When do you usually go to bed? Doses close to bedtime are marked against how long each substance stays in the body.</p>
            <div class="bedrow">
              <input type="time" aria-label="Usual bedtime" bind:value={bedInput} />
              <button class="primary-s" onclick={() => bedInput && answerBedtime(bedInput)}>Save</button>
              <button class="link" onclick={() => answerBedtime("varies")}>It varies</button>
              <button class="link" onclick={() => (bedEditing ? (bedEditing = false) : answerBedtime("skip"))}>{bedEditing ? "Cancel" : "Not now"}</button>
            </div>
            {#if bedErr}<p class="note">{bedErr}</p>{/if}
          </div>
        {:else if bedSupported}
          {#if bed != null}
            <p class="axisnote">
              Marked: bedtime, {fmtBedtime(bed)}{lateHours.some(Boolean) ? " · shaded: within one half-life before it" : ""}.
              <button class="link" onclick={changeBedtime}>Change</button>
            </p>
            {#if data && "sleep_checkin" in data}
              {#if data.sleep_checkin}
                {#if sleepCmp}
                  {#if sleepCmp.near.nights >= 3 && sleepCmp.other.nights >= 3}
                    <ul class="nearbed">
                      <li>After a dose near bedtime: <strong>{fmtNum(sleepCmp.near.average ?? 0)}</strong> out of 5, over {plural(sleepCmp.near.nights, "night")}</li>
                      <li>Other nights: <strong>{fmtNum(sleepCmp.other.average ?? 0)}</strong> out of 5, over {plural(sleepCmp.other.nights, "night")}</li>
                    </ul>
                    <p class="note">How you rated your sleep, 1 (badly) to 5 (well). Few nights and many causes, so this is something to notice, not a finding.</p>
                  {:else}
                    <p class="note">{plural(sleepCmp.total, "night")} rated in this range. The comparison shows once there are at least 3 nights each with and without a dose near bedtime.</p>
                  {/if}
                {/if}
                <p class="axisnote">Asking how you slept each morning. <button class="link" onclick={() => toggleSleep(false)}>Stop asking</button></p>
              {:else}
                <p class="axisnote">See how doses near bedtime line up with your sleep? <button class="link" onclick={() => toggleSleep(true)}>Ask me each morning</button></p>
              {/if}
            {/if}
          {:else}
            <p class="axisnote">
              {bedtime === "varies" ? "Bedtime varies, so doses aren't counted against it." : ""}
              <button class="link" onclick={changeBedtime}>{bedtime === "varies" ? "Set one" : "Add a bedtime"}</button>
            </p>
          {/if}
          {#if sub}
            {#if sub.half_life}
              {#if bed != null}
                <p class="note">
                  {closeToBed(timesOf(sub), bed, sub.half_life.low_hours)} of {plural(timesOf(sub).length, "dose")} were within
                  {fmtHours(sub.half_life.low_hours)} of bedtime.
                </p>
              {/if}
              <p class="note">
                {label(sub.key)}'s half-life is {sub.half_life.text} (dose reference): about half of a dose is still in the
                body that long after it's taken{sub.families.includes("stimulants") ? ", which is often longer than it feels active" : ""}.
              </p>
            {:else}
              <p class="note">The dose reference has no half-life for {label(sub.key)}, so this can't say how long a dose stays in the body.</p>
            {/if}
          {:else if bed != null}
            {#if nearBed.length}
              <ul class="nearbed">
                {#each nearBed as x}
                  <li><strong>{label(x.key)}</strong>: {x.n} of {plural(x.total, "dose")} within {fmtHours(x.hours)} of bedtime</li>
                {/each}
              </ul>
              <p class="note">Each counted against its own half-life (the low end, from the dose reference): about half a dose is still in the body that long after it's taken.</p>
            {:else}
              <p class="note">No doses with a known half-life were taken within one half-life of bedtime.</p>
            {/if}
          {/if}
        {/if}
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
  .bar.sel { stroke: var(--st-text); stroke-width: 2; }
  .cell { fill: var(--st-line); cursor: pointer; outline: none; }
  .cell.on { fill: var(--st-accent); }
  .cell.micro { fill: transparent; stroke: var(--st-accent); stroke-width: 1.5; }
  .bar.micro { fill: color-mix(in srgb, var(--st-accent) 40%, var(--st-surface)); }
  .sw.micro { display: inline-block; width: 0.8em; height: 0.8em; border-radius: 2px; vertical-align: -0.05em; background: color-mix(in srgb, var(--st-accent) 40%, var(--st-surface)); }
  .cell.sel, .cell:focus-visible { stroke: var(--st-text); stroke-width: 2; }
  .axisnote, .note { color: var(--st-muted); font-size: 0.85rem; margin: 0.4rem 0 0; }
  .detail { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 0.5rem; margin-top: 0.5rem; padding: 0.5rem 0.7rem; border: 1px solid var(--st-line); border-radius: 10px; font-size: 0.92rem; }
  .tablewrap { max-height: 320px; overflow: auto; }
  table { width: 100%; border-collapse: collapse; font-size: 0.9rem; }
  th, td { text-align: left; padding: 0.35rem 0.4rem; border-bottom: 1px solid var(--st-line); }
  th { color: var(--st-muted); font-weight: 500; }
  .pickhint { margin: -0.3rem 0 0.8rem; }
  .guide { border-left: 2px solid var(--st-line); padding-left: 0.6rem; margin-top: 0.6rem; }
  .facts .note { grid-column: 1 / -1; margin: 0; }
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
  .hcol { height: 100%; display: flex; align-items: flex-end; padding: 0; margin: 0; border: 0; background: none; min-width: 0; cursor: pointer; }
  .hcol:focus-visible { outline: 2px solid var(--focus, var(--st-text)); outline-offset: 1px; }
  .hours.picking .hcol:not(.sel) .hbar { opacity: 0.35; }
  .hbar { display: block; width: 100%; background: var(--st-accent); border-radius: 3px 3px 0 0; min-height: 0; }
  /* Within a half-life before bed: tinted behind the bar, so the count still reads. */
  .hcol.late { background: color-mix(in srgb, var(--caution, #d9a441) 18%, transparent); border-radius: 3px; }
  .hcol.bed { box-shadow: inset 2px 0 0 var(--st-text); }
  .bedq { margin-top: 0.6rem; }
  .bedrow { display: flex; flex-wrap: wrap; align-items: center; gap: 0.5rem; margin-top: 0.4rem; }
  .bedrow input {
    width: auto; margin: 0; font: inherit; min-height: var(--tap-min, 2rem); padding: 0.3rem 0.6rem;
    border: 1px solid var(--field-border, var(--st-line)); border-radius: 8px;
    background: var(--field, transparent); color: var(--st-text);
  }
  .primary-s {
    width: auto; margin: 0; min-height: var(--tap-min, 2rem); padding: 0.3rem 0.9rem; border: 0; border-radius: 8px;
    background: var(--st-accent); color: var(--on-accent, var(--accent-ink, #0c0e12)); font: inherit; font-weight: 600; cursor: pointer;
  }
  .axisnote .link { min-height: 0; padding: 0; margin-left: 0.25rem; }
  .kindrow { display: flex; flex-wrap: wrap; align-items: center; gap: 0.5rem 0.8rem; margin: 0 0 0.8rem; }
  .kindrow .note { margin: 0; }
  .seg.small button { padding: 0.35rem 0.7rem; min-height: 34px; font-size: 0.85rem; }
  .care { margin-bottom: 0.8rem; }
  .nearbed { margin: 0.5rem 0 0; padding-left: 1.1rem; font-size: 0.9rem; }
  .nearbed li + li { margin-top: 0.2rem; }
  .hlabels { display: grid; grid-template-columns: repeat(4, 1fr) 0; color: var(--st-muted); font-size: 11px; margin-top: 4px; }
</style>
