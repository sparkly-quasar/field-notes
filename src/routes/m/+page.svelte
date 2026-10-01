<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  The phone portal's UI (see src-tauri/src/portal.rs).

  A *mirror* of the desktop, not a copy of its layout: the same journal, the same
  deterministic safety checks, the same reference data — re-laid out for one hand,
  in the dark, possibly altered. Designed from a three-lens UX review (task flows,
  impaired-state accessibility, reading/editing — see ROADMAP.md, v0.13.0):

  - **One word per thing.** An *entry* is anything in the journal. A *session* is
    an entry with doses; a *journal note* is a plain written entry; a *moment* is
    a line in a session's timeline; the *write-up* is the story afterwards.
  - **Everything you act on opens at the thumb**, in a bottom sheet, never below
    a long list where it can't be seen.
  - **Nothing destructive is one slip away.** Delete sits apart from Save, and
    deleting waits a few seconds with Undo rather than asking a question people
    answer by reflex.
  - **Help is on every screen**, and works even if the server can't be reached.

  What is deliberately NOT here, and cannot be reached from here (portal.rs does not
  allowlist it): wiping the journal, the encryption passphrase, backups, Obsidian
  sync, installing Ollama, revealing the data directory. A phone is the device you
  lose. Everything you'd actually reach for mid-session is here.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import {
    serverUpdateStatus,
    serverUpdateInstall,
    type ServerUpdateStatus,
    listExperiences,
    getExperience,
    createExperience,
    endExperience,
    updateExperience,
    updateDose,
    deleteDose,
    addTimelineEvent,
    updateTimelineEvent,
    deleteTimelineEvent,
    deleteExperience,
    listSubstances,
    addSubstance,
    usageBySubstance,
    checkCombo,
    crisisScan,
    emergencyResources,
    companionEnabled as companionEnabledPref,
    companionChat,
    companionChatStart,
    companionChatPoll,
    type CompanionPoll,
    type CompanionReply,
    aiStatus,
    aiStart,
    pwLookup,
    knowledgeSearch,
    exportExperienceMarkdown,
    type ExperienceSummary,
    type ExperienceDetail,
    type Substance,
    type SubstanceUsage,
    type Dose,
    type TimelineEvent,
    type Warning,
    type CrisisResult,
    type CrisisResource,
    type ChatMsg,
    type PwInfo,
    type KnowledgeHit,
    type AiStatus,
  } from "$lib/api";
  import { acceptPairing, captureToken, hasToken, inTauri, isIos, isStandalone, pairingLink } from "$lib/portal";
  import TripImport from "$lib/TripImport.svelte";
  import DateTimeField from "$lib/DateTimeField.svelte";
  import UsageStats from "$lib/UsageStats.svelte";
  import {
    quickLog,
    recentSubstances,
    whenPresets,
    recallDoseShape,
    rememberDoseShape,
    stretchToCover,
    defaultUnitFor,
    UNITS,
  } from "$lib/quicklog";

  type View = "today" | "journal" | "check" | "talk";
  type Sheet =
    | "new"
    | "dose"
    | "moment"
    | "jot"
    | "start"
    | "past"
    | "help"
    | "editDose"
    | "editMoment"
    | "details"
    | "writeup"
    | "end"
    | "more"
    | "paste";

  const ROUTES = ["oral", "insufflated", "sublingual", "vaporized", "rectal", "IM", "IV"];

  let paired = $state(false);
  /** Pasted pairing link, for the Home Screen app (it can't see Safari's pairing). */
  let pairText = $state("");
  let pairErr = $state<string | null>(null);
  const standalone = typeof window !== "undefined" && isStandalone();
  /** In Safari on an iPhone: offer to carry the pairing into a Home Screen app. */
  let showHomeHint = $state(false);
  let homeLinkCopied = $state(false);
  const HOME_HINT_KEY = "fieldnotes.homeHintDismissed";
  let view = $state<View>("today");
  let sheet = $state<Sheet | null>(null);
  /** Which action is in flight, so only *its* button says "Saving…". */
  let busyKey = $state<string | null>(null);
  const busy = $derived(busyKey !== null);
  let err = $state<string | null>(null);

  let recent = $state<ExperienceSummary[]>([]);
  let session = $state<ExperienceDetail | null>(null);
  /** The entry on screen in the Journal (possibly the live session). */
  let open = $state<ExperienceDetail | null>(null);
  /** Opened from "Log a past session": new rows default to just after the last one. */
  let building = $state(false);

  /** Interaction warnings from the last dose logged into each entry. They stay until
   *  dismissed — a warning that scrolls away or times out is a warning missed. */
  let warnFor = $state<Record<number, Warning[]>>({});
  /** The combination in a session just became dangerous: the crisis layer's answer. */
  let crisis = $state<CrisisResult | null>(null);
  let crisisShown = $state(false);

  // ---------- the dose form (one form, three places: new entry, live session, past entry) ----------
  /** Where the dose goes. null = a new entry of its own (a quick log). */
  let target = $state<ExperienceDetail | null>(null);
  let dSub = $state("");
  let dAmt = $state("");
  let dUnit = $state("mg");
  let dRoute = $state("oral");
  let dWhen = $state("");
  /** What just saved, with its warnings — shown in the sheet, next to the button pressed. */
  let receipt = $state<{ id: number; title: string; line: string; warnings: Warning[] } | null>(null);
  const qRecents = $derived(recentSubstances(recent));

  // moment (timeline note)
  let mText = $state("");
  let mIntensity = $state("");
  let mWhen = $state("");

  // journal note (kind "note")
  let jTitle = $state("");
  let jBody = $state("");

  // start a live session
  let sTitle = $state("");

  // log a past session, step 1
  let pWhen = $state("");
  let pTitle = $state("");

  // editing a dose / moment already logged
  let editDose = $state<Dose | null>(null);
  let eSub = $state("");
  let eAmt = $state("");
  let eUnit = $state("mg");
  let eRoute = $state("oral");
  let eWhen = $state("");
  let eNote = $state("");
  let editMoment = $state<TimelineEvent | null>(null);
  let evText = $state("");
  let evIntensity = $state("");
  let evWhen = $state("");

  // entry details / write-up / ending
  let enTitle = $state("");
  let enStart = $state("");
  let enEnd = $state("");
  let enRating = $state("");
  let enIntention = $state("");
  let enSetting = $state("");
  let wuText = $state("");
  let endAt = $state("");
  let endRating = $state("");
  let endNotes = $state("");
  let confirmDelete = $state(false);

  // deferred deletes, with Undo
  let hidden = $state<string[]>([]);
  let pendingDelete = $state<{ label: string; key: string; commit: () => Promise<void>; timer: number } | null>(null);
  const UNDO_MS = 8000;

  // journal browsing
  let search = $state("");
  let filter = $state<"all" | "sessions" | "notes" | "writeup">("all");
  let subFilter = $state<string | null>(null);
  let usage = $state<SubstanceUsage[]>([]);
  let showUsage = $state(false);

  // check: combo + reference
  let checkMode = $state<"combo" | "lookup">("combo");
  let comboText = $state("");
  /** Journal tab: the list, or usage stats (roadmap #2). */
  let journalMode = $state<"entries" | "stats">("entries");
  let comboWarnings = $state<Warning[] | null>(null);
  let substances = $state<Substance[]>([]);
  let refQuery = $state("");
  let pw = $state<PwInfo | null>(null);
  let hits = $state<KnowledgeHit[]>([]);
  let searched = $state(false);
  let newSub = $state("");

  // help
  let resources = $state<CrisisResource[]>([]);
  const RESOURCES_KEY = "fieldnotes.emergencyResources";

  // companion
  /** Mirrors the desktop's companion-off switch. Unknown until asked: the Talk tab
   *  only appears once the server has said the Companion is on. */
  let companionEnabled = $state<boolean | null>(null);
  let ai = $state<AiStatus | null>(null);
  let models = $state<string[]>([]);
  let model = $state("");
  let chat = $state<ChatMsg[]>([]);
  let ask = $state("");
  let thinking = $state(false);
  let waking = $state(false);
  let chatCrisis = $state<CrisisResult | null>(null);
  let chatCrisisShown = $state(false);

  /** Height of the on-screen keyboard, so a sheet's buttons ride above it. */
  let kb = $state(0);

  function pairFromText() {
    if (!acceptPairing(pairText)) {
      pairErr = "That doesn't look like a pairing link. It should end in #t= followed by a long code.";
      return;
    }
    pairText = "";
    pairErr = null;
    paired = true;
    refresh();
    loadAi();
    loadResources();
  }

  async function copyHomeLink() {
    const link = pairingLink();
    if (!link) return;
    try {
      await navigator.clipboard.writeText(link);
      homeLinkCopied = true;
    } catch {
      err = "Couldn't copy the link. Try again.";
    }
  }

  function dismissHomeHint() {
    showHomeHint = false;
    try { localStorage.setItem(HOME_HINT_KEY, "1"); } catch { /* private mode */ }
  }

  onMount(() => {
    captureToken();
    paired = inTauri() || hasToken();
    try {
      showHomeHint = paired && !inTauri() && isIos() && !standalone && !localStorage.getItem(HOME_HINT_KEY);
    } catch {
      showHomeHint = false;
    }
    const vv = window.visualViewport;
    const onVv = () => {
      if (vv) kb = Math.max(0, window.innerHeight - vv.height - vv.offsetTop);
    };
    vv?.addEventListener("resize", onVv);
    vv?.addEventListener("scroll", onVv);
    if (paired) {
      refresh();
      loadAi();
      loadResources();
      loadServerUpdate();
    }
    return () => {
      vv?.removeEventListener("resize", onVv);
      vv?.removeEventListener("scroll", onVv);
    };
  });

  // ---------- updating the server (server_update.rs) ----------

  let srvUpd = $state<ServerUpdateStatus | null>(null);
  let srvUpdStage = $state<"idle" | "confirm" | "installing" | "restarting" | "done">("idle");
  let srvUpdHidden = $state(false);
  let srvUpdTarget = $state("");

  /** The server checks in the background and answers with what it last knew;
   *  when it says it's still checking, ask again shortly. */
  async function loadServerUpdate(retries = 2) {
    if (inTauri() || srvUpdStage !== "idle") return;
    try {
      srvUpd = await serverUpdateStatus();
    } catch {
      return; // an older server without this command, or out of reach: say nothing
    }
    if (srvUpd?.checking && retries > 0) setTimeout(() => loadServerUpdate(retries - 1), 6000);
  }

  async function installServerUpdate() {
    if (!srvUpd?.available) return;
    srvUpdTarget = srvUpd.available.version;
    srvUpdStage = "installing";
    try {
      srvUpd = await serverUpdateInstall();
    } catch (e) {
      err = e instanceof Error ? e.message : String(e);
      srvUpdStage = "idle";
      return;
    }
    // Wait for it to go away and come back on the new version (up to ~5 minutes),
    // then reload: this page is served by the server, so the new version's page
    // comes with it.
    for (let i = 0; i < 60; i++) {
      await new Promise((res) => setTimeout(res, 5000));
      try {
        const st = await serverUpdateStatus();
        srvUpd = st;
        if (st.current === srvUpdTarget) {
          srvUpdStage = "done";
          setTimeout(() => location.reload(), 1500);
          return;
        }
        if (!st.installing && st.error) {
          srvUpdStage = "idle";
          return;
        }
      } catch {
        srvUpdStage = "restarting";
      }
    }
    srvUpdStage = "idle";
    err = "The server hasn't come back yet. If it doesn't, check the computer: it may be waiting for someone.";
  }

  // ---------- plumbing ----------

  /** Anything that fails does so out loud — a phone that silently drops a dose you
   *  thought you logged is worse than a phone that says it couldn't. */
  async function run(key: string, f: () => Promise<unknown>) {
    busyKey = key;
    err = null;
    try {
      await f();
    } catch (e) {
      err = e instanceof Error ? e.message : String(e);
    } finally {
      busyKey = null;
    }
  }

  async function refresh() {
    try {
      recent = await listExperiences();
      // Only a *session* can be live — a plain note has no ended_at either, but
      // it isn't something you're "in".
      const live = recent.find((e) => e.kind === "session" && !e.ended_at);
      session = live ? await getExperience(live.id) : null;
      if (open) open = await getExperience(open.id).catch(() => null);
      if (target) target = await getExperience(target.id).catch(() => null);
    } catch (e) {
      err = e instanceof Error ? e.message : String(e);
    }
  }

  /** A number as typed on a phone. iOS keypads in many regions type "1,5"; that
   *  used to become NaN and quietly save no amount at all. Now it's 1.5, and
   *  anything that still isn't a number is refused out loud. */
  function num(s: string, what: string): number | null {
    const t = s.trim().replace(",", ".");
    if (!t) return null;
    const n = Number(t);
    if (!Number.isFinite(n)) throw new Error(`"${s}" isn't a number — check the ${what}.`);
    return n;
  }

  function closeSheet() {
    sheet = null;
    receipt = null;
    confirmDelete = false;
  }

  function goTo(v: View) {
    view = v;
    if (v === "today") loadServerUpdate();
    if (v === "talk") loadAi();
    if (v === "journal" && !open) refresh();
  }

  // ---------- help ----------

  /** Emergency resources, cached on the phone: the moment you need them may be the
   *  moment the server is out of reach. */
  async function loadResources() {
    try {
      resources = JSON.parse(localStorage.getItem(RESOURCES_KEY) ?? "[]");
    } catch {
      resources = [];
    }
    try {
      resources = await emergencyResources();
      localStorage.setItem(RESOURCES_KEY, JSON.stringify(resources));
    } catch {
      // Keep the cached list (or the built-in fallback in the sheet).
    }
  }

  /** The number to dial for a resource's contact text. The first run of digits that
   *  looks like a phone number — not every digit in the string: stripping the rest
   *  turned "911 (US) · 112 (EU)" into 911112, and "62-FIRESIDE (623-473-7433)"
   *  into a number that belongs to nobody. */
  function telOf(contact: string): string | null {
    const m = contact.match(/\+?\d[\d\s().-]{1,}\d/);
    return m ? m[0].replace(/[^\d+]/g, "") : null;
  }

  function openHelp() {
    sheet = "help";
    loadResources();
  }

  /** After a dose: has this session's combination become dangerous? The crisis layer
   *  reads doses here, never anything written — same rule as the desktop. */
  async function checkCrisis(id: number) {
    try {
      const c = await crisisScan("", id);
      if (c.level !== "none") {
        crisis = c;
        crisisShown = false;
      }
    } catch {
      // The warnings above already said what matters; this is the escalation.
    }
  }

  // ---------- opening things ----------

  async function openEntry(id: number, opts: { building?: boolean } = {}) {
    await flushDelete();
    await run("open", async () => {
      open = await getExperience(id);
      building = !!opts.building;
      view = "journal";
      window.scrollTo(0, 0);
    });
  }

  function openLive() {
    if (session) openEntry(session.id);
  }

  // ---------- times ----------

  const pad2 = (n: number) => String(n).padStart(2, "0");
  // <input type="datetime-local"> works in local time and has no zone; the journal
  // stores UTC. Shift across the offset in both directions rather than slicing an
  // ISO string, which silently backdates an evening entry by a day.
  const localOffset = (d: Date) => new Date(d.getTime() - d.getTimezoneOffset() * 60000);
  const toLocalInput = (d: Date) => localOffset(d).toISOString().slice(0, 16);
  const nowLocalInput = () => toLocalInput(new Date());
  const isoToLocalInput = (iso: string) => toLocalInput(new Date(iso));
  /** A blank time is refused rather than quietly becoming "now" — that's how an
   *  entry about last Tuesday ends up filed under today. */
  function localInputToIso(local: string, what = "time"): string {
    if (!local) throw new Error(`Pick a ${what}.`);
    return new Date(local).toISOString();
  }
  const shift = (local: string, mins: number) =>
    toLocalInput(new Date(new Date(local).getTime() + mins * 60000));

  const DOW = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
  const MON = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  const MONTH = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
  const hhmm = (iso: string) => {
    const d = new Date(iso);
    return `${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
  };
  /** "Sat 20 Sep", with the year only when it isn't this one. */
  const fmtDay = (iso: string) => {
    const d = new Date(iso);
    const y = d.getFullYear() !== new Date().getFullYear() ? ` ${d.getFullYear()}` : "";
    return `${DOW[d.getDay()]} ${d.getDate()} ${MON[d.getMonth()]}${y}`;
  };
  const monthKey = (iso: string) => {
    const d = new Date(iso);
    return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}`;
  };
  const monthLabel = (iso: string) => {
    const d = new Date(iso);
    return `${MONTH[d.getMonth()]} ${d.getFullYear()}`;
  };
  /** "4h 40m", "45m", "2d 3h" — or nothing for an instant (a quick log). */
  function dur(start: string, end: string | null): string {
    if (!end) return "";
    const mins = Math.round((new Date(end).getTime() - new Date(start).getTime()) / 60000);
    if (mins < 1) return "";
    const d = Math.floor(mins / 1440);
    const h = Math.floor((mins % 1440) / 60);
    const m = mins % 60;
    if (d) return `${d}d ${h}h`;
    if (h) return m ? `${h}h ${m}m` : `${h}h`;
    return `${m}m`;
  }
  const sameDay = (a: string, b: string) => new Date(a).toDateString() === new Date(b).toDateString();
  /** "Sat 20 Sep · 21:30 → 02:10 (+1 day) · 4h 40m" */
  function span(e: { started_at: string; ended_at: string | null; kind: string }): string {
    let s = `${fmtDay(e.started_at)} · ${hhmm(e.started_at)}`;
    if (e.kind === "note") return s;
    if (!e.ended_at) return `${s} · live`;
    const d = dur(e.started_at, e.ended_at);
    if (!d) return s;
    s += ` → ${hhmm(e.ended_at)}${sameDay(e.started_at, e.ended_at) ? "" : " (+1 day)"}`;
    return `${s} · ${d}`;
  }
  const fmtAmt = (d: { amount: number | null; unit: string }) =>
    `${d.amount == null ? "?" : +d.amount.toFixed(2)} ${d.unit}`;

  /** Doses and moments live in two tables but tell one story — merge into time order. */
  type Row =
    | { kind: "dose"; at: string; dose: Dose }
    | { kind: "moment"; at: string; ev: TimelineEvent };
  const rowsOf = (e: ExperienceDetail): Row[] =>
    [
      ...e.doses.filter((d) => !hidden.includes(`d:${d.id}`)).map((d) => ({ kind: "dose" as const, at: d.taken_at, dose: d })),
      ...e.timeline.filter((t) => !hidden.includes(`m:${t.id}`)).map((t) => ({ kind: "moment" as const, at: t.at, ev: t })),
    ].sort((a, b) => new Date(a.at).getTime() - new Date(b.at).getTime());

  /** T-zero: the first dose. t+ is counted from ingestion, same rule as the desktop. */
  const t0Of = (e: ExperienceDetail | null) => {
    const doses = e?.doses ?? [];
    if (!doses.length) return null;
    return doses.reduce((a, d) => (new Date(d.taken_at) < new Date(a) ? d.taken_at : a), doses[0].taken_at);
  };
  /** "T+1:20" since the first dose; "T−0:15" before it. */
  const rel = (iso: string, t0: string | null) => {
    if (!t0) return "";
    const ms = new Date(iso).getTime() - new Date(t0).getTime();
    const mins = Math.floor(Math.abs(ms) / 60000);
    return `T${ms < 0 ? "−" : "+"}${Math.floor(mins / 60)}:${pad2(mins % 60)}`;
  };
  /** When the last thing in an entry happened — where the next row most likely goes. */
  const lastAt = (e: ExperienceDetail) => {
    const rows = rowsOf(e);
    return rows.length ? rows[rows.length - 1].at : e.started_at;
  };

  // ---------- adding: doses ----------

  /** Open the dose sheet. `into` null = a quick log that becomes its own entry. */
  function startDose(into: ExperienceDetail | null) {
    target = into;
    receipt = null;
    dSub = dAmt = "";
    if (!into) dWhen = nowLocalInput();
    else if (!into.ended_at) dWhen = nowLocalInput();
    else dWhen = isoToLocalInput(lastAt(into));
    sheet = "dose";
  }

  function pickSubstance(name: string) {
    dSub = name;
    applyRemembered();
  }

  function applyRemembered() {
    const shape = recallDoseShape(dSub);
    if (shape) {
      dUnit = shape.unit;
      dRoute = shape.route;
    } else {
      dUnit = defaultUnitFor(dSub) ?? dUnit;
    }
  }

  /** Time chips that make sense for where the dose is going. */
  const doseTimeChips = $derived.by((): { label: string; value: () => string }[] => {
    if (target && target.ended_at) {
      // A past entry: most things go shortly after the last thing logged.
      const base = isoToLocalInput(lastAt(target));
      return [
        { label: "Same time", value: () => base },
        { label: "+15m", value: () => shift(base, 15) },
        { label: "+30m", value: () => shift(base, 30) },
        { label: "+1h", value: () => shift(base, 60) },
        { label: "+2h", value: () => shift(base, 120) },
      ];
    }
    if (target) {
      return [
        { label: "Now", value: () => nowLocalInput() },
        { label: "15m ago", value: () => shift(nowLocalInput(), -15) },
        { label: "30m ago", value: () => shift(nowLocalInput(), -30) },
        { label: "1h ago", value: () => shift(nowLocalInput(), -60) },
      ];
    }
    return whenPresets.map((p) => ({ label: p.label, value: () => toLocalInput(p.at()) }));
  });

  const submitDose = () =>
    run("dose", async () => {
      if (!dSub.trim()) return;
      const at = localInputToIso(dWhen);
      const substance = dSub.trim();
      const amount = num(dAmt, "amount");
      // quickLog for every dose, not just new entries: it widens the interaction
      // check to anything else taken within 12 hours, and stretches an entry whose
      // times no longer cover what's in it.
      const res = await quickLog({ substance, amount, unit: dUnit, route: dRoute, at, intoId: target?.id ?? null });
      rememberDoseShape(substance, { unit: dUnit, route: dRoute });
      warnFor = { ...warnFor, [res.id]: res.warnings };
      receipt = {
        id: res.id,
        title: res.title,
        line: `${substance} ${fmtAmt({ amount, unit: dUnit })} · ${hhmm(at)} ${fmtDay(at)}`,
        warnings: res.warnings,
      };
      dSub = dAmt = "";
      await refresh();
      target = await getExperience(res.id);
      if (!target.ended_at) dWhen = nowLocalInput();
      else dWhen = isoToLocalInput(lastAt(target));
      await checkCrisis(res.id);
    });

  // ---------- adding: moments ----------

  function startMoment(into: ExperienceDetail) {
    target = into;
    mText = mIntensity = "";
    mWhen = into.ended_at ? isoToLocalInput(lastAt(into)) : nowLocalInput();
    sheet = "moment";
  }

  const momentTimeChips = $derived.by((): { label: string; value: () => string }[] => {
    if (!target) return [];
    if (target.ended_at) {
      const base = isoToLocalInput(lastAt(target));
      return [
        { label: "Same time", value: () => base },
        { label: "+15m", value: () => shift(base, 15) },
        { label: "+30m", value: () => shift(base, 30) },
        { label: "+1h", value: () => shift(base, 60) },
      ];
    }
    return [
      { label: "Now", value: () => nowLocalInput() },
      { label: "15m ago", value: () => shift(nowLocalInput(), -15) },
      { label: "30m ago", value: () => shift(nowLocalInput(), -30) },
    ];
  });

  const submitMoment = () =>
    run("moment", async () => {
      if (!target || !mText.trim()) return;
      const at = localInputToIso(mWhen);
      // Saved as written. Nothing reads the journal over the user's shoulder: crisis
      // guardrails live where the user is *talking to* something (the Companion) and
      // in dangerous combinations — never in what they write here.
      await addTimelineEvent({
        experience_id: target.id,
        at,
        note: mText.trim(),
        intensity: num(mIntensity, "intensity"),
      });
      if (target.ended_at) await stretchToCover(target.id, at);
      await refresh();
      closeSheet();
    });

  // ---------- adding: entries ----------

  const submitJot = () =>
    run("jot", async () => {
      if (!jBody.trim()) return;
      const e = await createExperience({ kind: "note", title: jTitle.trim(), started_at: new Date().toISOString() });
      await updateExperience(e.id, {
        title: e.title,
        notes: jBody.trim(),
        rating: null,
        started_at: e.started_at,
        ended_at: null,
      });
      jTitle = jBody = "";
      closeSheet();
      await refresh();
      await openEntry(e.id);
    });

  // A title is offered but never demanded. Left blank, the backend names the session
  // after the first substance logged into it (`name_after_first_dose` in db.rs).
  const submitStart = () =>
    run("start", async () => {
      await createExperience({ title: sTitle.trim(), started_at: new Date().toISOString() });
      sTitle = "";
      closeSheet();
      await refresh();
      view = "today";
    });

  const pastPresets: { label: string; at: () => Date }[] = [
    { label: "Last night", at: () => atDaysAgo(1, 21) },
    { label: "Yesterday afternoon", at: () => atDaysAgo(1, 15) },
    { label: "2 nights ago", at: () => atDaysAgo(2, 21) },
    { label: "Last weekend", at: () => lastSaturday() },
  ];
  function atDaysAgo(days: number, hour: number) {
    const d = new Date();
    d.setDate(d.getDate() - days);
    d.setHours(hour, 0, 0, 0);
    return d;
  }
  function lastSaturday() {
    const d = new Date();
    d.setDate(d.getDate() - ((d.getDay() + 1) % 7 || 7));
    d.setHours(21, 0, 0, 0);
    return d;
  }

  function startPast() {
    pWhen = toLocalInput(atDaysAgo(1, 21));
    pTitle = "";
    sheet = "past";
  }

  /** Step 1 of logging something that already happened: when it started. The entry is
   *  created *already ended* (at its start, stretched as rows are added) so it never
   *  takes over as the live session. Step 2 is the entry itself, in building mode. */
  const submitPast = () =>
    run("past", async () => {
      const at = localInputToIso(pWhen, "start time");
      if (new Date(at).getTime() > Date.now()) throw new Error("That's in the future — pick when it started.");
      const e = await createExperience({ title: pTitle.trim(), started_at: at });
      await endExperience(e.id, at, null, "");
      closeSheet();
      await refresh();
      await openEntry(e.id, { building: true });
      startDose(open);
    });

  /** A pasted log was saved as a past session: open it, with its warnings. */
  async function pasted(r: { id: number; warnings: Warning[] }) {
    warnFor = { ...warnFor, [r.id]: r.warnings };
    closeSheet();
    await refresh();
    await openEntry(r.id);
    await checkCrisis(r.id);
  }

  // ---------- editing rows ----------

  function startEditDose(d: Dose) {
    editDose = d;
    eSub = d.substance_name;
    eAmt = d.amount?.toString() ?? "";
    eUnit = d.unit ?? "mg";
    eRoute = d.route ?? "oral";
    eWhen = isoToLocalInput(d.taken_at);
    eNote = d.note ?? "";
    sheet = "editDose";
  }

  const saveDose = () =>
    run("editDose", async () => {
      if (!editDose || !eSub.trim()) return;
      const at = localInputToIso(eWhen);
      await updateDose(editDose.id, {
        substance_name: eSub.trim(),
        amount: num(eAmt, "amount"),
        unit: eUnit,
        route: eRoute,
        taken_at: at,
        note: eNote,
      });
      const owner = recent.find((r) => r.id === editDose!.experience_id);
      if (owner?.ended_at) await stretchToCover(editDose.experience_id, at);
      closeSheet();
      await refresh();
    });

  function startEditMoment(t: TimelineEvent) {
    editMoment = t;
    evText = t.note;
    evIntensity = t.intensity != null ? String(t.intensity) : "";
    evWhen = isoToLocalInput(t.at);
    sheet = "editMoment";
  }

  const saveMoment = () =>
    run("editMoment", async () => {
      if (!editMoment || !evText.trim()) return;
      const at = localInputToIso(evWhen);
      await updateTimelineEvent(editMoment.id, {
        at,
        note: evText.trim(),
        mood: editMoment.mood,
        intensity: num(evIntensity, "intensity"),
      });
      const owner = recent.find((r) => r.id === editMoment!.experience_id);
      if (owner?.ended_at) await stretchToCover(editMoment.experience_id, at);
      closeSheet();
      await refresh();
    });

  // ---------- editing the entry ----------

  function startDetails() {
    if (!open) return;
    enTitle = open.title;
    enStart = isoToLocalInput(open.started_at);
    enEnd = open.ended_at ? isoToLocalInput(open.ended_at) : "";
    enRating = open.rating != null ? String(open.rating) : "";
    enIntention = open.intention;
    enSetting = open.setting;
    sheet = "details";
  }

  /** update_experience replaces the whole row, so every field is passed. */
  const fullUpdate = (e: ExperienceDetail, patch: Partial<ExperienceDetail>) =>
    updateExperience(e.id, {
      title: patch.title ?? e.title,
      intention: patch.intention ?? e.intention,
      setting: patch.setting ?? e.setting,
      notes: patch.notes ?? e.notes,
      rating: patch.rating !== undefined ? patch.rating : e.rating,
      started_at: patch.started_at ?? e.started_at,
      ended_at: patch.ended_at !== undefined ? patch.ended_at : e.ended_at,
    });

  const saveDetails = () =>
    run("details", async () => {
      if (!open) return;
      const start = localInputToIso(enStart, "start time");
      const end = enEnd.trim() ? localInputToIso(enEnd, "end time") : null;
      if (end && new Date(end) < new Date(start)) throw new Error("It can't end before it started.");
      await fullUpdate(open, {
        title: enTitle.trim(),
        intention: enIntention,
        setting: enSetting,
        rating: num(enRating, "rating"),
        started_at: start,
        ended_at: open.kind === "note" ? null : end,
      });
      closeSheet();
      await refresh();
    });

  function startWriteup(e: ExperienceDetail | null = open) {
    if (!e) return;
    open = e;
    wuText = e.notes;
    sheet = "writeup";
  }

  const saveWriteup = () =>
    run("writeup", async () => {
      if (!open) return;
      await fullUpdate(open, { notes: wuText });
      closeSheet();
      await refresh();
    });

  /** Ending a live session, or wrapping up a past one: when it ended, how it went,
   *  and the story — one sheet, so "write it up later" isn't another trip. */
  function startEnd(e: ExperienceDetail | null = open) {
    if (!e) return;
    open = e;
    endAt = e.ended_at ? isoToLocalInput(lastAt(e)) : nowLocalInput();
    endRating = e.rating != null ? String(e.rating) : "";
    endNotes = e.notes;
    sheet = "end";
  }

  const saveEnd = () =>
    run("end", async () => {
      if (!open) return;
      const at = localInputToIso(endAt, "end time");
      if (new Date(at) < new Date(open.started_at)) throw new Error("It can't end before it started.");
      const rating = num(endRating, "rating");
      if (!open.ended_at) await endExperience(open.id, at, rating, endNotes);
      else await fullUpdate(open, { ended_at: at, rating, notes: endNotes });
      building = false;
      closeSheet();
      await refresh();
    });

  // ---------- deleting, with Undo ----------

  /** Hide it now, delete it in a few seconds unless Undo is pressed. If the page is
   *  closed first, nothing is deleted — the safe way for this to fail. */
  async function scheduleDelete(label: string, key: string, commit: () => Promise<void>) {
    await flushDelete();
    hidden = [...hidden, key];
    const timer = window.setTimeout(flushDelete, UNDO_MS);
    pendingDelete = { label, key, commit, timer };
  }

  async function flushDelete() {
    const p = pendingDelete;
    if (!p) return;
    pendingDelete = null;
    clearTimeout(p.timer);
    try {
      await p.commit();
    } catch (e) {
      err = e instanceof Error ? e.message : String(e);
    }
    hidden = hidden.filter((k) => k !== p.key);
    await refresh();
  }

  function undoDelete() {
    const p = pendingDelete;
    if (!p) return;
    clearTimeout(p.timer);
    hidden = hidden.filter((k) => k !== p.key);
    pendingDelete = null;
  }

  function removeDose() {
    const d = editDose;
    if (!d) return;
    closeSheet();
    scheduleDelete(`${d.substance_name} dose deleted`, `d:${d.id}`, () => deleteDose(d.id));
  }

  function removeMoment() {
    const t = editMoment;
    if (!t) return;
    closeSheet();
    scheduleDelete("Moment deleted", `m:${t.id}`, () => deleteTimelineEvent(t.id));
  }

  function removeEntry() {
    const e = open;
    if (!e) return;
    closeSheet();
    open = null;
    building = false;
    scheduleDelete(`"${e.title || "Entry"}" deleted`, `e:${e.id}`, () => deleteExperience(e.id));
  }

  // ---------- export ----------

  /** Export the opened entry as a Markdown download. Three mobile-browser quirks:
   *  an opaque MIME type so Safari keeps the `.md` name; the anchor goes into the
   *  document before it's clicked; the object URL is revoked on a later tick. */
  const exportOpen = () =>
    run("export", async () => {
      if (!open) return;
      const note = await exportExperienceMarkdown(open.id);
      const url = URL.createObjectURL(new Blob([note.markdown], { type: "application/octet-stream" }));
      const a = document.createElement("a");
      a.href = url;
      a.download = note.filename;
      a.style.display = "none";
      document.body.appendChild(a);
      a.click();
      a.remove();
      setTimeout(() => URL.revokeObjectURL(url), 10_000);
      closeSheet();
    });

  // ---------- the journal list ----------

  const needsWriteup = (e: ExperienceSummary) => e.kind === "session" && !!e.ended_at && !e.notes.trim();
  const visible = $derived(recent.filter((e) => !hidden.includes(`e:${e.id}`)));
  const shown = $derived.by(() => {
    const q = search.trim().toLowerCase();
    return visible.filter((e) => {
      if (filter === "sessions" && e.kind !== "session") return false;
      if (filter === "notes" && e.kind !== "note") return false;
      if (filter === "writeup" && !needsWriteup(e)) return false;
      if (subFilter && !e.substances.some((s) => s.toLowerCase() === subFilter!.toLowerCase())) return false;
      if (q && !`${e.title} ${e.notes} ${e.substances.join(" ")}`.toLowerCase().includes(q)) return false;
      return true;
    });
  });
  const groups = $derived.by(() => {
    const out: { key: string; label: string; items: ExperienceSummary[] }[] = [];
    for (const e of shown) {
      const key = monthKey(e.started_at);
      if (out.at(-1)?.key !== key) out.push({ key, label: monthLabel(e.started_at), items: [] });
      out.at(-1)!.items.push(e);
    }
    return out;
  });
  const listSubs = $derived(recentSubstances(visible, 12));
  const toWriteUp = $derived(visible.filter(needsWriteup).slice(0, 3));

  const excerpt = (s: string) => {
    const t = s.trim().replace(/\s+/g, " ");
    return t.length > 90 ? `${t.slice(0, 90)}…` : t;
  };

  const loadUsage = () =>
    run("usage", async () => {
      usage = await usageBySubstance();
      showUsage = true;
    });

  // ---------- check ----------

  function useLiveInCombo() {
    if (!session) return;
    comboText = [...new Set(session.doses.map((d) => d.substance_name))].join(", ");
  }

  const runCombo = () =>
    run("combo", async () => {
      const names = comboText.split(/[,+\n]/).map((s) => s.trim()).filter(Boolean);
      if (names.length < 2) throw new Error("Name at least two, separated by commas.");
      comboWarnings = await checkCombo(names);
    });

  const lookUp = () =>
    run("lookup", async () => {
      const q = refQuery.trim();
      if (!q) return;
      // Doses come from the deterministic reference; prose comes from the corpus.
      pw = await pwLookup(q);
      hits = await knowledgeSearch(q, 6);
      searched = true;
    });

  const loadSubstances = () => run("subs", async () => (substances = await listSubstances()));

  const addToCatalogue = () =>
    run("addsub", async () => {
      if (!newSub.trim()) return;
      await addSubstance({ name: newSub.trim() });
      newSub = "";
      substances = await listSubstances();
    });

  const range = (r: { min: number | null; max: number | null }) =>
    r.min == null && r.max == null ? "—" : `${r.min ?? "?"}–${r.max ?? "?"}`;

  // ---------- companion ----------

  /** Ask the server what its local model is doing *right now* — re-checked whenever
   *  Talk opens, so a model that was asleep at page load isn't dead for good. */
  async function loadAi() {
    try {
      companionEnabled = await companionEnabledPref();
      ai = await aiStatus();
      models = ai.models;
      if (!models.includes(model)) model = models[0] ?? "";
    } catch {
      ai = null;
      models = [];
    }
    if (companionEnabled === false && view === "talk") view = "today";
  }

  const wakeAi = () =>
    run("wake", async () => {
      waking = true;
      try {
        await aiStart();
        await new Promise((r) => setTimeout(r, 1500));
        await loadAi();
      } finally {
        waking = false;
      }
    });

  // A Companion turn on a slow local model can take minutes, and mobile Safari kills
  // a silent request at ~60s (instantly when the screen locks). So the turn runs as
  // a job on the server and the phone polls: each poll is a fresh, fast request.
  async function awaitCompanionJob(job: number): Promise<Exclude<CompanionPoll, { status: "running" }>> {
    let misses = 0;
    for (;;) {
      await new Promise((r) => setTimeout(r, 2000));
      let poll: CompanionPoll;
      try {
        poll = await companionChatPoll(job);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        if (msg.includes("paired") || msg.includes("unknown or expired")) throw e;
        if (++misses >= 5) throw e;
        continue;
      }
      misses = 0;
      if (poll.status !== "running") return poll;
    }
  }

  const send = () =>
    run("send", async () => {
      if (!ask.trim() || !model) return;
      const text = ask.trim();
      const next: ChatMsg[] = [...chat, { role: "user", content: text }];
      chat = next;
      ask = "";
      thinking = true;
      // The deterministic crisis layer runs on what's *said to* the Companion,
      // independent of the model's reply — same as the desktop.
      crisisScan(text, session?.id ?? null, next.slice(0, -1).filter((m) => m.role === "user").map((m) => m.content))
        .then((r) => {
          if (r.level !== "none") {
            chatCrisis = r;
            chatCrisisShown = false;
          }
        })
        .catch(() => {});
      try {
        let reply: CompanionReply;
        if (inTauri()) {
          reply = await companionChat(model, next, session?.id ?? null, null);
        } else {
          const { job } = await companionChatStart(model, next, session?.id ?? null, null);
          const done = await awaitCompanionJob(job);
          if (done.status === "error") throw new Error(done.error);
          reply = done.reply;
        }
        chat = [...next, { role: "assistant", content: reply.reply }];
        if (reply.journal_changed) await refresh();
      } finally {
        thinking = false;
      }
    });
</script>

<svelte:head><title>Field Notes</title></svelte:head>

<!-- ================= snippets ================= -->

<!-- 0–10 as big taps, not a number typed with unsteady hands. Tap again to clear. -->
{#snippet scale(value: string, set: (v: string) => void, label: string)}
  <div class="scale" role="group" aria-label={label}>
    {#each Array.from({ length: 11 }, (_, i) => String(i)) as i}
      <button type="button" class="chip num" class:on={value === i} aria-pressed={value === i} onclick={() => set(value === i ? "" : i)}>{i}</button>
    {/each}
  </div>
{/snippet}

{#snippet warnings(list: Warning[])}
  {#each list as w}
    <p class="banner {w.severity}" role={w.severity === "danger" ? "alert" : "status"}>
      <strong>{w.severity === "danger" ? "⚠ Danger:" : w.severity === "caution" ? "Caution:" : "Note:"}</strong>
      {w.message}
    </p>
  {/each}
{/snippet}

{#snippet resourceList(list: CrisisResource[])}
  <ul class="resources">
    {#each list as r}
      <li>
        <strong>{r.label}</strong>
        {#if r.contact && telOf(r.contact)}
          <a class="call" href="tel:{telOf(r.contact)}">{r.contact}</a>
        {:else if r.contact}
          <span class="call">{r.contact}</span>
        {/if}
        {#if r.detail}<span class="muted small">{r.detail}</span>{/if}
      </li>
    {/each}
  </ul>
{/snippet}

<!-- One entry's doses and moments in time order; every line opens its editor. -->
{#snippet timeline(e: ExperienceDetail)}
  {@const t0 = t0Of(e)}
  {@const rows = rowsOf(e)}
  {#if rows.length}
    <ol class="tl">
      {#each rows as r (r.kind + (r.kind === "dose" ? r.dose.id : r.ev.id))}
        <li>
          <button
            class="row"
            onclick={() => (r.kind === "dose" ? startEditDose(r.dose) : startEditMoment(r.ev))}
            aria-label={r.kind === "dose"
              ? `Edit dose: ${r.dose.substance_name} ${fmtAmt(r.dose)} at ${hhmm(r.at)}`
              : `Edit moment at ${hhmm(r.at)}`}
          >
            <span class="when"><span>{hhmm(r.at)}</span>{#if t0}<span class="rel">{rel(r.at, t0)}</span>{/if}</span>
            <span class="what">
              {#if r.kind === "dose"}
                <span class="dot dose" aria-hidden="true"></span><strong>{r.dose.substance_name}</strong>
                {fmtAmt(r.dose)} <span class="muted">{r.dose.route}</span>
                {#if r.dose.note}<span class="sub">{r.dose.note}</span>{/if}
              {:else}
                <span class="dot moment" aria-hidden="true"></span>{r.ev.note}
                {#if r.ev.intensity != null}<span class="muted"> · {r.ev.intensity}/10</span>{/if}
              {/if}
            </span>
            <span class="chev" aria-hidden="true">›</span>
          </button>
        </li>
      {/each}
    </ol>
  {:else}
    <p class="muted">Nothing logged in it yet.</p>
  {/if}
{/snippet}

{#snippet entryRow(e: ExperienceSummary)}
  <button class="entry" onclick={() => openEntry(e.id)}>
    <span class="date">
      <span class="dow">{DOW[new Date(e.started_at).getDay()]}</span>
      <span class="dnum">{new Date(e.started_at).getDate()}</span>
    </span>
    <span class="body">
      <span class="title">
        {e.title || (e.kind === "note" ? "Untitled note" : "Untitled")}
        {#if e.kind === "note"}<span class="tag">note</span>{/if}
        {#if e.kind === "session" && !e.ended_at}<span class="tag live">live</span>{/if}
      </span>
      {#if e.kind === "session"}
        <span class="meta">
          {[e.substances.join(", ") === e.title ? "" : e.substances.join(", "), dur(e.started_at, e.ended_at), e.rating != null ? `${e.rating}/10` : ""].filter(Boolean).join(" · ")}
        </span>
      {/if}
      {#if e.notes.trim()}
        <span class="excerpt">{excerpt(e.notes)}</span>
      {:else if needsWriteup(e)}
        <span class="excerpt pending">No write-up yet</span>
      {/if}
    </span>
    <span class="chev" aria-hidden="true">›</span>
  </button>
{/snippet}

<!-- ================= page ================= -->

<main style="--kb: {kb}px">
  {#if !paired}
    <section class="pane">
      <h1>Not paired</h1>
      {#if standalone}
        <p>
          The Home Screen app keeps its own storage, separate from Safari, so it needs pairing once
          on its own. Paste your pairing link here:
        </p>
        <ul class="muted small steps">
          <li>If this phone is already paired in Safari, open Field Notes there and tap
            <strong>Copy link for the Home Screen app</strong>.</li>
          <li>Or, on your server, open <strong>Settings → Devices &amp; server → Pair a device</strong>
            and use <strong>Copy link</strong>. Send it to yourself privately; it's a key.</li>
        </ul>
        <form onsubmit={(e) => { e.preventDefault(); pairFromText(); }}>
          <label for="pair-link">Pairing link</label>
          <input id="pair-link" bind:value={pairText} placeholder="https://…/m#t=…"
            autocomplete="off" autocapitalize="off" spellcheck="false" />
          {#if pairErr}<p class="err" role="alert">{pairErr}</p>{/if}
          <button class="primary" type="submit" disabled={!pairText.trim()}>Pair this app</button>
        </form>
      {:else}
        <p>
          On the computer that runs your Field Notes server, open <strong>Settings → Devices &amp; server →
          Pair a device</strong>, name this phone, and scan the code with its camera.
        </p>
      {/if}
    </section>
  {:else}
    <header class="top">
      {#if view === "journal" && open}
        <button class="back" onclick={() => { open = null; building = false; }} aria-label="Back to journal">‹ Journal</button>
      {:else}
        <span class="brand">Field Notes</span>
      {/if}
      <span class="top-right">
        {#if session && !(view === "journal" && open?.id === session.id)}
          <button class="pill live" onclick={openLive} aria-label="Open the live session">
            <span class="live-dot" aria-hidden="true"></span>{session.title || "Live session"}
          </button>
        {/if}
        <button class="help" onclick={openHelp}>Help</button>
      </span>
    </header>

    {#if crisis && crisis.level !== "none"}
      <section class="banner danger crisis" role="alert">
        <strong>{crisis.headline}</strong>
        {#if crisis.presentation === "offer" && !crisisShown}
          <div class="pair">
            <button onclick={() => (crisisShown = true)}>Show me some options</button>
            <button class="ghost" onclick={() => (crisis = null)}>No thanks</button>
          </div>
        {:else}
          {@render resourceList(crisis.resources)}
          <button class="ghost" onclick={() => (crisis = null)}>Dismiss</button>
        {/if}
      </section>
    {/if}

    <!-- ================= TODAY ================= -->
    {#if view === "today"}
      {#if srvUpdStage !== "idle" || (srvUpd?.available && !srvUpdHidden)}
        <section class="pane update-card" role="status">
          {#if srvUpdStage === "installing"}
            <p><strong>Installing v{srvUpdTarget} on the server…</strong></p>
            <p class="muted">Field Notes on the computer will restart. This phone reconnects by itself.</p>
          {:else if srvUpdStage === "restarting"}
            <p><strong>The server is restarting…</strong></p>
            <p class="muted">This usually takes a minute or two.</p>
          {:else if srvUpdStage === "done"}
            <p><strong>The server is on v{srvUpdTarget}.</strong> Reloading…</p>
          {:else if srvUpd?.available}
            <p>
              <strong>Field Notes v{srvUpd.available.version}</strong> is ready to install on the server
              (it's on v{srvUpd.current}).
            </p>
            {#if srvUpdStage === "confirm"}
              <p class="muted">
                Field Notes on the computer will restart, and every paired device loses access for a minute or two.
              </p>
              <div class="pair">
                <button class="primary" onclick={installServerUpdate}>Install now</button>
                <button onclick={() => (srvUpdStage = "idle")}>Cancel</button>
              </div>
            {:else if srvUpd.blocked}
              <p class="muted">{srvUpd.blocked}</p>
              <button class="ghost small" onclick={() => (srvUpdHidden = true)}>Hide</button>
            {:else}
              <div class="pair">
                <button class="primary" onclick={() => (srvUpdStage = "confirm")}>Install on the server…</button>
                <button onclick={() => (srvUpdHidden = true)}>Not now</button>
              </div>
            {/if}
            {#if srvUpd.error}<p class="muted">{srvUpd.error}</p>{/if}
          {/if}
        </section>
      {/if}
      {#if session}
        {@const live = session}
        <section class="pane live-card">
          <p class="eyebrow"><span class="live-dot" aria-hidden="true"></span>Live session</p>
          <h1 class="entry-title">{live.title || "Untitled session"}</h1>
          <p class="muted">
            Started {hhmm(live.started_at)}{#if t0Of(live)} · now {rel(new Date().toISOString(), t0Of(live))}{/if}
          </p>
          {#if warnFor[live.id]?.length}
            {@render warnings(warnFor[live.id])}
            <button class="ghost small" onclick={() => (warnFor = { ...warnFor, [live.id]: [] })}>Dismiss warnings</button>
          {/if}
          {@render timeline({ ...live, doses: live.doses.slice(-4), timeline: live.timeline.slice(-3) })}
          <div class="pair">
            <button class="primary" onclick={() => startDose(live)}>+ Dose</button>
            <button class="primary" onclick={() => startMoment(live)}>+ Moment</button>
          </div>
          <div class="pair">
            <button onclick={openLive}>Open session</button>
            <button onclick={() => startEnd(live)}>End…</button>
          </div>
        </section>
      {:else}
        <section class="pane">
          <h1>Log something you took</h1>
          <p class="muted">A substance and roughly when. That's a whole entry — nothing to end, nothing to write.</p>
          <button class="primary big" onclick={() => startDose(null)}>+ Log a dose</button>
          <div class="pair">
            <button onclick={() => (sheet = "start")}>Start a session</button>
            <button onclick={startPast}>Log a past session</button>
          </div>
          <button onclick={() => (sheet = "jot")}>Write a journal note</button>
        </section>
      {/if}

      {#if toWriteUp.length}
        <section class="pane">
          <h2>Waiting for a write-up</h2>
          <ul class="entries">
            {#each toWriteUp as e (e.id)}
              <li>
                <button class="entry" onclick={async () => { await openEntry(e.id); startWriteup(open); }}>
                  <span class="body">
                    <span class="title">{e.title || "Untitled"}</span>
                    <span class="meta">{[fmtDay(e.started_at), e.substances.join(", ") === e.title ? "" : e.substances.join(", ")].filter(Boolean).join(" · ")}</span>
                  </span>
                  <span class="chev" aria-hidden="true">Write ›</span>
                </button>
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      {#if showHomeHint}
        <section class="pane">
          <h2>Saving to your Home Screen?</h2>
          <p class="muted small">
            iPhone keeps a Home Screen app's storage separate from Safari, so it opens unpaired. Copy
            this phone's pairing link first, then paste it when the app asks. It's a key, so don't
            share it.
          </p>
          <button onclick={copyHomeLink}>{homeLinkCopied ? "Link copied ✓" : "Copy link for the Home Screen app"}</button>
          <button class="ghost" onclick={dismissHomeHint}>Don't show this again</button>
        </section>
      {/if}

      <section class="pane">
        <h2>Recent</h2>
        <ul class="entries">
          {#each visible.slice(0, 4) as e (e.id)}
            <li>{@render entryRow(e)}</li>
          {:else}
            <li class="muted">Nothing logged yet.</li>
          {/each}
        </ul>
        {#if visible.length > 4}
          <button class="ghost" onclick={() => goTo("journal")}>All entries ›</button>
        {/if}
      </section>
    {/if}

    <!-- ================= JOURNAL ================= -->
    {#if view === "journal"}
      {#if open}
        {@const e = open}
        <article class="pane doc">
          {#if building}
            <p class="banner note" role="status">
              Logging a past session. Add what you took and what happened — each new line starts just after
              the last. Press <strong>Finish</strong> when you're done.
            </p>
          {/if}
          <h1 class="entry-title">{e.title || (e.kind === "note" ? "Untitled note" : "Untitled")}</h1>
          <p class="facts">
            {span(e)}{e.rating != null ? ` · Rated ${e.rating}/10` : ""}
          </p>

          {#if warnFor[e.id]?.length}
            {@render warnings(warnFor[e.id])}
            <button class="ghost small" onclick={() => (warnFor = { ...warnFor, [e.id]: [] })}>Dismiss warnings</button>
          {/if}

          {#if e.kind === "session"}
            <h2 class="sec">Timeline</h2>
            {@render timeline(e)}
          {/if}

          {#if e.intention}
            <h2 class="sec">Intention</h2>
            <p class="prose">{e.intention}</p>
          {/if}
          {#if e.setting}
            <h2 class="sec">Setting</h2>
            <p class="prose">{e.setting}</p>
          {/if}

          <div class="sec-head">
            <h2 class="sec">{e.kind === "note" ? "Note" : "Write-up"}</h2>
            <button class="ghost small" onclick={() => startWriteup(e)}>{e.notes.trim() ? "Edit" : "Write"}</button>
          </div>
          {#if e.notes.trim()}
            <p class="prose">{e.notes}</p>
          {:else}
            <p class="muted">{e.kind === "note" ? "Empty." : "No write-up yet."}</p>
          {/if}
        </article>

        <!-- Sticky, at the thumb: the things you do to an entry. -->
        <div class="actionbar">
          {#if e.kind === "session"}
            <button class="primary" onclick={() => startDose(e)}>+ Dose</button>
            <button class="primary" onclick={() => startMoment(e)}>+ Moment</button>
            {#if building || !e.ended_at}
              <button onclick={() => startEnd(e)}>{building ? "Finish" : "End…"}</button>
            {/if}
          {/if}
          <button class="more" onclick={() => (sheet = "more")} aria-label="More: edit details, export, delete">⋯</button>
        </div>
      {:else}
        <section class="pane">
          <h1>Journal</h1>
          <div class="seg" role="tablist" aria-label="Journal view">
            <button role="tab" aria-selected={journalMode === "entries"} class:on={journalMode === "entries"} onclick={() => (journalMode = "entries")}>Entries</button>
            <button role="tab" aria-selected={journalMode === "stats"} class:on={journalMode === "stats"} onclick={() => (journalMode = "stats")}>Stats</button>
          </div>
        </section>
        {#if journalMode === "stats"}
          <section class="pane">
            <UsageStats
              onOpen={(id) => { journalMode = "entries"; openEntry(id); }}
              onCheck={(names) => { comboText = names.join(", "); checkMode = "combo"; goTo("check"); runCombo(); }}
            />
          </section>
        {:else}
        <section class="pane">
          <label class="sr" for="search">Search the journal</label>
          <input id="search" type="search" placeholder="Search titles, write-ups, substances" bind:value={search} autocapitalize="none" enterkeyhint="search" />
          <div class="chips" role="group" aria-label="Show">
            {#each [["all", "All"], ["sessions", "Sessions"], ["notes", "Journal notes"], ["writeup", "No write-up"]] as [k, label]}
              <button class="chip" class:on={filter === k} aria-pressed={filter === k} onclick={() => (filter = k as typeof filter)}>{label}</button>
            {/each}
          </div>
          {#if listSubs.length}
            <div class="chips scroll" role="group" aria-label="Filter by substance">
              {#each listSubs as s}
                <button class="chip" class:on={subFilter === s} aria-pressed={subFilter === s} onclick={() => (subFilter = subFilter === s ? null : s)}>{s}</button>
              {/each}
            </div>
          {/if}
        </section>

        {#each groups as g (g.key)}
          <section class="pane month">
            <h2 class="month-label">{g.label}</h2>
            <ul class="entries">
              {#each g.items as e (e.id)}
                <li>{@render entryRow(e)}</li>
              {/each}
            </ul>
          </section>
        {:else}
          <section class="pane">
            {#if visible.length}
              <p>Nothing matches that.</p>
              <button onclick={() => { search = ""; filter = "all"; subFilter = null; }}>Clear filters</button>
            {:else}
              <p>Nothing here yet. Log something from Today, or add something that already happened.</p>
              <button class="primary" onclick={startPast}>Log a past session</button>
            {/if}
          </section>
        {/each}

        <section class="pane">
          <h2>By substance</h2>
          {#if !showUsage}
            <button disabled={busy} onclick={loadUsage}>Show the substance log</button>
          {:else}
            <ul class="plain">
              {#each usage as u}
                <li><strong>{u.substance_name}</strong> <span class="muted">· {u.times_used} time{u.times_used === 1 ? "" : "s"}</span></li>
              {/each}
            </ul>
          {/if}
        </section>
        {/if}
      {/if}
    {/if}

    <!-- ================= CHECK ================= -->
    {#if view === "check"}
      <section class="pane">
        <div class="seg" role="tablist">
          <button role="tab" aria-selected={checkMode === "combo"} class:on={checkMode === "combo"} onclick={() => (checkMode = "combo")}>Combination</button>
          <button role="tab" aria-selected={checkMode === "lookup"} class:on={checkMode === "lookup"} onclick={() => (checkMode = "lookup")}>Look up</button>
        </div>

        {#if checkMode === "combo"}
          <label for="combo">Two or more substances, separated by commas</label>
          <input id="combo" placeholder="e.g. MDMA, ketamine" bind:value={comboText} autocapitalize="none" enterkeyhint="go" onkeydown={(ev) => ev.key === "Enter" && runCombo()} />
          {#if session?.doses.length}
            <button class="ghost small" onclick={useLiveInCombo}>Use what's in the live session</button>
          {/if}
          <button class="primary" disabled={busy} onclick={runCombo}>{busyKey === "combo" ? "Checking…" : "Check"}</button>
          {#if comboWarnings}
            {#if comboWarnings.length === 0}
              <p class="banner note" role="status">Nothing flagged between those. That isn't the same as "safe".</p>
            {:else}
              {@render warnings(comboWarnings)}
            {/if}
          {/if}
        {:else}
          <label for="ref">Substance</label>
          <input id="ref" placeholder="e.g. ketamine" bind:value={refQuery} autocapitalize="none" enterkeyhint="search" onkeydown={(ev) => ev.key === "Enter" && lookUp()} />
          <button class="primary" disabled={busy || !refQuery.trim()} onclick={lookUp}>{busyKey === "lookup" ? "Looking…" : "Look up"}</button>

          {#if pw}
            <h2 class="sec">{pw.name} — doses</h2>
            {#each pw.roas as roa}
              <div class="roa">
                <strong>{roa.name}</strong> <span class="muted">{roa.units ?? ""}</span>
                <dl class="doses">
                  <dt>Threshold</dt><dd>{roa.threshold ?? "—"}</dd>
                  <dt>Light</dt><dd>{range(roa.light)}</dd>
                  <dt>Common</dt><dd>{range(roa.common)}</dd>
                  <dt>Strong</dt><dd>{range(roa.strong)}</dd>
                  <dt>Heavy</dt><dd>{roa.heavy ?? "—"}</dd>
                  {#if roa.onset}<dt>Onset</dt><dd>{roa.onset}</dd>{/if}
                  {#if roa.total}<dt>Total</dt><dd>{roa.total}</dd>{/if}
                </dl>
              </div>
            {/each}
            {#if pw.interactions.length}
              <h2 class="sec">Interactions</h2>
              {#each pw.interactions as i}
                <p class="banner {i.severity}">{i.name}{i.reason ? ` — ${i.reason}` : ""}</p>
              {/each}
            {/if}
          {:else if searched}
            <p class="muted">No dose data for that. The reference below may still help.</p>
          {/if}

          {#if hits.length}
            <h2 class="sec">From the reference</h2>
            {#each hits as h}
              <div class="hit">
                <strong>{h.title}</strong> <span class="muted">· {h.section}</span>
                {#if h.thin}<span class="flag">thin entry</span>{/if}
                <p>{h.text}</p>
              </div>
            {/each}
          {:else if searched}
            <p class="muted">Nothing in the reference for that.</p>
          {/if}
        {/if}
      </section>

      {#if checkMode === "lookup"}
        <section class="pane">
          <h2>Your substances</h2>
          {#if !substances.length}
            <button disabled={busy} onclick={loadSubstances}>Show the catalogue</button>
          {:else}
            <ul class="plain">
              {#each substances as s}
                <li><strong>{s.name}</strong> <span class="muted">{s.category ?? ""}</span></li>
              {/each}
            </ul>
          {/if}
          <label for="newsub">Add a substance</label>
          <input id="newsub" bind:value={newSub} autocapitalize="none" />
          <button disabled={busy || !newSub.trim()} onclick={addToCatalogue}>Add</button>
        </section>
      {/if}
    {/if}

    <!-- ================= TALK ================= -->
    {#if view === "talk"}
      <section class="pane">
        <h1>Companion</h1>
        {#if companionEnabled === false}
          <p class="muted">The Companion is turned off in Settings on your server. Everything else still works.</p>
        {:else if !ai}
          <p class="muted">Couldn't ask the server about its local model.</p>
          <button disabled={busy} onclick={loadAi}>Try again</button>
        {:else if !ai.installed}
          <p class="muted">
            The Companion runs on a local model, and your server doesn't have one installed. Install it
            there — it's a big download, not something to start from a phone.
          </p>
        {:else if !ai.running}
          <p class="muted">The local model isn't running on your server right now. Everything else still works.</p>
          <button class="primary" disabled={busy || waking} onclick={wakeAi}>{waking ? "Starting it…" : "Start it on the server"}</button>
        {:else if !models.length}
          <p class="muted">The model server is running, but no models are installed. Add one on your server.</p>
          <button disabled={busy} onclick={loadAi}>Check again</button>
        {:else}
          {#if models.length > 1}
            <label for="model">Model</label>
            <select id="model" bind:value={model}>
              {#each models as m}<option>{m}</option>{/each}
            </select>
          {/if}
          {#if chatCrisis && chatCrisis.level !== "none"}
            <div class="banner danger" role="alert">
              <strong>{chatCrisis.headline}</strong>
              {#if chatCrisis.presentation === "offer" && !chatCrisisShown}
                <div class="pair">
                  <button onclick={() => (chatCrisisShown = true)}>Show me some options</button>
                  <button class="ghost" onclick={() => (chatCrisis = null)}>No thanks</button>
                </div>
              {:else}
                {@render resourceList(chatCrisis.resources)}
                <button class="ghost" onclick={() => (chatCrisis = null)}>Dismiss</button>
              {/if}
            </div>
          {/if}
          <div class="chat" aria-live="polite">
            {#each chat as m}
              <p class="msg {m.role}">{m.content}</p>
            {/each}
            {#if thinking}<p class="msg assistant muted">…</p>{/if}
          </div>
          <label class="sr" for="ask">Message</label>
          <textarea id="ask" rows="2" placeholder="Say anything" bind:value={ask}></textarea>
          <button class="primary" disabled={thinking || !ask.trim()} onclick={send}>Send</button>
        {/if}
      </section>
    {/if}

    <!-- ================= messages: next to the thumb, announced ================= -->
    <div class="toasts" aria-live="assertive">
      {#if err}
        <div class="toast bad" role="alert">
          <span>{err}</span>
          <button class="ghost small" onclick={() => (err = null)}>OK</button>
        </div>
      {/if}
      {#if pendingDelete}
        <div class="toast" role="status">
          <span>{pendingDelete.label}</span>
          <button class="small" onclick={undoDelete}>Undo</button>
        </div>
      {/if}
    </div>

    <!-- ================= nav ================= -->
    <nav aria-label="Sections">
      <button class:on={view === "today"} aria-current={view === "today" ? "page" : undefined} onclick={() => goTo("today")}>Today</button>
      <button class:on={view === "journal"} aria-current={view === "journal" ? "page" : undefined} onclick={() => goTo("journal")}>Journal</button>
      <button class="plus" aria-label="New: log a dose, a session, or a note" onclick={() => (sheet = "new")}>＋</button>
      <button class:on={view === "check"} aria-current={view === "check" ? "page" : undefined} onclick={() => goTo("check")}>Check</button>
      {#if companionEnabled === true}
        <button class:on={view === "talk"} aria-current={view === "talk" ? "page" : undefined} onclick={() => goTo("talk")}>Talk</button>
      {/if}
    </nav>

    <!-- ================= sheets ================= -->
    {#if sheet}
      <button class="backdrop" aria-label="Close" onclick={closeSheet}></button>
      <div class="sheet" role="dialog" aria-modal="true" aria-labelledby="sheet-title">
        {#if sheet === "new"}
          <h2 id="sheet-title">New</h2>
          <ul class="menu">
            <li>
              <button onclick={() => startDose(session)}>
                <strong>Log a dose</strong>
                <span class="muted">{session ? "Into the live session" : "A substance and roughly when — any day"}</span>
              </button>
            </li>
            {#if !session}
              <li>
                <button onclick={() => (sheet = "start")}>
                  <strong>Start a live session</strong>
                  <span class="muted">Log doses and moments as it happens</span>
                </button>
              </li>
            {/if}
            <li>
              <button onclick={startPast}>
                <strong>Log a past session</strong>
                <span class="muted">Something that already happened: doses, timeline, write-up</span>
              </button>
            </li>
            <li>
              <button onclick={() => (sheet = "paste")}>
                <strong>Paste a trip log</strong>
                <span class="muted">From notes or a message — times become the timeline</span>
              </button>
            </li>
            <li>
              <button onclick={() => (sheet = "jot")}>
                <strong>Write a journal note</strong>
                <span class="muted">Just writing — no doses</span>
              </button>
            </li>
          </ul>
          <button class="help wide" onclick={openHelp}>Need help now?</button>
          <button class="ghost" onclick={closeSheet}>Cancel</button>

        {:else if sheet === "dose"}
          <div class="sheet-head">
            <h2 id="sheet-title">
              {#if !target}Log a dose{:else if !target.ended_at}Dose · live session{:else}Dose · {target.title || fmtDay(target.started_at)}{/if}
            </h2>
            <button class="ghost small" onclick={closeSheet}>Close</button>
          </div>

          {#if receipt}
            <div class="receipt" role="status">
              <p><strong>✓ Saved</strong> · {receipt.line}</p>
              {@render warnings(receipt.warnings)}
            </div>
          {/if}

          {#if qRecents.length}
            <div class="chips scroll" role="group" aria-label="Recent substances">
              {#each qRecents as s}
                <button class="chip" class:on={dSub === s} aria-pressed={dSub === s} onclick={() => pickSubstance(s)}>{s}</button>
              {/each}
            </div>
          {/if}
          <label for="d-sub">Substance</label>
          <input id="d-sub" bind:value={dSub} onblur={applyRemembered} autocapitalize="none" autocomplete="off" enterkeyhint="next" />
          <div class="grid3">
            <div>
              <label for="d-amt">Amount</label>
              <input id="d-amt" inputmode="decimal" bind:value={dAmt} enterkeyhint="done" />
            </div>
            <div>
              <label for="d-unit">Unit</label>
              <select id="d-unit" bind:value={dUnit}>{#each UNITS as u}<option>{u}</option>{/each}</select>
            </div>
            <div>
              <label for="d-route">Route</label>
              <select id="d-route" bind:value={dRoute}>{#each ROUTES as r}<option>{r}</option>{/each}</select>
            </div>
          </div>
          <label for="d-when">When</label>
          <div class="chips" role="group" aria-label="Quick times">
            {#each doseTimeChips as c}
              <button class="chip" onclick={() => (dWhen = c.value())}>{c.label}</button>
            {/each}
          </div>
          <DateTimeField id="d-when" bind:value={dWhen} variant="phone" />
          {#if target && t0Of(target) && dWhen}
            <p class="hint">{rel(new Date(dWhen).toISOString(), t0Of(target))} from the first dose</p>
          {/if}

          <div class="sheet-actions">
            <button class="primary" disabled={busy || !dSub.trim()} onclick={submitDose}>
              {busyKey === "dose" ? "Saving…" : receipt ? "Log another" : "Log it"}
            </button>
            {#if receipt && !building}
              <div class="pair">
                <button onclick={async () => { const id = receipt!.id; closeSheet(); await openEntry(id); startWriteup(open); }}>Write it up</button>
                <button onclick={closeSheet}>Done</button>
              </div>
            {:else if receipt}
              <button onclick={closeSheet}>Done adding doses</button>
            {/if}
          </div>

        {:else if sheet === "moment"}
          <div class="sheet-head">
            <h2 id="sheet-title">Moment</h2>
            <button class="ghost small" onclick={closeSheet}>Cancel</button>
          </div>
          <label for="m-text">What's happening?</label>
          <textarea id="m-text" rows="3" bind:value={mText}></textarea>
          <p class="label">Intensity (optional)</p>
          {@render scale(mIntensity, (v) => (mIntensity = v), "Intensity 0 to 10")}
          <label for="m-when">When</label>
          <div class="chips" role="group" aria-label="Quick times">
            {#each momentTimeChips as c}
              <button class="chip" onclick={() => (mWhen = c.value())}>{c.label}</button>
            {/each}
          </div>
          <DateTimeField id="m-when" bind:value={mWhen} variant="phone" />
          <div class="sheet-actions">
            <button class="primary" disabled={busy || !mText.trim()} onclick={submitMoment}>{busyKey === "moment" ? "Saving…" : "Add moment"}</button>
          </div>

        {:else if sheet === "jot"}
          <div class="sheet-head">
            <h2 id="sheet-title">Journal note</h2>
            <button class="ghost small" onclick={closeSheet}>Cancel</button>
          </div>
          <label for="j-title">Title (optional)</label>
          <input id="j-title" bind:value={jTitle} />
          <label for="j-body">Note</label>
          <textarea id="j-body" rows="6" bind:value={jBody}></textarea>
          <div class="sheet-actions">
            <button class="primary" disabled={busy || !jBody.trim()} onclick={submitJot}>{busyKey === "jot" ? "Saving…" : "Save note"}</button>
          </div>

        {:else if sheet === "start"}
          <div class="sheet-head">
            <h2 id="sheet-title">Start a live session</h2>
            <button class="ghost small" onclick={closeSheet}>Cancel</button>
          </div>
          <label for="s-title">Title (optional)</label>
          <input id="s-title" bind:value={sTitle} />
          <p class="hint">Leave it blank and it takes the name of the first substance you log.</p>
          <div class="sheet-actions">
            <button class="primary" disabled={busy} onclick={submitStart}>{busyKey === "start" ? "Starting…" : "Start"}</button>
          </div>

        {:else if sheet === "past"}
          <div class="sheet-head">
            <h2 id="sheet-title">Log a past session</h2>
            <button class="ghost small" onclick={closeSheet}>Cancel</button>
          </div>
          <p class="muted">First, roughly when it started. Then you'll add what you took and what happened, in order.</p>
          <label for="p-when">Started</label>
          <div class="chips" role="group" aria-label="Quick times">
            {#each pastPresets as p}
              <button class="chip" onclick={() => (pWhen = toLocalInput(p.at()))}>{p.label}</button>
            {/each}
          </div>
          <DateTimeField id="p-when" bind:value={pWhen} variant="phone" />
          <label for="p-title">Title (optional)</label>
          <input id="p-title" bind:value={pTitle} />
          <p class="hint">Leave it blank and it takes the name of the first substance.</p>
          <div class="sheet-actions">
            <button class="primary" disabled={busy || !pWhen} onclick={submitPast}>{busyKey === "past" ? "Creating…" : "Next: what you took"}</button>
          </div>

        {:else if sheet === "editDose" && editDose}
          <div class="sheet-head">
            <h2 id="sheet-title">Edit dose</h2>
            <button class="danger-text" onclick={removeDose}>Delete</button>
          </div>
          <label for="e-sub">Substance</label>
          <input id="e-sub" bind:value={eSub} autocapitalize="none" />
          <div class="grid3">
            <div>
              <label for="e-amt">Amount</label>
              <input id="e-amt" inputmode="decimal" bind:value={eAmt} />
            </div>
            <div>
              <label for="e-unit">Unit</label>
              <select id="e-unit" bind:value={eUnit}>{#each UNITS as u}<option>{u}</option>{/each}</select>
            </div>
            <div>
              <label for="e-route">Route</label>
              <select id="e-route" bind:value={eRoute}>{#each ROUTES as r}<option>{r}</option>{/each}</select>
            </div>
          </div>
          <label for="e-when">When</label>
          <DateTimeField id="e-when" bind:value={eWhen} variant="phone" />
          <label for="e-note">Note (optional)</label>
          <input id="e-note" bind:value={eNote} />
          <div class="sheet-actions pair">
            <button onclick={closeSheet}>Cancel</button>
            <button class="primary" disabled={busy || !eSub.trim()} onclick={saveDose}>{busyKey === "editDose" ? "Saving…" : "Save"}</button>
          </div>

        {:else if sheet === "editMoment" && editMoment}
          <div class="sheet-head">
            <h2 id="sheet-title">Edit moment</h2>
            <button class="danger-text" onclick={removeMoment}>Delete</button>
          </div>
          <label for="ev-text">What happened</label>
          <textarea id="ev-text" rows="3" bind:value={evText}></textarea>
          <p class="label">Intensity (optional)</p>
          {@render scale(evIntensity, (v) => (evIntensity = v), "Intensity 0 to 10")}
          <label for="ev-when">When</label>
          <DateTimeField id="ev-when" bind:value={evWhen} variant="phone" />
          <div class="sheet-actions pair">
            <button onclick={closeSheet}>Cancel</button>
            <button class="primary" disabled={busy || !evText.trim()} onclick={saveMoment}>{busyKey === "editMoment" ? "Saving…" : "Save"}</button>
          </div>

        {:else if sheet === "details" && open}
          <div class="sheet-head">
            <h2 id="sheet-title">Edit details</h2>
            <button class="ghost small" onclick={closeSheet}>Cancel</button>
          </div>
          <label for="en-title">Title</label>
          <input id="en-title" bind:value={enTitle} />
          <label for="en-start">{open.kind === "note" ? "Date" : "Started"}</label>
          <DateTimeField id="en-start" bind:value={enStart} variant="phone" />
          {#if open.kind === "session"}
            <label for="en-end">Ended</label>
            <DateTimeField id="en-end" bind:value={enEnd} variant="phone" />
            <p class="hint">Leave it blank and the session stays live.</p>
            <p class="label">Rating (optional)</p>
            {@render scale(enRating, (v) => (enRating = v), "Rating 0 to 10")}
            <label for="en-int">Intention</label>
            <textarea id="en-int" rows="2" bind:value={enIntention}></textarea>
            <label for="en-set">Setting</label>
            <textarea id="en-set" rows="2" bind:value={enSetting}></textarea>
          {/if}
          <div class="sheet-actions">
            <button class="primary" disabled={busy} onclick={saveDetails}>{busyKey === "details" ? "Saving…" : "Save"}</button>
          </div>

        {:else if sheet === "writeup" && open}
          <div class="sheet-head">
            <h2 id="sheet-title">{open.kind === "note" ? "Note" : "Write-up"}</h2>
            <button class="ghost small" onclick={closeSheet}>Cancel</button>
          </div>
          <label class="sr" for="wu">{open.kind === "note" ? "Note" : "How was it?"}</label>
          <textarea id="wu" rows="10" placeholder={open.kind === "note" ? "" : "How was it? What would you do differently?"} bind:value={wuText}></textarea>
          <div class="sheet-actions">
            <button class="primary" disabled={busy} onclick={saveWriteup}>{busyKey === "writeup" ? "Saving…" : "Save"}</button>
          </div>

        {:else if sheet === "end" && open}
          <div class="sheet-head">
            <h2 id="sheet-title">{open.ended_at ? "Finish this session" : "End the session"}</h2>
            <button class="ghost small" onclick={closeSheet}>Cancel</button>
          </div>
          <label for="end-at">Ended</label>
          {#if open.ended_at}
            <div class="chips" role="group" aria-label="Quick times">
              <button class="chip" onclick={() => (endAt = isoToLocalInput(lastAt(open!)))}>At the last line</button>
              <button class="chip" onclick={() => (endAt = shift(isoToLocalInput(lastAt(open!)), 60))}>An hour after</button>
            </div>
          {/if}
          <DateTimeField id="end-at" bind:value={endAt} variant="phone" />
          <p class="label">Rating (optional)</p>
          {@render scale(endRating, (v) => (endRating = v), "Rating 0 to 10")}
          <label for="end-notes">Write-up (optional — you can do it later)</label>
          <textarea id="end-notes" rows="5" bind:value={endNotes}></textarea>
          <div class="sheet-actions">
            <button class="primary" disabled={busy} onclick={saveEnd}>{busyKey === "end" ? "Saving…" : open.ended_at ? "Finish" : "End session"}</button>
          </div>

        {:else if sheet === "more" && open}
          <div class="sheet-head">
            <h2 id="sheet-title">{open.title || "This entry"}</h2>
            <button class="ghost small" onclick={closeSheet}>Close</button>
          </div>
          <ul class="menu">
            <li><button onclick={startDetails}><strong>Edit details</strong><span class="muted">Title, times{open.kind === "session" ? ", rating, intention, setting" : ""}</span></button></li>
            <li><button onclick={() => startWriteup(open)}><strong>{open.kind === "note" ? "Edit the note" : "Edit the write-up"}</strong></button></li>
            <li><button onclick={exportOpen}><strong>Export as Markdown</strong><span class="muted">Downloads to this phone</span></button></li>
          </ul>
          <!-- Apart from everything else, and two deliberate taps. -->
          <div class="danger-zone">
            {#if !confirmDelete}
              <button class="danger-text wide" onclick={() => (confirmDelete = true)}>Delete this entry…</button>
            {:else}
              <p>
                Delete <strong>{open.title || "this entry"}</strong>{open.doses.length ? ` and its ${open.doses.length} dose${open.doses.length === 1 ? "" : "s"}` : ""}?
                You'll have a few seconds to undo.
              </p>
              <div class="pair">
                <button onclick={() => (confirmDelete = false)}>Keep it</button>
                <button class="danger" onclick={removeEntry}>Delete</button>
              </div>
            {/if}
          </div>

        {:else if sheet === "paste"}
          <div class="sheet-head">
            <h2 id="sheet-title">Paste a trip log</h2>
            <button class="ghost small" onclick={closeSheet}>Close</button>
          </div>
          <TripImport oncancel={closeSheet} onsaved={pasted} />

        {:else if sheet === "help"}
          <div class="sheet-head">
            <h2 id="sheet-title">Help</h2>
            <button class="ghost small" onclick={closeSheet}>Close</button>
          </div>
          <p><strong>If someone is in danger, call your local emergency number now.</strong></p>
          {#if resources.length}
            {@render resourceList(resources)}
          {:else}
            <p class="muted">Couldn't load the full list from your server. Emergency services and poison control can always help.</p>
          {/if}
        {/if}
      </div>
    {/if}
  {/if}
</main>

<style>
  /* Tokens. Dark by default (night use), a light scheme for writing up in daylight.
     Field borders meet 3:1 against their surface (WCAG 1.4.11). */
  :global(:root) {
    color-scheme: dark light;
    --bg: #0f1115;
    --surface: #181b21;
    --surface-2: #22262e;
    --field: #0f1115;
    --field-border: #6b7380;
    --divider: #2a2f38;
    --text: #e8eaed;
    --text-2: #a9b1bc;
    --accent: #6ea8fe;
    --on-accent: #0b0e14;
    --danger: #ff6b6b;
    --danger-bg: #ff6b6b24;
    --caution: #ffb454;
    --caution-bg: #ffb4541f;
    --note-bg: #6ea8fe1a;
    --ok: #7ee787;
    --focus: #ffd866;
    --fs-title: 1.45rem;
    --fs-h: 1.15rem;
    --fs-body: 1.0625rem;
    --fs-sm: 0.9375rem;
    --fs-xs: 0.8125rem;
    --tap: 48px;
    --tap-min: 44px;
    --radius: 12px;
    --sa-t: env(safe-area-inset-top);
    --sa-b: env(safe-area-inset-bottom);
    --nav-h: calc(60px + var(--sa-b));
  }
  @media (prefers-color-scheme: light) {
    :global(:root) {
      --bg: #f4f5f7;
      --surface: #ffffff;
      --surface-2: #eceef2;
      --field: #ffffff;
      --field-border: #7a828e;
      --divider: #d6dae0;
      --text: #15181d;
      --text-2: #4f5763;
      --accent: #1f5fd1;
      --on-accent: #ffffff;
      --danger: #b3261e;
      --danger-bg: #b3261e14;
      --caution: #8a5300;
      --caution-bg: #8a53001a;
      --note-bg: #1f5fd112;
      --ok: #1a7f37;
    }
  }

  :global(body) {
    margin: 0;
    background: var(--bg);
    color: var(--text);
    font: -apple-system-body;
    font-family: -apple-system, system-ui, sans-serif;
    font-size: var(--fs-body);
    line-height: 1.45;
    -webkit-text-size-adjust: 100%;
  }
  main {
    max-width: 36rem;
    margin: 0 auto;
    padding: calc(0.5rem + var(--sa-t)) 0.8rem calc(var(--nav-h) + 5.5rem);
  }
  :global(:focus-visible) { outline: 3px solid var(--focus); outline-offset: 2px; }

  .muted { color: var(--text-2); }
  .small { font-size: var(--fs-sm); }
  .hint { color: var(--text-2); font-size: var(--fs-sm); margin: -0.2rem 0 0.7rem; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }

  h1 { font-size: var(--fs-title); margin: 0 0 0.4rem; line-height: 1.2; }
  h2 { font-size: var(--fs-h); margin: 0 0 0.6rem; }

  /* ---------- header ---------- */
  header.top {
    display: flex; justify-content: space-between; align-items: center; gap: 0.5rem;
    min-height: var(--tap); margin-bottom: 0.4rem;
  }
  .brand { font-weight: 700; }
  .top-right { display: flex; gap: 0.4rem; align-items: center; min-width: 0; }
  .back {
    width: auto; min-height: var(--tap); margin: 0; padding: 0 0.6rem 0 0;
    background: none; border: none; color: var(--accent); font-size: var(--fs-body);
  }
  .pill {
    width: auto; margin: 0; min-height: var(--tap-min); padding: 0 0.8rem; border-radius: 999px;
    font-size: var(--fs-sm); max-width: 11rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .pill.live { border-color: var(--ok); color: var(--ok); background: transparent; }
  .live-dot { display: inline-block; width: 0.55rem; height: 0.55rem; border-radius: 50%; background: var(--ok); margin-right: 0.4rem; vertical-align: 0.05em; }
  .help {
    width: auto; margin: 0; min-height: var(--tap-min); padding: 0 0.9rem;
    border: 2px solid var(--danger); color: var(--danger); background: transparent; border-radius: 999px;
  }
  .help.wide { width: 100%; margin: 0.4rem 0; }

  /* ---------- surfaces ---------- */
  .pane { background: var(--surface); border: 1px solid var(--divider); border-radius: 16px; padding: 1rem; margin-bottom: 0.8rem; }
  .eyebrow { margin: 0 0 0.2rem; color: var(--ok); font-size: var(--fs-sm); font-weight: 600; }
  .entry-title { font-size: var(--fs-title); }
  .facts { color: var(--text-2); margin: 0 0 0.8rem; }
  .sec { font-size: var(--fs-sm); text-transform: uppercase; letter-spacing: 0.06em; color: var(--text-2); margin: 1.2rem 0 0.3rem; }
  .sec-head { display: flex; justify-content: space-between; align-items: baseline; }
  .prose { white-space: pre-wrap; font-size: var(--fs-body); line-height: 1.6; margin: 0 0 0.6rem; max-width: 60ch; }

  /* ---------- fields ---------- */
  label, .label { display: block; font-size: var(--fs-sm); color: var(--text-2); margin: 0.2rem 0 0.25rem; }
  input, select, textarea {
    width: 100%; box-sizing: border-box; font: inherit; font-size: var(--fs-body);
    background: var(--field); color: var(--text); border: 1px solid var(--field-border);
    border-radius: 10px; padding: 0.7rem 0.75rem; margin-bottom: 0.6rem; min-height: var(--tap);
  }
  textarea { line-height: 1.5; }
  .grid3 { display: grid; grid-template-columns: 1.3fr 1fr 1.4fr; gap: 0.5rem; }

  /* ---------- buttons ---------- */
  button {
    font: inherit; font-size: var(--fs-body); font-weight: 600; border-radius: var(--radius);
    border: 1px solid var(--field-border); background: var(--surface-2); color: var(--text);
    padding: 0.7rem 1rem; width: 100%; min-height: var(--tap); margin: 0 0 0.5rem; cursor: pointer;
  }
  button.primary { background: var(--accent); color: var(--on-accent); border-color: var(--accent); }
  button.big { min-height: 56px; font-size: var(--fs-h); }
  button.ghost { background: transparent; border-color: transparent; color: var(--accent); }
  button.small { width: auto; min-height: var(--tap-min); padding: 0 0.7rem; margin: 0; font-size: var(--fs-sm); }
  button.danger { background: var(--danger); border-color: var(--danger); color: #fff; }
  button.danger-text { width: auto; margin: 0; background: transparent; border-color: transparent; color: var(--danger); min-height: var(--tap-min); }
  button.danger-text.wide { width: 100%; }
  button:disabled { background: var(--surface-2); color: var(--text-2); border-color: var(--divider); cursor: default; }
  .pair { display: flex; gap: 0.5rem; }
  .err { color: var(--danger); margin: -0.2rem 0 0.6rem; }
  .steps { padding-left: 1.2rem; margin: 0 0 0.8rem; }
  .steps li { margin-bottom: 0.4rem; }
  .pair > button { flex: 1; }

  /* ---------- chips ---------- */
  .chips { display: flex; flex-wrap: wrap; gap: 0.45rem; margin-bottom: 0.6rem; }
  .chips.scroll { flex-wrap: nowrap; overflow-x: auto; padding-bottom: 0.2rem; scrollbar-width: none; }
  .chip {
    width: auto; flex: none; margin: 0; min-height: var(--tap-min); padding: 0 0.9rem;
    font-size: var(--fs-sm); font-weight: 500; border-radius: 999px; background: var(--surface-2); white-space: nowrap;
  }
  .chip.on { background: var(--accent); color: var(--on-accent); border-color: var(--accent); }
  .scale { display: grid; grid-template-columns: repeat(6, 1fr); gap: 0.4rem; margin-bottom: 0.7rem; }
  .chip.num { width: 100%; min-height: var(--tap); padding: 0; font-variant-numeric: tabular-nums; }

  /* ---------- timeline ---------- */
  .tl { list-style: none; padding: 0; margin: 0 0 0.4rem; }
  .tl li + li { border-top: 1px solid var(--divider); }
  .row {
    display: grid; grid-template-columns: 4.6rem 1fr auto; gap: 0.5rem; align-items: start;
    text-align: left; background: none; border: none; border-radius: 0; margin: 0; padding: 0.7rem 0;
    font-weight: 400; min-height: var(--tap);
  }
  .when { display: flex; flex-direction: column; font-variant-numeric: tabular-nums; color: var(--text-2); font-size: var(--fs-sm); }
  .rel { font-size: var(--fs-xs); }
  .what { overflow-wrap: anywhere; }
  .sub { display: block; color: var(--text-2); font-size: var(--fs-sm); }
  .dot { display: inline-block; width: 0.5rem; height: 0.5rem; border-radius: 50%; margin-right: 0.45rem; vertical-align: 0.1em; }
  .dot.dose { background: var(--accent); }
  .dot.moment { background: transparent; border: 2px solid var(--text-2); box-sizing: border-box; }
  .chev { color: var(--text-2); align-self: center; font-weight: 400; }

  /* ---------- entry lists ---------- */
  .entries, .plain { list-style: none; padding: 0; margin: 0; }
  .entries li + li, .plain li + li { border-top: 1px solid var(--divider); }
  .plain li { padding: 0.55rem 0; }
  .entry {
    display: flex; gap: 0.8rem; align-items: center; text-align: left;
    background: none; border: none; border-radius: 0; margin: 0; padding: 0.7rem 0; font-weight: 400;
  }
  .date { display: flex; flex-direction: column; align-items: center; width: 2.6rem; flex: none; }
  .dow { font-size: var(--fs-xs); color: var(--text-2); text-transform: uppercase; }
  .dnum { font-size: 1.3rem; font-weight: 700; line-height: 1.1; }
  .body { display: flex; flex-direction: column; flex: 1; min-width: 0; }
  .title { font-weight: 600; }
  .meta { color: var(--text-2); font-size: var(--fs-sm); }
  .excerpt { color: var(--text-2); font-size: var(--fs-sm); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .excerpt.pending { color: var(--caution); }
  .tag { font-size: var(--fs-xs); font-weight: 500; border: 1px solid var(--divider); border-radius: 6px; padding: 0 0.35rem; margin-left: 0.35rem; color: var(--text-2); }
  .tag.live { border-color: var(--ok); color: var(--ok); }
  .month-label { font-size: var(--fs-sm); text-transform: uppercase; letter-spacing: 0.06em; color: var(--text-2); margin-bottom: 0.2rem; }

  /* ---------- check ---------- */
  .seg { display: flex; gap: 0.3rem; background: var(--surface-2); border-radius: var(--radius); padding: 0.25rem; margin-bottom: 0.8rem; }
  .seg button { margin: 0; border: none; background: transparent; }
  .seg button.on { background: var(--surface); color: var(--text); box-shadow: 0 0 0 1px var(--divider); }
  .roa { margin-bottom: 0.7rem; }
  .doses { display: grid; grid-template-columns: auto 1fr; gap: 0.2rem 0.8rem; margin: 0.3rem 0 0; }
  .doses dt { color: var(--text-2); }
  .doses dd { margin: 0; font-variant-numeric: tabular-nums; }
  .hit { border-top: 1px solid var(--divider); padding-top: 0.6rem; margin-top: 0.6rem; font-size: var(--fs-sm); }
  .hit p { margin: 0.3rem 0 0; }
  .flag { font-size: var(--fs-xs); border: 1px solid var(--caution); color: var(--caution); border-radius: 6px; padding: 0 0.35rem; margin-left: 0.35rem; }

  /* ---------- banners ---------- */
  .banner { border-radius: 10px; padding: 0.7rem 0.8rem; margin: 0.5rem 0; border: 1px solid; font-size: var(--fs-sm); }
  .banner.danger { border-color: var(--danger); background: var(--danger-bg); }
  .banner.caution { border-color: var(--caution); background: var(--caution-bg); }
  .banner.note { border-color: var(--accent); background: var(--note-bg); }
  .crisis { font-size: var(--fs-body); margin-bottom: 0.8rem; }
  .crisis .pair { margin-top: 0.6rem; }
  .resources { list-style: none; padding: 0; margin: 0.5rem 0; }
  .resources li { display: flex; flex-direction: column; padding: 0.5rem 0; border-top: 1px solid var(--divider); }
  .call { display: inline-flex; align-items: center; min-height: var(--tap-min); color: var(--accent); font-weight: 700; font-size: var(--fs-h); }
  .receipt { border: 1px solid var(--ok); border-radius: 10px; padding: 0.6rem 0.8rem; margin-bottom: 0.8rem; }
  .receipt p { margin: 0 0 0.3rem; }

  /* ---------- chat ---------- */
  .chat { max-height: 50vh; overflow-y: auto; margin-bottom: 0.6rem; }
  .msg { border-radius: 12px; padding: 0.6rem 0.75rem; margin: 0 0 0.5rem; white-space: pre-wrap; }
  .msg.user { background: var(--surface-2); }
  .msg.assistant { background: var(--note-bg); }

  /* ---------- the sticky action bar on an entry ---------- */
  .actionbar {
    position: fixed; left: 0; right: 0; bottom: var(--nav-h); z-index: 5;
    display: flex; gap: 0.4rem; padding: 0.5rem 0.8rem;
    background: color-mix(in srgb, var(--bg) 92%, transparent); backdrop-filter: blur(8px);
    border-top: 1px solid var(--divider);
  }
  .actionbar button { flex: 1; margin: 0; padding: 0 0.4rem; white-space: nowrap; }
  .actionbar button.more { flex: 0 0 3.2rem; font-size: 1.4rem; line-height: 1; }

  /* ---------- toasts, right above the thumb ---------- */
  .toasts { position: fixed; left: 0.8rem; right: 0.8rem; bottom: calc(var(--nav-h) + 4.2rem); z-index: 20; display: flex; flex-direction: column; gap: 0.4rem; pointer-events: none; }
  .toast {
    pointer-events: auto; display: flex; align-items: center; justify-content: space-between; gap: 0.6rem;
    background: var(--surface-2); border: 1px solid var(--field-border); border-radius: var(--radius);
    padding: 0.5rem 0.5rem 0.5rem 0.9rem; box-shadow: 0 6px 24px #0006;
  }
  .toast.bad { border-color: var(--danger); background: color-mix(in srgb, var(--danger) 18%, var(--surface-2)); }

  /* ---------- nav ---------- */
  nav {
    position: fixed; left: 0; right: 0; bottom: 0; z-index: 10;
    display: flex; gap: 0.3rem; padding: 0.4rem 0.5rem calc(0.4rem + var(--sa-b));
    background: color-mix(in srgb, var(--bg) 94%, transparent); backdrop-filter: blur(8px);
    border-top: 1px solid var(--divider);
  }
  nav button { flex: 1; margin: 0; padding: 0; min-height: 52px; font-size: var(--fs-sm); background: transparent; border-color: transparent; color: var(--text-2); }
  nav button.on { color: var(--accent); background: var(--surface-2); border-color: var(--divider); }
  nav button.plus { flex: 0 0 3.6rem; font-size: 1.7rem; font-weight: 500; background: var(--accent); color: var(--on-accent); border-radius: 999px; }

  /* ---------- sheets ---------- */
  .backdrop {
    position: fixed; inset: 0; z-index: 30; width: auto; min-height: 0; margin: 0; padding: 0;
    border: none; border-radius: 0; background: #0009;
  }
  .sheet {
    position: fixed; left: 0; right: 0; bottom: var(--kb, 0px); z-index: 31;
    max-width: 36rem; margin: 0 auto; max-height: min(88vh, calc(100vh - var(--kb, 0px) - var(--sa-t) - 1rem));
    overflow-y: auto; overscroll-behavior: contain;
    background: var(--surface); border-radius: 18px 18px 0 0; border: 1px solid var(--divider); border-bottom: none;
    padding: 0.9rem 1rem calc(1rem + var(--sa-b));
  }
  .sheet-head { display: flex; justify-content: space-between; align-items: center; gap: 0.5rem; margin-bottom: 0.4rem; }
  .sheet-head h2 { margin: 0; }
  .sheet-actions { position: sticky; bottom: calc(-1rem - var(--sa-b)); background: var(--surface); padding-top: 0.5rem; margin-top: 0.4rem; }
  .menu { list-style: none; padding: 0; margin: 0 0 0.4rem; }
  .menu button { display: flex; flex-direction: column; align-items: flex-start; text-align: left; gap: 0.1rem; }
  .menu .muted { font-weight: 400; font-size: var(--fs-sm); }
  .danger-zone { border-top: 1px solid var(--divider); margin-top: 1.2rem; padding-top: 0.8rem; }

  @media (prefers-reduced-motion: no-preference) {
    .sheet { animation: rise 0.18s ease-out; }
    @keyframes rise { from { transform: translateY(24px); opacity: 0.6; } }
  }
</style>
