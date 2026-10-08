<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  The phone portal's UI (see src-tauri/src/portal.rs).

  A *mirror* of the desktop, not a copy of its layout: the same journal, the same
  deterministic safety checks, the same reference data — re-laid out for one hand,
  in the dark, possibly altered. Designed from a three-lens UX review (task flows,
  impaired-state accessibility, reading/editing — see ROADMAP.md, v0.13.0):

  - **One word per thing.** An *entry* is anything in the journal. An *experience* is
    an entry with doses (a *live trip report* while it's happening); a *journal note* is a plain written entry; a *moment* is
    a line in an experience's timeline; the *write-up* is the story afterwards.
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
  import { clockTime } from "$lib/clock";
  import { clockPref, setClock24 } from "$lib/clock.svelte";
  import { onMount } from "svelte";
  import {
    tagPeople,
    tagAllow,
    tagSetOwnerName,
    tagSend,
    tagInbox,
    tagAnswer,
    tagSent,
    type TagPerson,
    type IncomingTag,
    type SentTag,
    serverUpdateStatus,
    serverUpdateInstall,
    type ServerUpdateStatus,
    dbStatus,
    personUnlock,
    personSetPin,
    personPinUnlock,
    personRemember,
    myDevices,
    pairOwnDevice,
    unpairMyDevice,
    ownerDeviceAuth,
    personChangePassword,
    exportMyJournal,
    type DbStatus,
    type MyDevice,
    listExperiences,
    getExperience,
    createExperience,
    endExperience,
    updateExperience,
    setWriteupSkipped,
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
    discreetAvailable,
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
    type DoseDetail,
    type TimelineEvent,
    type Warning,
    type CrisisResult,
    type CrisisResource,
    type ChatMsg,
    type PwInfo,
    type KnowledgeHit,
    type AiStatus,
  } from "$lib/api";
  import {
    acceptPairing,
    captureToken,
    forgetToken,
    hasToken,
    inTauri,
    isIos,
    isStandalone,
    LOCKED_EVENT,
    LockedError,
    pairingLink,
  } from "$lib/portal";
  import {
    discardFailed,
    onOfflineStatus,
    retryFailed,
    clearKept,
    flush as sendWaiting,
    startOffline,
    uncheckedOffline,
    type OfflineStatus,
  } from "$lib/offline";
  import { dev } from "$app/environment";
  import TripImport from "$lib/TripImport.svelte";
  import RiskNotes from "$lib/RiskNotes.svelte";
  import NameHint from "$lib/NameHint.svelte";
  import SubstanceInput, { forgetSubstanceNames } from "$lib/SubstanceInput.svelte";
  import DoseDetailFields from "$lib/DoseDetailFields.svelte";
  import KindQuestion from "$lib/KindQuestion.svelte";
  import SleepCheckin from "$lib/SleepCheckin.svelte";
  import { describeAmount, detailOf } from "$lib/dosedetail";
  import { ALL_PARTS, experiencePdf, pdfFilename, type PdfParts } from "$lib/pdf";
  import DateTimeField from "$lib/DateTimeField.svelte";
  import { lastDose as latestDose, span as gapText } from "$lib/livefacts";
  import ActiveArcs from "$lib/ActiveArcs.svelte";
  import { discreet, hiding, setDiscreet, shown as nameShown } from "$lib/discreet.svelte";
  import DiscreetToggle from "$lib/DiscreetToggle.svelte";
  import Icon from "$lib/Icon.svelte";
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
    STANDARD_DRINK,
    HIT_NOTE,
    DRINK_PICKS,
    recentDoses,
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
    | "paste"
    | "pdf"
    | "me";

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
  /** The dose being logged, as it will be saved ("acid" → LSD; NameHint). */
  let dSubAs = $state("");
  /** Whether the computer is reachable, and what's waiting to be sent to it. */
  let net = $state<OfflineStatus>({ offline: false, pending: 0, failed: [], ready: false, kept: { open: 0, recent: 0 } });
  /** Names the last offline check couldn't check (not in the phone's reference). */
  let comboUnchecked = $state<string[]>([]);
  let comboOffline = $state(false);

  let recent = $state<ExperienceSummary[]>([]);
  /** Doses from any entry that may still be active (quicklog.ts `recentDoses`). */
  let activeDoses = $state<Dose[]>([]);
  // False until the journal has loaded once, so the empty-journal card doesn't
  // flash for someone whose entries just haven't arrived yet.
  let loaded = $state(false);
  let session = $state<ExperienceDetail | null>(null);
  /** The entry on screen in the Journal (possibly the live session). */
  let open = $state<ExperienceDetail | null>(null);
  /** Opened from "Log a past session": new rows default to just after the last one. */
  let building = $state(false);

  /** Interaction warnings from the last dose logged into each entry. They stay until
   *  dismissed — a warning that scrolls away or times out is a warning missed. */
  let warnFor = $state<Record<number, Warning[]>>({});

  // ---------- the dose form (one form, three places: new entry, live session, past entry) ----------
  /** Where the dose goes. null = a new entry of its own (a quick log). */
  let target = $state<ExperienceDetail | null>(null);
  let dSub = $state("");
  let dAmt = $state("");
  let dUnit = $state("mg");
  let dDetail = $state<DoseDetail>({});
  /** The substance just logged, for KindQuestion; `n` asks again after another dose. */
  let dAsk = $state({ s: "", n: 0 });
  let dRoute = $state("oral");
  let dWhen = $state("");
  /** What just saved, with its warnings — shown in the sheet, next to the button pressed. */
  let receipt = $state<{ id: number; title: string; line: string; warnings: Warning[]; doseId: number | null; fresh: boolean } | null>(null);
  const qRecents = $derived(recentSubstances(recent));

  // moment (timeline note)
  let mText = $state("");
  /** One-tap states for a moment, so nothing has to be typed mid-session. */
  const MOODS = ["Coming up", "Peaking", "Calm", "Anxious", "Nauseous", "Need water", "Coming down"];
  let mMood = $state("");
  let mIntensity = $state("");
  let mWhen = $state("");

  // journal note (kind "note")
  let jTitle = $state("");
  let jBody = $state("");

  // start a live session
  let sTitle = $state("");
  let sIntention = $state("");

  // log a past session, step 1
  let pWhen = $state("");
  let pTitle = $state("");

  // editing a dose / moment already logged
  let editDose = $state<Dose | null>(null);
  let eSub = $state("");
  let eAmt = $state("");
  let eUnit = $state("mg");
  let eDetail = $state<DoseDetail>({});
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
  let endSkip = $state(false);
  let confirmDelete = $state(false);

  // deferred deletes, with Undo
  let hidden = $state<string[]>([]);
  let pendingDelete = $state<{ label: string; key: string; commit: () => Promise<void>; timer: number } | null>(null);
  const UNDO_MS = 8000;

  // journal browsing
  let search = $state("");
  let filter = $state<"all" | "sessions" | "notes" | "writeup">("all");
  let subFilter = $state<string | null>(null);
  /** "All" means everything, so it clears the substance pick too. Journal notes
   *  have no doses, so a substance pick can't apply to them either. Sessions and
   *  No write-up keep it: "LSD sessions with no write-up" is a fair question. */
  function setFilter(k: typeof filter) {
    filter = k;
    if (k === "all" || k === "notes") subFilter = null;
  }
  /** Picking a substance from Journal notes would show nothing; show sessions. */
  function pickSubFilter(s: string) {
    subFilter = subFilter === s ? null : s;
    if (subFilter && filter === "notes") filter = "all";
  }
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
  /** Height of what's visible above the keyboard, for sheets that fill it. */
  let vvh = $state(0);

  function pairFromText() {
    if (!acceptPairing(pairText)) {
      pairErr = "That doesn't look like a pairing link. It should end in #t= followed by a long code.";
      return;
    }
    pairText = "";
    pairErr = null;
    paired = true;
    startOffline();
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

  /** The live card's "now T+…" reads this, so it moves on its own. Every 30s,
   *  and at once when the phone wakes or the app comes back to the front; a
   *  sitter glancing at a stale elapsed time is worse than no number at all. */
  let nowTick = $state(Date.now());

  // ---------- discreet mode ($lib/discreet) ----------
  /** A title can name a drug ("Quiet LSD day"), so in discreet mode titles become
   *  what the entry is. The opened entry still shows its own words. */
  const titleOf = (e: { title: string; kind: string }, fallback = "Untitled") =>
    hiding() ? (e.kind === "note" ? "Journal note" : "Experience") : e.title || fallback;
  const subsOf = (names: string[]) => names.map(nameShown).join(", ");

  // ---------- dim (red) night theme ----------
  const NIGHT_KEY = "fieldnotes.night";
  function loadNight(): boolean {
    try { return localStorage.getItem(NIGHT_KEY) === "1"; } catch { return false; }
  }
  let night = $state(loadNight());
  let wake: { release: () => Promise<void> } | null = null;
  function setNight(on: boolean) {
    night = on;
    try { localStorage.setItem(NIGHT_KEY, on ? "1" : "0"); } catch {}
  }
  const toggleNight = () => setNight(!night);

  /** The nav's last slot: Talk or Stats, the owner's pick, on this phone only. */
  const NAV_SLOT_KEY = "fieldnotes.navSlot";
  let navSlot = $state<"talk" | "stats">((() => {
    try { return localStorage.getItem(NAV_SLOT_KEY) === "stats" ? "stats" : "talk"; } catch { return "talk"; }
  })());
  function setNavSlot(v: "talk" | "stats") {
    navSlot = v;
    try { localStorage.setItem(NAV_SLOT_KEY, v); } catch {}
  }
  function goStats() {
    journalMode = "stats";
    goTo("journal");
  }
  /** While dim, also keep the screen from locking in someone's hand. Browsers can
   *  refuse; that's fine, it's a convenience. Re-requested when the page returns. */
  async function holdWake() {
    if (!night || document.visibilityState !== "visible" || wake) return;
    try {
      wake = await (navigator as unknown as { wakeLock?: { request: (t: "screen") => Promise<{ release: () => Promise<void>; addEventListener: (e: string, f: () => void) => void }> } })
        .wakeLock?.request("screen") ?? null;
      (wake as unknown as { addEventListener?: (e: string, f: () => void) => void })?.addEventListener?.("release", () => (wake = null));
    } catch {
      wake = null;
    }
  }
  $effect(() => {
    const root = document.documentElement;
    if (night) root.dataset.theme = "night";
    else delete root.dataset.theme;
    document.querySelector('meta[name="theme-color"]')?.setAttribute("content", night ? "#000000" : "#14120f");
    if (night) holdWake();
    else if (wake) { wake.release().catch(() => {}); wake = null; }
  });

  onMount(() => {
    const tick = () => (nowTick = Date.now());
    const clock = setInterval(tick, 30_000);
    const onVisible = () => {
      if (document.visibilityState === "visible") { tick(); holdWake(); }
    };
    document.addEventListener("visibilitychange", onVisible);
    captureToken();
    paired = inTauri() || hasToken();
    try {
      showHomeHint = paired && !inTauri() && isIos() && !standalone && !localStorage.getItem(HOME_HINT_KEY);
    } catch {
      showHomeHint = false;
    }
    const vv = window.visualViewport;
    const onVv = () => {
      if (vv) {
        kb = Math.max(0, window.innerHeight - vv.height - vv.offsetTop);
        vvh = vv.height;
      }
    };
    vv?.addEventListener("resize", onVv);
    vv?.addEventListener("scroll", onVv);
    window.addEventListener(LOCKED_EVENT, onLocked);
    let stopNet = () => {};
    if (!inTauri()) {
      // Keep this page on the phone so it opens offline (src/service-worker.ts).
      if ("serviceWorker" in navigator) {
        navigator.serviceWorker.register("/service-worker.js", { type: dev ? "module" : "classic" }).catch(() => {});
      }
      stopNet = onOfflineStatus((s) => {
        const back = net.offline && !s.offline;
        net = s;
        // Back in touch: show the computer's copy, with everything sent.
        if (back && paired && !locked) refresh();
      });
      if (paired) startOffline();
    }
    if (paired) {
      // Help first and regardless: it needs no journal.
      loadResources();
      loadMe().then(() => {
        if (!locked) startJournal();
      });
    }
    return () => {
      stopNet();
      window.removeEventListener(LOCKED_EVENT, onLocked);
      clearInterval(clock);
      document.removeEventListener("visibilitychange", onVisible);
      vv?.removeEventListener("resize", onVv);
      vv?.removeEventListener("scroll", onVv);
    };
  });

  /** Everything that reads the journal, once it's open. */
  function startJournal() {
    refresh();
    loadAi();
    loadServerUpdate();
    loadTags();
  }

  // ---------- tagging someone else on this server (src-tauri/src/tagging.rs) ----------

  let tagWho = $state<TagPerson[]>([]);
  let tagOwnerName = $state("");
  let tagOwnerDraft = $state("");
  let tagIn = $state<IncomingTag[]>([]);
  let tagOut = $state<SentTag[]>([]);
  /** The tag the dose sheet was opened from, to accept once it's saved. */
  let fromTag = $state<IncomingTag | null>(null);
  /** Who each just-logged dose was sent to, by dose id. */
  let taggedTo = $state<Record<number, number[]>>({});
  const tagPartners = $derived(tagWho.filter((p) => p.you_allow && p.they_allow));

  /** Quietly: an older server, or no connection, just means no tagging here. */
  async function loadTags() {
    try {
      const r = await tagPeople();
      tagWho = r.people;
      tagOwnerName = r.owner_name;
      tagIn = await tagInbox();
      tagOut = await tagSent();
    } catch {
      /* offline or an older server */
    }
  }

  async function setTagAllow(p: TagPerson, allow: boolean) {
    try {
      tagWho = await tagAllow(p.id, allow);
      tagIn = await tagInbox();
    } catch (e) {
      err = e instanceof Error ? e.message : String(e);
    }
  }

  async function saveOwnerTagName() {
    try {
      tagOwnerName = await tagSetOwnerName(tagOwnerDraft);
      tagOwnerDraft = "";
    } catch (e) {
      err = e instanceof Error ? e.message : String(e);
    }
  }

  async function sendTag(doseId: number, p: TagPerson) {
    try {
      await tagSend(doseId, [p.id]);
      taggedTo = { ...taggedTo, [doseId]: [...(taggedTo[doseId] ?? []), p.id] };
      tagOut = await tagSent();
    } catch (e) {
      err = e instanceof Error ? e.message : String(e);
    }
  }

  /** Open the dose sheet filled in from a tag, to change before saving. */
  function reviewTag(t: IncomingTag) {
    startDose(null);
    const d = t.dose;
    dSub = d.substance_name;
    dAmt = d.amount == null ? "" : String(d.amount);
    dUnit = d.unit || dUnit;
    dRoute = d.route || dRoute;
    dWhen = isoToLocalInput(d.taken_at);
    dDetail = {
      form: d.form,
      per_unit: d.per_unit,
      per_unit_unit: d.per_unit_unit,
      unit_label: d.unit_label,
      estimate: d.estimate,
      estimate_unit: d.estimate_unit,
    };
    fromTag = t;
  }

  async function declineTag(t: IncomingTag) {
    try {
      await tagAnswer(t.id, false);
    } catch {
      /* already gone */
    }
    tagIn = tagIn.filter((x) => x.id !== t.id);
  }

  // ---------- another person's journal (src-tauri/src/people.rs) ----------
  // The owner's phone never sees any of this. Someone else on the same server has
  // their own encrypted journal, which they unlock here with a password only they
  // know; until then, only Help and the combination checker answer.

  /** Set only on another person's device; null for the owner (and older servers). */
  let me = $state<DbStatus | null>(null);
  const isOther = $derived(!!me?.person_name);
  const locked = $derived(!!me?.person_name && !me.unlocked);
  let pw1 = $state("");
  let pw2 = $state("");
  let lockErr = $state<string | null>(null);
  let unlocking = $state(false);

  async function loadMe() {
    try {
      const st = await dbStatus();
      me = st.person_name ? st : null;
    } catch {
      // The owner's journal locked at the desk, or the server is out of reach:
      // the usual paths below say so.
      me = null;
    }
  }

  // A PIN on this device (people.rs, rule 5). The server keeps a key; this device
  // keeps the sealed copy of the password it opens. Neither half works alone.
  const SEALED_KEY = "fieldnotes.pinSealed";
  const readSealed = () => {
    try {
      return localStorage.getItem(SEALED_KEY);
    } catch {
      return null;
    }
  };
  function writeSealed(v: string | null) {
    try {
      if (v) localStorage.setItem(SEALED_KEY, v);
      else localStorage.removeItem(SEALED_KEY);
    } catch {
      // No storage: the PIN can't be kept here, so the password it is.
    }
  }
  let pinCode = $state("");
  /** Unlocking with the password instead, though this device has a PIN. */
  let usePassword = $state(false);
  const canPin = $derived(!!me?.pin && !me.new_journal && !!readSealed());
  // The server forgot this device's PIN (too many wrong tries, a new password):
  // the sealed copy is no use now. Only on a server that says so.
  $effect(() => {
    if (me && me.pin === false) writeSealed(null);
  });

  async function unlockWithPin() {
    lockErr = null;
    const sealed = readSealed();
    if (!pinCode || !sealed) return;
    unlocking = true;
    try {
      me = await personPinUnlock(pinCode, sealed);
      pinCode = "";
      usePassword = false;
      startJournal();
    } catch (e) {
      lockErr = e instanceof Error ? e.message : String(e);
      pinCode = "";
      await loadMe();
    } finally {
      unlocking = false;
    }
  }

  /** The server answered "locked" (it restarted and forgot the password). */
  function onLocked() {
    err = null;
    if (me) me = { ...me, unlocked: false };
    else loadMe();
  }

  async function unlock() {
    lockErr = null;
    if (me?.new_journal) {
      if (pw1.length < 8) return void (lockErr = "Choose a password of at least 8 characters.");
      if (pw1 !== pw2) return void (lockErr = "Those two don't match. Type the same password twice.");
    } else if (!pw1) {
      return;
    }
    unlocking = true;
    try {
      const created = !!me?.new_journal;
      me = await personUnlock(pw1);
      if (created) {
        try {
          localStorage.setItem(CREATED_KEY, String(Date.now()));
        } catch {
          // No storage: no reminder, nothing else lost.
        }
      }
      pw1 = "";
      pw2 = "";
      startJournal();
    } catch (e) {
      lockErr = e instanceof Error ? e.message : String(e);
    } finally {
      unlocking = false;
    }
  }

  // A week after choosing a password, one reminder that a backup is the only way
  // back if it's forgotten. Remembered per device: the one they chose it on.
  const CREATED_KEY = "fieldnotes.journalCreatedAt";
  const NUDGED_KEY = "fieldnotes.backupNudged";
  const WEEK_MS = 7 * 24 * 60 * 60 * 1000;
  let backupNudge = $state(false);
  $effect(() => {
    if (!isOther || locked) return;
    try {
      const at = Number(localStorage.getItem(CREATED_KEY) ?? 0);
      backupNudge = at > 0 && Date.now() - at > WEEK_MS && !localStorage.getItem(NUDGED_KEY);
    } catch {
      backupNudge = false;
    }
  });
  function dismissBackupNudge() {
    backupNudge = false;
    try {
      localStorage.setItem(NUDGED_KEY, "1");
    } catch {
      // Shown again next time; harmless.
    }
  }

  // Their own Settings: devices, password, backup, keeping the journal unlocked,
  // and the limits.
  let mine = $state<MyDevice[]>([]);
  let newDevice = $state("");
  let pairedNew = $state<{ name: string; link: string; qr: string | null } | null>(null);
  let linkCopied = $state(false);
  let unpairAsk = $state<number | null>(null);
  let rememberPw = $state("");
  let rememberAsk = $state(false);

  /** On the owner's phone: what pairing or un-pairing asks for (owner_auth.rs). */
  let ownerAuth = $state<"password" | "pin" | "off" | null>(null);
  /** The journal's password or the phone PIN, typed for one pair or un-pair. */
  let ownerPw = $state("");
  let keptCleared = $state(false);
  const ownerPwLabel = $derived(ownerAuth === "pin" ? "Your phone PIN" : "Your journal password");

  function openMe() {
    sheet = "me";
    ownerPw = "";
    keptCleared = false;
    if (!isOther) {
      ownerDeviceAuth().then((m) => (ownerAuth = m)).catch(() => (ownerAuth = null));
      loadServerUpdate();
    }
    pairedNew = null;
    unpairAsk = null;
    rememberAsk = false;
    rememberPw = "";
    changingPw = false;
    pwChanged = false;
    pinAsk = false;
    pinNew = pinNew2 = pinPw = "";
    backedUp = false;
    run("me", async () => (mine = await myDevices()));
  }

  const pairAnother = () =>
    run("pairown", async () => {
      const name = newDevice.trim() || "My other device";
      if (!isOther && !ownerPw) throw new Error(`Type ${ownerPwLabel.toLowerCase()} first.`);
      const r = await pairOwnDevice(name, location.origin, isOther ? null : ownerPw);
      ownerPw = "";
      pairedNew = { name: r.device.name, link: `${location.origin}/m#t=${r.token}`, qr: r.qr };
      newDevice = "";
      linkCopied = false;
      mine = await myDevices();
    });

  async function copyPairLink() {
    if (!pairedNew) return;
    try {
      await navigator.clipboard.writeText(pairedNew.link);
      linkCopied = true;
    } catch {
      err = "Couldn't copy. Press and hold the link to copy it instead.";
    }
  }

  const unpair = (d: MyDevice) =>
    run("unpair", async () => {
      if (!isOther && !ownerPw) throw new Error(`Type ${ownerPwLabel.toLowerCase()} first.`);
      mine = await unpairMyDevice(d.id, isOther ? null : ownerPw);
      ownerPw = "";
      unpairAsk = null;
      if (d.this) {
        forgetToken();
        sheet = null;
        paired = false;
        me = null;
      }
    });

  let pwCurrent = $state("");
  let pwNew = $state("");
  let pwNew2 = $state("");
  let pwChanged = $state(false);
  let changingPw = $state(false);
  const changePassword = () =>
    run("changepw", async () => {
      pwChanged = false;
      if (pwNew.length < 8) throw new Error("Choose a new password of at least 8 characters.");
      if (pwNew !== pwNew2) throw new Error("The new password and the repeat don't match.");
      me = await personChangePassword(pwCurrent, pwNew);
      pwCurrent = pwNew = pwNew2 = "";
      changingPw = false;
      pwChanged = true;
    });

  let pinNew = $state("");
  let pinNew2 = $state("");
  let pinPw = $state("");
  let pinAsk = $state(false);
  const setPin = () =>
    run("pin", async () => {
      if (!/^\d{4,12}$/.test(pinNew)) throw new Error("Choose a PIN of 4 to 12 digits.");
      if (pinNew !== pinNew2) throw new Error("Those two PINs don't match.");
      const r = await personSetPin(pinNew, pinPw);
      writeSealed(r.sealed);
      me = r.status;
      pinNew = pinNew2 = pinPw = "";
      pinAsk = false;
    });
  const clearPin = () =>
    run("pin", async () => {
      const r = await personSetPin(null);
      writeSealed(null);
      me = r.status;
    });

  let backedUp = $state(false);
  /** Save their journal to this device as a file. It stays encrypted with their
   *  password, so it's no less private sitting in Downloads. */
  const downloadBackup = () =>
    run("backup", async () => {
      const { data } = await exportMyJournal();
      const bin = atob(data);
      const bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      const url = URL.createObjectURL(new Blob([bytes], { type: "application/octet-stream" }));
      const who = (me?.person_name ?? "journal").replace(/[^\p{L}\p{N}]+/gu, "-").replace(/^-|-$/g, "").toLowerCase() || "journal";
      const a = document.createElement("a");
      a.href = url;
      a.download = `field-notes-${who}-${new Date().toISOString().slice(0, 10)}.db`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      setTimeout(() => URL.revokeObjectURL(url), 10_000);
      backedUp = true;
      dismissBackupNudge();
    });

  const setRemember = (on: boolean) =>
    run("remember", async () => {
      me = await personRemember(on, on ? rememberPw : null);
      rememberPw = "";
      rememberAsk = false;
    });

  const seen = (secs: number | null) => {
    if (!secs) return "not used yet";
    const d = new Date(secs * 1000);
    return sameDay(d.toISOString(), new Date().toISOString()) ? `last used ${hhmm(d.toISOString())}` : `last used ${fmtDay(d.toISOString())}`;
  };

  // ---------- updating the server (server_update.rs) ----------

  let srvUpd = $state<ServerUpdateStatus | null>(null);
  let srvUpdStage = $state<"idle" | "confirm" | "installing" | "restarting" | "done">("idle");
  let srvUpdHidden = $state(false);
  let srvUpdTarget = $state("");

  /** The server checks in the background and answers with what it last knew;
   *  when it says it's still checking, ask again shortly. */
  async function loadServerUpdate(retries = 2) {
    // Installing restarts everyone's server, so only the owner's devices are offered it.
    if (inTauri() || isOther || srvUpdStage !== "idle") return;
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
      // Past a guard only when it's one the owner may wave through and they've read it.
      srvUpd = await serverUpdateInstall(!!srvUpd.blocked && srvUpd.can_override);
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
      // A locked journal swaps in the password screen; no need to say it twice.
      if (!(e instanceof LockedError)) err = e instanceof Error ? e.message : String(e);
    } finally {
      busyKey = null;
    }
  }

  async function refresh() {
    forgetSubstanceNames();
    // Older servers don't know this command: then discreet mode isn't offered.
    discreetAvailable().then((v) => (discreet.available = v)).catch(() => (discreet.available = false));
    try {
      recent = await listExperiences();
      loaded = true;
      // Only a *session* can be live — a plain note has no ended_at either, but
      // it isn't something you're "in".
      const live = recent.find((e) => e.kind === "session" && !e.ended_at);
      session = live ? await getExperience(live.id) : null;
      if (open) open = await getExperience(open.id).catch(() => null);
      if (target) target = await getExperience(target.id).catch(() => null);
      recentDoses(recent, Date.now(), session ? [session] : [])
        .then((d) => (activeDoses = d))
        .catch(() => {});
    } catch (e) {
      if (!(e instanceof LockedError)) err = e instanceof Error ? e.message : String(e);
    }
  }
  /** The live entry's doses (fresh right after logging) with everything else recent. */
  const arcDoses = $derived.by(() => {
    const own = session?.doses ?? [];
    return [...own, ...activeDoses.filter((d) => !own.some((o) => o.id === d.id))];
  });
  /** Fill the combination check with what's still active, ready for one more name. */
  function checkActive(names: string[]) {
    comboText = names.join(", ") + ", ";
    comboWarnings = null;
    checkMode = "combo";
    goTo("check");
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
    fromTag = null;
    receipt = null;
    confirmDelete = false;
  }

  function goTo(v: View) {
    view = v;
    if (v === "today") {
      loadServerUpdate();
      loadTags();
    }
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
  const hhmm = (iso: string) => clockTime(iso);
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
  /** "3 g fresh (about 0.3 g dried)": what was written, and what it works out to
   *  when that differs (dosedetail.ts). */
  const fmtAmt = (d: { amount: number | null; unit: string; substance_name?: string } & DoseDetail) =>
    describeAmount(d.substance_name ?? "", d.amount, d.unit, d);

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
      dUnit = defaultUnitFor(dSub, dRoute) ?? dUnit;
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
      const substance = dSubAs || dSub.trim();
      const amount = num(dAmt, "amount");
      // quickLog for every dose, not just new entries: it widens the interaction
      // check to anything else taken within 12 hours, and stretches an entry whose
      // times no longer cover what's in it.
      const detail = dDetail;
      const res = await quickLog({ substance, amount, unit: dUnit, route: dRoute, at, intoId: target?.id ?? null, detail });
      rememberDoseShape(substance, { unit: dUnit, route: dRoute });
      // Saved in this journal first; only then is the tag answered. If that can't
      // reach the server, the tag stays waiting rather than claiming otherwise.
      const tag = fromTag;
      if (tag) {
        fromTag = null;
        tagIn = tagIn.filter((x) => x.id !== tag.id);
        tagAnswer(tag.id, true).catch(() => {});
      }
      warnFor = { ...warnFor, [res.id]: res.warnings };
      receipt = {
        id: res.id,
        title: res.title,
        line: `${substance} ${fmtAmt({ amount, unit: dUnit, substance_name: substance, ...detail })} · ${hhmm(at)} ${fmtDay(at)}`,
        warnings: res.warnings,
        doseId: res.doseId,
        fresh: res.fresh,
      };
      dAsk = { s: substance, n: dAsk.n + 1 };
      dSub = dAmt = "";
      dDetail = {};
      await refresh();
      target = await getExperience(res.id);
      if (!target.ended_at) dWhen = nowLocalInput();
      else dWhen = isoToLocalInput(lastAt(target));
    });

  /** Undo the dose just logged, from its receipt: the same few-seconds Undo as a
   *  delete. A dose that started its own entry takes that entry with it. */
  function undoLastLog() {
    const rc = receipt;
    if (!rc || rc.doseId == null) return;
    receipt = null;
    closeSheet();
    if (rc.fresh) scheduleDelete(`${rc.line.split(" · ")[0]} removed`, `e:${rc.id}`, () => deleteExperience(rc.id));
    else scheduleDelete(`${rc.line.split(" · ")[0]} removed`, `d:${rc.doseId}`, () => deleteDose(rc.doseId!));
  }

  /** "Last LSD: 100 µg, 2h 05m before this one": the fact that matters before a
   *  redose, against the time being logged rather than the clock. */
  const sinceSame = $derived.by(() => {
    if (!target || !dSub.trim() || !dWhen) return null;
    const prev = latestDose(target.doses, dSub);
    if (!prev) return null;
    const at = new Date(dWhen).getTime();
    const was = Date.parse(prev.taken_at);
    if (!(at > was)) return null;
    return `Last ${prev.substance_name}: ${fmtAmt(prev)}, ${gapText(was, at)} before this one`;
  });

  // ---------- adding: moments ----------

  function startMoment(into: ExperienceDetail) {
    target = into;
    mText = mIntensity = mMood = "";
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
      if (!target || !(mText.trim() || mMood || mIntensity)) return;
      const at = localInputToIso(mWhen);
      // Saved as written. Nothing reads the journal over the user's shoulder: crisis
      // guardrails live where the user is *talking to* something (the Companion) and
      // in dangerous combinations — never in what they write here.
      await addTimelineEvent({
        experience_id: target.id,
        at,
        note: mText.trim(),
        mood: mMood,
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
      await createExperience({ title: sTitle.trim(), intention: sIntention.trim(), started_at: new Date().toISOString() });
      sTitle = "";
      sIntention = "";
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
    eDetail = detailOf(d);
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
        detail: detailOf(eDetail),
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
    endSkip = !!e.writeup_skipped;
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
      if (endSkip !== !!open.writeup_skipped) await setWriteupSkipped(open.id, endSkip);
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

  // ---------- share as PDF (pdf.ts) ----------

  let pdfParts = $state<PdfParts>({ ...ALL_PARTS });
  const pdfFile = (e: ExperienceDetail) =>
    new File([experiencePdf(e, pdfParts) as BlobPart], pdfFilename(e), { type: "application/pdf" });
  /** The phone's own share sheet (Messages, Mail, AirDrop…), where it has one. */
  const canShareFiles = $derived.by(() => {
    if (typeof navigator === "undefined" || !navigator.canShare || !open) return false;
    try {
      return navigator.canShare({ files: [new File([""], "x.pdf", { type: "application/pdf" })] });
    } catch {
      return false;
    }
  });

  function startPdf() {
    pdfParts = { ...ALL_PARTS };
    sheet = "pdf";
  }

  const sharePdf = () =>
    run("pdf", async () => {
      if (!open) return;
      try {
        await navigator.share({ files: [pdfFile(open)], title: open.title || "Experience" });
        closeSheet();
      } catch (e) {
        // Closing the share sheet isn't a failure.
        if (!(e instanceof DOMException && e.name === "AbortError")) throw e;
      }
    });

  const downloadPdf = () =>
    run("pdf", async () => {
      if (!open) return;
      const url = URL.createObjectURL(pdfFile(open));
      const a = document.createElement("a");
      a.href = url;
      a.download = pdfFilename(open);
      a.style.display = "none";
      document.body.appendChild(a);
      a.click();
      a.remove();
      setTimeout(() => URL.revokeObjectURL(url), 10_000);
      closeSheet();
    });

  // ---------- the journal list ----------

  /** A finished session with no write-up that nobody said was fine without one.
   *  A reflection is never owed (owner's decision 2026-10-02): "No need" takes an
   *  entry out of every "waiting" place, and can be undone. */
  const needsWriteup = (e: ExperienceSummary) =>
    e.kind === "session" && !!e.ended_at && !e.notes.trim() && !e.writeup_skipped && e.writeup_expected !== false;
  /** Whether an entry asks for a write-up by default (only alcohol or a stimulant
   *  in it doesn't). From the journal list, which knows. */
  const expectsWriteup = (id: number) => recent.find((r) => r.id === id)?.writeup_expected !== false;

  let skipNote = $state<{ id: number; timer: number } | null>(null);
  const skipWriteup = (id: number, skipped: boolean) =>
    run("skip", async () => {
      await setWriteupSkipped(id, skipped);
      if (skipNote) clearTimeout(skipNote.timer);
      skipNote = skipped ? { id, timer: window.setTimeout(() => (skipNote = null), UNDO_MS) } : null;
      if (open?.id === id) open = await getExperience(id);
      await refresh();
    });
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
      // Answered on the phone: say so, and name anything it couldn't check.
      comboOffline = net.offline;
      comboUnchecked = net.offline ? await uncheckedOffline(names) : [];
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

  const loadSubstances = () => run("subs", async () => ((substances = await listSubstances()), forgetSubstanceNames()));

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
  <RiskNotes warnings={list} />
{/snippet}

<!-- Tagging: who may tag whom in a dose, and what became of the tags you sent. -->
{#snippet tagging()}
  {#if tagWho.length}
    <h3 class="sec">Tagging</h3>
    <p class="muted small">
      When you both allow it, you and someone else on this server can tag each other in a dose. They get a copy of
      that one dose to change and add to their own journal, or decline. Neither of you can see the other's journal.
    </p>
    {#each tagWho as p (p.id)}
      <label class="check">
        <input type="checkbox" checked={p.you_allow} onchange={(e) => setTagAllow(p, e.currentTarget.checked)} />
        Let {p.name} tag me
      </label>
      <p class="muted small">
        {#if p.you_allow && p.they_allow}You can tag each other.{:else if p.they_allow}{p.name} lets you tag them.{:else if p.you_allow}Waiting for {p.name} to allow it too.{/if}
      </p>
    {/each}
    {#if !isOther}
      <label for="tag-owner-name">Your name, as others here see it</label>
      <div class="pair">
        <input id="tag-owner-name" bind:value={tagOwnerDraft} placeholder={tagOwnerName} autocomplete="off" />
        <button class="small" disabled={!tagOwnerDraft.trim()} onclick={saveOwnerTagName}>Save</button>
      </div>
    {/if}
    {#if tagOut.length}
      <p class="label">Tags you sent</p>
      <ul class="plain">
        {#each tagOut as t (t.id)}
          <li class="small">
            {t.to_name} · {t.substance_name} {fmtDay(t.taken_at)} ·
            {t.status === "accepted" ? "added" : t.status === "declined" ? "declined" : "waiting"}
          </li>
        {/each}
      </ul>
    {/if}
    <p class="muted small">Waiting tags are kept only while the server is running, so a restart clears them.</p>
  {/if}
{/snippet}

<!-- Settings that belong to this phone, for the owner and for anyone else alike. -->
{#snippet thisPhone()}
  {#if !isOther}
    <h3 class="sec">Updates</h3>
    {#if srvUpd?.available}
      <p>
        <strong>Field Notes v{srvUpd.available.version}</strong> is ready to install on your computer (it's on
        v{srvUpd.current}).
      </p>
      <button onclick={() => { srvUpdHidden = false; srvUpdStage = "confirm"; closeSheet(); goTo("today"); }}>Install on the computer…</button>
    {:else if srvUpd}
      <p class="muted small">
        Your computer is on v{srvUpd.current}{srvUpd.checking ? ", checking for a newer one…" : srvUpd.error ? `. ${srvUpd.error}` : ", the newest version."}
      </p>
      <button class="small" onclick={() => loadServerUpdate()}>Check again</button>
    {:else}
      <p class="muted small">Couldn't ask your computer about updates right now.</p>
    {/if}
  {/if}

  <h3 class="sec">Clock</h3>
  <p class="muted small">Times show as {clockPref.h24 ? "21:30" : "9:30 pm"} on this phone.</p>
  <button aria-pressed={clockPref.h24} onclick={() => setClock24(!clockPref.h24)}>{clockPref.h24 ? "Use the 12-hour clock" : "Use the 24-hour clock"}</button>

  <h3 class="sec">Discreet mode</h3>
  {#if discreet.available}
    <p class="muted small">Shows stand-ins like "Substance K7" instead of names and titles, on this phone only.</p>
    <button aria-pressed={discreet.on} onclick={() => setDiscreet(!discreet.on)}>{discreet.on ? "Turn off on this phone" : "Turn on on this phone"}</button>
  {:else}
    <p class="muted small">
      Hides substance names and titles behind stand-ins, for using the app where others can see. {isOther
        ? "The person who runs the server can offer it, in Settings on their computer."
        : "Offer it in Settings on your computer, and then turn it on here."}
    </p>
  {/if}

  <h3 class="sec">Bottom bar</h3>
  <p class="muted small">The last button in the bar along the bottom. On this phone only.</p>
  <div class="seg" role="radiogroup" aria-label="Last button in the bottom bar">
    <button role="radio" aria-checked={navSlot === "talk"} class:on={navSlot === "talk"} onclick={() => setNavSlot("talk")}>Talk</button>
    <button role="radio" aria-checked={navSlot === "stats"} class:on={navSlot === "stats"} onclick={() => setNavSlot("stats")}>Stats</button>
  </div>
  {#if navSlot === "talk" && companionEnabled === false}
    <p class="muted small">The Companion is off, so the Talk button is hidden. Pick Stats to fill the spot.</p>
  {/if}

  <h3 class="sec">Without your computer</h3>
  <p class="muted small">
    {#if net.ready}The dose reference is saved on this phone, so Check and Look up work offline.{:else}The dose reference isn't saved on this phone yet; it saves itself while you're connected.{/if}
    {#if net.kept.open || net.kept.recent}
      For checks offline, it also keeps {net.kept.recent} {net.kept.recent === 1 ? "entry" : "entries"} from the last day{net.kept.open ? `, including ${net.kept.open} in progress` : ""}.
    {/if}
    {#if net.pending}{net.pending === 1 ? "1 entry is" : `${net.pending} entries are`} waiting to be sent.{/if}
  </p>
  <button class="small" onclick={async () => { await clearKept(); keptCleared = true; }}>Clear what this phone keeps</button>
  {#if keptCleared}
    <p class="muted small" role="status">
      Cleared.{net.pending ? " Entries waiting to be sent are kept, so nothing you logged is lost." : ""} The reference saves
      itself again next time you're connected.
    </p>
  {/if}

  <h3 class="sec">Home Screen app</h3>
  {#if standalone}
    <p class="muted small">You're using the Home Screen app. It keeps its own pairing, separate from the browser's.</p>
  {:else}
    <p class="muted small">
      Added to your Home Screen, Field Notes opens full screen. The Home Screen app can't see this browser's pairing, so
      copy this link first and paste it there when it asks. The link is a key to your journal: paste it only into your
      own Home Screen app.
    </p>
    <button class="small" onclick={copyHomeLink}>{homeLinkCopied ? "Link copied ✓" : "Copy link for the Home Screen app"}</button>
  {/if}
{/snippet}

<!-- What else goes in a shared PDF; only what this entry actually has. -->
{#snippet pdfChoices(e: ExperienceDetail)}
  {#if e.intention.trim() || e.setting.trim()}
    <label class="check"><input type="checkbox" bind:checked={pdfParts.intention} /> Intention and setting</label>
  {/if}
  {#if e.timeline.length}
    <label class="check"><input type="checkbox" bind:checked={pdfParts.moments} /> Moments ({e.timeline.length})</label>
  {/if}
  {#if e.notes.trim()}
    <label class="check"><input type="checkbox" bind:checked={pdfParts.writeup} /> {e.kind === "note" ? "The note" : "Write-up"}</label>
  {/if}
  {#if e.rating != null}
    <label class="check"><input type="checkbox" bind:checked={pdfParts.rating} /> Rating ({e.rating}/10)</label>
  {/if}
{/snippet}

<!-- Under an answer from the phone's own checker: say so, and what it couldn't check. -->
{#snippet offlineCheckNote()}
  {#if comboOffline}
    <p class="muted small">
      Checked on this phone, by the same checker your computer runs, against the dose reference.
      {#if comboUnchecked.length}
        <strong>Not checked: {comboUnchecked.join(", ")}.</strong> {comboUnchecked.length === 1 ? "It isn't" : "They aren't"} in
        the reference, and substances you added yourself are only known to your computer.
      {/if}
    </p>
  {/if}
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
                <span class="dot dose" aria-hidden="true"></span><strong>{nameShown(r.dose.substance_name)}</strong>
                {fmtAmt(r.dose)} <span class="muted">{r.dose.route}</span>
                {#if r.dose.profile}<span class="sub">{r.dose.profile.label}</span>{/if}
                {#if r.dose.note}<span class="sub">{r.dose.note}</span>{/if}
              {:else}
                <span class="dot moment" aria-hidden="true"></span>{#if r.ev.mood}<strong>{r.ev.mood}</strong>{r.ev.note ? " · " : ""}{/if}<span class="moment-note">{r.ev.note}</span>
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
        {titleOf(e, e.kind === "note" ? "Untitled note" : "Untitled")}
        {#if e.kind === "note"}<span class="tag">note</span>{/if}
        {#if e.kind === "session" && !e.ended_at}<span class="tag live">live</span>{/if}
      </span>
      {#if e.kind === "session"}
        <span class="meta">
          {[!hiding() && e.substances.join(", ") === e.title ? "" : subsOf(e.substances), dur(e.started_at, e.ended_at), e.rating != null ? `${e.rating}/10` : ""].filter(Boolean).join(" · ")}
        </span>
      {/if}
      {#if e.notes.trim()}
        {#if !hiding()}<span class="excerpt">{excerpt(e.notes)}</span>{/if}
      {:else if needsWriteup(e)}
        <span class="excerpt pending">Add a write-up when you're ready</span>
      {/if}
    </span>
    <span class="chev" aria-hidden="true">›</span>
  </button>
{/snippet}

<!-- ================= page ================= -->

<main style="--kb: {kb}px; --vvh: {vvh ? vvh + 'px' : '100vh'}">
  {#if !paired}
    <section class="pane unpaired">
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
          <button class="pill live" onclick={openLive} aria-label="Open the live trip report">
            <span class="live-dot" aria-hidden="true"></span>{hiding() ? "Live" : session.title || "Live trip report"}
          </button>
        {/if}
        {#if !locked && (isOther || !inTauri())}
          <button class="pill me" onclick={openMe} aria-label={isOther ? "Your journal and devices" : "Settings"}><Icon name="settings" size={18} /></button>
        {/if}
        <DiscreetToggle />
        <button class="help" onclick={openHelp}>Help</button>
      </span>
    </header>

    {#if net.offline || net.pending || net.failed.length}
      <!-- ================= without the computer ================= -->
      <section class="pane offline" role="status">
        {#if net.offline}
          <p>
            <strong>Your computer can't be reached.</strong>
            {#if net.ready}
              Check, Look up and Help work on this phone. New doses, moments and entries save here and are sent when it's
              back. Editing, Stats and Talk wait for it.
            {:else}
              Help works on this phone. Check and Look up will too, once this phone has been connected long enough to save
              the dose reference. New entries still save here and are sent when it's back.
            {/if}
          </p>
        {/if}
        {#if net.pending}
          <p class="small">
            {net.pending === 1 ? "1 entry is" : `${net.pending} entries are`} waiting on this phone to be sent.
            {#if !net.offline}<button class="ghost small" onclick={() => sendWaiting()}>Send now</button>{/if}
          </p>
        {/if}
        {#each net.failed as f (f.seq)}
          <div class="failed">
            <p class="small"><strong>Not sent: {f.what}.</strong> {f.why}</p>
            <div class="pair">
              <button class="small" onclick={() => retryFailed(f.seq)}>Send again</button>
              <button class="ghost small" onclick={() => discardFailed(f.seq)}>Discard</button>
            </div>
          </div>
        {/each}
      </section>
    {/if}

    {#if locked && me}
      <!-- ================= LOCKED: another person's journal ================= -->
      <section class="pane bare lock">
        {#if me.new_journal}
          <h1>Choose a password</h1>
          <p>
            This is your journal{me.person_name ? `, ${me.person_name}` : ""}. Nobody else who uses this server can see
            it, and the person who runs the server can't read it without your password.
          </p>
        {:else}
          <h1>Your journal is locked</h1>
          <p>
            The server restarted, so it forgot your password.
            {canPin && !usePassword ? "Type this device's PIN to open your journal again." : "Type it to open your journal again."}
          </p>
        {/if}
        {#if canPin && !usePassword}
        <form onsubmit={(e) => { e.preventDefault(); unlockWithPin(); }}>
          <label for="lock-pin">Your PIN</label>
          <input id="lock-pin" type="password" bind:value={pinCode} inputmode="numeric" autocomplete="off" maxlength="12" />
          {#if lockErr}<p class="err" role="alert">{lockErr}</p>{/if}
          <button class="primary" type="submit" disabled={unlocking || !pinCode}>{unlocking ? "Unlocking…" : "Unlock"}</button>
          <button type="button" class="ghost small" onclick={() => { usePassword = true; lockErr = null; pinCode = ""; }}>Use my password instead</button>
        </form>
        {:else}
        <form onsubmit={(e) => { e.preventDefault(); unlock(); }}>
          <label for="lock-pw">{me.new_journal ? "Password (at least 8 characters)" : "Password"}</label>
          <input id="lock-pw" type="password" bind:value={pw1} minlength={me.new_journal ? 8 : undefined}
            autocomplete={me.new_journal ? "new-password" : "current-password"} autocapitalize="off" spellcheck="false" />
          {#if me.new_journal}
            <label for="lock-pw2">The same password again</label>
            <input id="lock-pw2" type="password" bind:value={pw2} autocomplete="new-password" autocapitalize="off" spellcheck="false" />
            <p class="banner caution small">
              Only you know this. If you forget it, your journal can't be recovered: nobody can reset it, including the
              person who runs the server. You can download a backup any time from your settings.
            </p>
          {/if}
          {#if lockErr}<p class="err" role="alert">{lockErr}</p>{/if}
          <button class="primary" type="submit" disabled={unlocking || !pw1}>
            {unlocking ? (me.new_journal ? "Creating…" : "Unlocking…") : me.new_journal ? "Create my journal" : "Unlock"}
          </button>
        </form>
        {/if}
        <button class="help wide" onclick={openHelp}>Help</button>
      </section>

      <section class="pane">
        <h2>Check a combination</h2>
        <p class="muted small">Works while your journal is locked.</p>
        <label for="lock-combo">Two or more substances, separated by commas</label>
        <input id="lock-combo" placeholder="e.g. MDMA, ketamine" bind:value={comboText} autocapitalize="none" enterkeyhint="go" onkeydown={(ev) => ev.key === "Enter" && runCombo()} />
        <button disabled={busy} onclick={runCombo}>{busyKey === "combo" ? "Checking…" : "Check"}</button>
        {#if comboWarnings}
          {#if comboWarnings.length === 0}
            <p class="banner note" role="status">Nothing flagged between those. That isn't the same as "safe".</p>
          {:else}
            {@render warnings(comboWarnings)}
          {/if}
          {@render offlineCheckNote()}
        {/if}
      </section>
    {:else}
    <!-- ================= TODAY ================= -->
    {#if view === "today"}
      {#each tagIn as t (t.id)}
        <section class="pane" role="status">
          <p>
            <strong>{t.from_name} tagged you</strong> · {t.dose.substance_name}
            {fmtAmt({ ...t.dose })} · {hhmm(t.dose.taken_at)} {fmtDay(t.dose.taken_at)}
          </p>
          <div class="pair">
            <button class="primary" onclick={() => reviewTag(t)}>Review and add</button>
            <button onclick={() => declineTag(t)}>Decline</button>
          </div>
        </section>
      {/each}
      <SleepCheckin />
      {#if backupNudge}
        <section class="pane" role="status">
          <p>
            <strong>It's been a week since you chose your password.</strong> If you ever forget it, a backup is the only
            way back to your journal. It stays locked with your password.
          </p>
          <div class="pair">
            <button class="primary" disabled={busy} onclick={downloadBackup}>{busyKey === "backup" ? "Preparing…" : "Download a backup"}</button>
            <button onclick={dismissBackupNudge}>Not now</button>
          </div>
        </section>
      {/if}
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
              {#if srvUpd.blocked}<p>{srvUpd.blocked}</p>{/if}
              <p class="muted">
                Field Notes on the computer will restart, and every paired device loses access for a minute or two.
              </p>
              <div class="pair">
                <button class="primary" onclick={installServerUpdate}>Install now</button>
                <button onclick={() => (srvUpdStage = "idle")}>Cancel</button>
              </div>
            {:else if srvUpd.blocked && srvUpd.can_override}
              <p class="muted">{srvUpd.blocked}</p>
              <div class="pair">
                <button onclick={() => (srvUpdStage = "confirm")}>Install anyway…</button>
                <button class="ghost small" onclick={() => (srvUpdHidden = true)}>Not now</button>
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
          <p class="eyebrow"><span class="live-dot" aria-hidden="true"></span>Live trip report</p>
          <h1 class="entry-title">{titleOf(live, "Untitled experience")}</h1>
          <p class="muted">
            Started {hhmm(live.started_at)}{#if t0Of(live)}{" · now "}{rel(new Date(nowTick).toISOString(), t0Of(live))}{/if}
          </p>
          {#if latestDose(live.doses)}
            {@const ld = latestDose(live.doses)!}
            <!-- The question before any redose: when was the last one? Big and
                 always on the card, never behind a tap. -->
            <p class="last-dose">Last: <strong>{nameShown(ld.substance_name)} {fmtAmt(ld)}</strong> · {gapText(Date.parse(ld.taken_at), nowTick)} ago</p>
          {/if}
          {#if warnFor[live.id]?.length}
            {@render warnings(warnFor[live.id])}
            <button class="ghost small" onclick={() => (warnFor = { ...warnFor, [live.id]: [] })}>Dismiss warnings</button>
          {/if}
          <ActiveArcs doses={arcDoses} moments={live.timeline} now={nowTick} compact />
          {@render timeline({ ...live, doses: live.doses.slice(-4), timeline: live.timeline.slice(-3) })}
          <div class="pair">
            <button class="primary log" onclick={() => startDose(live)}>+ Dose</button>
            <button class="moment log" onclick={() => startMoment(live)}>+ Moment</button>
          </div>
          <button class="ghost small open-link" onclick={openLive}>Open trip report ›</button>
          <!-- Ending sits apart from logging and from Help, so it isn't hit by accident. -->
          <div class="end-row">
            <button class="ghost small" onclick={toggleNight}>{night ? "Normal screen" : "Dim (red) screen"}</button>
            <button class="ghost small" onclick={() => startEnd(live)}>End trip report…</button>
          </div>
        </section>
      {:else}
        <!-- One obvious action. Sessions, past sessions, pasted logs and notes
             all live under ＋ New, so they aren't repeated here. -->
        {#if loaded && visible.length === 0}
          <section class="pane">
            <h1>Log something you took</h1>
            <button class="primary big" onclick={() => startDose(null)}>+ Log a dose</button>
            <button onclick={() => (sheet = "start")}>Start a live trip report</button>
          </section>
        {:else}
          <section class="pane">
            <button class="primary big" onclick={() => startDose(null)}>+ Log a dose</button>
          </section>
        {/if}
        <!-- No live trip report, but something logged lately may still be active. -->
        {#if arcDoses.length}
          <section class="pane">
            <ActiveArcs doses={arcDoses} now={nowTick} compact onCheck={checkActive} />
          </section>
        {/if}
      {/if}

      {#if toWriteUp.length}
        <section class="pane">
          <h2>When you're ready to write up</h2>
          <ul class="entries">
            {#each toWriteUp as e (e.id)}
              <li class="with-skip">
                <button class="entry" onclick={async () => { await openEntry(e.id); startWriteup(open); }}>
                  <span class="body">
                    <span class="title">{titleOf(e)}</span>
                    <span class="meta">{[fmtDay(e.started_at), !hiding() && e.substances.join(", ") === e.title ? "" : subsOf(e.substances)].filter(Boolean).join(" · ")}</span>
                  </span>
                  <span class="chev" aria-hidden="true">›</span>
                </button>
                <button class="ghost small skip" onclick={() => skipWriteup(e.id, true)} aria-label={`${e.title || "Untitled"} doesn't need a write-up`}>No need</button>
              </li>
            {/each}
          </ul>
        </section>
      {/if}


      <section class="pane">
        <h2>Recent</h2>
        <ul class="entries">
          {#each visible.slice(0, 6) as e (e.id)}
            <li>{@render entryRow(e)}</li>
          {:else}
            <li class="muted">Nothing logged yet. Tap <strong>＋ New</strong> for a live trip report, a past experience, a note or a pasted log.</li>
          {/each}
        </ul>
        {#if visible.length > 6}
          <button class="ghost" onclick={() => goTo("journal")}>All entries ›</button>
        {/if}
      </section>

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
            {#if e.ended_at && e.doses.length}
              <!-- How the doses likely played out, with the moments on top: felt
                   against expected, for looking back. -->
              <h2 class="sec">Arcs</h2>
              <ActiveArcs doses={e.doses} moments={e.timeline} now={Date.parse(e.ended_at)} past />
            {/if}
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
          {:else if e.kind === "note"}
            <p class="muted">Empty.</p>
          {:else if e.writeup_skipped}
            <p class="muted">Doesn't need a write-up. <button class="ghost small inline" onclick={() => skipWriteup(e.id, false)}>Undo</button></p>
          {:else if e.ended_at && !expectsWriteup(e.id)}
            <p class="muted">No write-up. Add one any time if you'd like.</p>
          {:else}
            <p class="muted">No write-up yet.
              {#if e.ended_at}<button class="ghost small inline" onclick={() => skipWriteup(e.id, true)}>Doesn't need one</button>{/if}
            </p>
          {/if}
        </article>

        <!-- Sticky, at the thumb: the things you do to an entry. -->
        <div class="actionbar">
          {#if e.kind === "session"}
            <button class="primary log" onclick={() => startDose(e)}>+ Dose</button>
            <button class="moment log" onclick={() => startMoment(e)}>+ Moment</button>
            {#if building || !e.ended_at}
              <button onclick={() => startEnd(e)}>{building ? "Finish" : "End…"}</button>
            {/if}
          {/if}
          <button class="more" onclick={() => (sheet = "more")} aria-label="More: edit details, export, delete">⋯</button>
        </div>
      {:else}
        <section class="pane bare">
          <h1>Journal</h1>
          <div class="seg" role="tablist" aria-label="Journal view">
            <button role="tab" aria-selected={journalMode === "entries"} class:on={journalMode === "entries"} onclick={() => (journalMode = "entries")}>Entries</button>
            <button role="tab" aria-selected={journalMode === "stats"} class:on={journalMode === "stats"} onclick={() => (journalMode = "stats")}>Stats</button>
          </div>
        </section>
        {#if journalMode === "stats"}
          <section class="pane bare">
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
            {#each [["all", "All"], ["sessions", "Experiences"], ["notes", "Journal notes"], ["writeup", "To write up"]] as [k, label]}
              <button class="chip" class:on={filter === k} aria-pressed={filter === k} onclick={() => setFilter(k as typeof filter)}>{label}</button>
            {/each}
          </div>
          {#if listSubs.length}
            <div class="chips scroll" role="group" aria-label="Filter by substance">
              {#each listSubs as s}
                <button class="chip" class:on={subFilter === s} aria-pressed={subFilter === s} onclick={() => pickSubFilter(s)}>{nameShown(s)}</button>
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
              <button class="primary" onclick={startPast}>Log a past experience</button>
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
                <li><strong>{nameShown(u.substance_name)}</strong> <span class="muted">· {u.times_used} time{u.times_used === 1 ? "" : "s"}</span></li>
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
            <button class="ghost small" onclick={useLiveInCombo}>Use what's in the live trip report</button>
          {/if}
          {#if arcDoses.length}
            <ActiveArcs doses={arcDoses} now={nowTick} showChart={false} onCheck={(names) => (comboText = names.join(", ") + ", ")} />
          {/if}
          <button class="primary" disabled={busy} onclick={runCombo}>{busyKey === "combo" ? "Checking…" : "Check"}</button>
          {#if comboWarnings}
            {#if comboWarnings.length === 0}
              <p class="banner note" role="status">Nothing flagged between those. That isn't the same as "safe".</p>
            {:else}
              {@render warnings(comboWarnings)}
            {/if}
            {@render offlineCheckNote()}
          {/if}
        {:else}
          <label for="ref">Substance</label>
          <input id="ref" placeholder="e.g. ketamine" bind:value={refQuery} autocapitalize="none" enterkeyhint="search" onkeydown={(ev) => ev.key === "Enter" && lookUp()} />
          <button class="primary" disabled={busy || !refQuery.trim()} onclick={lookUp}>{busyKey === "lookup" ? "Looking…" : "Look up"}</button>

          {#if searched && net.offline}
            <p class="muted small">From the copy of the reference saved on this phone.</p>
          {/if}
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
                  <dt>Heavy</dt><dd>{roa.heavy == null ? "—" : roa.heavy_max != null ? `${roa.heavy}–${roa.heavy_max}` : `${roa.heavy}+`}</dd>
                  {#if roa.onset}<dt>Onset</dt><dd>{roa.onset}</dd>{/if}
                  {#if roa.total}<dt>Total</dt><dd>{roa.total}</dd>{/if}
                </dl>
              </div>
            {/each}
            {#if pw.dose_note}<p class="muted small">{pw.dose_note}</p>{/if}
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
          <p class="muted">The Companion isn't turned on. It's optional: turn it on in Settings on your server if you want it. Everything else works either way.</p>
        {:else if net.offline}
          <p class="muted">
            Talk runs on your computer's local model, so it's unavailable until your computer can be reached. Help, Check and
            Look up still work on this phone.
          </p>
          <button disabled={busy} onclick={loadAi}>Try again</button>
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
            <div class="banner note" role="status">
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

    {/if}

    <!-- ================= messages: next to the thumb, announced ================= -->
    <div class="toasts" aria-live="assertive">
      {#if err}
        <div class="toast bad" role="alert">
          <span>{err}</span>
          <button class="ghost small" onclick={() => (err = null)}>OK</button>
        </div>
      {/if}
      {#if skipNote}
        {@const sid = skipNote.id}
        <div class="toast" role="status">
          <span>Won't ask for a write-up</span>
          <button class="small" onclick={() => skipWriteup(sid, false)}>Undo</button>
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
    {#if !locked}
    <nav aria-label="Sections">
      <button class:on={view === "today"} aria-current={view === "today" ? "page" : undefined} onclick={() => goTo("today")}><Icon name="today" />Today</button>
      <button class:on={view === "journal" && !(navSlot === "stats" && journalMode === "stats")} aria-current={view === "journal" ? "page" : undefined} onclick={() => { if (navSlot === "stats") journalMode = "entries"; goTo("journal"); }}><Icon name="journal" />Journal</button>
      <button class="plus" aria-label="New: log a dose, a moment, a trip report, or a note" onclick={() => (sheet = "new")}><span class="plus-glyph" aria-hidden="true">＋</span><span aria-hidden="true">New</span></button>
      <button class:on={view === "check"} aria-current={view === "check" ? "page" : undefined} onclick={() => goTo("check")}><Icon name="check" />Check</button>
      {#if navSlot === "stats"}
        <button class:on={view === "journal" && journalMode === "stats"} aria-current={view === "journal" && journalMode === "stats" ? "page" : undefined} onclick={goStats}><Icon name="stats" />Stats</button>
      {:else if companionEnabled === true}
        <button class:on={view === "talk"} aria-current={view === "talk" ? "page" : undefined} onclick={() => goTo("talk")}><Icon name="talk" />Talk</button>
      {/if}
    </nav>
    {/if}

    <!-- ================= sheets ================= -->
    {#if sheet}
      <button class="backdrop" aria-label="Close" onclick={closeSheet}></button>
      <div class="sheet" class:writing={sheet === "writeup"} class:kb-open={kb > 0} role="dialog" aria-modal="true" aria-labelledby="sheet-title">
        {#if sheet === "new"}
          <h2 id="sheet-title">New</h2>
          <ul class="menu">
            <li>
              <button onclick={() => startDose(session)}>
                <strong>Log a dose</strong>
                <span class="muted">{session ? "Into the live trip report" : "A substance and roughly when — any day"}</span>
              </button>
            </li>
            {#if session}
              <li>
                <button onclick={() => startMoment(session!)}>
                  <strong>Add a moment</strong>
                  <span class="muted">What's happening right now, in the live trip report</span>
                </button>
              </li>
            {/if}
            {#if !session}
              <li>
                <button onclick={() => (sheet = "start")}>
                  <strong>Start a live trip report</strong>
                  <span class="muted">Log doses and moments as it happens</span>
                </button>
              </li>
            {/if}
            <li>
              <button onclick={startPast}>
                <strong>Log a past experience</strong>
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
              {#if !target}Log a dose{:else if !target.ended_at}Dose · live trip report{:else}Dose · {target.title || fmtDay(target.started_at)}{/if}
            </h2>
            <button class="ghost small" onclick={closeSheet}>Close</button>
          </div>

          {#if receipt}
            <div class="receipt" role="status">
              <p class="receipt-line"><span><strong>✓ Saved</strong> · {receipt.line}</span>
                {#if receipt.doseId != null}<button class="ghost small" onclick={undoLastLog}>Undo</button>{/if}</p>
              {@render warnings(receipt.warnings)}
              <KindQuestion substance={dAsk.s} tick={dAsk.n} />
              {#if receipt.doseId != null && tagPartners.length}
                {@const sentTo = taggedTo[receipt.doseId] ?? []}
                <div class="chips" role="group" aria-label="Tag someone in this dose">
                  {#each tagPartners as p}
                    {#if sentTo.includes(p.id)}
                      <span class="chip on">Tagged {p.name}</span>
                    {:else}
                      <button class="chip" onclick={() => sendTag(receipt!.doseId!, p)}>Tag {p.name}</button>
                    {/if}
                  {/each}
                </div>
              {/if}
            </div>
          {/if}
          {#if fromTag}
            <p class="hint" role="status">
              From {fromTag.from_name}'s tag. Change anything that was different for you, then log it. Only this dose
              goes in your journal; {fromTag.from_name} sees just that you added it.
            </p>
          {/if}

          {#if qRecents.length}
            <div class="chips scroll" role="group" aria-label="Recent substances">
              {#each qRecents as s}
                <button class="chip" class:on={dSub === s} aria-pressed={dSub === s} onclick={() => pickSubstance(s)}>{s}</button>
              {/each}
            </div>
          {/if}
          <label for="d-sub">Substance</label>
          <SubstanceInput id="d-sub" bind:value={dSub} onblur={applyRemembered} onpick={pickSubstance} enterkeyhint="next" />
          <NameHint name={dSub} bind:saveAs={dSubAs} />
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
              <select id="d-route" bind:value={dRoute} onchange={() => { if (!recallDoseShape(dSub)) dUnit = defaultUnitFor(dSub, dRoute) ?? dUnit; }}>{#each ROUTES as r}<option>{r}</option>{/each}</select>
            </div>
          </div>
          <DoseDetailFields substance={dSubAs || dSub} unit={dUnit} amount={dAmt.replace(",", ".")} bind:detail={dDetail} />
          {#if dUnit === "drink"}
            <div class="chips" role="group" aria-label="Add a drink">
              {#each DRINK_PICKS as p}
                <button class="chip" onclick={() => (dAmt = String((Number(dAmt.replace(",", ".")) || 0) + 1))}>+ {p}</button>
              {/each}
            </div>
            <p class="hint">{STANDARD_DRINK}</p>
          {:else if dUnit === "hit"}
            <p class="hint">{HIT_NOTE}</p>
          {/if}
          <label for="d-when">When</label>
          <div class="chips" role="group" aria-label="Quick times">
            {#each doseTimeChips as c}
              <button class="chip" onclick={() => (dWhen = c.value())}>{c.label}</button>
            {/each}
          </div>
          <DateTimeField id="d-when" bind:value={dWhen} variant="phone" />
          {#if sinceSame}
            <p class="since-same">{sinceSame}</p>
          {/if}
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
          <p class="label">How is it? (one tap is enough)</p>
          <div class="chips" role="group" aria-label="How it is">
            {#each MOODS as m}
              <button class="chip" class:on={mMood === m} aria-pressed={mMood === m} onclick={() => (mMood = mMood === m ? "" : m)}>{m}</button>
            {/each}
          </div>
          <label for="m-text">Anything to add? (optional)</label>
          <textarea id="m-text" class="reflect" rows="2" bind:value={mText}></textarea>
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
            <button class="primary" disabled={busy || !(mText.trim() || mMood || mIntensity)} onclick={submitMoment}>{busyKey === "moment" ? "Saving…" : "Add moment"}</button>
          </div>

        {:else if sheet === "jot"}
          <div class="sheet-head">
            <h2 id="sheet-title">Journal note</h2>
            <button class="ghost small" onclick={closeSheet}>Cancel</button>
          </div>
          <label for="j-title">Title (optional)</label>
          <input id="j-title" bind:value={jTitle} />
          <label for="j-body">Note</label>
          <textarea id="j-body" class="reflect" rows="6" bind:value={jBody}></textarea>
          <div class="sheet-actions">
            <button class="primary" disabled={busy || !jBody.trim()} onclick={submitJot}>{busyKey === "jot" ? "Saving…" : "Save note"}</button>
          </div>

        {:else if sheet === "start"}
          <div class="sheet-head">
            <h2 id="sheet-title">Start a live trip report</h2>
            <button class="ghost small" onclick={closeSheet}>Cancel</button>
          </div>
          <label for="s-intention">What's your intention?</label>
          <textarea id="s-intention" class="reflect" rows="2" bind:value={sIntention} placeholder="Optional"></textarea>
          <label for="s-title">Title (optional)</label>
          <input id="s-title" bind:value={sTitle} />
          <p class="hint">Leave it blank and it takes the name of the first substance you log.</p>
          <label class="check"><input type="checkbox" checked={night} onchange={(e) => setNight(e.currentTarget.checked)} /> Dim (red) screen for this trip report</label>
          <div class="sheet-actions">
            <button class="primary" disabled={busy} onclick={submitStart}>{busyKey === "start" ? "Starting…" : "Start"}</button>
          </div>

        {:else if sheet === "past"}
          <div class="sheet-head">
            <h2 id="sheet-title">Log a past experience</h2>
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
          <SubstanceInput id="e-sub" bind:value={eSub} />
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
          <DoseDetailFields substance={eSub} unit={eUnit} amount={eAmt.replace(",", ".")} bind:detail={eDetail} suggest={false} />
          <label for="e-when">When</label>
          <DateTimeField id="e-when" bind:value={eWhen} variant="phone" />
          <label for="e-note">Note (optional)</label>
          <input id="e-note" bind:value={eNote} />
          {#if tagPartners.length}
            {@const sentTo = taggedTo[editDose.id] ?? []}
            <p class="label">Tag someone in this dose</p>
            <div class="chips" role="group" aria-label="Tag someone in this dose">
              {#each tagPartners as p}
                {#if sentTo.includes(p.id)}
                  <span class="chip on">Tagged {p.name}</span>
                {:else}
                  <button class="chip" onclick={() => sendTag(editDose!.id, p)}>Tag {p.name}</button>
                {/if}
              {/each}
            </div>
            <p class="hint">Sends the dose as it's saved now. Save any changes first.</p>
          {/if}
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
          <textarea id="ev-text" class="reflect" rows="3" bind:value={evText}></textarea>
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
            <p class="hint">Leave it blank and the trip report stays live.</p>
            <p class="label">Rating (optional)</p>
            {@render scale(enRating, (v) => (enRating = v), "Rating 0 to 10")}
            <label for="en-int">Intention</label>
            <textarea id="en-int" class="reflect" rows="2" bind:value={enIntention}></textarea>
            <label for="en-set">Setting</label>
            <textarea id="en-set" class="reflect" rows="2" bind:value={enSetting}></textarea>
          {/if}
          <div class="sheet-actions">
            <button class="primary" disabled={busy} onclick={saveDetails}>{busyKey === "details" ? "Saving…" : "Save"}</button>
          </div>

        {:else if sheet === "writeup" && open}
          <div class="sheet-head">
            <h2 id="sheet-title">{open.kind === "note" ? "Note" : "Write-up"}</h2>
            <button class="ghost small" onclick={closeSheet}>Cancel</button>
          </div>
          {#if open.kind === "session" && open.intention.trim()}
            <p class="set-out">You set out to: <em>{open.intention.trim()}</em></p>
          {/if}
          <label class="sr" for="wu">{open.kind === "note" ? "Note" : "Write-up"}</label>
          <textarea id="wu" class="reflect" rows="10" placeholder={open.kind === "note" ? "" : "What came up? What do you want to carry forward?"} bind:value={wuText}></textarea>
          <div class="sheet-actions">
            <button class="primary" disabled={busy} onclick={saveWriteup}>{busyKey === "writeup" ? "Saving…" : "Save"}</button>
          </div>

        {:else if sheet === "end" && open}
          <div class="sheet-head">
            <h2 id="sheet-title">{open.ended_at ? "Finish this experience" : "End the trip report"}</h2>
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
          <!-- Story first, score after: rating a session before anything is written
               about it grades the person instead of inviting reflection. -->
          {#if open.intention.trim()}
            <p class="set-out">You set out to: <em>{open.intention.trim()}</em></p>
          {/if}
          <label for="end-notes">Write-up (optional, now or later)</label>
          <textarea id="end-notes" class="reflect" rows="5" bind:value={endNotes} disabled={endSkip} placeholder="What came up? What do you want to carry forward?"></textarea>
          {#if !endNotes.trim()}
            <label class="check"><input type="checkbox" bind:checked={endSkip} /> This one doesn't need a write-up</label>
          {/if}
          <p class="label">Rating (optional)</p>
          {@render scale(endRating, (v) => (endRating = v), "Rating 0 to 10")}
          <div class="sheet-actions">
            <button class="primary" disabled={busy} onclick={saveEnd}>{busyKey === "end" ? "Saving…" : open.ended_at ? "Finish" : "End trip report"}</button>
          </div>

        {:else if sheet === "pdf" && open}
          <div class="sheet-head">
            <h2 id="sheet-title">Share as PDF</h2>
            <button class="ghost small" onclick={closeSheet}>Close</button>
          </div>
          <p class="muted small">
            The title, times and doses are always in it. Choose what else to include. It uses real names, even in discreet
            mode, and once it's shared it's out of Field Notes' hands.
          </p>
          {@render pdfChoices(open)}
          <div class="sheet-actions">
            {#if canShareFiles}
              <button class="primary" disabled={busy} onclick={sharePdf}>{busyKey === "pdf" ? "Preparing…" : "Share…"}</button>
              <button disabled={busy} onclick={downloadPdf}>Download</button>
            {:else}
              <button class="primary" disabled={busy} onclick={downloadPdf}>{busyKey === "pdf" ? "Preparing…" : "Download"}</button>
            {/if}
          </div>

        {:else if sheet === "more" && open}
          <div class="sheet-head">
            <h2 id="sheet-title">{open.title || "This entry"}</h2>
            <button class="ghost small" onclick={closeSheet}>Close</button>
          </div>
          <ul class="menu">
            <li><button onclick={startDetails}><strong>Edit details</strong><span class="muted">Title, times{open.kind === "session" ? ", rating, intention, setting" : ""}</span></button></li>
            <li><button onclick={() => startWriteup(open)}><strong>{open.kind === "note" ? "Edit the note" : "Edit the write-up"}</strong></button></li>
            <li><button onclick={startPdf}><strong>Share as PDF</strong><span class="muted">A tidy report to send to someone, with what you choose in it</span></button></li>
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

        {:else if sheet === "me"}
          <div class="sheet-head">
            <h2 id="sheet-title">{isOther ? "Your journal" : "Settings"}</h2>
            <button class="ghost small" onclick={closeSheet}>Close</button>
          </div>
          {#if err && !changingPw && !rememberAsk}<p class="err" role="alert">{err}</p>{/if}
          {#if isOther}
            <p>
              <strong>Your journal is yours.</strong> Nobody else who uses this server can see it, and the person who runs
              the server can't read it without your password.
            </p>
          {/if}

          <h3 class="sec">Your devices</h3>
          {#if !isOther}
            {#if ownerAuth === "off"}
              <p class="muted small">
                To pair or un-pair devices from this phone, set a phone PIN in Settings on your computer (Devices &amp;
                server). Your journal isn't encrypted, so there's no password to ask for instead.
              </p>
            {:else if ownerAuth}
              <p class="muted small">
                Pairing gives a device its own key to your journal, so this asks for
                {ownerAuth === "pin" ? "your phone PIN" : "your journal's password"} first. Someone holding this phone
                can't add a device of their own without it.
              </p>
              <label for="owner-pw">{ownerPwLabel}</label>
              <input id="owner-pw" type="password" bind:value={ownerPw} autocomplete={ownerAuth === "pin" ? "off" : "current-password"}
                inputmode={ownerAuth === "pin" ? "numeric" : undefined} autocapitalize="off" spellcheck="false" />
            {/if}
          {/if}
          {#if mine.length}
            <ul class="plain devices">
              {#each mine as d (d.id)}
                <li>
                  <span><strong>{d.name}</strong>{#if d.this} <span class="tag">this one</span>{/if}<br />
                    <span class="muted small">{seen(d.last_seen)}</span></span>
                  {#if unpairAsk === d.id}
                    <span class="pair">
                      <button class="small danger" disabled={busy || (!isOther && (ownerAuth === "off" || !ownerPw))} onclick={() => unpair(d)}>Un-pair</button>
                      <button class="small" onclick={() => (unpairAsk = null)}>Keep</button>
                    </span>
                  {:else}
                    <button class="small" onclick={() => (unpairAsk = d.id)}>Un-pair…</button>
                  {/if}
                </li>
                {#if unpairAsk === d.id}
                  <li class="muted small">
                    {d.this
                      ? "This device will stop opening your journal straight away. Your journal stays; pair again from another device of yours."
                      : "That device will stop opening your journal straight away. Your journal stays."}
                  </li>
                {/if}
              {/each}
            </ul>
          {:else if busyKey === "me"}
            <p class="muted">Loading…</p>
          {/if}

          {#if pairedNew}
            <div class="paired-new">
              <p><strong>Scan this with {pairedNew.name}.</strong></p>
              {#if pairedNew.qr}<div class="qr">{@html pairedNew.qr}</div>{/if}
              <p class="link small">{pairedNew.link}</p>
              <button class="small" onclick={copyPairLink}>{linkCopied ? "Copied" : "Copy link"}</button>
              <p class="muted small">
                This code is a key to your journal. Show it only to your own device, and send the link to yourself
                privately if you send it at all. It keeps working until you un-pair that device.
              </p>
              <button class="ghost small" onclick={() => (pairedNew = null)}>Done</button>
            </div>
          {:else if isOther || (ownerAuth && ownerAuth !== "off")}
            <label for="new-device">Pair another device of yours</label>
            <input id="new-device" bind:value={newDevice} placeholder="e.g. Laptop" autocomplete="off" />
            <button disabled={busy || (!isOther && !ownerPw)} onclick={pairAnother}>{busyKey === "pairown" ? "Making a code…" : "Show a pairing code"}</button>
          {/if}

          {@render tagging()}

          {@render thisPhone()}

          {#if isOther && me}
          <h3 class="sec">Back up your journal</h3>
          <p>
            Download a copy to this device. It stays locked with your password and opens only with that. To open it,
            use Field Notes on a computer of your own: Settings, Backup &amp; restore, Restore from backup (this replaces
            the journal on that computer). It's the only way back if you forget your password.
          </p>
          <button disabled={busy} onclick={downloadBackup}>{busyKey === "backup" ? "Preparing…" : "Download a backup"}</button>
          {#if backedUp}<p class="muted small" role="status">Saved to your downloads.</p>{/if}

          <h3 class="sec">Your password</h3>
          {#if changingPw}
            <form onsubmit={(e) => { e.preventDefault(); changePassword(); }}>
              <label for="pw-current">Current password</label>
              <input id="pw-current" type="password" bind:value={pwCurrent} autocomplete="current-password" autocapitalize="off" spellcheck="false" />
              <label for="pw-new">New password (at least 8 characters)</label>
              <input id="pw-new" type="password" bind:value={pwNew} autocomplete="new-password" autocapitalize="off" spellcheck="false" />
              <label for="pw-new2">The new password again</label>
              <input id="pw-new2" type="password" bind:value={pwNew2} autocomplete="new-password" autocapitalize="off" spellcheck="false" />
              {#if err}<p class="err" role="alert">{err}</p>{/if}
              <p class="muted small">
                Your other devices stay paired and keep working, but any PIN you've set turns off. Backups you've
                already downloaded still open with the old password.
              </p>
              <span class="pair">
                <button class="primary" type="submit" disabled={busy || !pwCurrent || !pwNew}>{busyKey === "changepw" ? "Changing…" : "Change password"}</button>
                <button type="button" onclick={() => { changingPw = false; pwCurrent = pwNew = pwNew2 = ""; }}>Cancel</button>
              </span>
            </form>
          {:else}
            {#if pwChanged}<p class="muted small" role="status">Password changed.</p>{/if}
            <button onclick={() => { changingPw = true; pwChanged = false; }}>Change password…</button>
          {/if}

          <h3 class="sec">Unlock with a PIN on this device</h3>
          {#if canPin}
            <p>
              <strong>On.</strong> When your journal locks, this device can open it with a short PIN instead of your
              password. Five wrong PINs and it turns off, and then your password will do.
            </p>
            <button disabled={busy} onclick={clearPin}>{busyKey === "pin" ? "Turning off…" : "Turn off"}</button>
          {:else}
            <p>
              <strong>Off.</strong> A PIN works only on this device, and only with your server. Your password stays the
              real key: the server never keeps it, and this device keeps it only sealed with a key the server holds.
              Changing your password turns the PIN off on all your devices.
            </p>
            {#if pinAsk}
              <form onsubmit={(e) => { e.preventDefault(); setPin(); }}>
                <label for="pin-new">PIN (4 to 12 digits)</label>
                <input id="pin-new" type="password" bind:value={pinNew} inputmode="numeric" autocomplete="off" maxlength="12" />
                <label for="pin-new2">The same PIN again</label>
                <input id="pin-new2" type="password" bind:value={pinNew2} inputmode="numeric" autocomplete="off" maxlength="12" />
                <label for="pin-pw">Your password</label>
                <input id="pin-pw" type="password" bind:value={pinPw} autocomplete="current-password" autocapitalize="off" spellcheck="false" />
                {#if err}<p class="err" role="alert">{err}</p>{/if}
                <span class="pair">
                  <button class="primary" type="submit" disabled={busy || !pinNew || !pinPw}>{busyKey === "pin" ? "Setting…" : "Set PIN"}</button>
                  <button type="button" onclick={() => { pinAsk = false; pinNew = pinNew2 = pinPw = ""; }}>Cancel</button>
                </span>
              </form>
            {:else}
              <button onclick={() => (pinAsk = true)}>Set a PIN…</button>
            {/if}
          {/if}

          <h3 class="sec">Keep my journal unlocked on this server</h3>
          {#if me.remembered}
            <p>
              <strong>On.</strong> Your password is kept in the server computer's keychain, so your journal opens by
              itself after a restart. Anyone who can log in to that computer could open it too.
            </p>
            <button disabled={busy} onclick={() => setRemember(false)}>{busyKey === "remember" ? "Turning off…" : "Turn off"}</button>
          {:else}
            <p>
              <strong>Off.</strong> When the server restarts, your journal locks until you type your password on one of
              your devices. Turning this on keeps your password in the server computer's keychain so it opens by itself,
              but then anyone who can log in to that computer could open your journal too.
            </p>
            {#if rememberAsk}
              <form onsubmit={(e) => { e.preventDefault(); setRemember(true); }}>
                <label for="remember-pw">Your password</label>
                <input id="remember-pw" type="password" bind:value={rememberPw} autocomplete="current-password" autocapitalize="off" spellcheck="false" />
                {#if err}<p class="err" role="alert">{err}</p>{/if}
                <span class="pair">
                  <button class="primary" type="submit" disabled={busy || !rememberPw}>{busyKey === "remember" ? "Checking…" : "Keep it unlocked"}</button>
                  <button type="button" onclick={() => { rememberAsk = false; rememberPw = ""; }}>Cancel</button>
                </span>
              </form>
            {:else}
              <button onclick={() => (rememberAsk = true)}>Turn on…</button>
            {/if}
          {/if}

          <h3 class="sec">The limit</h3>
          <p class="muted small">
            While your journal is unlocked it is open in the server's memory, so someone with full control of that
            computer and the skill to inspect a running program could reach it. This protects you from other people who
            share the server, not from a determined person who controls the machine.
          </p>
          {/if}
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
  /* Warm, per the 2026-10-02 design decision: "a field notebook, not a dashboard
     or an ER". Warm near-black paper, warm ink, one soft cool accent kept apart
     from Help's warm sand, the live green and caution amber. Contrast: body and
     secondary text 7:1 or more on every surface; field borders 3:1. */
  :global(:root) {
    color-scheme: dark light;
    --bg: #14120f;
    --surface: #1c1916;
    --surface-2: #27231e;
    --field: #0f0d0b;
    --field-border: #6f665b;
    --divider: #322c25;
    --text: #eee7dc;
    --text-2: #b8ad9c;
    --accent: #b9a3c9;
    --on-accent: #11141c;
    /* Outline of secondary buttons: 3:1 or better on every surface. */
    --edge: #8f8576;
    /* Help has its own colour: warm, filled, unmistakable, and not the red of
       danger or Delete. Red means only "these two don't mix". */
    --help-bg: #f2dcc4;
    --help-ink: #1d140b;
    --danger: #f2897a;
    --danger-bg: #f2897a24;
    --caution: #e6b062;
    --caution-bg: #e6b0621f;
    --note-bg: #b9a3c91a;
    --ok: #93cf9e;
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
    --nav-h: calc(69px + var(--sa-b));
  }
  @media (prefers-color-scheme: light) {
    :global(:root) {
      --bg: #f6f1e8;
      --surface: #fffbf5;
      --surface-2: #efe7da;
      --field: #fffdf9;
      --field-border: #857a6b;
      --divider: #e2d8c8;
      --text: #211b13;
      --text-2: #5a5043;
      --accent: #5f4a72;
      --on-accent: #ffffff;
      --edge: #8a7e6d;
      --help-bg: #5a3214;
      --help-ink: #fff6ec;
      --danger: #ad2b1f;
      --danger-bg: #ad2b1f14;
      --caution: #855000;
      --caution-bg: #8550001a;
      --note-bg: #5f4a7212;
      --ok: #2c7339;
    }
  }

  /* Dim (red) night theme: true black and red-shifted text, so a glance doesn't
     wreck dark adaptation. Every pair here meets 4.5:1 or better on black; warnings
     still carry their words ("Known dangerous:"), never hue alone. Help stays a
     filled pill, its shape setting it apart now that everything is red. */
  :global(:root[data-theme="night"]) {
    color-scheme: dark;
    --bg: #000000;
    --surface: #0a0100;
    --surface-2: #170403;
    --field: #000000;
    --field-border: #8a3322;
    --divider: #2a0a06;
    --text: #ff7a5c;
    --text-2: #d9533a;
    --accent: #ff5a36;
    --on-accent: #000000;
    --edge: #a8432c;
    --help-bg: #ff7a5c;
    --help-ink: #000000;
    --danger: #ffb199;
    --danger-bg: #ffb19922;
    --caution: #ff9a6b;
    --caution-bg: #ff9a6b1c;
    --note-bg: #ff5a3614;
    --ok: #ff9a6b;
    --focus: #ffb199;
  }

  :global(body) {
    margin: 0;
    background: var(--bg);
    color: var(--text);
    font: -apple-system-body;
    font-family: var(--font-data, -apple-system, system-ui, sans-serif);
    font-size: var(--fs-body);
    line-height: 1.45;
    -webkit-text-size-adjust: 100%;
  }
  main {
    max-width: 36rem;
    margin: 0 auto;
    padding: 0 0.8rem calc(var(--nav-h) + 5.5rem);
  }
  :global(:focus-visible) { outline: 3px solid var(--focus); outline-offset: 2px; }

  .muted { color: var(--text-2); }
  .small { font-size: var(--fs-sm); }
  .hint { color: var(--text-2); font-size: var(--fs-sm); margin: -0.2rem 0 0.7rem; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }

  h1 { font-size: 1.25rem; margin: 0 0 0.4rem; line-height: 1.2; }
  h2 { font-size: var(--fs-h); margin: 0 0 0.6rem; }

  /* ---------- header ---------- */
  /* Pinned, so Help is on screen however far down a page you are, and padded by
     the safe area so it clears the status bar and notch instead of sitting under
     the clock. Below the sheets (30+) and toasts (20), above the nav strip (5). */
  /* No header on this screen, so it clears the status bar itself. */
  .unpaired { margin-top: calc(0.8rem + var(--sa-t)); }
  header.top {
    display: flex; justify-content: space-between; align-items: center; gap: 0.5rem;
    min-height: var(--tap);
    position: sticky; top: 0; z-index: 15;
    margin: 0 -0.8rem 0.4rem; padding: calc(0.5rem + var(--sa-t)) 0.8rem 0.4rem;
    background: var(--bg);
  }
  .brand { font-weight: 700; white-space: nowrap; flex-shrink: 0; }
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
    width: auto; margin: 0; min-height: var(--tap-min); padding: 0 1rem;
    border: 0; color: var(--help-ink); background: var(--help-bg); border-radius: 999px; font-weight: 700;
  }
  .help.wide { width: 100%; margin: 0.4rem 0; }
  .pill.me { padding: 0 0.6rem; display: inline-flex; align-items: center; }
  .lock h1 { margin-top: 0.6rem; }
  .devices li { display: flex; justify-content: space-between; align-items: center; gap: 0.6rem; }
  .paired-new { margin: 0.6rem 0; }
  .paired-new .qr { background: #fff; border-radius: 8px; padding: 0.4rem; width: fit-content; margin: 0.4rem 0; }
  .paired-new .qr :global(svg) { display: block; width: 220px; height: 220px; }
  .paired-new .link { word-break: break-all; user-select: all; }

  /* ---------- surfaces ---------- */
  /* Clean and utilitarian: sections sit on the page, full width, divided by a
     hairline, rather than floating as cards. Body text, buttons and tap sizes
     are unchanged; only the scaffolding got quieter. */
  .pane { background: none; border: 0; border-top: 1px solid var(--divider); border-radius: 0; padding: 1rem 0.2rem 1.1rem; margin: 0; }
  .pane > h2:not(.sec):not(.month-label) { font-size: var(--fs-xs); letter-spacing: 0.08em; text-transform: uppercase; color: var(--text-2); font-weight: 700; margin-bottom: 0.5rem; }
  /* One container level per screen: headers and wrappers sit on the page itself. */
  .pane.bare { background: transparent; border-top: 0; padding: 0.2rem 0.2rem 0; }
  .eyebrow { margin: 0 0 0.2rem; color: var(--ok); font-size: var(--fs-sm); font-weight: 600; }
  .entry-title { font-size: 1.3rem; }
  .facts { color: var(--text-2); margin: 0 0 0.8rem; }
  .sec { font-size: var(--fs-sm); text-transform: uppercase; letter-spacing: 0.06em; color: var(--text-2); margin: 1.2rem 0 0.3rem; }
  .sec-head { display: flex; justify-content: space-between; align-items: baseline; }
  /* Reflection (intention, setting, write-up, moments) is set in Literata. */
  .prose { white-space: pre-wrap; font-family: var(--font-reflect); font-size: var(--fs-body); line-height: 1.6; margin: 0 0 0.6rem; max-width: 60ch; }
  textarea.reflect { font-family: var(--font-reflect); line-height: 1.55; }
  .excerpt, .set-out em, .moment-note { font-family: var(--font-reflect); }

  /* ---------- fields ---------- */
  label, .label { display: block; font-size: var(--fs-sm); color: var(--text-2); margin: 0.2rem 0 0.25rem; }
  input, select, textarea {
    width: 100%; box-sizing: border-box; font: inherit; font-size: var(--fs-body);
    background: var(--field); color: var(--text); border: 1px solid var(--field-border);
    border-radius: 10px; padding: 0.7rem 0.75rem; margin-bottom: 0.6rem; min-height: var(--tap);
  }
  textarea { line-height: 1.5; }
  /* The substance box lives in its own component, so the rule above doesn't
     reach it: without this it gets the browser's small default font, which
     also makes iOS zoom in on focus (it zooms on any field under 16px). */
  :global(.sub-suggest input) {
    width: 100%; box-sizing: border-box; font: inherit; font-size: var(--fs-body);
    background: var(--field); color: var(--text); border: 1px solid var(--field-border);
    border-radius: 10px; padding: 0.7rem 0.75rem; margin-bottom: 0.6rem; min-height: var(--tap);
  }
  .grid3 { display: grid; grid-template-columns: 1.3fr 1fr 1.4fr; gap: 0.5rem; }

  /* ---------- buttons ---------- */
  /* One rule you can see: rectangles hold things (cards, sheets, text boxes),
     pills do things. Three tiers by shape, so they still read in dim red where
     colour can't help: filled (the one main action), outlined, plain text. */
  button {
    font: inherit; font-size: var(--fs-body); font-weight: 600; border-radius: 999px;
    border: 1.5px solid var(--edge); background: transparent; color: var(--text);
    padding: 0.5rem 1.1rem; width: 100%; min-height: var(--tap-min); margin: 0 0 0.5rem; cursor: pointer;
    position: relative;
  }
  /* The visible button is smaller than the area that takes the tap: actions keep
     48px, and the session's + Dose / + Moment keep 56px, even though they look
     lighter. Taps a few pixels outside still count. */
  button::after { content: ""; position: absolute; inset: -2px -1px; }
  button:active:not(:disabled) { transform: translateY(1px); background: var(--surface-2); }
  button.primary { background: var(--accent); color: var(--on-accent); border-color: var(--accent); }
  button.primary:active:not(:disabled) { background: var(--accent); filter: brightness(0.93); }
  /* + Moment: the same size and place as + Dose, outlined so + Dose leads. */
  button.moment { border-color: var(--accent); color: var(--accent); }
  .pair > button.log, .actionbar button.log { min-height: 46px; }
  .pair > button.log::after, .actionbar button.log::after { inset: -5px -1px; }
  button.big { min-height: 50px; font-size: var(--fs-body); }
  button.big::after { inset: -3px -1px; }
  button.ghost { background: transparent; border-color: transparent; color: var(--accent); }
  button.small { width: auto; min-height: var(--tap-min); padding: 0 0.7rem; margin: 0; font-size: var(--fs-sm); }
  button.danger { background: var(--danger); border-color: var(--danger); color: #fff; }
  /* Delete reads as an ordinary action until its confirm step, which is red. */
  button.danger-text { width: auto; margin: 0; background: transparent; border-color: transparent; color: var(--text-2); font-weight: 500; text-decoration: underline; text-underline-offset: 3px; min-height: var(--tap-min); }
  button.danger-text.wide { width: 100%; }
  button:disabled { background: var(--surface-2); color: var(--text-2); border-color: transparent; cursor: default; }
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
    font-size: var(--fs-sm); font-weight: 500; border-radius: 999px; border-width: 1px; background: transparent; white-space: nowrap;
  }
  /* Selected chips are tinted, not slabs: a row of filters stays calm. */
  .chip.on { background: color-mix(in srgb, var(--accent) 22%, var(--surface)); color: var(--accent); border-color: transparent; font-weight: 700; }
  .chip.num { border-radius: 12px; }
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
  /* Muted, not amber: an unwritten reflection isn't a warning or overdue homework. */
  .excerpt.pending { color: var(--text-2); font-style: italic; }
  .last-dose { font-size: 1.25rem; margin: 0.2rem 0 0.6rem; font-variant-numeric: tabular-nums; }
  .since-same { font-size: var(--fs-body); font-weight: 600; margin: -0.2rem 0 0.4rem; font-variant-numeric: tabular-nums; }
  .receipt-line { display: flex; justify-content: space-between; align-items: center; gap: 0.5rem; }
  .receipt-line .ghost { width: auto; margin: 0; flex: none; }
  .end-row { display: flex; justify-content: space-between; gap: 1.5rem; margin-top: 1.2rem; padding-top: 0.6rem; border-top: 1px solid var(--divider); }
  .end-row .ghost { width: auto; margin: 0; }
  .set-out { margin: 0 0 0.6rem; color: var(--text-2); }
  /* Writing sheets: the text box takes whatever room is left, so the heading and
     "You set out to" stay in view above it while typing. Before, the box kept its
     full ten rows with the keyboard up, the sheet scrolled to the cursor, and
     everything above the box went off the top of the screen. */
  .sheet.writing { display: flex; flex-direction: column; }
  .sheet.writing > .sheet-head, .sheet.writing > .set-out, .sheet.writing > .sheet-actions { flex: none; }
  .sheet.writing > .set-out { max-height: 5.5em; overflow-y: auto; }
  .sheet.writing > textarea { flex: 1 1 14rem; min-height: 4.5rem; resize: none; }
  .sheet.writing.kb-open { height: calc(var(--vvh, 100vh) - var(--sa-t) - 1rem); max-height: none; }
  .sheet.writing.kb-open > .sheet-actions { position: static; }
  .with-skip { display: flex; align-items: center; gap: 0.3rem; }
  .with-skip .entry { flex: 1; min-width: 0; }
  .with-skip .skip { width: auto; flex: none; margin: 0; }
  .inline { width: auto; display: inline; margin: 0 0 0 0.3rem; min-height: 0; padding: 0.2rem 0.3rem; }
  label.check { display: flex; align-items: center; gap: 0.5rem; color: var(--text); font-size: var(--fs-body); min-height: var(--tap); margin: 0 0 0.6rem; }
  label.check input { width: 1.3rem; height: 1.3rem; min-height: 0; margin: 0; }
  .tag { font-size: var(--fs-xs); font-weight: 500; border: 1px solid var(--divider); border-radius: 6px; padding: 0 0.35rem; margin-left: 0.35rem; color: var(--text-2); }
  .tag.live { border-color: var(--ok); color: var(--ok); }
  .month-label { font-size: var(--fs-sm); text-transform: uppercase; letter-spacing: 0.06em; color: var(--text-2); margin-bottom: 0.2rem; }

  /* ---------- check ---------- */
  .seg { display: flex; gap: 0.3rem; background: var(--surface-2); border-radius: 999px; padding: 0.25rem; margin-bottom: 0.8rem; }
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
  /* Without the computer: calm, not alarming, and always visible while it applies. */
  .pane.offline { border: 1px solid var(--divider); border-radius: 12px; padding: 0.7rem 0.8rem; margin: 0.5rem 0; }
  .pane.offline p { margin: 0.2rem 0; }
  .pane.offline .failed { border-top: 1px solid var(--divider); padding-top: 0.4rem; margin-top: 0.4rem; }
  .banner { border-radius: 10px; padding: 0.7rem 0.8rem; margin: 0.5rem 0; border: 1px solid; font-size: var(--fs-sm); }
  .banner.danger { border-color: var(--danger); background: var(--danger-bg); }
  .banner.caution { border-color: var(--caution); background: var(--caution-bg); }
  .banner.note { border-color: var(--accent); background: var(--note-bg); }
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
  nav button { flex: 1; margin: 0; padding: 0.15rem 0 0; min-height: 56px; font-size: var(--fs-sm); background: transparent; border-color: transparent; color: var(--text-2);
    display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 0.15rem; }
  /* The tab you're on: a filled pill behind it, not just a tint, so it reads at a
     glance (and in Dim (red), where every colour is a shade of the same red). */
  nav button.on {
    color: var(--accent); font-weight: 800; border-radius: 16px; border-color: transparent;
    background: color-mix(in srgb, var(--accent) 20%, transparent);
  }
  @media (forced-colors: active) {
    nav button.on { border: 2px solid CanvasText; }
  }
  nav button.plus {
    flex: 0 0 4.2rem; gap: 0; padding: 0; font-weight: 700; background: var(--accent); color: var(--on-accent); border-radius: 999px;
    transform: translateY(-6px); box-shadow: 0 4px 12px #0004;
  }
  nav button.plus:active:not(:disabled) { background: var(--accent); filter: brightness(0.93); transform: translateY(-5px); }
  .plus-glyph { font-size: 1.45rem; font-weight: 500; line-height: 1; }

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
  .menu button { display: flex; flex-direction: column; align-items: flex-start; text-align: left; gap: 0.1rem; border-radius: var(--radius); border-color: transparent; background: var(--surface-2); }
  .open-link { display: block; padding-left: 0; margin: -0.2rem 0 0.2rem; color: var(--text); }
  .menu .muted { font-weight: 400; font-size: var(--fs-sm); }
  .danger-zone { border-top: 1px solid var(--divider); margin-top: 1.2rem; padding-top: 0.8rem; }

  @media (prefers-reduced-motion: no-preference) {
    .sheet { animation: rise 0.18s ease-out; }
    @keyframes rise { from { transform: translateY(24px); opacity: 0.6; } }
  }
</style>
