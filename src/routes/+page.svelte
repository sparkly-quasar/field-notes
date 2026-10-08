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
    updateExperience,
    setWriteupSkipped,
    updateDose,
    updateTimelineEvent,
    type TimelineEvent,
    deleteExperience,
    deleteDose,
    deleteTimelineEvent,
    deleteSubstance,
    type Dose,
    type DoseDetail,
    aiStatus,
    aiRecommendedModels,
    aiInstall,
    aiStart,
    aiPull,
    aiPreferredModel,
    setCompanionEnabled,
    aiSwitchModel,
    companionChat,
    companionWarm,
    computeStatus,
    type ComputeStatus,
    crisisScan,
    emergencyResources,
    type CrisisResult,
    type CrisisResource,
    parseExperience,
    importExperience,
    pwLookup,
    knowledgeSearch,
    knowledgeEntry,
    knowledgeEntries,
    knowledgeStatus,
    type KnowledgeHit,
    type KnowledgeEntry,
    type KnowledgeStatus,
    contributionCandidates,
    contributionDraft,
    contributionSave,
    type ContributionCandidate,
    type ContributionDraft,
    portalStatus,
    portalEnable,
    portalDisable,
    portalQr,
    portalTailscale,
    portalServe,
    portalUnserve,
    tailnetConnect,
    tailnetSignOut,
    tailnetUseApp,
    type PortalStatus,
    type TailscaleStatus,
    dbStatus,
    unlockDb,
    enableEncryption,
    disableEncryption,
    changePassphrase,
    exportBackup,
    importBackup,
    obsidianExport,
    obsidianImport,
    exportExperienceMarkdown,
    exportExperienceFile,
    dataDir,
    revealDataDir,
    wipeAllData,
    type DbStatus,
    type ParsedExperience,
    type PwInfo,
    type PwRoa,
    type ExperienceSummary,
    type ExperienceDetail,
    type Substance,
    type SubstanceUsage,
    type Warning,
    checkCombo,
    type ChatMsg,
    type AiStatus,
    portalPair,
    portalDevices,
    portalRevoke,
    peopleList,
    personAdd,
    personRemove,
    type PersonInfo,
    serverPrefs,
    setServerPrefs,
    setPhoneCanUpdate,
    phonePinStatus,
    setPhonePin,
    type PhonePinStatus,
    setMenuBar,
    discreetAvailable,
    setDiscreetAvailable,
    keychainStatus,
    keychainRemember,
    keychainForget,
    remoteStatus,
    remoteConnect,
    remoteDisconnect,
    remoteDiscard,
    remoteFlush,
    remoteUploadLocal,
    saveMarkdownFile,
    savePdfFile,
    setRemoteMode,
    type DeviceInfo,
    type PairResult,
    type ServerPrefs,
    type KeychainStatus,
    type RemoteStatus,
    type RemoteFailed,
  } from "$lib/api";
  import { inTauri } from "$lib/portal";
  import NameHint from "$lib/NameHint.svelte";
  import SubstanceInput, { forgetSubstanceNames } from "$lib/SubstanceInput.svelte";
  import DoseDetailFields from "$lib/DoseDetailFields.svelte";
  import KindQuestion from "$lib/KindQuestion.svelte";
  import SleepCheckin from "$lib/SleepCheckin.svelte";
  import { describeAmount, detailOf, inUnit, measure, microCutoff } from "$lib/dosedetail";
  import { ALL_PARTS, experiencePdf, pdfFilename, type PdfParts } from "$lib/pdf";
  import {
    enable as autostartEnable,
    disable as autostartDisable,
    isEnabled as autostartIsEnabled,
  } from "@tauri-apps/plugin-autostart";
  import {
    quickLog,
    recentSubstances,
    whenPresets,
    recallDoseShape,
    rememberDoseShape,
    defaultUnitFor,
    sameUnit,
    UNITS,
    STANDARD_DRINK,
    HIT_NOTE,
    recentDoses,
  } from "$lib/quicklog";
  import { getVersion } from "@tauri-apps/api/app";
  import TripImport from "$lib/TripImport.svelte";
  import RiskNotes from "$lib/RiskNotes.svelte";
  import ActiveArcs from "$lib/ActiveArcs.svelte";
  import DateTimeField from "$lib/DateTimeField.svelte";
  import UsageStats from "$lib/UsageStats.svelte";
  import { lastDose as latestDose, span as gapText } from "$lib/livefacts";
  import { discreet, hiding, shown as nameShown } from "$lib/discreet.svelte";
  import DiscreetToggle from "$lib/DiscreetToggle.svelte";
  import Icon from "$lib/Icon.svelte";
  import { listen } from "@tauri-apps/api/event";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { save, open as openDialog } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { exit } from "@tauri-apps/plugin-process";

  type Tab = "journal" | "companion" | "substances" | "bysub" | "stats" | "data";

  const HIDE_DISCLAIMER_KEY = "fieldnotes.hideDisclaimer";

  let acknowledged = $state(false);
  let tab = $state<Tab>("journal");
  // Settings is split into a few sections so nothing is a long scroll away.
  type SettingsSection = "general" | "devices" | "data" | "feedback";
  const SETTINGS_SECTIONS: [SettingsSection, string][] = [
    ["general", "General"],
    ["devices", "Devices & sync"],
    ["data", "Data & privacy"],
    ["feedback", "Feedback"],
  ];
  let settingsSection = $state<SettingsSection>("general");

  // at-rest encryption / unlock gate
  let db = $state<DbStatus>({ encrypted: false, unlocked: true });
  let openingSlow = $state(false);
  const FIRST_STEPS_KEY = "fieldnotes.firstStepsDismissed";
  let firstStepsDismissed = $state((() => { try { return localStorage.getItem(FIRST_STEPS_KEY) === "1"; } catch { return false; } })());
  function dismissFirstSteps() {
    firstStepsDismissed = true;
    try { localStorage.setItem(FIRST_STEPS_KEY, "1"); } catch {}
  }
  let statusLoaded = $state(false);
  let unlockPass = $state("");
  let unlockErr = $state<string | null>(null);
  let unlockBusy = $state(false);
  let dontShowDisclaimer = $state(false);

  // security & backup controls (Data tab)
  let secBusy = $state(false);
  let secErr = $state<string | null>(null);
  let secMsg = $state<string | null>(null);
  let encNewPass = $state("");
  let encNewPass2 = $state("");
  let encDisablePass = $state("");
  let chgCurrent = $state("");
  let chgNew = $state("");
  let chgNew2 = $state("");
  // encrypt-this-backup option (only offered when the journal itself is plaintext)
  let bkEncrypt = $state(false);
  let bkPassword = $state("");
  let bkPassword2 = $state("");
  // erase / uninstall
  let dataDirPath = $state("");
  const isMac = typeof navigator !== "undefined" && navigator.userAgent.includes("Mac");
  const isWindows = typeof navigator !== "undefined" && navigator.userAgent.includes("Windows");

  // Obsidian vault sync
  const VAULT_KEY = "fieldnotes.vaultFolder";
  const COMPANION_OFF_KEY = "fieldnotes.companionOff";
  // Set once the first-run Companion opt-in has been answered, so the question is
  // asked exactly once (on the disclaimer splash of a fresh install).
  const COMPANION_CHOICE_KEY = "fieldnotes.companionChoiceMade";
  let vaultFolder = $state("");
  let obsBusy = $state(false);
  let obsErr = $state<string | null>(null);
  let obsMsg = $state<string | null>(null);

  let experiences = $state<ExperienceSummary[]>([]);
  /** Doses from any entry that may still be active (quicklog.ts `recentDoses`). */
  let activeDoses = $state<Dose[]>([]);
  /** A minute clock for "Likely still active" outside the live view. */
  let minuteNow = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (minuteNow = Date.now()), 60_000);
    return () => clearInterval(t);
  });
  /** The open entry's doses (fresh right after logging) with everything else recent. */
  const arcDoses = $derived.by(() => {
    const own = liveSession && selected ? selected.doses : [];
    return [...own, ...activeDoses.filter((d) => !own.some((o) => o.id === d.id))];
  });
  /** Fill the combination check with what's still active, ready for one more name. */
  function checkActive(names: string[]) {
    comboText = names.join(", ") + ", ";
    comboResult = null;
    goTab("substances");
  }
  let substances = $state<Substance[]>([]);
  let classesVocab = $state<string[]>([]);
  let usage = $state<SubstanceUsage[]>([]);
  let selected = $state<ExperienceDetail | null>(null);
  let lastWarnings = $state<Warning[]>([]);

  // new-experience form
  let neTitle = $state("");
  let neIntention = $state("");
  let neSetting = $state("");
  let neStart = $state("");
  // "This already happened" — when set, the session is created already ended
  // (with `neEnd`) so it reads as history rather than an ongoing session.
  let nePast = $state(false);
  let neEnd = $state("");
  let showNewExp = $state(false);
  // plain note (kind: "note") — title, body, date; nothing else
  let nnTitle = $state("");
  let nnBody = $state("");
  let nnDate = $state("");
  let showNewNote = $state(false);

  // Quick log — a dose and a time, without a session around it. Same path the
  // phone uses ($lib/quicklog.ts); most of what people want to record is this
  // and not a trip report, and it stays open to be written up later.
  let showQuickLog = $state(false);
  let qlSub = $state("");
  let qlAmt = $state("");
  let qlUnit = $state("mg");
  let qlDetail = $state<DoseDetail>({});
  let qlRoute = $state("oral");
  let qlWhen = $state("");
  let qlWarnings = $state<Warning[]>([]);
  let qlSaved = $state<{ id: number; title: string; at: string } | null>(null);
  /** The substance just logged in each dose form, for KindQuestion; `n` asks again. */
  let qlAsk = $state({ s: "", n: 0 });
  let dAsk = $state({ s: "", n: 0 });
  let qAsk = $state({ s: "", n: 0 });
  let qlInto = $state<number | null>(null);
  const qlRecents = $derived(recentSubstances(experiences));

  // dose form
  let dSubstance = $state("");
  let dAmount = $state("");
  let dUnit = $state("mg");
  let dDetail = $state<DoseDetail>({});
  let dRoute = $state("oral");
  let dTime = $state("");

  // DoseWiki reference data
  let dRef = $state<PwInfo | null>(null); // reference for the dose being logged

  // Knowledge corpus — DoseWiki prose, searched offline. Reference reading only:
  // doses and interaction verdicts come from the deterministic layers, not from here.
  let kbStat = $state<KnowledgeStatus | null>(null);
  let kbQuery = $state("");
  let kbHits = $state<KnowledgeHit[] | null>(null);
  let kbBusy = $state(false);
  // Reading one substance whole, rather than as excerpts. `kbOpen` is the entry
  // on screen; `kbDose` is its deterministic dose data, shown alongside the prose
  // so the exact numbers sit next to the discursive text instead of a tab away.
  let kbEntries = $state<KnowledgeEntry[]>([]);
  let kbOpen = $state<{ title: string; slug: string; sections: KnowledgeHit[] } | null>(null);
  let kbDose = $state<PwInfo | null>(null);
  let kbBrowse = $state("");
  let kbShowAll = $state(false);

  // Upstream contribution drafts. The consent gate is the preview: a draft is only
  // ever built for the user to read, and only ever leaves the app as a file they
  // saved themselves. Nothing here talks to the network.
  let contribCands = $state<ContributionCandidate[]>([]);
  let contribDraft = $state<ContributionDraft | null>(null);
  let contribMsg = $state<string | null>(null);

  // Device access (the portal) — off by default, and it stays off until the user says otherwise.
  let portal = $state<PortalStatus>({ running: false, port: null, paired: false });
  let devices = $state<DeviceInfo[]>([]);
  let pairName = $state("");
  // The device just paired on this screen, with its token — shown once, then gone.
  let pairing = $state<PairResult | null>(null);
  let pairedId = $state<number | null>(null);
  let pairLinkCopied = $state(false);
  // Other people on this server (people.rs). Names, device counts and locked or
  // unlocked: never anything from inside their journals.
  let people = $state<PersonInfo[]>([]);
  let personName = $state("");
  let personErr = $state<string | null>(null);
  // A device just paired for someone else, shown once like `pairing` above.
  let personPairing = $state<{ who: PersonInfo; pair: PairResult } | null>(null);
  let personQrSvg = $state<string | null>(null);
  let personLinkCopied = $state(false);
  let removingPerson = $state<number | null>(null);
  let removeTyped = $state("");
  // Serving for your other devices: start with the app, open at login, and the
  // opt-in keychain password so a reboot doesn't leave everything locked out.
  let sprefs = $state<ServerPrefs>({ serve_on_launch: false, served_https: null, builtin_tailnet: false, tailscale_app: false, phone_can_update: false, discreet_available: false, menu_bar: false });
  let loginStart = $state(false);
  let kc = $state<KeychainStatus>({ applicable: false, remembered: false });
  let kcPass = $state("");
  let kcErr = $state<string | null>(null);
  let kcBusy = $state(false);

  // Another computer as this one's journal (the client half). Everything that
  // touches the journal goes there while connected; see `remote.rs`.
  let remote = $state<RemoteStatus>({
    connected: false, server: null, server_name: null, online: false, pending: 0, failed: [], unpaired: false,
    local_unsynced: 0,
  });
  let remoteLink = $state("");
  let remoteBusy = $state(false);
  let remoteErr = $state<string | null>(null);
  const serverName = $derived(remote.server_name ?? "your server");
  let portalQrSvg = $state<string | null>(null);
  let ts = $state<TailscaleStatus | null>(null);
  let portalErr = $state<string | null>(null);
  // The pairing QR carries the bearer token, so it is hidden until asked for —
  // it should not be sitting on screen behind you while you're screen-sharing.
  let showQr = $state(false);
  let serving = $state(false);
  // Not necessarily port 443 — if something else on this machine already holds it we
  // publish alongside on another, and the phone needs to be told which.
  let tailscaleUrl = $derived(
    ts?.host && portal.port
      ? `https://${ts.host}${ts.https_port && ts.https_port !== 443 ? `:${ts.https_port}` : ""}/m`
      : null,
  );

  // import-from-text state
  let showImport = $state(false);
  let importText = $state("");
  let importBusy = $state(false);
  let importErr = $state<string | null>(null);
  let importParsed = $state<ParsedExperience | null>(null);
  let importTitle = $state("");
  let importStart = $state("");

  // edit state
  let editExp = $state(false);
  let eTitle = $state("");
  let eIntention = $state("");
  let eSetting = $state("");
  let eNotes = $state("");
  let eRating = $state("");
  let eStart = $state("");
  let editingDoseId = $state<number | null>(null);
  let edSub = $state("");
  let edAmt = $state("");
  let edUnit = $state("mg");
  let edDetail = $state<DoseDetail>({});
  let edRoute = $state("");
  let edTime = $state("");

  // timeline form
  let tNote = $state("");
  let tMood = $state("");
  let tIntensity = $state("");
  let editingTimelineId = $state<number | null>(null);
  let etNote = $state("");
  let etMood = $state("");
  let etIntensity = $state("");
  let etTime = $state("");

  // new-substance form
  let nsName = $state("");
  let nsCategory = $state("");
  let nsClasses = $state<string[]>([]);
  let nsDose = $state("");
  let nsNotes = $state("");

  // local AI (Ollama) — shared setup used by Companion & Import
  let ai = $state<AiStatus | null>(null);
  let aiModel = $state("");
  let aiRecommended = $state<[string, string][]>([]);
  let aiPullTag = $state("");
  let aiPreferred = $state("");
  /**
   * Companion turned off by choice — for machines where a local model is more
   * frustration than help. Nothing safety-critical depends on it: the interaction
   * checker, crisis scan and dose reference are all deterministic and keep working.
   * The switch lives in Settings, not in the Companion tab it hides.
   */
  // Off unless chosen (2026-10-02): a fresh install has nothing stored and starts
  // with the Companion off. Every install that has run before has "0" or "1"
  // saved (the effect below writes it on each launch), so nobody who already has
  // the Companion loses it. Read here, not in onMount, so the save effect can
  // never write the new default over a stored choice.
  let companionOff = $state((() => {
    try { return localStorage.getItem(COMPANION_OFF_KEY) !== "0"; } catch { return true; }
  })());
  /** Whether the first-run Companion opt-in has been answered. Defaults true so a
   * returning user is never re-asked; onMount sets it false for a fresh install. */
  let companionChoiceMade = $state(true);
  /** Dismissed for this session only — no localStorage, so it returns next launch. */
  let upgradeDismissed = $state(false);
  let aiLog = $state<string[]>([]);
  let aiBusy = $state(false);
  let aiErr = $state<string | null>(null);
  let showModels = $state(false); // "manage models" panel toggle
  const aiReady = $derived(!!ai && ai.running && ai.models.length > 0);

  // app self-update
  let update = $state<Update | null>(null);
  let updateBusy = $state(false);
  let updateMsg = $state("");
  let updateDismissed = $state(false);
  // Re-check on a timer so a machine that stays open for days still learns about
  // a release without being restarted. Six hours is often enough to catch a
  // release the same day without hammering GitHub.
  let updateTimer: ReturnType<typeof setInterval> | null = null;
  const UPDATE_CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;

  // companion
  let cMessages = $state<ChatMsg[]>([]);
  let cInput = $state("");
  let cSending = $state(false);
  let cShareSession = $state(true);
  // null = the current (newest) session; a number = a specific past session.
  let cSessionChoice = $state<number | null>(null);
  let cActions = $state<string[]>([]); // journal actions the companion took this reply

  // support style (session intake) — honored by the companion
  const SUPPORT_STYLES = [
    "Mostly just listen",
    "Help me stay grounded",
    "Talk me through the hard parts",
    "Stay quiet unless I reach out",
    "Practical reminders (water, rest, breathing)",
  ];
  let supportStyle = $state("");

  // deterministic crisis layer (independent of the model)
  let crisis = $state<CrisisResult | null>(null);
  // Set when the person accepts an `offer`-mode banner's invitation to see options.
  let crisisResourcesShown = $state(false);
  let showHelp = $state(false); // emergency-resources / panic screen
  let helpResources = $state<CrisisResource[]>([]);
  // Split the panic list: the people you can reach in person (no phone number)
  // come first, then the reassurance-about-calling note, then the phone lines it
  // actually refers to.
  const helpInPerson = $derived(helpResources.filter((r) => !r.contact));
  const helpCallable = $derived(helpResources.filter((r) => r.contact));

  // live session mode
  let liveSession = $state(false);
  // Dim (red) night theme for the live screen, as on the phone: a sitter glancing
  // at a laptop in a dark room keeps their dark adaptation. Remembered per device,
  // and only applied while the live screen is up.
  const NIGHT_KEY = "fieldnotes.night";
  let night = $state((() => { try { return localStorage.getItem(NIGHT_KEY) === "1"; } catch { return false; } })());
  function toggleNight() {
    night = !night;
    try { localStorage.setItem(NIGHT_KEY, night ? "1" : "0"); } catch {}
  }
  $effect(() => {
    const root = document.documentElement;
    if (liveSession && night) root.dataset.theme = "night";
    else delete root.dataset.theme;
  });
  let lsNow = $state(Date.now());
  let lsTimer: ReturnType<typeof setInterval> | null = null;
  // one-tap logging inside the live session (distinct from the journal's quick
  // log, which makes a whole entry — see `submitQuickLog`)
  let qSub = $state("");
  let qAmt = $state("");
  let qUnit = $state("mg");
  let qDetail = $state<DoseDetail>({});
  let qRoute = $state("oral");
  let qNote = $state("");
  let lsNote = $state("");

  const nowIso = () => new Date().toISOString();
  const fmtDate = (iso: string) => new Date(iso).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
  // The journal list: a date column, and a heading at each new month.
  const shortDate = (iso: string) => new Date(iso).toLocaleDateString(undefined, { month: "short", day: "numeric" });
  const monthOf = (iso: string) => new Date(iso).toLocaleDateString(undefined, { month: "long", year: "numeric" });
  const TAB_TITLE: Record<string, string> = { journal: "Journal", stats: "Stats", bysub: "Stats", substances: "Check", companion: "Talk", data: "Settings" };
  const fmtTime = (iso: string) => new Date(iso).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });

  /** T-zero for the open experience: the *first dose*, not the session start.
   *  Sessions often get opened well before anything is taken, and "t+" is only
   *  meaningful counted from ingestion — that's the clock a comedown, a redose
   *  window, or a peak is actually measured against. Null until a dose exists,
   *  which is the honest answer: there is no t-zero yet. */
  const sessionT0 = $derived.by(() => {
    const doses = selected?.doses ?? [];
    if (!doses.length) return null;
    return doses.reduce(
      (earliest, d) => (new Date(d.taken_at) < new Date(earliest) ? d.taken_at : earliest),
      doses[0].taken_at,
    );
  });

  /** `t+1:20` — hours and minutes since the first dose. Negative for anything
   *  logged before it (backdated notes, a session opened early), shown with a
   *  minus rather than clamped to zero, because "20 minutes before dosing" is
   *  real information about a timeline. */
  function relTime(iso: string, t0: string | null): string {
    if (!t0) return "";
    const ms = new Date(iso).getTime() - new Date(t0).getTime();
    const mins = Math.floor(Math.abs(ms) / 60000);
    return `t${ms < 0 ? "−" : "+"}${Math.floor(mins / 60)}:${String(mins % 60).padStart(2, "0")}`;
  }

  // <input type="datetime-local"> <-> ISO helpers (local time)
  const localOffset = (d: Date) => new Date(d.getTime() - d.getTimezoneOffset() * 60000);
  const nowLocalInput = () => localOffset(new Date()).toISOString().slice(0, 16);
  const isoToLocalInput = (iso: string) => localOffset(new Date(iso)).toISOString().slice(0, 16);
  const localInputToIso = (local: string) => (local ? new Date(local).toISOString() : nowIso());

  onMount(() => {
    // A phone browsing to the server's root gets the desktop page, which needs
    // Tauri; its page is /m (and that's what works offline).
    if (!inTauri()) {
      location.replace(`/m${location.hash}`);
      return;
    }
    interactionClasses().then((c) => (classesVocab = c));
    checkForUpdate();
    updateTimer = setInterval(checkForUpdate, UPDATE_CHECK_INTERVAL_MS);
    getVersion().then((v) => (appVersion = v)).catch(() => {});
    discreetAvailable().then((v) => (discreet.available = v)).catch(() => {});
    dontShowDisclaimer = localStorage.getItem(HIDE_DISCLAIMER_KEY) === "1";
    companionChoiceMade = localStorage.getItem(COMPANION_CHOICE_KEY) === "1";
    vaultFolder = localStorage.getItem(VAULT_KEY) ?? "";
    loadDbStatus();
    const un = listen<string>("ai-progress", (e) => {
      aiLog = [...aiLog.slice(-200), e.payload];
    });
    // The backend fires this the first time a phone uses the token, so the answer to
    // "did the scan work?" arrives on its own rather than being hunted for.
    const unPaired = listen<number>("portal-paired", async (e) => {
      portal = { ...portal, paired: true };
      pairedId = e.payload;
      devices = await portalDevices();
    });
    // The backend's view of the server connection: online or not, and what's still
    // waiting to be sent. When entries land, reload so their temporary numbers give
    // way to the server's.
    const unRemote = listen<RemoteStatus>("remote-status", async (e) => {
      const was = remote;
      remote = e.payload;
      setRemoteMode(remote.connected);
      if (acknowledged && remote.connected && (was.pending !== remote.pending || was.online !== remote.online)) {
        await loadJournal();
        await refreshSelected().catch(() => {});
      }
    });
    return () => {
      un.then((f) => f());
      unPaired.then((f) => f());
      unRemote.then((f) => f());
      if (lsTimer) clearInterval(lsTimer);
      if (updateTimer) clearInterval(updateTimer);
    };
  });

  // Decide the startup screen: a locked encrypted journal shows the unlock
  // prompt; otherwise the disclaimer splash, unless the user opted out of it.
  async function loadDbStatus() {
    // The journal opens in the background at launch, so the window can appear
    // straight away. Wait for it here. An encrypted journal whose keychain lookup
    // is slow gets the unlock screen after a few seconds rather than a wait.
    const t0 = Date.now();
    for (;;) {
      try {
        db = await dbStatus();
      } catch (_) {
        db = { encrypted: false, unlocked: true };
      }
      if (!db.opening || (db.encrypted && Date.now() - t0 > 6000)) break;
      openingSlow = Date.now() - t0 > 6000;
      await new Promise((r) => setTimeout(r, 200));
    }
    openingSlow = false;
    statusLoaded = true;
    if (db.unlocked && dontShowDisclaimer) {
      await enter();
    }
  }

  async function checkForUpdate() {
    // Leave a download/install alone — re-checking mid-flight would swap the
    // object out from under it.
    if (updateBusy) return;
    try {
      const found = await check();
      // If a timer tick turns up a version newer than the one already on screen,
      // surface it even if the user dismissed the previous banner — dismissing
      // v0.10.2 shouldn't hide v0.10.3.
      if (found && found.version !== update?.version) updateDismissed = false;
      update = found;
    } catch (_) {
      // offline, or no published release with an updater manifest yet — ignore
    }
  }

  /** The Settings button. Same check as the timer, but it says what it found,
   *  including when it couldn't reach the update server. */
  let manualCheck = $state<"idle" | "checking" | "current" | "found" | "failed">("idle");
  let manualCheckErr = $state("");
  let appVersion = $state("");
  async function checkForUpdateNow() {
    if (updateBusy) return;
    manualCheck = "checking";
    try { appVersion = await getVersion(); } catch {}
    try {
      const found = await check();
      update = found;
      if (found) {
        updateDismissed = false;
        manualCheck = "found";
      } else {
        manualCheck = "current";
      }
    } catch (e) {
      manualCheckErr = typeof e === "string" ? e : String(e);
      manualCheck = "failed";
    }
  }

  async function installUpdate() {
    if (!update) return;
    updateBusy = true;
    updateMsg = "Downloading…";
    try {
      await update.downloadAndInstall((e) => {
        if (e.event === "Progress") updateMsg = "Downloading…";
        if (e.event === "Finished") updateMsg = "Installing…";
      });
      updateMsg = "Restarting…";
      await relaunch();
    } catch (e) {
      updateMsg = `Update failed: ${typeof e === "string" ? e : String(e)}`;
      updateBusy = false;
    }
  }

  async function enter() {
    if (dontShowDisclaimer) {
      localStorage.setItem(HIDE_DISCLAIMER_KEY, "1");
    } else {
      localStorage.removeItem(HIDE_DISCLAIMER_KEY);
    }
    // Record that the first-run Companion opt-in has now been answered. The
    // `companionOff` value it set is already persisted by its own effect; this
    // just stops the question from being asked again.
    if (!companionChoiceMade) {
      companionChoiceMade = true;
      localStorage.setItem(COMPANION_CHOICE_KEY, "1");
    }
    acknowledged = true;
    await loadRemote();
    await Promise.all([loadJournal(), loadSubstances()]);
  }

  /** Which journal are we talking to? Must run before anything reads the journal —
   *  the answer lives inside the (now unlocked) local database. */
  async function loadRemote() {
    try {
      remote = await remoteStatus();
    } catch (_) {
      // Leave it as it was — local.
    }
    setRemoteMode(remote.connected);
  }

  async function doUnlock() {
    unlockErr = null;
    unlockBusy = true;
    try {
      await unlockDb(unlockPass);
      unlockPass = "";
      db = await dbStatus();
      // Unlocking implies the user knows the app; skip straight past the splash.
      await enter();
    } catch (e) {
      unlockErr = typeof e === "string" ? e : String(e);
    } finally {
      unlockBusy = false;
    }
  }

  function secReset() {
    secErr = null;
    secMsg = null;
  }

  async function doEnableEncryption() {
    secReset();
    if (encNewPass.length < 1) return (secErr = "Choose a password.");
    if (encNewPass !== encNewPass2) return (secErr = "The passwords don't match.");
    secBusy = true;
    try {
      await enableEncryption(encNewPass);
      encNewPass = encNewPass2 = "";
      db = await dbStatus();
      secMsg = "Encryption is on. You'll need this password each time you open the app.";
    } catch (e) {
      secErr = typeof e === "string" ? e : String(e);
    } finally {
      secBusy = false;
    }
  }

  async function doDisableEncryption() {
    secReset();
    if (!encDisablePass) return (secErr = "Enter your current password.");
    secBusy = true;
    try {
      await disableEncryption(encDisablePass);
      encDisablePass = "";
      db = await dbStatus();
      secMsg = "Encryption is off. The journal is now stored unencrypted.";
    } catch (e) {
      secErr = typeof e === "string" ? e : String(e);
    } finally {
      secBusy = false;
    }
  }

  async function doChangePassphrase() {
    secReset();
    if (!chgCurrent) return (secErr = "Enter your current password.");
    if (chgNew.length < 1) return (secErr = "Choose a new password.");
    if (chgNew !== chgNew2) return (secErr = "The new passwords don't match.");
    secBusy = true;
    try {
      await changePassphrase(chgCurrent, chgNew);
      chgCurrent = chgNew = chgNew2 = "";
      secMsg = "Password changed.";
    } catch (e) {
      secErr = typeof e === "string" ? e : String(e);
    } finally {
      secBusy = false;
    }
  }

  async function doExportBackup() {
    secReset();
    // When the journal is plaintext, allow encrypting just the backup file.
    let password: string | null = null;
    if (!db.encrypted && bkEncrypt) {
      if (bkPassword.length < 1) return (secErr = "Choose a password for the backup.");
      if (bkPassword !== bkPassword2) return (secErr = "The backup passwords don't match.");
      password = bkPassword;
    }
    try {
      const path = await save({
        title: "Save journal backup",
        defaultPath: `field-notes-backup-${new Date().toISOString().slice(0, 10)}.db`,
        filters: [{ name: "Field Notes journal", extensions: ["db"] }],
      });
      if (!path) return;
      secBusy = true;
      await exportBackup(path, password);
      bkPassword = bkPassword2 = "";
      secMsg =
        db.encrypted || password
          ? "Backup written — it's encrypted, so you'll need its password to restore or open it."
          : "Backup written (unencrypted — keep it somewhere safe).";
    } catch (e) {
      secErr = typeof e === "string" ? e : String(e);
    } finally {
      secBusy = false;
    }
  }

  async function chooseVaultFolder() {
    obsErr = obsMsg = null;
    try {
      const path = await openDialog({ title: "Choose a folder in your Obsidian vault", directory: true, multiple: false });
      if (!path || typeof path !== "string") return;
      vaultFolder = path;
      localStorage.setItem(VAULT_KEY, path);
    } catch (e) {
      obsErr = typeof e === "string" ? e : String(e);
    }
  }

  async function doObsidianExport() {
    obsErr = obsMsg = null;
    if (!vaultFolder) return (obsErr = "Choose a vault folder first.");
    obsBusy = true;
    try {
      const r = await obsidianExport(vaultFolder);
      obsMsg = `Exported ${r.written} note${r.written === 1 ? "" : "s"} to your vault.`;
    } catch (e) {
      obsErr = typeof e === "string" ? e : String(e);
    } finally {
      obsBusy = false;
    }
  }

  async function doObsidianImport() {
    obsErr = obsMsg = null;
    if (!vaultFolder) return (obsErr = "Choose a vault folder first.");
    if (!confirm("Importing pulls entries from the vault into this journal. For any entry already here, the vault's version wins. Continue?")) return;
    obsBusy = true;
    try {
      const r = await obsidianImport(vaultFolder);
      obsMsg = `Imported from vault — ${r.created} new, ${r.updated} updated, ${r.skipped} skipped.`;
      selected = null;
      await Promise.all([loadJournal(), loadSubstances(), loadUsage()]);
    } catch (e) {
      obsErr = typeof e === "string" ? e : String(e);
    } finally {
      obsBusy = false;
    }
  }

  async function doImportBackup() {
    secReset();
    if (!confirm("Importing a backup replaces your current journal on this device. Continue?")) return;
    try {
      const path = await openDialog({
        title: "Choose a journal backup to restore",
        multiple: false,
        directory: false,
        filters: [{ name: "Field Notes journal", extensions: ["db"] }],
      });
      if (!path || typeof path !== "string") return;
      secBusy = true;
      await importBackup(path);
      db = await dbStatus();
      if (db.unlocked) {
        secMsg = "Backup restored.";
        selected = null;
        await Promise.all([loadJournal(), loadSubstances(), loadUsage()]);
      } else {
        // Imported an encrypted journal — send the user back to the unlock gate.
        secMsg = null;
        acknowledged = false;
      }
    } catch (e) {
      secErr = typeof e === "string" ? e : String(e);
    } finally {
      secBusy = false;
    }
  }

  // ---- erase all data / uninstall ----
  async function showDataFolder() {
    secReset();
    try {
      await revealDataDir();
    } catch (e) {
      secErr = typeof e === "string" ? e : String(e);
    }
  }

  async function eraseAllData() {
    secReset();
    const others = people.length
      ? `\n\nIt also deletes the journals of everyone else on this server (${people.map((p) => p.name).join(", ")}) and un-pairs their devices.`
      : "";
    if (!confirm(`Erase ALL Field Notes data on this device?\n\nThis permanently deletes every entry, dose, moment, substance, and setting, and turns off encryption. It cannot be undone.${others}`)) return;
    if (!confirm("Last chance — there is no recovery. Really erase everything?")) return;
    secBusy = true;
    try {
      await wipeAllData();
      // Clear locally-stored preferences too.
      localStorage.removeItem(HIDE_DISCLAIMER_KEY);
      localStorage.removeItem(VAULT_KEY);
      localStorage.removeItem("fn.model");
      localStorage.removeItem(COMPANION_OFF_KEY);
      // Reset in-memory state to a fresh journal.
      selected = null;
      experiences = [];
      substances = [];
      usage = [];
      cMessages = [];
      supportStyle = "";
      vaultFolder = "";
      dontShowDisclaimer = false;
      db = await dbStatus();
      await Promise.all([loadJournal(), loadSubstances()]);
      secMsg = "All data erased. You can keep using a fresh journal, or quit and remove the app.";
    } catch (e) {
      secErr = typeof e === "string" ? e : String(e);
    } finally {
      secBusy = false;
    }
  }

  async function quitApp() {
    await exit(0);
  }

  async function loadJournal() {
    experiences = await listExperiences();
    recentDoses(experiences, Date.now(), selected ? [selected] : [])
      .then((d) => (activeDoses = d))
      .catch(() => {});
  }
  async function loadSubstances() {
    substances = await listSubstances();
    forgetSubstanceNames();
  }
  async function loadUsage() {
    usage = await usageBySubstance();
  }

  /** What a fresh dose-time field should default to. For a session that already
   *  ended (a past trip being written up), new doses belong to when it happened,
   *  not now — so seed from its start. Live sessions and notes default to now. */
  function defaultDoseTime(): string {
    return selected?.kind === "session" && selected.ended_at
      ? isoToLocalInput(selected.started_at)
      : nowLocalInput();
  }

  async function openExperience(id: number) {
    lastWarnings = [];
    editExp = false;
    editingDoseId = null;
    dRef = null;
    exportErr = exportMsg = null;
    selected = await getExperience(id);
    dTime = defaultDoseTime();
  }

  function openNewExp() {
    showNewExp = !showNewExp;
    if (showNewExp) showNewNote = showQuickLog = false;
    if (showNewExp && !neStart) neStart = nowLocalInput();
  }

  // When "this already happened" is ticked, seed the end time from the start so
  // the field isn't empty — the person adjusts both to the real dates.
  function onPastToggle() {
    if (nePast && !neEnd) neEnd = neStart;
  }

  function openNewNote() {
    showNewNote = !showNewNote;
    if (showNewNote) showNewExp = showQuickLog = false;
    if (showNewNote && !nnDate) nnDate = nowLocalInput();
  }

  async function submitNewNote() {
    const e = await createExperience({
      kind: "note",
      title: nnTitle || "Untitled note",
      started_at: localInputToIso(nnDate),
    });
    // create doesn't take a body — write it in the same breath.
    if (nnBody.trim()) {
      await updateExperience(e.id, {
        title: e.title,
        notes: nnBody,
        rating: null,
        started_at: e.started_at,
        ended_at: null,
      });
    }
    nnTitle = nnBody = nnDate = "";
    showNewNote = false;
    await loadJournal();
    await openExperience(e.id);
  }

  // ---- import a past experience from pasted text ----
  async function openImport() {
    showImport = !showImport;
    showPaste = false;
    if (showImport) {
      importParsed = null;
      importErr = null;
      await loadAi();
    }
  }

  async function runParse() {
    if (!importText.trim() || !aiModel || importBusy) return;
    importBusy = true;
    importErr = null;
    importParsed = null;
    try {
      const p = await parseExperience(aiModel, importText);
      importParsed = p;
      importTitle = p.title || "Imported experience";
      importStart = p.started_at ? isoToLocalInput(p.started_at) : nowLocalInput();
    } catch (e) {
      importErr = typeof e === "string" ? e : String(e);
    } finally {
      importBusy = false;
    }
  }

  async function confirmImport() {
    if (!importParsed) return;
    const startIso = localInputToIso(importStart);
    // The backend rebases a T+ report's times onto this start (normalize_import).
    const exp = await importExperience({ ...importParsed, title: importTitle }, startIso);
    importParsed = null;
    importText = "";
    showImport = false;
    await loadJournal();
    await openExperience(exp.id);
  }

  // ---- quick log ----
  function openQuickLog() {
    showQuickLog = !showQuickLog;
    if (showQuickLog) showNewExp = showNewNote = false;
    if (showQuickLog && !qlWhen) qlWhen = nowLocalInput();
  }

  /** The names as they'll be saved ("acid" → LSD; NameHint). */
  let qlSubAs = $state("");
  let dSubstanceAs = $state("");

  async function submitQuickLog() {
    if (!qlSub.trim()) return;
    const at = localInputToIso(qlWhen);
    const res = await quickLog({
      substance: qlSubAs || qlSub.trim(),
      amount: qlAmt ? parseFloat(qlAmt) : null,
      unit: qlUnit,
      route: qlRoute,
      at,
      intoId: qlInto,
      detail: qlDetail,
    });
    rememberDoseShape(qlSub, { unit: qlUnit, route: qlRoute });
    qlWarnings = res.warnings;
    qlSaved = { id: res.id, title: res.title, at };
    qlAsk = { s: qlSubAs || qlSub.trim(), n: qlAsk.n + 1 };
    qlInto = null;
    qlSub = qlAmt = "";
    qlDetail = {};
    qlWhen = nowLocalInput();
    await loadJournal();
  }

  /** A second substance goes into the entry we just made rather than beside it:
   *  that's how the evening reads back, and it's what lets `log_dose` compare
   *  the two against each other. */
  function quickLogAnother() {
    if (!qlSaved) return;
    qlInto = qlSaved.id;
    qlWhen = isoToLocalInput(qlSaved.at);
    qlSaved = null;
  }

  function pickRecentSubstance(name: string) {
    qlSub = name;
    applyRememberedShape();
  }

  /** Unit and route follow the substance, remembered from last time — see
   *  `recallDoseShape`. Nothing about the journal changes; it's a default. */
  function applyRememberedShape() {
    const shape = recallDoseShape(qlSub);
    if (shape) {
      qlUnit = shape.unit;
      qlRoute = shape.route;
    } else {
      qlUnit = defaultUnitFor(qlSub, qlRoute) ?? qlUnit;
    }
  }

  /** "Logged now, written up later" only works if later is one click away: open
   *  the entry with its write-up field focused and waiting. */
  async function addNotesTo(id: number) {
    qlSaved = null;
    showQuickLog = false;
    await openExperience(id);
    startEditExp();
    await new Promise((r) => setTimeout(r, 0));
    const el = document.getElementById("exp-writeup");
    el?.scrollIntoView({ block: "center" });
    el?.focus();
  }

  async function submitNewExperience() {
    const startIso = localInputToIso(neStart);
    const e = await createExperience({
      // Blank stays blank rather than being stamped with the literal string
      // "Untitled experience": an empty title is what lets the first logged dose
      // name the session (`name_after_first_dose` in db.rs). The journal already
      // falls back to "Untitled experience" for display.
      title: neTitle.trim(),
      intention: neIntention,
      setting: neSetting,
      started_at: startIso,
    });
    // A past experience is over the moment it's logged — mark it ended so it's
    // history, not a live session (no "ongoing", no live-session controls). If
    // the end time was left blank, fall back to the start rather than to now.
    if (nePast) {
      await endExperience(e.id, neEnd ? localInputToIso(neEnd) : startIso, null, "");
    }
    neTitle = neIntention = neSetting = neStart = neEnd = "";
    nePast = false;
    showNewExp = false;
    await loadJournal();
    await openExperience(e.id);
  }

  async function submitDose() {
    if (!selected || !dSubstance.trim()) return;
    const res = await logDose({
      experience_id: selected.id,
      substance_name: dSubstanceAs || dSubstance.trim(),
      amount: dAmount ? parseFloat(dAmount) : null,
      unit: dUnit,
      route: dRoute,
      taken_at: localInputToIso(dTime),
      ...dDetail,
    });
    lastWarnings = res.warnings;
    dAsk = { s: res.dose.substance_name, n: dAsk.n + 1 };
    dSubstance = dAmount = "";
    dDetail = {};
    dTime = defaultDoseTime();
    await openExperienceKeepWarnings(selected.id);
    await loadJournal();
  }

  // ---- edit / delete ----
  function startEditExp() {
    if (!selected) return;
    eTitle = selected.title;
    eIntention = selected.intention;
    eSetting = selected.setting;
    eNotes = selected.notes;
    eRating = selected.rating != null ? String(selected.rating) : "";
    eStart = isoToLocalInput(selected.started_at);
    editExp = true;
  }

  async function saveExp() {
    if (!selected) return;
    await updateExperience(selected.id, {
      title: eTitle,
      intention: eIntention,
      setting: eSetting,
      notes: eNotes,
      rating: eRating ? parseInt(eRating) : null,
      started_at: localInputToIso(eStart),
      ended_at: selected.ended_at,
    });
    editExp = false;
    await openExperienceKeepWarnings(selected.id);
    await loadJournal();
  }

  // single-entry markdown export
  let exportErr = $state<string | null>(null);
  let exportMsg = $state<string | null>(null);

  async function exportEntry() {
    if (!selected) return;
    exportErr = exportMsg = null;
    try {
      const note = await exportExperienceMarkdown(selected.id);
      const path = await save({
        title: "Export this entry",
        defaultPath: note.filename,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });
      if (!path) return;
      // Connected to a server: it rendered the note; write it here.
      if (remote.connected) await saveMarkdownFile(path, note.markdown);
      else await exportExperienceFile(selected.id, path);
      exportMsg = "Entry exported as Markdown.";
    } catch (e) {
      exportErr = typeof e === "string" ? e : String(e);
    }
  }

  // single-entry PDF (pdf.ts), to share with someone
  let pdfOpen = $state(false);
  let pdfParts = $state<PdfParts>({ ...ALL_PARTS });

  async function exportPdf() {
    if (!selected) return;
    exportErr = exportMsg = null;
    try {
      const path = await save({
        title: "Save this entry as a PDF",
        defaultPath: pdfFilename(selected),
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });
      if (!path) return;
      await savePdfFile(path.toLowerCase().endsWith(".pdf") ? path : `${path}.pdf`, experiencePdf(selected, pdfParts));
      exportMsg = "Saved as a PDF.";
      pdfOpen = false;
    } catch (e) {
      exportErr = typeof e === "string" ? e : String(e);
    }
  }

  /** A reflection is never owed: mark this one as fine without a write-up. */
  async function skipWriteup(skipped: boolean) {
    if (!selected) return;
    await setWriteupSkipped(selected.id, skipped);
    await refreshSelected();
    await loadJournal();
  }

  async function delExp() {
    const msg =
      selected?.kind === "note"
        ? "Delete this note? This cannot be undone."
        : "Delete this experience and all its doses? This cannot be undone.";
    if (!selected || !confirm(msg)) return;
    await deleteExperience(selected.id);
    selected = null;
    await loadJournal();
  }

  async function delDose(id: number) {
    if (!selected) return;
    await deleteDose(id);
    await openExperienceKeepWarnings(selected.id);
    await loadJournal();
  }

  async function delTimeline(id: number) {
    if (!selected) return;
    await deleteTimelineEvent(id);
    await openExperienceKeepWarnings(selected.id);
  }

  function startEditTimeline(t: TimelineEvent) {
    editingTimelineId = t.id;
    etNote = t.note;
    etMood = t.mood;
    etIntensity = t.intensity != null ? String(t.intensity) : "";
    etTime = isoToLocalInput(t.at);
  }

  async function saveTimeline() {
    if (!selected || editingTimelineId == null) return;
    await updateTimelineEvent(editingTimelineId, {
      at: localInputToIso(etTime),
      note: etNote.trim(),
      mood: etMood,
      intensity: etIntensity ? parseInt(etIntensity) : null,
    });
    editingTimelineId = null;
    await openExperienceKeepWarnings(selected.id);
  }

  function startEditDose(d: Dose) {
    editingDoseId = d.id;
    edSub = d.substance_name;
    edAmt = d.amount != null ? String(d.amount) : "";
    edUnit = d.unit;
    edRoute = d.route;
    edTime = isoToLocalInput(d.taken_at);
    edDetail = detailOf(d);
  }

  async function saveDose() {
    if (!selected || editingDoseId == null) return;
    await updateDose(editingDoseId, {
      substance_name: edSub.trim(),
      amount: edAmt ? parseFloat(edAmt) : null,
      unit: edUnit,
      route: edRoute,
      taken_at: localInputToIso(edTime),
      detail: detailOf(edDetail),
    });
    editingDoseId = null;
    await openExperienceKeepWarnings(selected.id);
    await loadJournal();
  }

  async function delSubstance(id: number) {
    if (!confirm("Delete this substance? Logged doses keep their name but lose the link.")) return;
    await deleteSubstance(id);
    await loadSubstances();
    await loadContrib();
  }

  // reload the detail but preserve the warning banner we just set
  // ---- paste a trip log (deterministic; shared with the phone) ----
  let showPaste = $state(false);
  async function pastedLog(r: { id: number; warnings: Warning[] }) {
    showPaste = false;
    await loadJournal();
    await openExperienceKeepWarnings(r.id);
    lastWarnings = r.warnings;
  }

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
    await loadContrib();
  }

  function toggleClass(c: string) {
    nsClasses = nsClasses.includes(c) ? nsClasses.filter((x) => x !== c) : [...nsClasses, c];
  }

  async function loadAi() {
    ai = await aiStatus();
    if (!aiRecommended.length) aiRecommended = await aiRecommendedModels();
    if (!aiPreferred) aiPreferred = await aiPreferredModel();
    if (!aiPullTag && aiRecommended.length) aiPullTag = aiRecommended[0][0];
    if (ai.running && ai.models.length) {
      const saved = localStorage.getItem("fn.model");
      if (!aiModel || !ai.models.includes(aiModel)) {
        aiModel = saved && ai.models.includes(saved) ? saved : ai.models[0];
      }
      // Pre-load the model now, while they're still reading the intro, so the
      // first message lands at warm speed instead of a cold multi-second load.
      // Once per model per session; best-effort, so a failure changes nothing.
      if (!companionOff && aiModel && aiModel !== warmedModel) {
        warmedModel = aiModel;
        companionWarm(aiModel).catch(() => {});
      }
      checkCompute();
    }
    if (!experiences.length) await loadJournal();
  }
  // The model warm-up already fired for, so we don't re-request it each tab visit.
  let warmedModel = "";

  // ---- compute watcher ----
  // Advisory read of whether this machine can run the chosen model comfortably.
  // Re-checked when the Companion opens, after the model loads, and after each
  // reply, since free memory (and whether the model spilled to CPU) shifts.
  let compute = $state<ComputeStatus | null>(null);
  // Generation speed of the last reply (tokens/sec), fed back into the watcher so
  // it can flag a machine that fits the model in memory but runs it too slowly.
  let lastTps = $state<number | null>(null);
  async function checkCompute() {
    if (companionOff || !aiModel || !ai?.running) {
      compute = null;
      return;
    }
    try {
      compute = await computeStatus(aiModel, lastTps);
    } catch {
      compute = null; // Never let a failed check get in the person's way.
    }
  }
  // A speed reading belongs to the model that produced it; forget it the moment
  // the active model changes (select, download, or upgrade), so a fast old model
  // can't vouch for a slow new one.
  let tpsModel: string | null = null;
  $effect(() => {
    if (aiModel !== tpsModel) {
      tpsModel = aiModel;
      lastTps = null;
    }
  });

  // Preflight the model someone is *about* to download against this machine, so a
  // too-big pick is caught before the multi-GB download rather than after. Estimate
  // only (it isn't installed yet), which is exactly what a preflight needs.
  let pullCompute = $state<ComputeStatus | null>(null);
  $effect(() => {
    const tag = aiPullTag?.trim();
    if (!tag) {
      pullCompute = null;
      return;
    }
    let cancelled = false;
    computeStatus(tag, null)
      .then((s) => {
        if (!cancelled) pullCompute = s;
      })
      .catch(() => {
        if (!cancelled) pullCompute = null;
      });
    return () => {
      cancelled = true;
    };
  });

  // ---- feedback (open a prefilled GitHub issue) ----
  // The repo is public, so "report a bug / request a feature" is a link to a
  // prefilled new-issue page — no server, no telemetry, nothing leaves the
  // journal. GitHub handles identity, and issues collect in one place to work
  // through in batches.
  const FEEDBACK_REPO = "https://github.com/sparkly-quasar/field-notes";
  let fbKind = $state<"bug" | "feature">("bug");
  let fbSummary = $state("");
  let fbDetail = $state("");

  // A quick way in from the header, so reporting a bug isn't buried in Settings.
  let showFeedback = $state(false);
  function openBugReport() {
    fbKind = "bug";
    showFeedback = true;
  }
  // From the modal: open the prefilled issue, then close. (The Settings card
  // calls openFeedback directly and stays put.)
  async function sendFeedback() {
    await openFeedback();
    showFeedback = false;
  }

  async function openFeedback() {
    const isBug = fbKind === "bug";
    let version = "";
    try {
      version = await getVersion();
    } catch {
      // Version is a nicety for triage, not required — send without it.
    }
    const platform = isMac ? "macOS" : isWindows ? "Windows" : "Linux";
    const template = isBug
      ? "**What happened?**\n\n\n**What did you expect instead?**\n\n\n**Steps to reproduce, if you can:**\n1. \n"
      : "**What would you like Field Notes to do?**\n\n\n**When would you reach for it?**\n";
    const detail = fbDetail.trim() || template;
    const body = `${detail}\n\n---\n_Field Notes${version ? ` v${version}` : ""} · ${platform}_`;
    const url =
      `${FEEDBACK_REPO}/issues/new?labels=${isBug ? "bug" : "enhancement"}` +
      `&title=${encodeURIComponent(`${isBug ? "[Bug] " : "[Feature] "}${fbSummary.trim()}`)}` +
      `&body=${encodeURIComponent(body)}`;
    await openUrl(url);
    fbSummary = "";
    fbDetail = "";
  }
  $effect(() => {
    if (aiModel) localStorage.setItem("fn.model", aiModel);
  });
  $effect(() => {
    localStorage.setItem(COMPANION_OFF_KEY, companionOff ? "1" : "0");
    // Push it to the backend so the phone portal sees it too — the phone has its
    // own browser storage and cannot read this preference.
    setCompanionEnabled(!companionOff).catch(() => {});
    // Don't strand someone on a tab that just disappeared from the nav.
    if (companionOff && tab === "companion") tab = "journal";
  });

  async function doInstall() {
    aiBusy = true;
    aiErr = null;
    aiLog = [];
    try {
      await aiInstall();
      await loadAi();
    } catch (e) {
      aiErr = typeof e === "string" ? e : String(e);
    } finally {
      aiBusy = false;
    }
  }
  async function doStart() {
    aiBusy = true;
    aiErr = null;
    try {
      await aiStart();
      await loadAi();
    } catch (e) {
      aiErr = typeof e === "string" ? e : String(e);
    } finally {
      aiBusy = false;
    }
  }
  /**
   * Show the upgrade offer only when it is actionable: Ollama is up, we know the
   * recommended tag, the person is on something else, and they don't already have
   * the new model sitting there (in which case switching is just picking it).
   */
  const upgradeAvailable = $derived(
    !upgradeDismissed &&
      !!ai?.running &&
      !!aiPreferred &&
      !!aiModel &&
      aiModel !== aiPreferred &&
      !(ai?.models ?? []).includes(aiPreferred),
  );

  async function doSwitchModel() {
    const old = aiModel;
    aiBusy = true;
    aiErr = null;
    aiLog = [];
    try {
      aiModel = await aiSwitchModel(old);
      upgradeDismissed = true;
      await loadAi();
    } catch (e) {
      aiErr = typeof e === "string" ? e : String(e);
    } finally {
      aiBusy = false;
    }
  }

  async function doPull() {
    if (!aiPullTag) return;
    aiBusy = true;
    aiErr = null;
    aiLog = [];
    try {
      await aiPull(aiPullTag);
      aiModel = aiPullTag;
      await loadAi();
    } catch (e) {
      aiErr = typeof e === "string" ? e : String(e);
    } finally {
      aiBusy = false;
    }
  }

  // Sessions the companion could be pointed at, newest first. Plain notes are
  // never offered: session context is about doses, and notes are private writing.
  const shareableSessions = $derived(
    experiences
      .filter((e) => e.kind === "session")
      .slice()
      .sort((a, b) => b.started_at.localeCompare(a.started_at)),
  );
  // Which session the companion is aware of. `null` choice means "the current
  // one" — the newest session — so a fresh session is picked up automatically
  // without re-selecting. A specific id lets someone attach a past session, e.g.
  // to talk it through for integration. A dangling id (session deleted) falls
  // back to nothing attached rather than the wrong one.
  const attachedExp = $derived(
    !cShareSession
      ? null
      : cSessionChoice != null
        ? (shareableSessions.find((e) => e.id === cSessionChoice) ?? null)
        : (shareableSessions[0] ?? null),
  );
  const companionExpId = $derived(
    liveSession && selected ? selected.id : attachedExp ? attachedExp.id : null,
  );

  async function refreshSelected() {
    if (selected) selected = await getExperience(selected.id);
  }

  async function sendCompanion() {
    if (!cInput.trim() || !aiModel || cSending) return;
    const text = cInput.trim();
    const history: ChatMsg[] = [...cMessages, { role: "user", content: text }];
    cMessages = history;
    cInput = "";
    cSending = true;
    cActions = [];
    // The deterministic crisis layer runs independently of the model's reply.
    // `history` already ends with `text`, and the backend appends it — so pass the
    // messages *before* it, or one message would count as a repeat of itself.
    crisisScan(text, companionExpId, history.slice(0, -1).filter((m) => m.role === "user").map((m) => m.content))
      .then((r) => { if (r.level !== "none") { crisis = r; crisisResourcesShown = false; } })
      .catch(() => {});
    try {
      const res = await companionChat(aiModel, history, companionExpId, supportStyle || null);
      cMessages = [...cMessages, { role: "assistant", content: res.reply || "…" }];
      cActions = res.actions;
      if (res.tokens_per_sec != null) lastTps = res.tokens_per_sec;
      if (res.journal_changed) {
        await loadJournal();
        await refreshSelected();
      }
    } catch (e) {
      cMessages = [...cMessages, { role: "assistant", content: `⚠️ ${typeof e === "string" ? e : String(e)}` }];
    } finally {
      cSending = false;
      // The model is resident now (or just changed footprint) — re-read the machine.
      checkCompute();
    }
  }

  // ---- crisis / emergency resources ----
  async function openHelp() {
    helpResources = await emergencyResources();
    showHelp = true;
  }

  // ---- live session ----
  function startLiveSession() {
    if (!selected) return;
    liveSession = true;
    lsNow = Date.now();
    lsTimer = setInterval(() => (lsNow = Date.now()), 1000);
  }
  function endLiveSession() {
    liveSession = false;
    if (lsTimer) { clearInterval(lsTimer); lsTimer = null; }
  }
  function elapsedSince(iso: string): string {
    const ms = Math.max(0, lsNow - new Date(iso).getTime());
    const s = Math.floor(ms / 1000);
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    return h > 0 ? `${h}h ${m}m` : `${m}m`;
  }

  async function logIntoSession() {
    if (!selected || !qSub.trim()) return;
    const n = qAmt.trim() === "" ? null : Number(qAmt);
    const res = await logDose({
      experience_id: selected.id,
      substance_name: qSub.trim(),
      amount: n !== null && Number.isFinite(n) ? n : null,
      unit: qUnit || "mg",
      route: qRoute,
      taken_at: nowIso(),
      note: qNote,
      ...qDetail,
    });
    const r = res.dose;
    qAsk = { s: r.substance_name, n: qAsk.n + 1 };
    lastLogged = { id: r.id, label: `${r.substance_name}${r.amount != null ? ` ${describeAmount(r.substance_name, r.amount, r.unit, r)}` : ""}` };
    qSub = ""; qAmt = ""; qNote = "";
    qDetail = {};
    if (res.warnings.length) lastWarnings = res.warnings;
    await refreshSelected();
    await loadJournal();
  }

  // ---- standalone combination check (the Check view) ----
  let comboText = $state("");
  let comboResult = $state<Warning[] | null>(null);
  async function runComboCheck() {
    const names = comboText.split(",").map((x) => x.trim()).filter(Boolean);
    if (names.length < 2) return;
    comboResult = await checkCombo(names);
  }

  /** One-tap states, so a moment never needs typing mid-session. */
  const MOODS = ["Coming up", "Peaking", "Calm", "Anxious", "Nauseous", "Need water", "Coming down"];
  async function quickNote(mood = "") {
    if (!selected || !(lsNote.trim() || mood)) return;
    await addTimelineEvent({ experience_id: selected.id, at: nowIso(), note: lsNote.trim(), mood, intensity: null });
    lsNote = "";
    await refreshSelected();
  }

  /** Undo the dose just logged on the live screen. */
  let lastLogged = $state<{ id: number; label: string } | null>(null);
  async function undoLastLogged() {
    const l = lastLogged;
    if (!l) return;
    lastLogged = null;
    await deleteDose(l.id);
    lastWarnings = [];
    await refreshSelected();
    await loadJournal();
  }

  async function goTab(t: Tab, section?: SettingsSection) {
    tab = t;
    if (section) settingsSection = section;
    selected = null;
    if (t === "bysub" || t === "stats") await loadUsage();
    if (t === "substances") { await loadSubstances(); await loadKbStatus(); await loadContrib(); }
    if (t === "journal") await loadJournal();
    if (t === "companion") await loadAi();
    if (t === "data") { secReset(); db = await dbStatus(); dataDirPath = await dataDir(); await loadPortal(); }
  }

  // ---- DoseWiki reference ----
  async function lookupRef(name: string) {
    dRef = name.trim() ? await pwLookup(name.trim()) : null;
  }

  // ---- Knowledge corpus ----
  async function loadKbStatus() {
    kbStat = await knowledgeStatus();
    if (kbStat.available && !kbEntries.length) kbEntries = await knowledgeEntries();
  }

  /** Open a substance's entry to read whole. Pulls its dose data alongside, so
   *  the exact numbers travel with the prose — but they stay visibly separate,
   *  because only the dose panel is authoritative (see knowledge.rs). */
  async function openKbEntry(slug: string, title: string) {
    kbOpen = { title, slug, sections: [] };
    const [sections, dose] = await Promise.all([knowledgeEntry(slug), pwLookup(title)]);
    // A second click while this was in flight wins; don't clobber it.
    if (kbOpen?.slug !== slug) return;
    kbOpen = { title, slug, sections };
    kbDose = dose;
  }
  function closeKbEntry() {
    kbOpen = null;
    kbDose = null;
  }

  /** The browse list, filtered by the substance-name box. Substring match on the
   *  name only — this is a name picker, not a second search over the prose. */
  const kbBrowseHits = $derived.by(() => {
    const q = kbBrowse.trim().toLowerCase();
    return q ? kbEntries.filter((e) => e.title.toLowerCase().includes(q)) : kbEntries;
  });
  // ---- Devices & server ----
  async function loadPortal() {
    portal = await portalStatus();
    ts = await portalTailscale();
    devices = await portalDevices();
    people = await peopleList().catch(() => []);
    sprefs = await serverPrefs();
    pinStatus = await phonePinStatus().catch(() => null);
    kc = await keychainStatus();
    loginStart = await autostartIsEnabled().catch(() => false);
    await loadRemote();
    if (!portal.running) { portalQrSvg = null; showQr = false; }
  }

  /** The link a device opens. Over the tailnet if Tailscale can carry it there;
   *  otherwise the loopback URL, which only this machine can reach — so the QR is
   *  honest about the portal being this-computer-only until Tailscale is set up. */
  function pairLink(token: string): string | null {
    // Prefer the URL the backend actually published on — a link built from a guessed
    // port sends the device to whatever else is serving there.
    const base = ts?.url ?? tailscaleUrl;
    if (base) return `${base}#t=${token}`;
    if (portal.port) return `http://127.0.0.1:${portal.port}/m#t=${token}`;
    return null;
  }

  // ---- phone setup checklist ----
  // Most people set this up alone, and every step that can go wrong is outside
  // Field Notes (Tailscale, the tailnet's admin page). So: one list, ticked from
  // what's actually true right now, with the fix for the first unticked step.
  type SetupStep = { key: string; label: string; done: boolean };
  // The phone has used its key at least once: the only proof the scan worked.
  const phonePaired = $derived(pairedId != null || devices.some((d) => d.person === 1 && d.last_seen));
  // This computer is on the tailnet and answering there.
  const computerConnected = $derived(!!ts?.signed_in && !!ts?.https_enabled && !!ts?.serving);
  // With the built-in Tailscale, the whole path is four steps, and only the first
  // happens on this computer. Steps 2 and 3 happen on the phone, where we can't
  // see; they tick together with step 4, when the phone first uses its key.
  const builtinSteps = $derived<SetupStep[]>([
    { key: "connect", label: "Connect this computer", done: computerConnected },
    { key: "phone-app", label: "Install Tailscale on your phone", done: phonePaired },
    {
      key: "phone-signin",
      label: ts?.login ? `Sign in on your phone with the same account: ${ts.login}` : "Sign in on your phone with the same account",
      done: phonePaired,
    },
    { key: "scan", label: "Scan the code with your phone's camera", done: phonePaired },
  ]);
  const appSteps = $derived<SetupStep[]>([
    { key: "install", label: "Install Tailscale on this computer", done: !!ts?.installed },
    { key: "signin", label: "Sign in to Tailscale", done: !!ts?.signed_in },
    { key: "https", label: "Turn on HTTPS for your tailnet", done: !!ts?.https_enabled },
    { key: "access", label: "Turn on device access", done: portal.running },
    { key: "publish", label: "Publish to your tailnet", done: !!ts?.serving },
    { key: "pair", label: "Pair your phone", done: devices.some((d) => d.person === 1) },
  ]);
  const setupSteps = $derived(ts?.builtin ? builtinSteps : appSteps);
  const setupNext = $derived(setupSteps.find((st) => !st.done)?.key ?? null);
  // Once this computer is connected, the three phone steps are all live at once:
  // they're done in a row on the phone, not one at a time here.
  const setupActive = (key: string) =>
    ts?.builtin && computerConnected && !phonePaired ? key !== "connect" : key === setupNext;
  let setupOpen = $state(false);
  let setupChecking = $state(false);
  async function recheckSetup() {
    setupChecking = true;
    try { await loadPortal(); } finally { setupChecking = false; }
  }
  // Coming back from Tailscale or its admin page re-checks, so the list ticks
  // itself rather than waiting for "Check again".
  $effect(() => {
    const onFocus = () => { if (tab === "data" && !remote.connected) loadPortal().catch(() => {}); };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  });
  // ---- built-in Tailscale ----
  let connecting = $state(false);
  // The sign-in page is opened once per link, not on every status poll.
  let openedAuthUrl: string | null = null;
  let storeQr = $state<{ ios: string; android: string } | null>(null);
  // Pairing from step 4 shows its code in the step, not in "Pair a device" below.
  let setupPairing = $state(false);

  /** "Connect this computer": device access on, the built-in Tailscale started,
   *  and Tailscale's sign-in page opened in the browser as soon as it exists. */
  async function connectBuiltin() {
    portalErr = null;
    connecting = true;
    openedAuthUrl = null;
    try {
      ts = await tailnetConnect();
      portal = await portalStatus();
    } catch (e) {
      portalErr = e instanceof Error ? e.message : String(e);
      connecting = false;
    }
  }

  // While the built-in Tailscale is getting going (starting, waiting for sign-in,
  // fetching its certificate), follow it closely so each step ticks on its own.
  $effect(() => {
    const following = tab === "data" && settingsSection === "devices" && !!ts?.builtin && portal.running && (!computerConnected || connecting);
    if (!following) return;
    const id = setInterval(async () => {
      try { ts = await portalTailscale(); } catch {}
    }, 1500);
    return () => clearInterval(id);
  });
  $effect(() => {
    const url = ts?.auth_url;
    if (connecting && url && url !== openedAuthUrl) {
      openedAuthUrl = url;
      openUrl(url).catch(() => {});
    }
    if (ts?.signed_in) connecting = false;
  });

  async function showStoreCodes() {
    storeQr = {
      ios: await portalQr("https://apps.apple.com/app/tailscale/id1470499037"),
      android: await portalQr("https://play.google.com/store/apps/details?id=com.tailscale.ipn"),
    };
  }

  /** Step 4: make this phone's key and show it as a code, right in the step. */
  async function pairFromSetup() {
    await doPair();
    if (!pairing) return;
    setupPairing = true;
    await revealQr();
  }

  async function disconnectBuiltin() {
    portalErr = null;
    try { ts = await portalUnserve(); } catch (e) { portalErr = e instanceof Error ? e.message : String(e); }
  }

  async function signOutBuiltin() {
    if (!confirm("Sign this computer out of Tailscale? Your phone can't reach the journal until you connect again. Pairings are kept.")) return;
    portalErr = null;
    try { ts = await tailnetSignOut(); } catch (e) { portalErr = e instanceof Error ? e.message : String(e); }
  }

  async function switchTailscale(useApp: boolean) {
    portalErr = null;
    try {
      ts = await tailnetUseApp(useApp);
      donePairing();
    } catch (e) {
      portalErr = e instanceof Error ? e.message : String(e);
    }
  }

  let pairInput = $state<HTMLInputElement | null>(null);
  function startPairFromChecklist() {
    if (!pairName.trim()) pairName = "Phone";
    pairInput?.scrollIntoView({ behavior: "smooth", block: "center" });
    pairInput?.focus();
  }

  async function togglePortal() {
    portalErr = null;
    try {
      portal = portal.running ? await portalDisable() : await portalEnable();
      ts = await portalTailscale();
      portalQrSvg = null;
      showQr = false;
    } catch (e) {
      portalErr = e instanceof Error ? e.message : String(e);
    }
  }

  async function doPair() {
    portalErr = null;
    try {
      pairing = await portalPair(pairName.trim() || "Phone");
      pairName = "";
      pairedId = null;
      pairLinkCopied = false;
      portalQrSvg = null;
      showQr = false;
      devices = await portalDevices();
    } catch (e) {
      portalErr = e instanceof Error ? e.message : String(e);
    }
  }

  async function revealQr() {
    const target = pairing && pairLink(pairing.token);
    if (!target) return;
    portalQrSvg = await portalQr(target);
    showQr = true;
  }

  async function copyPairLink() {
    const target = pairing && pairLink(pairing.token);
    if (!target) return;
    await navigator.clipboard.writeText(target);
    pairLinkCopied = true;
  }

  /** Finished pairing: drop the token from the screen for good. */
  function donePairing() {
    setupPairing = false;
    pairing = null;
    portalQrSvg = null;
    showQr = false;
    pairLinkCopied = false;
  }

  async function revokeDevice(d: DeviceInfo) {
    if (!confirm(`Un-pair “${d.name}”? It stops reaching this journal immediately. You can pair it again later.`)) return;
    devices = await portalRevoke(d.id);
    if (pairing?.device.id === d.id) donePairing();
    if (personPairing?.pair.device.id === d.id) donePersonPairing();
  }

  // ---- People: more than one person on this server ----
  const personOf = (id: number) => people.find((p) => p.id === id)?.name ?? "someone removed";
  const deviceCount = (id: number) => devices.filter((d) => d.person === id).length;
  function personState(p: PersonInfo): string {
    if (!p.has_journal) return "waiting for them to choose a password";
    if (!p.unlocked) return "locked";
    return p.remembered ? "unlocked, kept unlocked on this server" : "unlocked";
  }

  async function addPerson() {
    personErr = null;
    try {
      const p = await personAdd(personName);
      personName = "";
      people = await peopleList();
      await pairForPerson(p);
    } catch (e) {
      personErr = e instanceof Error ? e.message : String(e);
    }
  }

  /** Their first device, or a replacement for a lost one. */
  async function pairForPerson(p: PersonInfo) {
    personErr = null;
    try {
      const n = deviceCount(p.id);
      const pair = await portalPair(n ? `${p.name}'s new device` : `${p.name}'s phone`, p.id);
      personPairing = { who: p, pair };
      personQrSvg = null;
      personLinkCopied = false;
      pairedId = null;
      devices = await portalDevices();
    } catch (e) {
      personErr = e instanceof Error ? e.message : String(e);
    }
  }

  async function revealPersonQr() {
    const target = personPairing && pairLink(personPairing.pair.token);
    if (target) personQrSvg = await portalQr(target);
  }

  async function copyPersonLink() {
    const target = personPairing && pairLink(personPairing.pair.token);
    if (!target) return;
    await navigator.clipboard.writeText(target);
    personLinkCopied = true;
  }

  function donePersonPairing() {
    personPairing = null;
    personQrSvg = null;
    personLinkCopied = false;
  }

  async function removePerson(p: PersonInfo) {
    personErr = null;
    try {
      people = await personRemove(p.id, removeTyped);
      devices = await portalDevices();
      removingPerson = null;
      removeTyped = "";
      if (personPairing?.who.id === p.id) donePersonPairing();
    } catch (e) {
      personErr = e instanceof Error ? e.message : String(e);
    }
  }

  function whenSeen(unix: number | null): string {
    if (!unix) return "never connected";
    const mins = Math.round((Date.now() / 1000 - unix) / 60);
    if (mins < 2) return "active just now";
    if (mins < 60) return `last seen ${mins} min ago`;
    return `last seen ${new Date(unix * 1000).toLocaleString()}`;
  }

  /** Publish (or stop publishing) the portal to the tailnet. This is the step that
   *  makes the journal reachable from another device, so it's one button and it's
   *  reversible — and Tailscale's own refusal (not logged in, HTTPS not enabled in
   *  the admin console) is shown verbatim, because that message is the fix. */
  async function toggleServe() {
    portalErr = null;
    serving = true;
    try {
      ts = ts?.serving ? await portalUnserve() : await portalServe();
      // The pairing link changes with it, so any QR on screen is now stale.
      portalQrSvg = null;
      showQr = false;
    } catch (e) {
      portalErr = e instanceof Error ? e.message : String(e);
    } finally {
      serving = false;
    }
  }

  // "Menu bar" on a Mac, "system tray" elsewhere: the words people use for that spot.
  const trayName = isMac ? "menu bar" : "system tray";
  async function toggleMenuBar() {
    portalErr = null;
    try {
      sprefs = await setMenuBar(!sprefs.menu_bar);
    } catch (e) {
      portalErr = e instanceof Error ? e.message : String(e);
    }
  }

  async function toggleServeOnLaunch() {
    portalErr = null;
    try {
      sprefs = await setServerPrefs(!sprefs.serve_on_launch);
      portal = await portalStatus();
      ts = await portalTailscale();
    } catch (e) {
      portalErr = e instanceof Error ? e.message : String(e);
    }
  }

  let discreetErr = $state("");
  async function toggleDiscreetAvailable() {
    discreetErr = "";
    try {
      sprefs = await setDiscreetAvailable(!discreet.available);
      discreet.available = sprefs.discreet_available;
    } catch (e) {
      discreetErr = e instanceof Error ? e.message : String(e);
    }
  }

  // The owner's phone pairing and un-pairing devices (owner_auth.rs): it asks for
  // the journal's password, or, with no password, this PIN.
  let pinStatus = $state<PhonePinStatus | null>(null);
  let pinNew = $state("");
  let pinNew2 = $state("");
  let pinMsg = $state<string | null>(null);
  async function savePhonePin(clear = false) {
    portalErr = null;
    pinMsg = null;
    try {
      if (!clear && pinNew !== pinNew2) throw new Error("The PIN and the repeat don't match.");
      pinStatus = await setPhonePin(clear ? null : pinNew);
      pinMsg = clear ? "Phone PIN removed." : "Phone PIN set.";
      pinNew = pinNew2 = "";
    } catch (e) {
      portalErr = e instanceof Error ? e.message : String(e);
    }
  }

  async function togglePhoneCanUpdate() {
    portalErr = null;
    try {
      sprefs = await setPhoneCanUpdate(!sprefs.phone_can_update);
    } catch (e) {
      portalErr = e instanceof Error ? e.message : String(e);
    }
  }

  async function toggleLoginStart() {
    portalErr = null;
    try {
      if (loginStart) await autostartDisable();
      else await autostartEnable();
      loginStart = await autostartIsEnabled();
    } catch (e) {
      portalErr = e instanceof Error ? e.message : String(e);
    }
  }

  async function rememberPassword() {
    kcErr = null;
    kcBusy = true;
    try {
      kc = await keychainRemember(kcPass);
      kcPass = "";
    } catch (e) {
      kcErr = typeof e === "string" ? e : String(e);
    } finally {
      kcBusy = false;
    }
  }

  async function forgetPassword() {
    kcErr = null;
    try {
      kc = await keychainForget();
    } catch (e) {
      kcErr = typeof e === "string" ? e : String(e);
    }
  }

  async function switchJournal() {
    selected = null;
    await Promise.all([loadJournal(), loadSubstances()]);
  }

  async function connectRemote() {
    remoteErr = null;
    remoteBusy = true;
    try {
      remote = await remoteConnect(remoteLink);
      remoteLink = "";
      setRemoteMode(true);
      await switchJournal();
    } catch (e) {
      remoteErr = typeof e === "string" ? e : String(e);
    } finally {
      remoteBusy = false;
    }
  }

  async function disconnectRemote() {
    remoteErr = null;
    let discard = false;
    if (remote.pending > 0) {
      const n = remote.pending;
      if (!confirm(`${n} new ${n === 1 ? "entry hasn't" : "entries haven't"} reached ${serverName} yet. Disconnecting now throws ${n === 1 ? "it" : "them"} away. Disconnect anyway?`)) return;
      discard = true;
    } else if (!confirm(`Stop using ${serverName} as this computer's journal? Nothing on ${serverName} changes — this computer just goes back to its own journal.`)) {
      return;
    }
    try {
      remote = await remoteDisconnect(discard);
      setRemoteMode(false);
      await switchJournal();
    } catch (e) {
      remoteErr = typeof e === "string" ? e : String(e);
    }
  }

  async function syncNow() {
    remoteErr = null;
    remoteBusy = true;
    try {
      remote = await remoteFlush();
      await loadJournal();
      await refreshSelected().catch(() => {});
    } finally {
      remoteBusy = false;
    }
  }

  let syncBusy = $state(false);
  let syncMsg = $state<string | null>(null);

  /** Copy this computer's own entries to the server. Only new ones, every time. */
  async function syncToServer() {
    remoteErr = syncMsg = null;
    syncBusy = true;
    try {
      const r = await remoteUploadLocal();
      remote = r.status;
      const parts = [`${r.copied} ${r.copied === 1 ? "entry" : "entries"}`];
      if (r.substances) parts.push(`${r.substances} ${r.substances === 1 ? "substance" : "substances"}`);
      syncMsg = `Copied ${parts.join(" and ")} to ${serverName}.`;
      if (r.skipped)
        syncMsg += ` ${r.skipped} ${r.skipped === 1 ? "entry was" : "entries were"} already there, so ${r.skipped === 1 ? "it wasn't" : "they weren't"} copied again.`;
      if (r.removed)
        syncMsg += ` Removed ${r.removed} duplicate ${r.removed === 1 ? "entry" : "entries"} from ${serverName}.`;
      await loadJournal();
      await loadSubstances();
    } catch (e) {
      remoteErr = typeof e === "string" ? e : String(e);
      remote = await remoteStatus().catch(() => remote);
    } finally {
      syncBusy = false;
    }
  }

  const QUEUED_LABEL: Record<string, string> = {
    create_experience: "A new entry",
    log_dose: "A dose",
    add_timeline_event: "A timeline note",
    end_experience: "Ending a trip report",
  };

  async function discardFailed(f: RemoteFailed) {
    if (!confirm(`Throw this away? ${QUEUED_LABEL[f.cmd] ?? f.cmd} that ${serverName} refused won't be saved anywhere.`)) return;
    remote = await remoteDiscard(f.seq);
  }

  // ---- Upstream contribution ----
  async function loadContrib() {
    contribCands = await contributionCandidates();
  }
  async function previewDraft(id: number) {
    contribMsg = null;
    contribDraft = await contributionDraft(id);
  }
  async function saveDraft(d: ContributionDraft, id: number) {
    const path = await save({
      defaultPath: `${d.slug}.json`,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) return;
    await contributionSave(id, path);
    contribMsg = `Draft saved to ${path}. Nothing was sent — submitting it is up to you.`;
    contribDraft = null;
    await loadContrib();
  }

  async function runKbSearch() {
    const q = kbQuery.trim();
    if (!q) { kbHits = null; return; }
    kbBusy = true;
    try {
      kbHits = await knowledgeSearch(q, 8);
    } finally {
      kbBusy = false;
    }
  }
  const num = (n: number | null) => (n == null ? "" : `${n}`);
  function roaSummary(r: PwRoa): string {
    const u = r.units ?? "";
    const parts: string[] = [];
    if (r.threshold != null) parts.push(`thresh ${r.threshold}`);
    if (r.common.min != null) parts.push(`common ${num(r.common.min)}–${num(r.common.max)}`);
    if (r.strong.min != null) parts.push(`strong ${num(r.strong.min)}–${num(r.strong.max)}`);
    if (r.heavy != null) parts.push(r.heavy_max != null ? `heavy ${r.heavy}–${r.heavy_max}` : `heavy ${r.heavy}+`);
    return `${parts.join(" · ")} ${u}`.trim();
  }
  // Compact duration line from DoseWiki stages (onset → total, plus half-life).
  function durationSummary(r: PwRoa): string {
    const parts: string[] = [];
    if (r.onset) parts.push(`onset ${r.onset}`);
    if (r.total) parts.push(`total ${r.total}`);
    if (r.half_life) parts.push(`t½ ${r.half_life}`);
    return parts.join(" · ");
  }
  const refInteractions = (info: PwInfo, severity: "danger" | "caution" | "note") =>
    info.interactions.filter((i) => i.severity === severity);

  function classifyDose(amount: number, r: PwRoa): { label: string; level: string } {
    // Caution, not danger: a heavy dose is worth knowing, not an alarm. Red is
    // kept for combinations that are known to be dangerous.
    if (r.heavy != null && amount >= r.heavy) return { label: "heavy", level: "caution" };
    if (r.strong.min != null && amount >= r.strong.min) return { label: "strong", level: "caution" };
    if (r.common.min != null && amount >= r.common.min) return { label: "common", level: "ok" };
    if (r.light.min != null && amount >= r.light.min) return { label: "light", level: "ok" };
    if (r.threshold != null && amount >= r.threshold) return { label: "threshold", level: "muted" };
    return { label: "below threshold", level: "muted" };
  }

  // Live classification of the dose being entered against PW ranges.
  const doseClass = $derived.by(() => {
    if (!dRef || !dAmount) return null;
    const amt = parseFloat(dAmount);
    if (isNaN(amt) || amt <= 0) return null;
    const roa = dRef.roas.find((r) => r.name.toLowerCase() === dRoute.trim().toLowerCase()) ?? dRef.roas[0];
    if (!roa) return null;
    if (roa.threshold == null && roa.light.min == null && roa.common.min == null) return null;
    // What the dose works out to (fresh mushrooms as dried, capsules in all),
    // in the reference's unit. Never across units that don't convert, and
    // never for a form the ranges aren't for (kratom extract, 7-OH, an edible
    // without an estimate).
    const u = roa.units ?? "";
    const m = measure(dSubstanceAs || dSubstance, amt, dUnit, dDetail);
    let v: number | null;
    if (m) v = u ? inUnit(m.amount, m.unit, u) : m.amount;
    else if (dDetail.form || dDetail.per_unit != null) return null;
    else v = !u || !dUnit || sameUnit(u, dUnit) ? amt : null;
    if (v == null) return null;
    const differs = !!m && (m.approx || !sameUnit(m.unit, dUnit));
    const as = differs ? `${m!.approx ? "about " : ""}${+v.toFixed(3)} ${u || m!.unit}` : "";
    // Psychedelics have a microdose (stats.rs `tier_of`): at or under the
    // cutoff it's a microdose, and above it but not yet common, a low dose.
    const cut = microCutoff(dSubstanceAs || dSubstance, dRef.psychoactive.some((p) => p.toLowerCase() === "psychedelic"), roa.threshold ?? null, u);
    const vc = cut ? inUnit(v, u || m?.unit || dUnit, cut.unit) : null;
    if (cut && vc != null && vc <= cut.amount) return { label: "microdose", level: "ok", as };
    const c = classifyDose(v, roa);
    if (cut && ["light", "threshold", "below threshold"].includes(c.label)) return { label: "low", level: "ok", as };
    return { ...c, as };
  });

  const sevClass = (s: string) => (s === "danger" ? "danger" : s === "caution" ? "caution" : "note");
  /** Words that match the weight. Shouting DANGER at every flag teaches people
   *  to stop reading them. */
  const sevLabel = (s: string) => (s === "danger" ? "Known dangerous" : s === "caution" ? "Use care" : "Note");
</script>

<!-- Shared with the live-session screen, which covers the whole window: the
     crisis banner and interaction warnings must show there too, or a dangerous
     combination logged mid-session flags nothing anyone can see. -->
{#snippet crisisBanner()}
  {#if crisis && crisis.level !== "none"}
    <div class="crisis-banner {crisis.level}">
      <div class="crisis-head">
        <strong>{crisis.headline}</strong>
        <button class="icon-btn" title="Dismiss" onclick={() => (crisis = null)}>✕</button>
      </div>
      {#if crisis.presentation === "offer" && !crisisResourcesShown}
        <!-- A hard moment is not an emergency. Offer, and let it be declined. -->
        <div class="crisis-offer">
          <button class="primary" onclick={() => (crisisResourcesShown = true)}>Show me some options</button>
          <button class="ghost" onclick={() => (crisis = null)}>No thanks</button>
        </div>
      {:else}
        <ul class="crisis-res">
          {#each crisis.resources as r}
            <li><strong>{r.label}</strong>{#if r.contact} — <span class="contact">{r.contact}</span>{/if}<br /><span class="muted small">{r.detail}</span></li>
          {/each}
        </ul>
        <p class="muted small">This is an automatic safety prompt, not a diagnosis. You know your situation best — reaching out for help is always okay.</p>
      {/if}
    </div>
  {/if}
{/snippet}

{#snippet doseWarnings()}
  {#if lastWarnings.length}
    <RiskNotes warnings={lastWarnings} />
  {/if}
{/snippet}


{#if !statusLoaded}
  <div class="gate">
    <div class="gate-card">
      <h1>Field Notes</h1>
      <p class="muted">Opening your journal…</p>
      {#if openingSlow}<p class="muted small">This is taking longer than usual. It will carry on by itself.</p>{/if}
    </div>
  </div>
{:else if db.error}
  <div class="gate">
    <div class="gate-card">
      <h1>Field Notes</h1>
      <p class="lead">Your journal couldn't be opened.</p>
      <p class="notice bad-notice">{db.error}</p>
      <p class="muted small">Nothing has been changed or deleted. Quit and open Field Notes again; if this keeps happening, report it with the message above.</p>
    </div>
  </div>
{:else if db.encrypted && !db.unlocked}
  <div class="gate">
    <div class="gate-card">
      <h1>Field Notes</h1>
      <p class="lead">This journal is encrypted. Enter your password to unlock it.</p>
      <form class="unlock-form" onsubmit={(e) => { e.preventDefault(); doUnlock(); }}>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          type="password"
          autocomplete="current-password"
          autofocus
          placeholder="Password"
          bind:value={unlockPass}
        />
        {#if unlockErr}<p class="notice bad-notice">{unlockErr}</p>{/if}
        <button class="primary" type="submit" disabled={unlockBusy || !unlockPass}>
          {unlockBusy ? "Unlocking…" : "Unlock"}
        </button>
      </form>
      <p class="muted small">There is no recovery — if you lose this password, the journal cannot be opened.</p>
    </div>
  </div>
{:else if !acknowledged}
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
      {#if !companionChoiceMade}
        <label class="companion-optin">
          <input
            type="checkbox"
            checked={!companionOff}
            onchange={(e) => (companionOff = !(e.currentTarget as HTMLInputElement).checked)}
          />
          <span>
            <strong>Turn on the Companion</strong> (optional): an AI you can chat with before,
            during, or after an experience. It runs entirely on this device, offline, and needs a
            separate free download of a few gigabytes (Ollama and a model). The interaction checker,
            crisis scan, and dose reference don't use it and work either way. You can turn it on
            anytime in Settings.
          </span>
        </label>
      {/if}
      <label class="dont-show">
        <input type="checkbox" bind:checked={dontShowDisclaimer} />
        Don't show this again on startup
      </label>
      <button class="primary" onclick={enter}>I understand — continue</button>
    </div>
  </div>
{:else}
  {#snippet aiSetup()}
    {#if remote.connected}
      {#if !remote.online}
        <p class="muted small">The Companion runs on {serverName}, which can't be reached right now.</p>
      {:else if !ai}
        <p class="muted small">Checking {serverName}…</p>
      {:else if !ai.installed}
        <p class="muted small">The Companion runs on {serverName}. Set it up there — open Field Notes on {serverName} and go to the Companion tab.</p>
      {:else if !ai.running}
        <button class="primary small-btn" disabled={aiBusy} onclick={doStart}>{aiBusy ? "Starting…" : `Start Ollama on ${serverName}`}</button>
      {:else}
        <p class="muted small">{serverName} has no model yet — download one in Field Notes on {serverName}.</p>
      {/if}
    {:else if !ai}
      <p class="muted small">Checking for local AI…</p>
    {:else if !ai.installed}
      <button class="primary small-btn" disabled={aiBusy} onclick={doInstall}>{aiBusy ? "Installing Ollama…" : "Install Ollama"}</button>
      <p class="muted small">A one-time install of Ollama, the local model runner. macOS uses Homebrew; Linux uses the official installer; Windows uses WinGet.</p>
    {:else if !ai.running}
      <button class="primary small-btn" disabled={aiBusy} onclick={doStart}>{aiBusy ? "Starting…" : "Start Ollama"}</button>
      <p class="muted small">Ollama is installed but not running.</p>
    {:else if ai.models.length === 0}
      <p class="muted small">Almost there — download a model to power this.</p>
      <select bind:value={aiPullTag} class="model-sel">
        {#each aiRecommended as [tag, label]}<option value={tag}>{label}</option>{/each}
      </select>
      {@render pullPreflight()}
      <button class="primary small-btn" disabled={aiBusy} onclick={doPull}>{aiBusy ? "Downloading…" : "Download model"}</button>
    {/if}
    {#if aiErr}<p class="notice bad-notice">{aiErr}</p>{/if}
    {#if aiLog.length}<pre class="ai-log">{aiLog.slice(-14).join("\n")}</pre>{/if}
  {/snippet}

  <!-- Advisory shown before a download: will the picked model fit this machine? -->
  {#snippet pullPreflight()}
    {#if pullCompute && (pullCompute.verdict === "insufficient" || pullCompute.verdict === "tight")}
      <div class="compute-notice {pullCompute.verdict}">
        <span class="compute-icon" aria-hidden="true">{pullCompute.verdict === "insufficient" ? "⚠️" : "⚡"}</span>
        <span>{pullCompute.message}</span>
      </div>
    {/if}
  {/snippet}

  <main>
    {#if update && !updateDismissed}
      <div class="update-banner">
        <div class="update-head">
          <span>A new version <strong>v{update.version}</strong> of Field Notes is available.</span>
          {#if updateBusy}
            <span class="muted small">{updateMsg}</span>
          {:else}
            <span class="row-actions">
              <button class="primary small-btn" onclick={installUpdate}>Install &amp; restart</button>
              <button class="ghost small-btn" onclick={() => (updateDismissed = true)}>Later</button>
            </span>
          {/if}
        </div>
        {#if update.body?.trim()}
          <!-- The changelog section for this version, straight from latest.json.
               Shown as plain text (not rendered as markdown) — it's short and the
               Markdown-ish dashes read fine as-is. -->
          <p class="update-body">{update.body.trim()}</p>
        {/if}
      </div>
    {/if}

    {@render crisisBanner()}

    <header>
      <h1>Field Notes</h1>
      {#if remote.connected}
        <button
          class="remote-pill"
          class:off={!remote.online}
          title={remote.online ? `Your journal lives on ${remote.server}` : `Can't reach ${remote.server}`}
          onclick={() => goTab("data", "devices")}
        >
          <span class="remote-dot" class:off={!remote.online} aria-hidden="true"></span>
          {remote.online ? `Journal on ${serverName}` : `${serverName} unreachable`}{remote.pending ? ` · ${remote.pending} waiting` : ""}{remote.failed.length ? ` · ${remote.failed.length} not saved` : ""}
        </button>
      {/if}
      <!-- The same places as the phone: Journal, Stats, Check, Talk. Settings and
           Get help sit apart at the bottom; "Report a bug" lives in Settings. -->
      <nav aria-label="Sections">
        <div class="nav-main">
          <button class:active={tab === "journal"} aria-current={tab === "journal" ? "page" : undefined} onclick={() => goTab("journal")}><Icon name="journal" />Journal</button>
          <button class:active={tab === "stats" || tab === "bysub"} aria-current={tab === "stats" ? "page" : undefined} onclick={() => goTab("stats")}><Icon name="stats" />Stats</button>
          <button class:active={tab === "substances"} aria-current={tab === "substances" ? "page" : undefined} onclick={() => goTab("substances")}><Icon name="check" />Check</button>
          {#if !companionOff}
            <button class:active={tab === "companion"} aria-current={tab === "companion" ? "page" : undefined} onclick={() => goTab("companion")}><Icon name="talk" />Talk</button>
          {/if}
        </div>
        <div class="nav-foot">
          <button class:active={tab === "data"} aria-current={tab === "data" ? "page" : undefined} onclick={() => goTab("data")}><Icon name="settings" />Settings</button>
          <div class="nav-help-row">
            <DiscreetToggle />
            <button class="nav-help" title="Emergency &amp; support resources" onclick={openHelp}><Icon name="help" />Get help</button>
          </div>
        </div>
      </nav>
      <!-- One quiet bar per view: where you are on the left, that view's actions on
           the right. Only + Dose is filled. -->
      <div class="topbar">
        <div class="topbar-title">
          {TAB_TITLE[tab] ?? ""}
          {#if tab === "journal" && !selected && experiences.length}<span>{experiences.length} {experiences.length === 1 ? "entry" : "entries"}</span>{/if}
        </div>
        {#if tab === "journal" && !selected}
          <div class="tools">
            <button class="tb" onclick={() => { showPaste = !showPaste; showImport = false; }}><Icon name="paste" size={15} />Paste a log</button>
            {#if !remote.connected}
              <button class="tb" onclick={openImport}><Icon name="import" size={15} />Import</button>
            {/if}
            <button class="tb" onclick={openNewNote}><Icon name="note" size={15} />Note</button>
            <button class="tb" onclick={openNewExp}><Icon name="session" size={15} />Experience</button>
            <!-- Leads, because it's the thing most often being recorded. -->
            <button class="primary small-btn tb-primary" onclick={openQuickLog}>+ Dose</button>
          </div>
        {/if}
      </div>
    </header>

    <!-- ============ JOURNAL ============ -->
    {#if tab === "journal"}
      {#if !selected}<SleepCheckin />{/if}
      {#if !selected && activeDoses.length}
        <!-- Something logged lately may still be active, live trip report or not. -->
        <section class="card">
          <ActiveArcs doses={activeDoses} now={minuteNow} compact onCheck={checkActive} />
        </section>
      {/if}
      {#if selected && selected.kind === "note"}
        <!-- A plain entry: a title, a body, a date. Deliberately quiet — no session chrome. -->
        <section class="card">
          <button class="link" onclick={() => (selected = null)}>← Journal</button>
          <div class="exp-head">
            <h2>{selected.title || "Untitled note"}</h2>
            <span class="row-actions">
              {#if !editExp}<button class="link" onclick={startEditExp}>Edit</button>{/if}
              <button class="link danger-link" onclick={delExp}>Delete</button>
            </span>
          </div>
          <span class="muted small">{fmtDate(selected.started_at)}</span>

          {#if editExp}
            <div class="edit-form">
              <label>Title<input bind:value={eTitle} /></label>
              <label>Date<DateTimeField bind:value={eStart} /></label>
              <label>Entry<textarea class="reflect" bind:value={eNotes} rows="10"></textarea></label>
              <div class="row-actions">
                <button class="primary small-btn" onclick={saveExp}>Save</button>
                <button class="ghost small-btn" onclick={() => (editExp = false)}>Cancel</button>
              </div>
            </div>
          {:else if selected.notes}
            <p class="note-body">{selected.notes}</p>
          {:else}
            <p class="muted small">Nothing written yet — Edit to start.</p>
          {/if}
        </section>
      {:else if selected}
        <section class="card">
          <button class="link" onclick={() => (selected = null)}>← Journal</button>
          <div class="exp-head">
            <h2>{selected.title || "Untitled experience"}</h2>
            <span class="row-actions">
              {#if !selected.ended_at}<button class="primary small-btn" onclick={startLiveSession}>Live trip report</button>{/if}
              {#if !editExp}<button class="link" onclick={startEditExp}>Edit</button>{/if}
              <button class="link danger-link" onclick={delExp}>Delete</button>
            </span>
          </div>
          <span class="muted small">{fmtDate(selected.started_at)} · {fmtTime(selected.started_at)}{selected.ended_at ? " → " + fmtTime(selected.ended_at) : " · ongoing"}</span>

          {#if editExp}
            <div class="edit-form">
              <label>Title<input bind:value={eTitle} /></label>
              <label>Started<DateTimeField bind:value={eStart} /></label>
              <label>Intention<input class="reflect" bind:value={eIntention} /></label>
              <label>Setting<input bind:value={eSetting} /></label>
              <label>Write-up<textarea id="exp-writeup" class="reflect" bind:value={eNotes} rows="6"></textarea></label>
              <label>Rating (0–10)<input type="number" min="0" max="10" bind:value={eRating} /></label>
              <div class="row-actions">
                <button class="primary small-btn" onclick={saveExp}>Save</button>
                <button class="ghost small-btn" onclick={() => (editExp = false)}>Cancel</button>
              </div>
            </div>
          {:else}
            {#if selected.intention}<p><strong>Intention:</strong> <span class="reflect">{selected.intention}</span></p>{/if}
            {#if selected.setting}<p><strong>Setting:</strong> <span class="reflect">{selected.setting}</span></p>{/if}
            {#if selected.notes}
              <p><strong>Write-up:</strong></p>
              <p class="reflect write-up">{selected.notes}</p>
            {:else if selected.ended_at && selected.writeup_skipped}
              <p class="muted small">Doesn't need a write-up. <button class="link" onclick={() => skipWriteup(false)}>Undo</button></p>
            {:else if selected.ended_at && experiences.find((x) => x.id === selected?.id)?.writeup_expected === false}
              <p class="muted small">No write-up. <button class="link" onclick={startEditExp}>Write one</button> any time if you'd like.</p>
            {:else if selected.ended_at}
              <p class="muted small">No write-up yet. <button class="link" onclick={startEditExp}>Write one</button> · <button class="link" onclick={() => skipWriteup(true)}>Doesn't need one</button></p>
            {/if}
            {#if selected.rating != null}<p class="muted small">Rating: {selected.rating}/10</p>{/if}
          {/if}

          {@render doseWarnings()}

          {#if selected.ended_at && selected.doses.length}
            <!-- How the doses likely played out, with the moments on top. -->
            <h3>Arcs</h3>
            <ActiveArcs doses={selected.doses} moments={selected.timeline} now={Date.parse(selected.ended_at)} past />
          {/if}

          <h3>Doses</h3>
          {#if selected.doses.length}
            <ul class="doses">
              {#each selected.doses as d}
                <li>
                  {#if editingDoseId === d.id}
                    <div class="dose-form inline">
                      <SubstanceInput bind:value={edSub} />
                      <input type="number" step="any" bind:value={edAmt} class="narrow" />
                      <input bind:value={edUnit} class="narrow" />
                      <input bind:value={edRoute} class="narrow" />
                      <DateTimeField bind:value={edTime} />
                      <button class="primary small-btn" onclick={saveDose}>Save</button>
                      <button class="ghost small-btn" onclick={() => (editingDoseId = null)}>Cancel</button>
                    </div>
                    <DoseDetailFields substance={edSub} unit={edUnit} amount={edAmt} bind:detail={edDetail} suggest={false} />
                  {:else}
                    <span class="dtime">{fmtTime(d.taken_at)}{#if sessionT0}<span class="rel"> ({relTime(d.taken_at, sessionT0)})</span>{/if}</span>
                    <span class="dname">{d.substance_name}</span>
                    <span class="damt">{describeAmount(d.substance_name, d.amount, d.unit, d)}{d.route ? " · " + d.route : ""}{#if d.profile}<span class="muted" title={d.profile.note}> · {d.profile.label}</span>{/if}</span>
                    <span class="row-actions">
                      <button class="icon-btn" title="Edit dose" onclick={() => startEditDose(d)}>✎</button>
                      <button class="icon-btn" title="Delete dose" onclick={() => delDose(d.id)}>✕</button>
                    </span>
                  {/if}
                </li>
              {/each}
            </ul>
          {:else}
            <p class="muted small">No doses logged yet.</p>
          {/if}

          <div class="dose-form">
            <SubstanceInput placeholder="Substance" bind:value={dSubstance} onchange={() => { lookupRef(dSubstance); dUnit = defaultUnitFor(dSubstance, dRoute) ?? dUnit; }} />
            <input type="number" step="any" placeholder="Amount" bind:value={dAmount} />
            <input placeholder="unit" bind:value={dUnit} class="narrow" />
            <input placeholder="route" bind:value={dRoute} class="narrow" />
            <DateTimeField bind:value={dTime} title="Time taken" />
            <button class="primary small-btn" onclick={submitDose}>Log dose</button>
          </div>
          <NameHint name={dSubstance} bind:saveAs={dSubstanceAs} />
          <DoseDetailFields substance={dSubstanceAs || dSubstance} unit={dUnit} amount={dAmount} bind:detail={dDetail} />
          <KindQuestion substance={dAsk.s} tick={dAsk.n} />
          {#if dRef}
            <div class="ref-inline">
              {#if doseClass}
                <div class="dose-class {doseClass.level}">{dAmount}{dUnit}{doseClass.as ? ` (${doseClass.as})` : ""} · {#if doseClass.label === "microdose"}a <strong>microdose</strong>{:else}<strong>{doseClass.label}</strong> dose{/if}{doseClass.label === "heavy" ? ": above the usual strong range" : ""}</div>
              {/if}
              <strong>{dRef.name}</strong> — reference doses
              {#each dRef.roas as r}
                {#if roaSummary(r)}<div class="muted small">{r.name}: {roaSummary(r)}{durationSummary(r) ? ` · ${durationSummary(r)}` : ""}</div>{/if}
              {/each}
              {#if dRef.dose_note}<div class="muted small">{dRef.dose_note}</div>{/if}
              {#if refInteractions(dRef, "danger").length}
                <div class="small warn-text">Known dangerous with: {refInteractions(dRef, "danger").map((i) => i.name).join(", ")}</div>
              {/if}
              {#if refInteractions(dRef, "caution").length}
                <div class="small warn-text muted">Use care with: {refInteractions(dRef, "caution").map((i) => i.name).join(", ")}</div>
              {/if}
              <div class="muted attribution">via DoseWiki · CC0 public domain · reference only, verify before dosing</div>
            </div>
          {/if}

          <h3>Timeline</h3>
          {#if selected.timeline.length}
            <ul class="timeline">
              {#each selected.timeline as t}
                <li>
                  {#if editingTimelineId === t.id}
                    <div class="dose-form inline">
                      <input bind:value={etNote} />
                      <input bind:value={etMood} class="narrow" placeholder="mood" />
                      <input type="number" min="0" max="10" bind:value={etIntensity} class="narrow" placeholder="0-10" />
                      <DateTimeField bind:value={etTime} />
                      <button class="primary small-btn" onclick={saveTimeline}>Save</button>
                      <button class="ghost small-btn" onclick={() => (editingTimelineId = null)}>Cancel</button>
                    </div>
                  {:else}
                    <span class="dtime">{fmtTime(t.at)}{#if sessionT0}<span class="rel"> ({relTime(t.at, sessionT0)})</span>{/if}</span>
                    <span class="tl-note">{#if t.mood}<strong>{t.mood}</strong>{t.note ? " · " : ""}{/if}<span class="reflect">{t.note}</span>{t.intensity != null ? ` (${t.intensity}/10)` : ""}</span>
                    <span class="row-actions">
                      <button class="icon-btn" title="Edit note" onclick={() => startEditTimeline(t)}>✎</button>
                      <button class="icon-btn" title="Delete" onclick={() => delTimeline(t.id)}>✕</button>
                    </span>
                  {/if}
                </li>
              {/each}
            </ul>
          {:else}
            <p class="muted small">No moments yet.</p>
          {/if}
          <div class="dose-form">
            <input placeholder="How are you feeling?" bind:value={tNote} />
            <input placeholder="mood" bind:value={tMood} class="narrow" />
            <input type="number" min="0" max="10" placeholder="0-10" bind:value={tIntensity} class="narrow" />
            <button class="ghost small-btn" onclick={submitTimeline}>Add moment</button>
          </div>

          {#if !selected.ended_at}
            <button class="ghost" onclick={finishExperience}>End trip report</button>
          {/if}
          <button class="ghost" onclick={exportEntry}>Export this entry</button>
          <button class="ghost" onclick={() => { pdfOpen = !pdfOpen; pdfParts = { ...ALL_PARTS }; }}>Save as PDF…</button>
          {#if pdfOpen}
            <div class="pdf-choices">
              <p class="muted small">
                A tidy report to send to someone. The title, times and doses are always in it; choose what else. It uses
                real names, even in discreet mode.
              </p>
              {#if selected.intention.trim() || selected.setting.trim()}
                <label><input type="checkbox" bind:checked={pdfParts.intention} /> Intention and setting</label>
              {/if}
              {#if selected.timeline.length}
                <label><input type="checkbox" bind:checked={pdfParts.moments} /> Moments ({selected.timeline.length})</label>
              {/if}
              {#if selected.notes.trim()}
                <label><input type="checkbox" bind:checked={pdfParts.writeup} /> {selected.kind === "note" ? "The note" : "Write-up"}</label>
              {/if}
              {#if selected.rating != null}
                <label><input type="checkbox" bind:checked={pdfParts.rating} /> Rating ({selected.rating}/10)</label>
              {/if}
              <div class="row-actions">
                <button class="primary small-btn" onclick={exportPdf}>Save PDF…</button>
                <button class="ghost small-btn" onclick={() => (pdfOpen = false)}>Cancel</button>
              </div>
            </div>
          {/if}
          {#if exportErr}<p class="notice bad-notice">{exportErr}</p>{/if}
          {#if exportMsg}<p class="notice good-notice">{exportMsg}</p>{/if}
        </section>
      {:else}
        <section class="card">
          {#if remote.connected && !remote.online}
            <p class="notice warn-notice small">
              Can't reach {serverName}. This is the journal as it was when it was last reachable, plus
              anything new you've logged since{remote.pending ? ` (${remote.pending} waiting to be sent)` : ""}.
              New entries save here and go to {serverName} when it's back.
            </p>
          {/if}

          {#if showQuickLog}
            <div class="quick-log">
              {#if qlInto}
                <p class="muted small">
                  Adding to the entry you just logged — a second substance in the same night
                  belongs with the first, so they're checked against each other.
                  <button class="link" onclick={() => (qlInto = null)}>Separate entry instead</button>
                </p>
              {:else}
                <p class="muted small">
                  A dose and roughly when — that's the whole entry. No trip report to end and
                  nothing to write up, though you can add all of that later.
                </p>
              {/if}

              {#if qlRecents.length}
                <div class="chips">
                  {#each qlRecents as s}
                    <button class="chip" class:on={qlSub === s} onclick={() => pickRecentSubstance(s)}>{s}</button>
                  {/each}
                </div>
              {/if}

              <div class="quick-row">
                <SubstanceInput placeholder="Substance" bind:value={qlSub} onblur={applyRememberedShape} />
                <input class="narrow" placeholder="Amount" bind:value={qlAmt} />
                <select bind:value={qlUnit}>
                  {#each UNITS as u}<option>{u}</option>{/each}
                </select>
                <select bind:value={qlRoute} onchange={() => { if (!recallDoseShape(qlSub)) qlUnit = defaultUnitFor(qlSub, qlRoute) ?? qlUnit; }}>
                  {#each ["oral", "insufflated", "sublingual", "vaporized", "rectal", "IM", "IV"] as r}<option>{r}</option>{/each}
                </select>
              </div>
              <NameHint name={qlSub} bind:saveAs={qlSubAs} />
              <DoseDetailFields substance={qlSubAs || qlSub} unit={qlUnit} amount={qlAmt} bind:detail={qlDetail} />
              {#if qlUnit === "drink"}
                <p class="muted small">{STANDARD_DRINK}</p>
              {:else if qlUnit === "hit"}
                <p class="muted small">{HIT_NOTE}</p>
              {/if}

              <!-- A preset fills the field beside it rather than replacing it, so
                   the time that will be saved is always visible. -->
              <div class="quick-row">
                <div class="chips">
                  {#each whenPresets as p}
                    <button class="chip" onclick={() => (qlWhen = isoToLocalInput(p.at().toISOString()))}>
                      {p.label}
                    </button>
                  {/each}
                </div>
                <DateTimeField bind:value={qlWhen} title="When you took it" />
                <button class="primary small-btn" disabled={!qlSub.trim()} onclick={submitQuickLog}>
                  Log it
                </button>
              </div>

              {#if qlWarnings.length}
                <RiskNotes warnings={qlWarnings} />
              {/if}

              {#if qlSaved}
                <p class="saved-line">
                  <strong>{qlSaved.title || "Logged"}</strong>
                  <span class="muted small">{fmtDate(qlSaved.at)} · {fmtTime(qlSaved.at)}</span>
                  <button class="ghost small-btn" onclick={() => addNotesTo(qlSaved!.id)}>Add notes</button>
                  <button class="ghost small-btn" onclick={quickLogAnother}>Log another into it</button>
                </p>
                <KindQuestion substance={qlAsk.s} tick={qlAsk.n} />
              {/if}
            </div>
          {/if}

          {#if showNewExp}
            <div class="new-exp">
              <input
                placeholder="Title (optional — else the first substance)"
                title="Leave blank and the experience takes the name of the first substance you log."
                bind:value={neTitle}
              />
              <DateTimeField bind:value={neStart} title={nePast ? "When it started" : "Start time"} />
              {#if nePast}
                <DateTimeField bind:value={neEnd} title="When it ended (optional)" />
              {/if}
              <input placeholder={nePast ? "What was your intention? (optional)" : "What's your intention? (optional)"} class="reflect" bind:value={neIntention} />
              <input placeholder="Set & setting (optional)" bind:value={neSetting} />
              <button class="primary small-btn" onclick={submitNewExperience}>{nePast ? "Log it" : "Start"}</button>
            </div>
            <label class="past-toggle">
              <input type="checkbox" bind:checked={nePast} onchange={onPastToggle} />
              This already happened — I'm logging a past experience
            </label>
          {/if}

          {#if showNewNote}
            <!-- A plain entry — not a session. Title, words, date. -->
            <div class="new-note">
              <input placeholder="Title (optional)" bind:value={nnTitle} />
              <textarea class="reflect" rows="6" placeholder="Write anything." bind:value={nnBody}></textarea>
              <div class="row-actions">
                <DateTimeField bind:value={nnDate} title="Date" />
                <button class="primary small-btn" onclick={submitNewNote}>Save note</button>
              </div>
            </div>
          {/if}

          {#if showPaste}
            <div class="import-panel">
              <TripImport oncancel={() => (showPaste = false)} onsaved={pastedLog} />
            </div>
          {/if}
          {#if showImport}
            <div class="import-panel">
              {#if !aiReady}
                <p class="muted small">Import uses a local model to read your text — nothing leaves this computer. Let's get it set up:</p>
                {@render aiSetup()}
                <button class="ghost small-btn" onclick={() => (showImport = false)}>Cancel</button>
              {:else if !importParsed}
                <p class="muted small">
                  Paste a past experience in your own words — a quick note or a full trip report, whatever you have.
                  The local model reads it and pulls out the substances, doses, and timeline for you to review and
                  edit before anything is saved. Nothing has to be formatted a particular way.
                </p>
                <p class="muted small import-tip">
                  It works best when each dose names the <strong>substance</strong>, <strong>how much</strong>, and
                  roughly <strong>when</strong> — e.g. “around 9pm, 100&nbsp;µg LSD” or “T+2:00 took 15&nbsp;mg 2C-B”.
                  You can add anything it misses by hand after importing.
                </p>
                <select bind:value={aiModel} class="model-sel">
                  {#each ai?.models ?? [] as m}<option value={m}>{m}</option>{/each}
                </select>
                <textarea class="import-text" rows="7" placeholder="e.g.
Saturday night at a friend's place, relaxed and in a good headspace.
Around 9pm I took 100 µg of LSD (one tab). About two hours in, 15 mg of 2C-B.
Peak was intense and connected; gentle comedown by 1am. Drank lots of water, no nausea." bind:value={importText}></textarea>
                {#if importErr}<p class="notice bad-notice">{importErr}</p>{/if}
                <div class="row-actions">
                  <button class="primary small-btn" disabled={importBusy || !importText.trim()} onclick={runParse}>
                    {importBusy ? "Reading…" : "Read & preview"}
                  </button>
                  <button class="ghost small-btn" onclick={() => (showImport = false)}>Cancel</button>
                </div>
                {#if importBusy}<p class="muted small">Reading your account — this can take up to a minute on larger models.</p>{/if}
              {:else}
                <p class="muted small">Review what was found, then import. You can edit or delete anything afterward.</p>
                <div class="new-exp">
                  <input placeholder="Title" bind:value={importTitle} />
                  <DateTimeField bind:value={importStart} title="Start time" />
                </div>
                {#if importParsed.intention || importParsed.setting || importParsed.notes}
                  <dl class="import-summary">
                    {#if importParsed.intention}<div><dt>Intention</dt><dd>{importParsed.intention}</dd></div>{/if}
                    {#if importParsed.setting}<div><dt>Setting</dt><dd>{importParsed.setting}</dd></div>{/if}
                    {#if importParsed.notes}<div><dt>Notes</dt><dd>{importParsed.notes}</dd></div>{/if}
                  </dl>
                {/if}
                {#if importParsed.doses.length}
                  <h3>Doses found</h3>
                  <ul class="doses">
                    {#each importParsed.doses as d}
                      <li><span class="dname">{d.substance}</span><span class="damt">{d.amount ?? "?"} {d.unit}{d.route ? " · " + d.route : ""}</span></li>
                    {/each}
                  </ul>
                {:else}
                  <div class="notice">
                    <p><strong>No doses were picked out.</strong> If your account did mention them, the model may have
                    missed them — going <em>Back</em> and naming each substance with an amount (e.g. “100&nbsp;µg LSD”,
                    “15&nbsp;mg 2C-B”) usually does the trick.</p>
                    <p class="muted small">You can also import as-is and add doses by hand from the entry afterward.</p>
                  </div>
                {/if}
                {#if importParsed.timeline.length}
                  <h3>Timeline</h3>
                  <ul class="timeline">
                    {#each importParsed.timeline as t}<li><span class="tl-note">{t.note}{t.intensity != null ? ` (${t.intensity}/10)` : ""}</span></li>{/each}
                  </ul>
                {/if}
                <div class="row-actions">
                  <button class="primary small-btn" onclick={confirmImport}>Import</button>
                  <button class="ghost small-btn" onclick={() => (importParsed = null)}>Back to edit</button>
                </div>
              {/if}
            </div>
          {/if}

          {#if experiences.length}
            <ul class="exp-list">
              {#each experiences as e, i}
                {#if i === 0 || monthOf(experiences[i - 1].started_at) !== monthOf(e.started_at)}
                  <li class="month" aria-hidden="true">{monthOf(e.started_at)}</li>
                {/if}
                <li>
                  <button class="exp-row" onclick={() => openExperience(e.id)}>
                    <span class="exp-date">{shortDate(e.started_at)}</span>
                    {#if e.kind === "note"}
                      <div class="exp-main">
                        <strong>{hiding() ? "Journal note" : e.title || "Untitled note"}</strong>
                      </div>
                      <div class="exp-meta">
                        <span class="pill note-pill">note</span>
                      </div>
                    {:else}
                      <div class="exp-main">
                        <strong>{hiding() ? "Experience" : e.title || "Untitled"}</strong>
                        <!-- A gentle marker for an entry that may still want its story,
                             unless it's been marked as not needing one. -->
                        {#if !e.ended_at}
                          <span class="muted small">ongoing</span>
                        {:else if !(e.notes.trim() || e.writeup_skipped) && e.writeup_expected !== false}
                          <span class="muted small">no write-up yet</span>
                        {/if}
                      </div>
                      <div class="exp-meta">
                        {#each e.substances as s}<span class="pill">{nameShown(s)}</span>{/each}
                        <span class="muted small">{e.dose_count} dose{e.dose_count === 1 ? "" : "s"}</span>
                      </div>
                    {/if}
                  </button>
                </li>
              {/each}
            </ul>
          {:else}
            {#if !firstStepsDismissed}
              <!-- First steps, for someone setting this up alone. Gone once there's
                   anything in the journal, or when dismissed. -->
              <div class="first-steps">
                <div class="setup-head">
                  <h3>First steps</h3>
                  <button class="link" onclick={dismissFirstSteps}>Dismiss</button>
                </div>
                <ul>
                  <li>
                    <strong>Log a dose</strong> with <strong>+ Dose</strong>, or start an <strong>experience</strong> for a
                    night you'll want to look back on: doses, moments as they happen, and a write-up after.
                  </li>
                  <li>
                    <strong>Combinations are checked as you log.</strong> Log a second substance and Field Notes tells
                    you about known risks between them. "Nothing flagged" isn't the same as "safe". You can also
                    check a combination without logging, under <button class="link-inline" onclick={() => goTab("substances")}>Check</button>.
                  </li>
                  <li>
                    <strong>Get help</strong> is always at the bottom left: emergency numbers and support lines,
                    whether or not anything is logged.
                  </li>
                  {#if !remote.connected}
                    <li>
                      <strong>Want it on your phone?</strong> <button class="link-inline" onclick={() => goTab("data", "devices")}>Set up your phone</button>
                      in Settings. It walks you through each step.
                    </li>
                  {/if}
                  <li>Everything stays on this computer. There's no account, and nothing is sent anywhere.</li>
                </ul>
              </div>
            {:else}
              <p class="muted">Nothing here yet. Start a trip report, or just write a note.</p>
            {/if}
          {/if}
        </section>
      {/if}
    {/if}

    <!-- ============ COMPANION ============ -->
    {#if tab === "companion"}
      <section class="card companion">
        <div class="exp-head">
          <h2>Companion</h2>
          {#if aiReady}
            <span class="row-actions">
              <select bind:value={aiModel} class="model-sel" title="Model the Companion talks to">
                {#each ai?.models ?? [] as m}<option value={m}>{m}</option>{/each}
              </select>
              <button class="ghost small-btn" onclick={() => (showModels = !showModels)}>
                {showModels ? "Close" : "Models…"}
              </button>
            </span>
          {/if}
        </div>

        {#if !aiReady}
          <p class="muted small">
            A calm harm-reduction companion that runs a model entirely on <em>this</em> computer —
            nothing you say leaves the device. Let's get it set up:
          </p>
          {@render aiSetup()}
        {:else}
          <p class="muted small disclaimer">
            A calm harm-reduction companion, running locally. Not medical advice. In an emergency,
            contact emergency services or poison control.
          </p>

          {#if compute && compute.verdict !== "ample"}
            <div class="compute-notice {compute.verdict}">
              <span class="compute-icon" aria-hidden="true">
                {compute.verdict === "insufficient" ? "⚠️" : compute.verdict === "tight" ? "⚡" : "ℹ️"}
              </span>
              <span>{compute.message}</span>
            </div>
          {/if}

          {#if upgradeAvailable && !remote.connected}
            <div class="upgrade-notice">
              <p>
                <strong>A better model is available.</strong> <code>{aiModel}</code> often declines to
                discuss substances, including in an emergency. <code>{aiPreferred}</code> handles those
                moments properly.
              </p>
              <div class="upgrade-actions">
                <button class="primary small-btn" disabled={aiBusy} onclick={doSwitchModel}>
                  {aiBusy ? "Switching…" : `Switch to ${aiPreferred}`}
                </button>
                <button class="ghost small-btn" disabled={aiBusy} onclick={() => (upgradeDismissed = true)}>
                  Not now
                </button>
              </div>
              <p class="muted small">
                Downloads {aiPreferred} (~5 GB) first, then removes {aiModel} to free the space.
                Your journal isn't touched.
              </p>
              {#if aiErr}<p class="notice bad-notice">{aiErr}</p>{/if}
              {#if aiLog.length}<pre class="ai-log">{aiLog.slice(-14).join("\n")}</pre>{/if}
            </div>
          {/if}

          {#if showModels}
            <div class="models-panel">
              <p class="muted small">
                Installed: {(ai?.models ?? []).join(", ") || "none"}. The active model above is used by both
                the Companion and text-import. Download another to switch between them — everything runs locally.
              </p>
              {#if remote.connected}
                <p class="muted small">
                  The Companion runs on {serverName}, using its models — to add one, open Field Notes on
                  {serverName}.
                </p>
              {:else}
              <div class="pull-row">
                <input placeholder="Model tag to download, e.g. qwen3:8b" bind:value={aiPullTag} list="rec-models" />
                <datalist id="rec-models">
                  {#each aiRecommended as [tag, label]}<option value={tag}>{label}</option>{/each}
                </datalist>
                <button class="primary small-btn" disabled={aiBusy || !aiPullTag.trim()} onclick={doPull}>
                  {aiBusy ? "Downloading…" : "Download"}
                </button>
              </div>
              <div class="style-chips">
                {#each aiRecommended as [tag, label]}
                  <button type="button" class="chip" class:on={aiPullTag === tag} onclick={() => (aiPullTag = tag)}>{label}</button>
                {/each}
              </div>
              {@render pullPreflight()}
              <p class="muted small">
                These models run on this computer, so your hardware sets the ceiling. On an older or
                lower-memory machine the Companion will be slower and less capable. You can turn it
                off in Settings.
              </p>
              {/if}
              {#if aiErr}<p class="notice bad-notice">{aiErr}</p>{/if}
              {#if aiLog.length}<pre class="ai-log">{aiLog.slice(-14).join("\n")}</pre>{/if}
            </div>
          {/if}

          <label class="share">
            <input type="checkbox" bind:checked={cShareSession} />
            Share an experience with the companion
          </label>
          {#if cShareSession}
            <div class="share-pick">
              <select bind:value={cSessionChoice} class="model-sel" aria-label="Which experience to share">
                <option value={null}>Current experience (most recent)</option>
                {#each shareableSessions as s}
                  <option value={s.id}>{s.title || "Untitled"} · {fmtDate(s.started_at)}</option>
                {/each}
              </select>
              <span class="muted small">
                {#if attachedExp}Sharing “{attachedExp.title || "Untitled"}”{:else}No experiences yet — nothing to share.{/if}
              </span>
            </div>
          {/if}

          <div class="support-style">
            <span class="muted small">What kind of support do you want?</span>
            <div class="style-chips">
              {#each SUPPORT_STYLES as sstyle}
                <button
                  type="button"
                  class="chip"
                  class:on={supportStyle === sstyle}
                  onclick={() => (supportStyle = supportStyle === sstyle ? "" : sstyle)}
                >{sstyle}</button>
              {/each}
            </div>
          </div>

          <div class="chat">
            {#if !cMessages.length}
              <p class="muted small chat-empty">Start chatting about an experience you're planning, having currently, or want to discuss for integration. If you share an experience with me, I can see what doses you've taken and, at your request, I can log doses or take notes for you while we chat.</p>
            {/if}
            {#each cMessages as m}
              <div class="bubble {m.role}">{m.content}</div>
            {/each}
            {#if cSending}
              <div class="bubble assistant muted">…</div>
              {#if !cMessages.some((m) => m.role === "assistant")}
                <p class="muted small warm-hint">The model is loading into memory — the first reply can take a little longer. It's quicker after that.</p>
              {/if}
            {/if}
          </div>

          {#if cActions.length}
            <div class="actions-note">
              {#each cActions as a}<span class="action-chip">✓ {a}</span>{/each}
            </div>
          {/if}

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
        <h2>Check a combination</h2>
        <p class="muted small">Two or more substances, separated by commas. This is the same deterministic check that runs when you log a dose.</p>
        <form class="combo-form" onsubmit={(e) => { e.preventDefault(); runComboCheck(); }}>
          <input id="combo-names" placeholder="e.g. MDMA, ketamine" bind:value={comboText} autocomplete="off" />
          <button class="primary small-btn" type="submit" disabled={comboText.split(",").filter((x) => x.trim()).length < 2}>Check</button>
        </form>
        {#if activeDoses.length}
          <ActiveArcs doses={activeDoses} now={minuteNow} showChart={false} onCheck={(names) => { comboText = names.join(", ") + ", "; comboResult = null; }} />
        {/if}
        {#if comboResult}
          {#if comboResult.length === 0}
            <p class="notice">Nothing flagged between those. That isn't the same as "safe".</p>
          {:else}
            <RiskNotes warnings={comboResult} />
          {/if}
        {/if}
      </section>

      <section class="card">
        <h2>Search the reference</h2>
        {#if kbStat && kbStat.available}
          <p class="muted small">{kbStat.chunks.toLocaleString()} passages of DoseWiki prose — pharmacology, harm potential, tolerance, legality — searchable offline. Background reading, not dose advice: dose ranges and combo warnings appear while you log, and those are exact.</p>
        {:else}
          <p class="muted small">The reference text isn't loaded, so search is unavailable.</p>
        {/if}
        <p class="muted attribution">Dose and reference data from <strong>DoseWiki</strong> (dose.wiki), dedicated to the public domain under CC0. Reference only — not a prescription. Updates ship with new versions of the app.</p>

        {#if kbOpen}
          <!-- Reading one substance whole. Its dose data rides along above the
               prose, but stays in its own panel: that panel is authoritative,
               the prose below it is background reading. -->
          <div class="kb-entry">
            <div class="kb-entry-head">
              <h3>{kbOpen.title}</h3>
              <button class="ghost small-btn" onclick={closeKbEntry}>← Back to search</button>
            </div>

            {#if kbDose}
              <div class="ref-inline">
                <strong>{kbDose.name}</strong> — reference doses
                {#each kbDose.roas as r}
                  {#if roaSummary(r)}<div class="muted small">{r.name}: {roaSummary(r)}{durationSummary(r) ? ` · ${durationSummary(r)}` : ""}</div>{/if}
                {/each}
                {#if kbDose.dose_note}<div class="muted small">{kbDose.dose_note}</div>{/if}
                {#if refInteractions(kbDose, "danger").length}
                  <div class="small warn-text">Known dangerous with: {refInteractions(kbDose, "danger").map((i) => i.name).join(", ")}</div>
                {/if}
                {#if refInteractions(kbDose, "caution").length}
                  <div class="small warn-text muted">Use care with: {refInteractions(kbDose, "caution").map((i) => i.name).join(", ")}</div>
                {/if}
                <div class="muted attribution">via DoseWiki · CC0 public domain · reference only, verify before dosing</div>
              </div>
            {/if}

            {#if kbOpen.sections.length}
              {#if kbOpen.sections[0].thin || !kbOpen.sections[0].reviewed}
                <p class="kb-flags entry-flags">
                  {#if kbOpen.sections[0].thin}<span class="flag sparse" title="DoseWiki has very little written about this substance — treat it as a starting point, not a full picture.">sparse entry</span>{/if}
                  {#if !kbOpen.sections[0].reviewed}<span class="flag" title="DoseWiki's editors have not signed off on this entry. Almost none are — that's the state of the source, not a red flag about this one in particular.">unreviewed</span>{/if}
                </p>
              {/if}
              <div class="kb-sections">
                {#each kbOpen.sections as s}
                  <section class="kb-section">
                    <h4>{s.section}</h4>
                    <p class="kb-text">{s.text}</p>
                  </section>
                {/each}
              </div>
              <p class="muted attribution">{kbOpen.sections.length} {kbOpen.sections.length === 1 ? "passage" : "passages"} quoted from DoseWiki as written — this is everything the corpus holds on {kbOpen.title}. Where it's silent, it's silent; that's not the same as safe.</p>
            {:else}
              <p class="muted">Loading…</p>
            {/if}
          </div>
        {:else}
          <form class="kb-search" onsubmit={(e) => { e.preventDefault(); runKbSearch(); }}>
            <input placeholder="e.g. ketamine tolerance, MAOI interactions, 2C-B pharmacology" bind:value={kbQuery} />
            <button class="primary small-btn" type="submit" disabled={kbBusy || !kbStat?.available}>
              {kbBusy ? "Searching…" : "Search"}
            </button>
          </form>

          {#if kbHits}
            {#if kbHits.length === 0}
              <p class="muted">Nothing in the reference matches that. Better to have no answer than a made-up one — try a substance name, or browse the list below.</p>
            {:else}
              <ul class="kb-hits">
                {#each kbHits as h}
                  <li>
                    <div class="kb-head">
                      <span><strong>{h.title}</strong><span class="muted small"> · {h.section}</span></span>
                      <span class="kb-flags">
                        {#if h.thin}<span class="flag sparse" title="DoseWiki has very little written about this substance — treat it as a starting point, not a full picture.">sparse entry</span>{/if}
                        {#if !h.reviewed}<span class="flag" title="DoseWiki's editors have not signed off on this entry. Almost none are — that's the state of the source, not a red flag about this one in particular.">unreviewed</span>{/if}
                      </span>
                    </div>
                    <p class="kb-text">{h.text}</p>
                    <button class="link-btn" onclick={() => openKbEntry(h.slug, h.title)}>Read the full {h.title} entry →</button>
                  </li>
                {/each}
              </ul>
              <p class="muted attribution">Passages are quoted from DoseWiki as written. An <strong>unreviewed</strong> tag is the norm, not the exception; <strong>sparse entry</strong> means there's little written about that substance anywhere — which is exactly when it pays to be careful.</p>
            {/if}
          {/if}

          {#if kbEntries.length}
            <h3 class="browse-head">Or look up a substance</h3>
            <p class="muted small">Every one of the {kbEntries.length} substances in the reference, readable in full — doses, pharmacology, harm potential, tolerance, legality.</p>
            <input class="kb-browse" placeholder="Type a substance name" bind:value={kbBrowse} />
            {#if kbBrowseHits.length === 0}
              <p class="muted small">No substance by that name. The reference covers {kbEntries.length} of them, but it doesn't cover everything.</p>
            {:else}
              <ul class="kb-browse-list">
                {#each (kbShowAll || kbBrowse.trim() ? kbBrowseHits : kbBrowseHits.slice(0, 24)) as e}
                  <li>
                    <button class="link-btn" onclick={() => openKbEntry(e.slug, e.title)}>{e.title}</button>
                    {#if e.thin}<span class="flag sparse" title="Very little is written about this one upstream.">sparse</span>{/if}
                  </li>
                {/each}
              </ul>
              {#if !kbBrowse.trim() && !kbShowAll && kbBrowseHits.length > 24}
                <button class="ghost small-btn" onclick={() => (kbShowAll = true)}>Show all {kbBrowseHits.length}</button>
              {/if}
            {/if}
          {/if}
        {/if}
      </section>

      <section class="card">
        <h2>Substances you track</h2>
        <p class="muted small">Your own list, separate from the reference above. Add a substance — especially one DoseWiki doesn't cover — so the interaction checker recognises it when you log. Assign classes and it can flag combinations; leave them blank and common ones are auto-classified. The optional dose note is just your own reminder.</p>

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
                <div class="sub-head">
                  <span><strong>{s.name}</strong>{#if s.category}<span class="muted small"> · {s.category}</span>{/if}</span>
                  <button class="icon-btn" title="Delete substance" onclick={() => delSubstance(s.id)}>✕</button>
                </div>
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

      {#if !remote.connected && contribCands.some((c) => !c.in_dosewiki)}
        <section class="card">
          <h2>Missing from DoseWiki</h2>
          <p class="muted small">
            You've catalogued substances the reference doesn't cover. Those gaps are usually the obscure
            compounds where the next person has nowhere at all to look — and DoseWiki is CC0 and open,
            so they can be closed. Field Notes can draft an entry for you to review.
          </p>
          <p class="muted small">
            <strong>Nothing is ever uploaded.</strong> A draft is built from what you typed about the
            <em>substance</em> — never from your journal, never from a dose you logged. You read it, you
            save it, and you submit it yourself, or you don't.
          </p>

          <ul class="sub-list">
            {#each contribCands.filter((c) => !c.in_dosewiki) as c}
              <li>
                <div class="sub-head">
                  <span>
                    <strong>{c.name}</strong>
                    {#if c.contributed}<span class="flag" title="You've saved a draft for this one. It says nothing about whether you submitted it.">draft saved</span>{/if}
                  </span>
                  <button class="ghost small-btn" onclick={() => previewDraft(c.id)}>Review draft…</button>
                </div>

                {#if contribDraft && contribDraft.name === c.name}
                  <pre class="draft">{contribDraft.json}</pre>
                  <div class="row-actions draft-actions">
                    <button class="link" onclick={() => (contribDraft = null)}>Cancel</button>
                    <button class="ghost small-btn" onclick={() => openUrl(contribDraft!.upstream_url)}>Open DoseWiki</button>
                    <button class="primary small-btn" onclick={() => saveDraft(contribDraft!, c.id)}>Save draft to a file…</button>
                  </div>
                  <p class="muted small">
                    Dose and duration are blank on purpose — the app has no verified figures for a substance
                    you added yourself, and a guessed number published upstream is worse than a blank one.
                    Fill them in from sources you trust.
                  </p>
                {/if}
              </li>
            {/each}
          </ul>

          {#if contribMsg}<p class="muted small">{contribMsg}</p>{/if}
        </section>
      {/if}
    {/if}

    {#if tab === "stats"}
      <section class="card">
        <p class="muted small">Patterns from the doses in your journal. Nothing here is a judgement or a warning; it's what you logged, laid out.</p>
        <UsageStats onOpen={async (id) => { await goTab("journal"); await openExperience(id); }} />
      </section>
      <!-- The old Substance Log, folded into Stats: every dose, grouped by substance. -->
      <section class="card">
        <h2>Every dose, by substance</h2>
        {#if usage.length}
          {#each usage as u}
            <div class="usage">
              <div class="usage-head">
                <strong>{nameShown(u.substance_name)}</strong>
                <span class="muted small">{u.times_used} dose{u.times_used === 1 ? "" : "s"}</span>
              </div>
              <ul class="doses">
                {#each u.doses as d}
                  <li>
                    <span class="dtime">{fmtDate(d.taken_at)}</span>
                    <span class="damt">{describeAmount(d.substance_name, d.amount, d.unit, d)}{d.route ? " · " + d.route : ""}</span>
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

    <!-- ============ SETTINGS ============ -->
    {#if tab === "data"}
      <div class="settings-nav" role="group" aria-label="Settings sections">
        {#each SETTINGS_SECTIONS as [key, label]}
          <button class:on={settingsSection === key} aria-current={settingsSection === key ? "true" : undefined} onclick={() => (settingsSection = key)}>{label}</button>
        {/each}
      </div>

      {#if secErr}<p class="notice bad-notice">{secErr}</p>{/if}
      {#if secMsg}<p class="notice good-notice">{secMsg}</p>{/if}

      {#if settingsSection === "general"}
        <section class="card">
          <h2>Updates</h2>
          <p class="muted small">
            Field Notes checks for a new version when it opens and every few hours while it
            runs. The check asks GitHub for the latest version number; nothing about your
            journal is sent.
          </p>
          <div class="row-actions">
            <button class="ghost small-btn" disabled={manualCheck === "checking" || updateBusy} onclick={checkForUpdateNow}>
              {manualCheck === "checking" ? "Checking…" : "Check for updates"}
            </button>
            {#if manualCheck === "found" && update}
              <button class="primary small-btn" disabled={updateBusy} onclick={installUpdate}>Install v{update.version} &amp; restart</button>
            {/if}
          </div>
          <p class="muted small" role="status">
            {#if updateBusy}{updateMsg}
            {:else if manualCheck === "current"}You're on the latest version{appVersion ? ` (v${appVersion})` : ""}.
            {:else if manualCheck === "found" && update}Version {update.version} is available{appVersion ? ` (you have v${appVersion})` : ""}.
            {:else if manualCheck === "failed"}Couldn't check right now: {manualCheckErr}. You may be offline.
            {:else if appVersion}You have v{appVersion}.
            {/if}
          </p>
        </section>

        <section class="card">
          <h2>Companion <span class="off-badge" class:on={!companionOff}>{companionOff ? "off" : "on"}</span></h2>
          <label class="share">
            <input
              type="checkbox"
              checked={!companionOff}
              onchange={(e) => (companionOff = !(e.currentTarget as HTMLInputElement).checked)}
            />
            Turn on the Companion
          </label>
          <p class="muted small">
            An optional AI to talk with before, during or after an experience. It runs only on this
            computer and needs a separate free download of a few gigabytes (Ollama and a model), which
            the Companion tab walks you through. The journal, timeline, dose reference, interaction
            checker and crisis resources don't use it and work the same either way.
          </p>
        </section>

        <section class="card">
          <h2>Discreet mode</h2>
          <p class="muted small">
            For using Field Notes in public or while sharing your screen. When it's on here, an eye
            button appears next to <strong>Get help</strong> on this computer and on paired phones.
            Tapping it swaps substance names for stand-ins like "Substance K7" and hides entry titles
            and previews in lists. Each device decides for itself when to hide; nothing in the journal
            changes.
          </p>
          <label class="share">
            <input type="checkbox" checked={discreet.available} onchange={toggleDiscreetAvailable} />
            Offer discreet mode
          </label>
          {#if discreetErr}<p class="notice bad-notice">{discreetErr}</p>{/if}
        </section>

        <section class="card">
          <h2>Startup disclaimer</h2>
          <label class="dont-show">
            <input
              type="checkbox"
              checked={dontShowDisclaimer}
              onchange={(e) => {
                dontShowDisclaimer = (e.currentTarget as HTMLInputElement).checked;
                if (dontShowDisclaimer) localStorage.setItem(HIDE_DISCLAIMER_KEY, "1");
                else localStorage.removeItem(HIDE_DISCLAIMER_KEY);
              }}
            />
            Skip the disclaimer splash on startup
          </label>
        </section>
      {/if}

      {#if settingsSection === "devices"}
        <section class="card">
          <h2>Use another computer as your server <span class="off-badge" class:on={remote.connected}>{remote.connected ? "on" : "off"}</span></h2>
          {#if !remote.connected}
            <p class="muted small">
              Keep one journal on a computer that stays on — a desktop at home, say — and have this one read
              and write there over your <strong>tailnet</strong>, the same way a phone does. Pair this
              computer on the server (Settings → Devices &amp; sync → Pair a device), copy the link it
              shows, and paste it here.
            </p>
            <p class="muted small">
              If the server can't be reached, new experiences, doses and timeline notes still save here and are
              sent the moment it's back; the interaction checker and crisis resources keep working on this
              computer. Editing and deleting wait for the connection. Entries already on this computer stay
              here, hidden while you're connected — once connected, <strong>Sync journal to server</strong>
              copies them over.
            </p>
            {#if portal.running}
              <p class="muted small">⚠️ This computer is serving its own journal right now. Turn off device access below first.</p>
            {:else}
              <form class="remote-form" onsubmit={(e) => { e.preventDefault(); connectRemote(); }}>
                <input
                  type="password"
                  autocomplete="off"
                  spellcheck="false"
                  placeholder="Pairing link from the server — https://…/m#t=…"
                  bind:value={remoteLink}
                />
                <button class="primary small-btn" type="submit" disabled={remoteBusy || !remoteLink.trim()}>
                  {remoteBusy ? "Connecting…" : "Connect"}
                </button>
              </form>
              <p class="muted small">The link is a key to your journal — it's hidden as you paste it, and stored only inside this computer's journal.</p>
            {/if}
          {:else}
            <p class="small remote-line">
              <span class="remote-dot" class:off={!remote.online} aria-hidden="true"></span>
              {#if remote.unpaired}
                <strong>{serverName} no longer recognises this computer.</strong> It was un-paired there — pair it again and reconnect.
              {:else if remote.online}
                Your journal lives on <strong>{serverName}</strong> <span class="muted">({remote.server})</span>.
              {:else}
                <strong>Can't reach {serverName} right now.</strong> New entries save here and are sent when it's back.
              {/if}
            </p>
            {#if remote.pending > 0}
              <p class="small">{remote.pending} new {remote.pending === 1 ? "entry is" : "entries are"} waiting to be sent.</p>
            {/if}
            <div class="sec-block">
              <h3>Sync journal to server</h3>
              {#if remote.local_unsynced > 0}
                <p class="muted small">
                  This computer has <strong>{remote.local_unsynced}</strong> {remote.local_unsynced === 1 ? "entry" : "entries"} of
                  its own that {remote.local_unsynced === 1 ? "isn't" : "aren't"} on {serverName} — hidden while you're connected.
                  Copy {remote.local_unsynced === 1 ? "it" : "them"} there, with doses, timelines, write-ups and any substances
                  you added. {remote.local_unsynced === 1 ? "It stays" : "They stay"} on this computer too, and syncing again
                  only copies what's new.
                </p>
                <button class="primary small-btn" disabled={syncBusy || !remote.online} onclick={syncToServer}>
                  {syncBusy ? "Syncing…" : `Sync journal to ${serverName}`}
                </button>
                {#if !remote.online}<p class="muted small">Needs the connection to {serverName}.</p>{/if}
              {:else}
                <p class="muted small">✓ Everything in this computer's own journal is on {serverName}.</p>
              {/if}
              {#if syncMsg}<p class="notice good-notice">{syncMsg}</p>{/if}
            </div>
            {#if remote.failed.length}
              <div class="sec-block">
                <h3>Couldn't be saved on {serverName}</h3>
                <p class="muted small">The server refused these. They're kept here so nothing disappears quietly.</p>
                <ul class="device-list">
                  {#each remote.failed as f (f.seq)}
                    <li>
                      <span><strong>{QUEUED_LABEL[f.cmd] ?? f.cmd}</strong><br /><span class="muted small">{f.error}</span></span>
                      <button class="ghost small-btn" onclick={() => discardFailed(f)}>Discard</button>
                    </li>
                  {/each}
                </ul>
              </div>
            {/if}
            {#if remoteErr}<p class="notice bad-notice">{remoteErr}</p>{/if}
            <div class="row-actions">
              <button class="ghost small-btn" disabled={remoteBusy} onclick={syncNow}>{remoteBusy ? "Checking…" : "Sync now"}</button>
              <button class="ghost small-btn" onclick={disconnectRemote}>Disconnect</button>
            </div>
          {/if}
          {#if remoteErr && !remote.connected}<p class="notice bad-notice">{remoteErr}</p>{/if}
        </section>

        <section class="card">
          <h2>Devices &amp; server <span class="off-badge" class:on={portal.running}>{portal.running ? "on" : "off"}</span></h2>
          {#if remote.connected}
            <p class="muted small">
              This computer uses {serverName} as its journal, so it doesn't serve one of its own. Pair your
              phone with {serverName} instead.
            </p>
          {:else}
          <p class="muted small">
            Optional. Field Notes is an offline, on-device app and it stays that way unless you turn this
            on: it lets your phone — or another computer — on your <strong>tailnet</strong> reach this
            journal while the app is running here.
          </p>
          <p class="muted small">
            The server listens on <strong>127.0.0.1 only</strong>, so nothing is exposed to your local
            network; Tailscale is what carries it to your devices, encrypted. Every device has its own key,
            which you can revoke on its own, and nothing is served while the journal is locked. Your data
            still never reaches a third party.
          </p>

          {#if ts != null}
            {@const allDone = setupNext === null}
            <div class="setup" class:done={allDone}>
              <div class="setup-head">
                <h3>{allDone ? "Your phone is set up" : "Set up your phone"}</h3>
                {#if allDone}
                  <button class="link" onclick={() => (setupOpen = !setupOpen)}>{setupOpen ? "Hide steps" : "Show steps"}</button>
                {:else}
                  <button class="ghost small-btn" disabled={setupChecking} onclick={recheckSetup}>{setupChecking ? "Checking…" : "Check again"}</button>
                {/if}
              </div>
              {#if !allDone || setupOpen}
                <ol class="setup-steps">
                  {#each setupSteps as st, n}
                    {@const active = !st.done && setupActive(st.key)}
                    <li class:done={st.done} class:next={active}>
                      <span class="setup-mark" aria-hidden="true">{st.done ? "✓" : n + 1}</span>
                      <div>
                        <span class="setup-label">{st.label}<span class="visually-hidden">{st.done ? " (done)" : active ? " (next)" : ""}</span></span>
                        {#if active}
                          <div class="setup-fix small">
                            {#if st.key === "connect"}
                              {#if !ts.installed}
                                <p>This copy of Field Notes is missing its built-in Tailscale. Reinstall Field Notes{ts.app_available ? ", or use the Tailscale app that's already on this computer" : ""}.</p>
                                {#if ts.app_available}
                                  <button class="primary small-btn" onclick={() => switchTailscale(true)}>Use the Tailscale app</button>
                                {/if}
                              {:else if ts.signed_in && !ts.https_enabled}
                                <p>Signed in as <strong>{ts.login ?? "you"}</strong>. One last switch: your Tailscale account needs secure (HTTPS) connections turned on. It's off on new accounts and only needs doing once.</p>
                                <p>On the page this opens, find <strong>HTTPS Certificates</strong> and click <strong>Enable HTTPS</strong>. If it asks you to turn on MagicDNS first, do that too. Then come back here.</p>
                                <button class="primary small-btn" onclick={() => openUrl("https://login.tailscale.com/admin/dns")}>Open Tailscale's DNS settings</button>
                              {:else if ts.signed_in}
                                <p>Signed in as <strong>{ts.login ?? "you"}</strong>. Setting up a secure connection; the first time takes a few seconds.</p>
                              {:else if ts.auth_url}
                                <p>Sign in on the Tailscale page that opened in your browser, with Google, Apple, Microsoft or GitHub. No account yet? Signing in creates one. Then come back here: this step ticks by itself.</p>
                                <button class="ghost small-btn" onclick={() => openUrl(ts!.auth_url!)}>Open the sign-in page again</button>
                              {:else if connecting}
                                <p>Starting Tailscale…</p>
                              {:else}
                                <p>Your phone reaches this computer through <strong>Tailscale</strong>, a free, private connection between your own devices. It's built into Field Notes, so there's nothing to install here. Tailscale can see which of your devices are connected, never what's in your journal.</p>
                                <button class="primary small-btn" onclick={connectBuiltin}>Connect</button>
                                <p class="muted">Opens Tailscale's sign-in page in your browser. Sign in with Google, Apple, Microsoft or GitHub; no account yet? Signing in creates one.</p>
                              {/if}
                              {#if ts.problem}<p class="muted">Tailscale said: {ts.problem}</p>{/if}
                            {:else if st.key === "phone-app"}
                              <p>Get <strong>Tailscale</strong> from the App Store or Google Play. It's free.</p>
                              {#if storeQr}
                                <div class="store-qrs">
                                  <figure><div class="qr">{@html storeQr.ios}</div><figcaption>iPhone (App Store)</figcaption></figure>
                                  <figure><div class="qr">{@html storeQr.android}</div><figcaption>Android (Google Play)</figcaption></figure>
                                </div>
                              {:else}
                                <button class="ghost small-btn" onclick={showStoreCodes}>Show download codes</button>
                              {/if}
                            {:else if st.key === "phone-signin"}
                              <p>Open Tailscale on your phone and sign in{ts.login ? ` as ${ts.login}` : ""}. It has to be <strong>the same account</strong> as this computer: signing in with a different one (say Google here, Apple there) is the most common reason a phone can't connect.</p>
                              <p>Allow the VPN when your phone asks, and check that Tailscale says <strong>Connected</strong>.</p>
                            {:else if st.key === "scan"}
                              {#if setupPairing && pairing}
                                {#if showQr && portalQrSvg}<div class="qr">{@html portalQrSvg}</div>{/if}
                                <p>Field Notes opens on your phone, already set up. To keep it as an app, use <strong>Share → Add to Home Screen</strong>.</p>
                                <p class="muted"><strong>This code is a key</strong>: whoever scans it can read and write this journal. Don't photograph it. It disappears when you press Done.</p>
                                <button class="ghost small-btn" onclick={donePairing}>Done</button>
                              {:else}
                                <p>Field Notes opens on your phone, already set up. Nothing to type.</p>
                                <button class="primary small-btn" onclick={pairFromSetup}>Show the code</button>
                              {/if}
                            {:else if st.key === "install"}
                              <p>Tailscale is a free app that privately links your own devices. It's what carries the connection between this computer and your phone, encrypted.</p>
                              <button class="primary small-btn" onclick={() => openUrl("https://tailscale.com/download")}>Download Tailscale</button>
                            {:else if st.key === "signin"}
                              <p>Open Tailscale and sign in{isMac ? " (its icon is at the top of the screen, next to the clock)" : ""}. Use the same account you'll use on your phone.</p>
                            {:else if st.key === "https"}
                              <p>Tailscale only lets this computer publish once HTTPS is on for your tailnet. It's off on a new tailnet, and it's a one-time switch.</p>
                              <p>On the page this opens, find <strong>HTTPS Certificates</strong> and click <strong>Enable HTTPS</strong>. If it asks you to turn on MagicDNS first, do that too.</p>
                              <button class="primary small-btn" onclick={() => openUrl("https://login.tailscale.com/admin/dns")}>Open Tailscale's DNS settings</button>
                            {:else if st.key === "access"}
                              <p>This lets your devices reach the journal while Field Notes is running here. It listens only on this computer until the next step.</p>
                              <button class="primary small-btn" onclick={togglePortal}>Turn on device access</button>
                            {:else if st.key === "publish"}
                              <p>Makes this computer reachable from devices on your tailnet, and nowhere else. Nothing is opened to the internet or your home network.</p>
                              <button class="primary small-btn" disabled={serving} onclick={toggleServe}>{serving ? "Publishing…" : "Publish to my tailnet"}</button>
                            {:else if st.key === "pair"}
                              <p>Install Tailscale on your phone and sign in with the same account. Then pair it below and scan the code with the phone's camera.</p>
                              <button class="primary small-btn" onclick={startPairFromChecklist}>Pair a phone</button>
                            {/if}
                          </div>
                        {/if}
                      </div>
                    </li>
                  {/each}
                </ol>
                {#if !allDone && ["install", "signin", "https"].includes(setupNext ?? "")}
                  <p class="muted small">Come back here when that's done; this list updates by itself.</p>
                {/if}
              {/if}
            </div>
          {/if}

          {#if portalErr}
            <!-- Tailscale's approval link (Serve or HTTPS not yet enabled for the
                 tailnet) is the fix, so it's a button, not text to copy. -->
            {@const approve = portalErr.match(/https:\/\/login\.tailscale\.com\/\S+/)?.[0]}
            <p class="notice bad-notice">
              {portalErr}
              {#if approve}<br /><button class="primary small-btn" onclick={() => openUrl(approve)}>Open the approval page</button>{/if}
            </p>
          {/if}

          {#if portal.running || !ts?.builtin}
            <button
              class="primary small-btn"
              onclick={togglePortal}
              disabled={!portal.running && ts != null && !ts.installed}
            >
              {portal.running ? "Turn off device access" : "Turn on device access"}
            </button>
          {/if}
          {#if !portal.running && ts != null && !ts.installed && !ts.builtin}
            <p class="muted small">Install and sign into Tailscale first — there's no point serving this where only this machine can reach it.</p>
          {/if}

          {#if portal.running}
            <div class="sec-block">
              <h3>Your tailnet</h3>
              {#if ts?.builtin}
                {#if ts.serving}
                  <p class="muted small">
                    Connected to Tailscale as <strong>{ts.login ?? "you"}</strong>, at <strong>{ts.url}</strong>. Only
                    devices signed in to that account can reach it, and every request still needs a paired device's key.
                  </p>
                {:else if ts.signed_in}
                  <p class="muted small">Signed in to Tailscale as <strong>{ts.login ?? "you"}</strong>, but not answering on your tailnet.</p>
                  <button class="primary small-btn" onclick={connectBuiltin}>Connect</button>
                {:else}
                  <p class="muted small">Not connected to Tailscale. Use <strong>Connect</strong> in the steps above.</p>
                {/if}
                <div class="row-actions">
                  {#if ts.serving}<button class="ghost small-btn" onclick={disconnectBuiltin}>Disconnect</button>{/if}
                  {#if ts.signed_in}<button class="ghost small-btn" onclick={signOutBuiltin}>Sign out of Tailscale</button>{/if}
                </div>
                {#if ts.app_available}
                  <p class="muted small">
                    The Tailscale app is installed on this computer too.
                    <button class="link-inline" onclick={() => switchTailscale(true)}>Use it instead of the built-in one</button>
                  </p>
                {/if}
              {:else if !ts?.installed}
                <p class="muted small">
                  ⚠️ Tailscale isn't installed, so the portal is only reachable from this machine.
                  Install Tailscale on this computer and your devices, then come back.
                </p>
              {:else if !tailscaleUrl}
                <p class="muted small">⚠️ Tailscale is installed but isn't logged in — sign in, then reopen this tab.</p>
              {:else if ts.serving}
                <p class="muted small">
                  Published to your tailnet at <strong>{ts.url ?? tailscaleUrl}</strong> — and nothing else can
                  reach it: it's your tailnet, encrypted end to end, and every request still needs a paired
                  device's key.
                </p>
                <button class="ghost small-btn" disabled={serving} onclick={toggleServe}>
                  {serving ? "Working…" : "Stop publishing to my tailnet"}
                </button>

              {:else}
                <p class="muted small">
                  One more step: publish the portal to your tailnet, so your devices can reach it.
                  Tailscale carries it, encrypted — this does not open anything to the internet or to
                  your local network. You can undo it here at any time.
                </p>
                <button class="primary small-btn" disabled={serving} onclick={toggleServe}>
                  {serving ? "Publishing…" : "Publish to my tailnet"}
                </button>
              {/if}
              {#if ts && !ts.builtin}
                <p class="muted small">
                  Using the Tailscale app on this computer.
                  <button class="link-inline" onclick={() => switchTailscale(false)}>Use Field Notes' built-in Tailscale instead</button>
                  (paired phones will need a new code scanned, since the address changes).
                </p>
              {/if}
            </div>
          {/if}

          {#if pinStatus}
            <div class="sec-block">
              <h3>Pairing from your phone</h3>
              {#if pinStatus.method === "password"}
                <p class="muted small">
                  Your phone can pair and un-pair your devices (the gear next to Help). It asks for your journal's
                  password first, so someone holding your phone can't give themselves a key.
                </p>
              {:else}
                <p class="muted small">
                  Your phone can pair and un-pair your devices (the gear next to Help) once you set a PIN here. It asks
                  for the PIN first, so someone holding your phone can't give themselves a key. Your journal isn't
                  encrypted, so there's no password to ask for instead. After 5 wrong tries it waits 15 minutes.
                </p>
                <form class="remote-form" onsubmit={(e) => { e.preventDefault(); savePhonePin(); }}>
                  <input type="password" placeholder={pinStatus.has_pin ? "New PIN" : "PIN (at least 6)"} autocomplete="new-password" bind:value={pinNew} />
                  <input type="password" placeholder="The same again" autocomplete="new-password" bind:value={pinNew2} />
                  <button class="ghost small-btn" type="submit" disabled={pinNew.length < 6}>{pinStatus.has_pin ? "Change PIN" : "Set PIN"}</button>
                </form>
                {#if pinStatus.has_pin}
                  <button class="ghost small-btn" onclick={() => savePhonePin(true)}>Remove PIN (turns pairing from the phone off)</button>
                {/if}
                {#if pinMsg}<p class="muted small" role="status">{pinMsg}</p>{/if}
              {/if}
            </div>
          {/if}

          <div class="sec-block">
            <h3>Pair a device</h3>
            {#if setupPairing}
              <p class="muted small">The code is showing in the setup steps above.</p>
            {:else if !pairing}
              <p class="muted small">
                Give it a name you'll recognise later — you can un-pair each device separately.
              </p>
              <form class="remote-form" onsubmit={(e) => { e.preventDefault(); doPair(); }}>
                <input placeholder="e.g. Phone, Laptop" maxlength="60" bind:value={pairName} bind:this={pairInput} />
                <button class="ghost small-btn" type="submit">Pair</button>
              </form>
            {:else}
              {#if pairedId === pairing.device.id}
                <p class="paired" role="status">
                  <span class="paired-dot" aria-hidden="true"></span>{pairing.device.name} is connected
                </p>
              {/if}
              {#if !ts?.serving}
                <p class="muted small">
                  ⚠️ Not published to your tailnet yet, so this link only works on this computer. Publish
                  first, then the link and code here update to your tailnet address.
                </p>
              {/if}
              <p class="small"><strong>{pairing.device.name}</strong> — pair it one of two ways:</p>
              <ul class="muted small pair-ways">
                <li><strong>A phone:</strong> scan the code with its camera.</li>
                <li><strong>A computer:</strong> copy the link and paste it into Field Notes there, under Settings → Devices &amp; sync → Use another computer as your server. (Send it to yourself some private way — it's a key.)</li>
              </ul>
              <div class="row-actions">
                {#if showQr && portalQrSvg}
                  <button class="ghost small-btn" onclick={() => (showQr = false)}>Hide code</button>
                {:else}
                  <button class="ghost small-btn" onclick={revealQr}>Show QR code…</button>
                {/if}
                <button class="ghost small-btn" onclick={copyPairLink}>{pairLinkCopied ? "Link copied ✓" : "Copy link"}</button>
                <button class="ghost small-btn" onclick={donePairing}>Done</button>
              </div>
              {#if showQr && portalQrSvg}
                <div class="qr">{@html portalQrSvg}</div>
              {/if}
              <p class="muted small">
                <strong>This is a key</strong> — whoever has it can read and write this journal. Don't leave
                it on screen, and don't photograph it. It's shown only until you press Done.
              </p>
            {/if}
          </div>

          {#if devices.length}
            <div class="sec-block">
              <h3>Paired devices</h3>
              <ul class="device-list">
                {#each devices as d (d.id)}
                  <li>
                    <span><strong>{d.name}</strong>{#if d.person !== 1} <span class="person-tag">{personOf(d.person)}</span>{/if}<br /><span class="muted small">{whenSeen(d.last_seen)}</span></span>
                    <button class="ghost small-btn" onclick={() => revokeDevice(d)}>Un-pair</button>
                  </li>
                {/each}
              </ul>
            </div>
          {/if}

          <div class="sec-block">
            <h3>People</h3>
            <p class="muted small">
              Someone else who uses this server gets their own journal, locked with a password only they know. You can
              pair their devices and remove them, but you can't open their journal: this list shows names, devices and
              whether a journal is locked, never entries.
            </p>
            {#if people.length}
              <ul class="device-list">
                {#each people as p (p.id)}
                  <li class="person-row">
                    <span>
                      <strong>{p.name}</strong><br />
                      <span class="muted small">
                        {deviceCount(p.id)} {deviceCount(p.id) === 1 ? "device" : "devices"} · {personState(p)}
                      </span>
                    </span>
                    <span class="row-actions">
                      <button class="ghost small-btn" onclick={() => pairForPerson(p)}>Pair a device for them</button>
                      <button class="ghost small-btn" onclick={() => { removingPerson = removingPerson === p.id ? null : p.id; removeTyped = ""; }}>Remove…</button>
                    </span>
                  </li>
                  {#if removingPerson === p.id}
                    <li class="person-remove">
                      <form onsubmit={(e) => { e.preventDefault(); removePerson(p); }}>
                        <p class="small">
                          <strong>This deletes {p.name}'s journal and un-pairs all their devices.</strong> It can't be undone,
                          and because their journal is encrypted with their password, nobody can open it first to check
                          what's in it or keep a copy. If they want to keep any entries, they can export them one at a time from their own phone first.
                        </p>
                        <label class="small" for="remove-{p.id}">Type <strong>{p.name}</strong> to confirm</label>
                        <span class="remote-form">
                          <input id="remove-{p.id}" bind:value={removeTyped} autocomplete="off" />
                          <button class="danger-btn" type="submit" disabled={removeTyped.trim().toLowerCase() !== p.name.toLowerCase()}>Remove {p.name}</button>
                          <button class="ghost small-btn" type="button" onclick={() => (removingPerson = null)}>Cancel</button>
                        </span>
                      </form>
                    </li>
                  {/if}
                {/each}
              </ul>
            {/if}

            {#if personPairing}
              {@const pp = personPairing}
              {#if pairedId === pp.pair.device.id}
                <p class="paired" role="status">
                  <span class="paired-dot" aria-hidden="true"></span>{pp.pair.device.name} is connected
                </p>
              {/if}
              <p class="small">
                <strong>Pair {pp.who.name}'s device.</strong> Let them scan the code with their phone's camera. The first
                time, their phone asks them to choose a password for their journal.
              </p>
              {#if !ts?.serving}
                <p class="muted small">
                  Not published to your tailnet yet, so this link only works on this computer. Publish first.
                </p>
              {/if}
              <div class="row-actions">
                {#if personQrSvg}
                  <button class="ghost small-btn" onclick={() => (personQrSvg = null)}>Hide code</button>
                {:else}
                  <button class="ghost small-btn" onclick={revealPersonQr}>Show QR code…</button>
                {/if}
                <button class="ghost small-btn" onclick={copyPersonLink}>{personLinkCopied ? "Link copied ✓" : "Copy link"}</button>
                <button class="ghost small-btn" onclick={donePersonPairing}>Done</button>
              </div>
              {#if personQrSvg}
                <div class="qr">{@html personQrSvg}</div>
              {/if}
              <p class="muted small">
                <strong>This is a key to {pp.who.name}'s journal,</strong> though it can't open it without their password.
                Show it only to them. It's shown only until you press Done.
              </p>
            {:else}
              <form class="remote-form" onsubmit={(e) => { e.preventDefault(); addPerson(); }}>
                <input placeholder="Their name, e.g. Sam" maxlength="40" bind:value={personName} />
                <button class="ghost small-btn" type="submit" disabled={!personName.trim()}>Add a person</button>
              </form>
            {/if}
            {#if personErr}<p class="notice bad-notice">{personErr}</p>{/if}
          </div>

          <div class="sec-block">
            <h3>Server Mode</h3>
            <p class="muted small">
              For a computer that <em>is</em> your server — one that stays on so your other devices can
              always reach it. Leave these off on a laptop you carry around.
            </p>
            <label class="share">
              <input type="checkbox" checked={sprefs.serve_on_launch} onchange={toggleServeOnLaunch} />
              Turn on device access whenever Field Notes opens (and republish to my tailnet at the same address)
            </label>
            <label class="share">
              <input type="checkbox" checked={loginStart} onchange={toggleLoginStart} />
              Open Field Notes when I log in to this computer
            </label>
            <label class="share">
              <input type="checkbox" checked={sprefs.menu_bar} onchange={toggleMenuBar} />
              Run from the {trayName} instead of the {isMac ? "Dock" : "taskbar"}
            </label>
            {#if sprefs.menu_bar}
              <p class="muted small">
                Field Notes keeps running when you close its window, so your devices can still reach it.
                Open the window or quit from the small notebook icon {isMac ? "at the top of the screen, next to the clock" : "next to the clock"}.
                {#if loginStart}When it opens at login, it starts without a window.{/if}
              </p>
            {/if}
            <label class="share">
              <input type="checkbox" checked={sprefs.phone_can_update} onchange={togglePhoneCanUpdate} />
              Let paired phones install Field Notes updates on this computer
            </label>
            {#if sprefs.phone_can_update}
              <p class="muted small">
                Installing restarts Field Notes here. A phone can only do it when device access turns on by
                itself at launch, the journal can unlock itself (its password saved below, if it's encrypted),
                and no trip report is live, including anyone else's who has used Field Notes in the last two hours.
                If it would lock someone else's journal, the phone says whose and lets you install anyway.
                Only signed Field Notes releases can be installed.
              </p>
            {/if}
            {#if kc.applicable}
              {#if kc.remembered}
                <p class="small">
                  ✓ Your journal password is saved in this computer's keychain, so the journal unlocks by itself
                  when Field Notes opens.
                </p>
                <button class="ghost small-btn" onclick={forgetPassword}>Forget the saved password</button>
              {:else}
                <p class="muted small">
                  Your journal is encrypted, so after a restart it stays locked — and serves nothing — until
                  someone types the password here. You can save the password in this computer's keychain
                  instead. <strong>The trade-off:</strong> anyone who can log in to this computer's user account
                  could then open the journal. Encryption still protects the file if the disk or a backup is
                  taken.
                </p>
                <form class="remote-form" onsubmit={(e) => { e.preventDefault(); rememberPassword(); }}>
                  <input type="password" autocomplete="current-password" placeholder="Journal password" bind:value={kcPass} />
                  <button class="ghost small-btn" type="submit" disabled={kcBusy || !kcPass}>Save in keychain</button>
                </form>
              {/if}
              {#if kcErr}<p class="notice bad-notice">{kcErr}</p>{/if}
            {/if}
          </div>
          {/if}
        </section>
      {/if}

      {#if settingsSection === "data"}
        <section class="card">
          <h2>Encryption at rest</h2>
          {#if remote.connected}
            <p class="muted small">
              While connected to {serverName}, this computer keeps only the connection's key, the last copy it
              saw, and anything waiting to be sent — encrypting it protects those.
            </p>
          {/if}
          {#if db.encrypted}
            <p class="muted small">
              This journal is <strong>encrypted</strong>. Its contents are unreadable on disk without your
              password, which you enter each time you open the app.
            </p>

            <div class="sec-block">
              <h3>Change password</h3>
              <input type="password" autocomplete="current-password" placeholder="Current password" bind:value={chgCurrent} />
              <input type="password" autocomplete="new-password" placeholder="New password" bind:value={chgNew} />
              <input type="password" autocomplete="new-password" placeholder="Confirm new password" bind:value={chgNew2} />
              <button class="primary small-btn" disabled={secBusy} onclick={doChangePassphrase}>Change password</button>
            </div>

            <div class="sec-block">
              <h3>Turn off encryption</h3>
              <p class="muted small">Returns the journal to plaintext on this device.</p>
              <input type="password" autocomplete="current-password" placeholder="Current password" bind:value={encDisablePass} />
              <button class="ghost small-btn" disabled={secBusy} onclick={doDisableEncryption}>Disable encryption</button>
            </div>
          {:else}
            <p class="muted small">
              The journal is currently stored <strong>unencrypted</strong>. Turn on encryption to protect it with a
              password (AES-256 via SQLCipher). You'll enter the password each time you open the app.
            </p>
            <p class="notice warn-notice">
              There is no recovery. If you forget this password, the journal cannot be opened by anyone — including you.
            </p>
            <div class="sec-block">
              <input type="password" autocomplete="new-password" placeholder="Choose a password" bind:value={encNewPass} />
              <input type="password" autocomplete="new-password" placeholder="Confirm password" bind:value={encNewPass2} />
              <button class="primary small-btn" disabled={secBusy} onclick={doEnableEncryption}>Enable encryption</button>
            </div>
          {/if}
        </section>

        <section class="card">
          <h2>Backup &amp; restore</h2>
          {#if remote.connected}
            <p class="muted small">Your journal lives on {serverName} — back it up there.</p>
          {:else}
          <p class="muted small">
            A backup is a single-file copy of your whole journal. {db.encrypted
              ? "It keeps its encryption — you'll need this password to restore or open it elsewhere."
              : "The journal itself is unencrypted, so a plain backup is too — store it somewhere safe, or encrypt it below."}
          </p>

          {#if !db.encrypted}
            <label class="dont-show">
              <input type="checkbox" bind:checked={bkEncrypt} />
              Encrypt this backup with a password
            </label>
            {#if bkEncrypt}
              <div class="sec-block">
                <input type="password" autocomplete="new-password" placeholder="Backup password" bind:value={bkPassword} />
                <input type="password" autocomplete="new-password" placeholder="Confirm backup password" bind:value={bkPassword2} />
                <p class="muted small">You'll need this password to restore the backup — there's no recovery if you lose it.</p>
              </div>
            {/if}
          {/if}

          <div class="row-actions">
            <button class="primary small-btn" disabled={secBusy} onclick={doExportBackup}>Export backup…</button>
            <button class="ghost small-btn" disabled={secBusy} onclick={doImportBackup}>Restore from backup…</button>
          </div>
          <p class="muted small">Restoring replaces the journal on this device with the backup's contents. An encrypted backup opens the unlock screen so you can enter its password.</p>
          {/if}
        </section>

        <section class="card">
          <h2>Obsidian vault sync</h2>
          {#if remote.connected}
            <p class="muted small">Your journal lives on {serverName} — sync your vault from Field Notes there.</p>
          {:else}
          <p class="muted small">
            Keep a copy of your journal in an Obsidian vault as Markdown notes — one per experience, with a
            readable summary you can annotate. The sync itself is fully offline.
          </p>
          <p class="notice warn-notice">
            Notes exported to your vault are <strong>plain, unencrypted Markdown</strong> that lives outside this
            app — Field Notes' encryption does <em>not</em> protect them. If your vault syncs to iCloud, Obsidian
            Sync, Dropbox, Git, or similar, this sensitive data <strong>leaves your device</strong> and is subject
            to that service's security. <strong>Sync at your own risk</strong>, and prefer a local-only vault for
            anything you want kept private.
          </p>
          {#if obsErr}<p class="notice bad-notice">{obsErr}</p>{/if}
          {#if obsMsg}<p class="notice good-notice">{obsMsg}</p>{/if}

          <div class="vault-pick">
            <input readonly placeholder="No vault folder chosen" value={vaultFolder} />
            <button class="ghost small-btn" disabled={obsBusy} onclick={chooseVaultFolder}>Choose folder…</button>
          </div>
          <div class="row-actions">
            <button class="primary small-btn" disabled={obsBusy || !vaultFolder} onclick={doObsidianExport}>Export to vault →</button>
            <button class="ghost small-btn" disabled={obsBusy || !vaultFolder} onclick={doObsidianImport}>← Import from vault</button>
          </div>
          <p class="muted small">
            Export overwrites this app's own notes in that folder (app → vault). Import pulls experiences back in;
            for anything already here, the vault's copy wins (vault → app). Hand-written notes are left untouched.
          </p>
          {/if}
        </section>

        <section class="card danger-card">
          <h2>Erase &amp; uninstall</h2>
          <p class="muted small">
            Your journal lives entirely on this device, in:
          </p>
          {#if dataDirPath}
            <div class="vault-pick">
              <input readonly value={dataDirPath} />
              <button class="ghost small-btn" disabled={secBusy} onclick={showDataFolder}>Show folder</button>
            </div>
          {/if}

          <div class="sec-block">
            <h3>Erase all data</h3>
            <p class="muted small">
              Permanently delete every experience, dose, note, substance, and setting on this device, and turn off
              encryption. This cannot be undone. Backups you've exported and notes already in an Obsidian vault are
              <em>not</em> touched.
            </p>
            <button class="danger-btn" disabled={secBusy} onclick={eraseAllData}>Erase all data…</button>
          </div>

          <div class="sec-block">
            <h3>Remove the app</h3>
            {#if isMac}
              <p class="muted small">
                Quit Field Notes, then open <strong>Applications</strong> and drag <strong>Field Notes</strong> to the
                Trash. To leave nothing behind, also delete the data folder above (erase your data first, or just
                delete the folder).
              </p>
            {:else if isWindows}
              <p class="muted small">
                Quit Field Notes, then open <strong>Settings → Apps → Installed apps</strong>, find
                <strong>Field Notes</strong>, and choose <strong>Uninstall</strong>. To leave nothing behind, also
                delete the data folder above (erase your data first, or just delete the folder).
              </p>
            {:else}
              <p class="muted small">
                Quit Field Notes, then remove it the way you installed it — delete the <code>.AppImage</code>, or
                <code>sudo apt remove field-notes</code> / <code>sudo dnf remove field-notes</code>. To leave nothing
                behind, also delete the data folder above.
              </p>
            {/if}
            <button class="ghost small-btn" disabled={secBusy} onclick={quitApp}>Quit Field Notes</button>
          </div>
        </section>
      {/if}

      {#if settingsSection === "feedback"}
        <section class="card">
          <h2>Feedback</h2>
          <p class="muted small">
            Hit a bug, or wish it did something it doesn't? Open an issue on GitHub. It opens
            prefilled in your browser — nothing leaves your journal, and you write and send it
            there. Reports are worked through in batches.
          </p>
          <div class="fb-kind">
            <label><input type="radio" bind:group={fbKind} value="bug" /> Report a bug</label>
            <label><input type="radio" bind:group={fbKind} value="feature" /> Request a feature</label>
          </div>
          <input
            class="fb-field"
            placeholder={fbKind === "bug" ? "What went wrong? (short summary)" : "What would you like? (short summary)"}
            bind:value={fbSummary}
          />
          <textarea
            class="fb-field"
            rows="3"
            placeholder="Any detail you want to include (optional — you can also write it on GitHub)"
            bind:value={fbDetail}
          ></textarea>
          <button class="ghost small-btn" onclick={openFeedback}>Continue on GitHub →</button>
        </section>
      {/if}
    {/if}

    <footer>
      “There are no casual experiments.”
      <div class="footer-sub">— Sasha Shulgin</div>
    </footer>
  </main>

  <!-- ============ EMERGENCY / PANIC RESOURCES ============ -->
  {#if showHelp}
    <!-- Only Close closes it. A click outside used to dismiss it, and one stray
         click is easy to make at exactly the moment this screen is needed. -->
    <div class="modal-overlay" role="presentation">
      <div class="help-modal" role="dialog" aria-modal="true" aria-labelledby="help-title" tabindex="-1">
        <h2 id="help-title">Get help now</h2>
        <p class="help-first"><strong>If someone is in danger, call your local emergency number now.</strong></p>
        <ul class="crisis-res">
          {#each helpInPerson as r}
            <li>
              <strong>{r.label}</strong>{#if r.contact} — <span class="contact">{r.contact}</span>{/if}
              <br /><span class="muted small">{r.detail}</span>
            </li>
          {/each}
        </ul>
        <p class="muted small">
          You're not alone. These lines are staffed by people who want to help, and calling is always okay.
          Field Notes is not an emergency service.
        </p>
        <ul class="crisis-res">
          {#each helpCallable as r}
            <li>
              <strong>{r.label}</strong>{#if r.contact} — <span class="contact">{r.contact}</span>{/if}
              <br /><span class="muted small">{r.detail}</span>
            </li>
          {/each}
        </ul>
        <button class="primary" onclick={() => (showHelp = false)}>Close</button>
      </div>
    </div>
  {/if}

  {#if showFeedback}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="modal-overlay" role="presentation" onclick={() => (showFeedback = false)}>
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div class="help-modal" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()}>
        <h2>{fbKind === "bug" ? "Report a bug" : "Request a feature"}</h2>
        <p class="muted small">
          Opens a prefilled issue on GitHub in your browser — nothing leaves your journal; you
          write and send it there.
        </p>
        <div class="fb-kind">
          <label><input type="radio" bind:group={fbKind} value="bug" /> Report a bug</label>
          <label><input type="radio" bind:group={fbKind} value="feature" /> Request a feature</label>
        </div>
        <input
          class="fb-field"
          placeholder={fbKind === "bug" ? "What went wrong? (short summary)" : "What would you like? (short summary)"}
          bind:value={fbSummary}
        />
        <textarea
          class="fb-field"
          rows="3"
          placeholder="Any detail you want to include (optional — you can also write it on GitHub)"
          bind:value={fbDetail}
        ></textarea>
        <div class="row-actions">
          <button class="ghost small-btn" onclick={() => (showFeedback = false)}>Cancel</button>
          <button class="primary small-btn" onclick={sendFeedback}>Continue on GitHub →</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- ============ LIVE SESSION ============ -->
  {#if liveSession && selected}
    <div class="live">
      <div class="live-bar">
        <div>
          <div class="live-title">{selected.title || "Live trip report"}</div>
          <div class="muted">
            Started {fmtTime(selected.started_at)} · {elapsedSince(selected.started_at)} in
            <!-- Ticks with lsNow: the current t+ is the number a sitter actually
                 wants at a glance, against which every row below is read. -->
            {#if sessionT0}<span class="rel live-rel">· now {relTime(new Date(lsNow).toISOString(), sessionT0)}</span>{/if}
          </div>
          {#if latestDose(selected.doses)}
            {@const ld = latestDose(selected.doses)!}
            <!-- The question before any redose, answered without looking for it. -->
            <div class="live-last">Last: <strong>{nameShown(ld.substance_name)} {ld.amount ?? "?"} {ld.unit}</strong> · {gapText(Date.parse(ld.taken_at), lsNow)} ago</div>
          {/if}
        </div>
        <span class="live-bar-actions">
          <button class="tb" aria-pressed={night} onclick={toggleNight}>{night ? "Normal colours" : "Dim (red)"}</button>
          <button class="help-btn" onclick={openHelp}>Get help now</button>
        </span>
      </div>

      <div class="live-alerts">
        {@render crisisBanner()}
        {@render doseWarnings()}
      </div>

      <!-- What each substance is likely doing now, all doses of it added in. -->
      <ActiveArcs doses={arcDoses} moments={selected.timeline} now={lsNow} />

      <div class="live-body">
        <section class="live-timeline">
          <h3>Doses</h3>
          {#if selected.doses.length}
            <ul class="live-doses">
              {#each selected.doses as d}
                <li><span class="muted">{fmtTime(d.taken_at)}{#if sessionT0}<span class="rel"> ({relTime(d.taken_at, sessionT0)})</span>{/if}</span> — {nameShown(d.substance_name)} {describeAmount(d.substance_name, d.amount, d.unit, d)}{d.route ? " · " + d.route : ""}{#if d.profile}<span class="muted" title={d.profile.note}> · {d.profile.label}</span>{/if}</li>
              {/each}
            </ul>
          {:else}
            <p class="muted">Nothing logged yet.</p>
          {/if}

          <h3>Quick log</h3>
          <div class="quick-log">
            <SubstanceInput placeholder="Substance" bind:value={qSub} />
            <input placeholder="Amount" inputmode="decimal" bind:value={qAmt} />
            <input placeholder="Unit" bind:value={qUnit} />
            <input placeholder="Route" bind:value={qRoute} />
            <button class="primary" disabled={!qSub.trim()} onclick={logIntoSession}>Log dose</button>
          </div>
          <DoseDetailFields substance={qSub} unit={qUnit} amount={qAmt} bind:detail={qDetail} />
          {#if lastLogged}
            <p class="small live-saved">✓ Logged {lastLogged.label}. <button class="link" onclick={undoLastLogged}>Undo</button></p>
            <KindQuestion substance={qAsk.s} tick={qAsk.n} />
          {/if}

          <h3>Timeline</h3>
          <div class="quick-log">
            <input placeholder="How are you feeling right now?" bind:value={lsNote} onkeydown={(e) => e.key === "Enter" && quickNote()} />
            <button class="moment-btn" disabled={!lsNote.trim()} onclick={() => quickNote()}>Add moment</button>
          </div>
          <div class="mood-row" role="group" aria-label="Add a moment in one click">
            {#each MOODS as m}<button class="chip-btn" onclick={() => quickNote(m)}>{m}</button>{/each}
          </div>
          {#if selected.timeline.length}
            <ul class="live-events">
              {#each selected.timeline as t}
                <li><span class="muted">{fmtTime(t.at)}{#if sessionT0}<span class="rel"> ({relTime(t.at, sessionT0)})</span>{/if}</span> {#if t.mood}<strong>{t.mood}</strong>{t.note ? " · " : ""}{/if}<span class="reflect">{t.note}</span></li>
              {/each}
            </ul>
          {/if}
        </section>

        {#if !companionOff}
          <section class="live-companion">
            <h3>Companion</h3>
            {#if !aiReady}
              <p class="muted">The local companion isn't set up. You can still log and use the timeline.</p>
            {:else}
              <div class="chat live-chat">
                {#if !cMessages.length}
                  <p class="muted chat-empty">I'm here with you. Say anything — or just check in.</p>
                {/if}
                {#each cMessages as m}
                  <div class="bubble {m.role}">{m.content}</div>
                {/each}
                {#if cSending}<div class="bubble assistant muted">…</div>{/if}
              </div>
              {#if cActions.length}
                <div class="actions-note">{#each cActions as a}<span class="action-chip">✓ {a}</span>{/each}</div>
              {/if}
              <div class="chat-input">
                <input placeholder="Talk to your companion…" bind:value={cInput} onkeydown={(e) => e.key === "Enter" && sendCompanion()} />
                <button class="primary" disabled={cSending} onclick={sendCompanion}>Send</button>
              </div>
            {/if}
          </section>
        {/if}
      </div>
      <!-- Leaving sits at the bottom, away from Get help and logging. -->
      <div class="live-foot">
        <button class="ghost" onclick={endLiveSession}>Leave live view</button>
      </div>
    </div>
  {/if}
{/if}

<style>
  :global(:root) {
    --bg: #14120f;
    --card: #1c1916;
    --ink: #eee7dc;
    --muted: #b8ad9c;
    --line: #322c25;
    --accent: #b9a3c9;
    --accent-ink: #11141c;
    /* Outline of secondary buttons, 3:1 or better on the card. */
    --edge: #8f8576;
    /* Help's own colour, apart from danger red (see the phone page). */
    --help-bg: #f2dcc4;
    --help-ink: #1d140b;
    --danger: #f2897a;
    --caution: #e6b062;
    --note: #93cf9e;
    --surface-2: #27231e;
    --field: #0f0d0b;
    /* The sidebar, a shade below the page. */
    --pane: #100e0c;
  }
  /* Dim (red) night theme on the live screen. Same reds as the phone; every pair
     meets 4.5:1 on black, and warnings keep their words, never hue alone. */
  :global(:root[data-theme="night"]) {
    --bg: #000000;
    --card: #0a0100;
    --ink: #ff7a5c;
    --muted: #d9533a;
    --line: #2a0a06;
    --accent: #ff5a36;
    --accent-ink: #000000;
    --edge: #a8432c;
    --help-bg: #ff7a5c;
    --help-ink: #000000;
    --danger: #ffb199;
    --caution: #ff9a6b;
    --note: #ff9a6b;
    --surface-2: #170403;
    --field: #000000;
    --pane: #000000;
  }
  :global(body) {
    margin: 0;
    background: var(--bg);
    color: var(--ink);
    font-family: var(--font-data, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif);
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
  /* Clean and utilitarian, after the panel's Obsidian comparison: quiet chrome,
     loud content. A small sidebar a shade below the page, one thin bar per view,
     and content directly on the page in a reading column, not in cards. The live
     screen and Help keep their size. A narrow window folds the sidebar into a row. */
  nav {
    position: fixed; top: 0; left: 0; bottom: 0; width: 210px; z-index: 20;
    display: flex; flex-direction: column; justify-content: space-between; gap: 1rem;
    padding: 3rem 0.5rem 0.6rem; background: var(--pane); border-right: 1px solid var(--line);
    box-sizing: border-box; overflow-y: auto;
  }
  .nav-main, .nav-foot { display: flex; flex-direction: column; gap: 1px; }
  .nav-help-row { display: flex; align-items: center; gap: 0.3rem; margin-top: 0.4rem; }
  nav button {
    display: flex; align-items: center; gap: 0.6rem; width: 100%; min-height: 34px;
    border: none; background: transparent; color: var(--muted); font: inherit; font-size: 0.875rem; font-weight: 600;
    padding: 0.35rem 0.65rem; border-radius: 6px; cursor: pointer; text-align: left;
  }
  nav button :global(svg) { width: 16px; height: 16px; }
  nav button:hover { color: var(--ink); background: var(--surface-2); }
  nav button.active { color: var(--ink); background: var(--surface-2); }
  nav button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  nav button.nav-help { flex: 1; justify-content: center; border-radius: 999px; min-height: 36px; }
  main:has(> header nav) { max-width: 760px; margin-left: max(210px, calc(210px + (100vw - 210px - 760px) / 2)); margin-right: auto; padding-top: calc(44px + 1.2rem); }
  /* The app's name sits small at the top of the sidebar. */
  main:has(> header nav) > header h1 { position: fixed; top: 1rem; left: 1.15rem; z-index: 21; font-size: 0.78rem; letter-spacing: 0.08em; text-transform: uppercase; color: var(--muted); }
  main:has(> header nav) > header { margin: 0; }
  .topbar {
    position: fixed; top: 0; left: 210px; right: 0; height: 44px; z-index: 19; box-sizing: border-box;
    display: flex; align-items: center; justify-content: space-between; gap: 1rem;
    padding: 0 1rem 0 1.4rem; background: var(--bg); border-bottom: 1px solid var(--line);
  }
  .topbar-title { font-size: 0.9rem; font-weight: 700; display: flex; gap: 0.5rem; align-items: baseline; white-space: nowrap; }
  .topbar-title span { color: var(--muted); font-weight: 400; }
  .tools { display: flex; gap: 2px; align-items: center; flex-wrap: wrap; justify-content: flex-end; }
  .tb {
    font: inherit; font-size: 0.85rem; color: var(--muted); background: transparent; border: 0; border-radius: 6px;
    padding: 0.3rem 0.55rem; display: inline-flex; align-items: center; gap: 0.4rem; cursor: pointer; white-space: nowrap;
  }
  .tb:hover, .tb[aria-pressed="true"] { background: var(--surface-2); color: var(--ink); }
  .tb:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  .tb-primary { margin-left: 0.5rem; padding: 0.3rem 0.85rem; font-size: 0.85rem; }
  @media (max-width: 820px) {
    nav {
      position: static; width: 100%; flex-direction: row; flex-wrap: wrap; padding: 4px;
      border: 1px solid var(--line); border-radius: 14px; overflow: visible;
    }
    .nav-main, .nav-foot { flex-direction: row; flex-wrap: wrap; align-items: center; }
    .nav-help-row { margin-top: 0; }
    nav button { width: auto; padding: 0.4rem 0.8rem; }
    main:has(> header nav) { margin-left: auto; padding-top: 1.2rem; }
    main:has(> header nav) > header { margin-bottom: 1rem; }
    main:has(> header nav) > header h1 { position: static; font-size: 1rem; }
    .topbar { position: static; width: 100%; padding: 0; background: none; }
  }

  /* Sections sit on the page, divided by a hairline, rather than in cards. */
  .card { background: none; border: 0; border-radius: 0; padding: 1.2rem 0 1.4rem; }
  .card + .card, .card + section, section + .card { border-top: 1px solid var(--line); }
  header + .card, header + section { padding-top: 0.2rem; }
  h2 { margin: 0 0 0.6rem; font-size: 1.05rem; }
  h3 { margin: 1.2rem 0 0.4rem; font-size: 0.95rem; color: var(--muted); text-transform: uppercase; letter-spacing: 0.04em; }
  p { line-height: 1.5; }
  .muted { color: var(--muted); }
  .small { font-size: 0.85rem; }

  button { font: inherit; cursor: pointer; border-radius: 9px; border: 1px solid transparent; }
  /* As on the phone: rectangles hold things, pills do things. Filled for the main
     action, outlined for the rest, plain text for the least. */
  .primary { background: var(--accent); color: var(--accent-ink); font-weight: 600; padding: 0.55rem 1.15rem; border-radius: 999px; }
  .primary:hover { filter: brightness(1.08); }
  .ghost { background: transparent; color: var(--ink); border: 1.5px solid var(--edge); font-weight: 600; padding: 0.55rem 1.15rem; margin-top: 0.8rem; border-radius: 999px; }
  .ghost:hover { background: var(--surface-2); }
  .primary:active:not(:disabled), .ghost:active:not(:disabled) { transform: translateY(1px); }
  .small-btn { padding: 0.4rem 0.9rem; margin: 0; white-space: nowrap; font-size: 0.92rem; }
  /* + Moment on the live screen: outlined in the accent, so Log dose leads. */
  .moment-btn { background: transparent; color: var(--accent); border: 1.5px solid var(--accent); font-weight: 600; padding: 0.55rem 1.15rem; border-radius: 999px; }
  .moment-btn:hover { background: color-mix(in srgb, var(--accent) 12%, transparent); }
  button:disabled { opacity: 0.55; cursor: default; }
  .link { background: none; border: none; color: var(--accent); padding: 0; font-weight: 600; cursor: pointer; margin-bottom: 0.6rem; }
  .link-inline { background: none; border: none; color: var(--accent); padding: 0; font: inherit; font-weight: 600; cursor: pointer; text-decoration: underline; }
  .prereq { border: 1px solid var(--line); border-radius: 10px; padding: 0.5rem 0.8rem; margin: 0.2rem 0 0.9rem; }
  .prereq.missing { border-color: var(--caution); }
  .prereq p { color: var(--muted); margin: 0.3rem 0; }
  .prereq-status { font-weight: 600; }

  input { font: inherit; background: var(--bg); color: var(--ink); border: 1px solid var(--line); border-radius: 8px; padding: 0.55rem 0.7rem; }
  input.narrow { width: 5.5rem; }

  .exp-head { display: flex; flex-wrap: wrap; justify-content: space-between; align-items: center; gap: 0.8rem; }
  .exp-list, .sub-list, .doses, .timeline { list-style: none; padding: 0; margin: 0.6rem 0 0; }
  /* Flat rows with dividers: the card is the only box, not a card of cards. */
  .exp-row { width: 100%; text-align: left; background: transparent; border: none; border-bottom: 1px solid var(--line); border-radius: 0; padding: 0.6rem 0.3rem; margin: 0; display: grid; grid-template-columns: 4rem 1fr auto; align-items: baseline; gap: 0.7rem; color: var(--ink); font: inherit; cursor: pointer; }
  .exp-date { font-size: 0.82rem; color: var(--muted); font-variant-numeric: tabular-nums; }
  .exp-main { min-width: 0; }
  .exp-list .month { font-size: 0.75rem; letter-spacing: 0.08em; text-transform: uppercase; color: var(--muted); font-weight: 700; padding: 1rem 0.3rem 0.2rem; }
  .exp-list .month:first-child { padding-top: 0.2rem; }
  .exp-list li:last-child .exp-row { border-bottom: none; }
  .exp-row:hover { background: var(--surface-2); }
  .exp-row:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .first-steps { border: 1px solid var(--line); border-radius: 12px; padding: 0.9rem 1.1rem; margin-top: 0.6rem; }
  .first-steps ul { margin: 0.6rem 0 0; padding-left: 1.1rem; display: grid; gap: 0.55rem; }
  .first-steps li { line-height: 1.5; color: var(--muted); }
  .first-steps li strong { color: var(--ink); }
  /* Phone setup checklist. */
  .setup { border: 1px solid var(--line); border-radius: 12px; padding: 0.9rem 1rem; margin: 1rem 0; }
  .store-qrs { display: flex; flex-wrap: wrap; gap: 1rem; }
  .store-qrs figure { margin: 0; text-align: center; }
  .store-qrs figcaption { font-size: 0.85rem; color: var(--muted); }
  .setup.done { border-color: color-mix(in srgb, var(--note) 40%, var(--line)); }
  .setup-head { display: flex; justify-content: space-between; align-items: center; gap: 0.6rem; }
  .setup-head h3 { margin: 0; font-size: 1rem; }
  .setup-steps { list-style: none; padding: 0; margin: 0.8rem 0 0; display: grid; gap: 0.55rem; }
  .setup-steps li { display: grid; grid-template-columns: 1.7rem 1fr; gap: 0.6rem; align-items: start; color: var(--muted); }
  .setup-steps li.next, .setup-steps li.done { color: var(--ink); }
  .setup-mark { width: 1.6rem; height: 1.6rem; border-radius: 999px; display: grid; place-items: center; font-size: 0.85rem; font-weight: 700; border: 1px solid var(--line); }
  .setup-steps li.done .setup-mark { background: color-mix(in srgb, var(--note) 22%, transparent); border-color: transparent; color: var(--note); }
  .setup-steps li.next .setup-mark { background: var(--accent); color: var(--accent-ink); border-color: transparent; }
  .setup-label { display: block; line-height: 1.6rem; font-weight: 600; }
  .setup-steps li.done .setup-label { font-weight: 400; }
  .setup-fix { margin-top: 0.3rem; }
  .setup-fix p { margin: 0 0 0.55rem; color: var(--muted); line-height: 1.5; }
  .visually-hidden { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }
  .live-bar-actions { display: inline-flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; }
  .combo-form { display: flex; gap: 0.5rem; margin: 0.6rem 0; }
  .combo-form input { flex: 1; min-width: 0; }
  .exp-row strong { display: block; }
  .exp-meta { display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap; justify-content: flex-end; }
  .pill { font-size: 0.72rem; border: 1px solid var(--line); border-radius: 999px; padding: 0.1rem 0.5rem; color: var(--muted); }

  /* The quick log sits above the journal list, so it's bounded like the import
     panel rather than floating loose in the card. */
  .quick-log { margin: 0.6rem 0 1rem; }
  .quick-row { display: flex; flex-wrap: wrap; gap: 0.5rem; margin: 0.6rem 0 0; align-items: center; }
  .quick-row > input:first-child,
  .quick-row > :global(.sub-suggest):first-child { flex: 1; min-width: 9rem; }
  /* Selects aren't styled app-wide (nothing else uses one in a form row), and a
     default white dropdown in this dark card reads as a rendering bug. */
  .quick-row select {
    font: inherit; background: var(--bg); color: var(--ink);
    border: 1px solid var(--line); border-radius: 8px; padding: 0.55rem 0.5rem;
  }
  .chips { display: flex; flex-wrap: wrap; gap: 0.35rem; }
  .saved-line { display: flex; flex-wrap: wrap; align-items: center; gap: 0.6rem; margin: 0.8rem 0 0; padding-top: 0.7rem; border-top: 1px solid var(--line); }
  .saved-line .small-btn { margin-top: 0; }

  .new-exp, .dose-form, .new-sub { display: flex; flex-wrap: wrap; gap: 0.5rem; margin: 0.8rem 0; align-items: center; }
  .new-exp input, .new-sub input { flex: 1; min-width: 8rem; }
  .past-toggle { display: flex; align-items: center; gap: 0.45rem; margin: -0.3rem 0 0.8rem; color: var(--muted); font-size: 0.85rem; cursor: pointer; }
  .past-toggle input { cursor: pointer; }
  .new-note { display: flex; flex-direction: column; gap: 0.5rem; margin: 0.8rem 0; }
  .new-note textarea { width: 100%; resize: vertical; }
  .note-pill { font-style: italic; }
  .note-body { white-space: pre-wrap; margin-top: 0.6rem; line-height: 1.6; font-family: var(--font-reflect); font-size: 1.06rem; max-width: 66ch; }
  /* Reflection is set in Literata; everything read at a glance stays in Atkinson. */
  .reflect { font-family: var(--font-reflect); }
  textarea.reflect, input.reflect { font-family: var(--font-reflect); font-size: 1.02rem; line-height: 1.55; }
  .write-up { white-space: pre-wrap; line-height: 1.6; font-size: 1.06rem; max-width: 66ch; margin-top: 0.2rem; }
  .dose-form input:first-child { flex: 1; min-width: 8rem; }

  .doses li, .timeline li { display: flex; gap: 0.7rem; padding: 0.4rem 0; border-bottom: 1px solid var(--line); font-size: 0.92rem; align-items: baseline; }
  .doses li:last-child, .timeline li:last-child { border-bottom: none; }
  /* Wide enough for "14:02 (t+10:35)" so the notes beside it stay aligned. */
  .dtime { color: var(--muted); font-variant-numeric: tabular-nums; min-width: 3.4rem; flex-shrink: 0; white-space: nowrap; }
  .dname { font-weight: 600; }
  .damt { color: var(--muted); }

  .warnings { margin: 0.8rem 0; display: flex; flex-direction: column; gap: 0.5rem; }
  .warn { border-radius: 10px; padding: 0.7rem 0.9rem; font-size: 0.9rem; line-height: 1.45; border: 1px solid; }
  .warn.danger { border-color: var(--danger); background: color-mix(in srgb, var(--danger) 16%, transparent); }
  .warn.caution { border-color: var(--caution); background: color-mix(in srgb, var(--caution) 14%, transparent); }
  .warn.note { border-color: var(--note); background: color-mix(in srgb, var(--note) 12%, transparent); }

  .classes { display: flex; flex-wrap: wrap; gap: 0.35rem; width: 100%; }
  .classes.ro { margin-top: 0.3rem; }
  /* Settings sections: a segmented row, like the Stats time range. */
  .settings-nav { display: flex; flex-wrap: wrap; border: 1px solid var(--line); border-radius: 10px; overflow: hidden; margin-bottom: 1rem; width: fit-content; max-width: 100%; }
  .settings-nav button { background: none; border: 0; border-radius: 0; color: var(--muted); padding: 0.55rem 0.9rem; font-size: 0.9rem; min-height: 40px; }
  .settings-nav button + button { border-left: 1px solid var(--line); }
  .settings-nav button:hover:not(.on) { background: var(--surface-2); color: var(--ink); }
  .settings-nav button.on { background: var(--accent); color: var(--accent-ink); }
  .chip { font-size: 0.75rem; border: 1px solid var(--line); border-radius: 999px; padding: 0.2rem 0.6rem; background: transparent; color: var(--muted); cursor: pointer; }
  .chip.on { background: var(--accent); color: var(--accent-ink); border-color: var(--accent); }

  .sub-list li { border: 1px solid var(--line); border-radius: 10px; padding: 0.7rem 0.9rem; margin-bottom: 0.5rem; }
  .usage { border: 1px solid var(--line); border-radius: 10px; padding: 0.8rem 1rem; margin-bottom: 0.6rem; }
  .usage-head { display: flex; justify-content: space-between; align-items: baseline; }

  .row-actions { display: inline-flex; flex-wrap: wrap; justify-content: flex-end; gap: 0.5rem; align-items: center; margin-left: auto; }
  .icon-btn { background: transparent; border: 1px solid transparent; color: var(--muted); padding: 0.15rem 0.35rem; border-radius: 6px; font-size: 0.85rem; line-height: 1; }
  .icon-btn:hover { color: var(--ink); border-color: var(--line); }
  /* Neutral until confirmed: Delete isn't danger, and red is kept for danger. */
  .link.danger-link { color: var(--muted); text-decoration: underline; text-underline-offset: 3px; }
  .edit-form { display: flex; flex-direction: column; gap: 0.5rem; margin: 0.8rem 0; }
  .edit-form label { display: flex; flex-direction: column; gap: 0.2rem; font-size: 0.8rem; color: var(--muted); }
  .edit-form input, .edit-form textarea { font: inherit; background: var(--bg); color: var(--ink); border: 1px solid var(--line); border-radius: 8px; padding: 0.5rem 0.6rem; }
  .dose-form.inline { margin: 0; width: 100%; }
  .tl-note { flex: 1; }
  .sub-head { display: flex; justify-content: space-between; align-items: center; gap: 0.5rem; }

  .kb-search { display: flex; gap: 0.5rem; margin: 0.6rem 0 0.2rem; }
  .kb-search input { flex: 1; font: inherit; background: var(--bg); color: var(--ink); border: 1px solid var(--line); border-radius: 8px; padding: 0.5rem 0.6rem; }
  .kb-hits { list-style: none; padding: 0; margin: 0.8rem 0 0; }
  .kb-hits li { border: 1px solid var(--line); border-radius: 10px; padding: 0.7rem 0.9rem; margin-bottom: 0.5rem; }
  .kb-head { display: flex; justify-content: space-between; align-items: baseline; gap: 0.5rem; }
  .kb-flags { display: inline-flex; gap: 0.3rem; flex-shrink: 0; }
  .flag { font-size: 0.68rem; text-transform: uppercase; letter-spacing: 0.03em; border: 1px solid var(--line); border-radius: 999px; padding: 0.1rem 0.5rem; color: var(--muted); white-space: nowrap; cursor: help; }
  .flag.sparse { color: var(--caution); border-color: var(--caution); }
  .kb-text { font-size: 0.88rem; line-height: 1.5; margin: 0.45rem 0 0; white-space: pre-wrap; }
  /* t+ offsets: present but subordinate to the wall-clock time they annotate */
  .rel { font-variant-numeric: tabular-nums; opacity: 0.75; font-size: 0.9em; }
  .live-rel { margin-left: 0.2rem; }
  .link-btn { background: none; border: none; padding: 0; margin: 0.5rem 0 0; font: inherit; font-size: 0.85rem; color: var(--accent); cursor: pointer; text-align: left; }
  .link-btn:hover { text-decoration: underline; }

  /* reading one entry whole */
  .kb-entry-head { display: flex; justify-content: space-between; align-items: baseline; gap: 0.7rem; }
  .kb-entry-head h3 { margin: 0.4rem 0; }
  .entry-flags { margin: 0.3rem 0 0; }
  .kb-sections { margin-top: 0.8rem; }
  .kb-section { border-top: 1px solid var(--line); padding-top: 0.7rem; margin-top: 0.7rem; }
  .kb-section:first-child { border-top: none; padding-top: 0; margin-top: 0; }
  .kb-section h4 { margin: 0; font-size: 0.82rem; text-transform: uppercase; letter-spacing: 0.04em; color: var(--muted); font-weight: 600; }
  .browse-head { margin: 1.4rem 0 0.3rem; }
  .kb-browse { width: 100%; font: inherit; background: var(--bg); color: var(--ink); border: 1px solid var(--line); border-radius: 8px; padding: 0.5rem 0.6rem; margin: 0.5rem 0 0.2rem; }
  .kb-browse-list { list-style: none; padding: 0; margin: 0.6rem 0 0.4rem; display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 0.15rem 0.9rem; }
  .kb-browse-list li { display: flex; align-items: baseline; gap: 0.35rem; padding: 0.15rem 0; }

  .off-badge { font-size: 0.7rem; text-transform: uppercase; letter-spacing: 0.04em; border: 1px solid var(--line); color: var(--muted); border-radius: 999px; padding: 0.1rem 0.5rem; vertical-align: middle; margin-left: 0.4rem; }
  .off-badge.on { color: var(--note); border-color: var(--note); }
  /* The one green light in the pairing flow: a phone has used the token. */
  .paired { display: flex; align-items: center; gap: 0.5rem; margin: 0.2rem 0 0.6rem; color: var(--note); font-size: 0.9rem; }
  .paired-dot { width: 0.7rem; height: 0.7rem; border-radius: 50%; background: var(--note); box-shadow: 0 0 0 4px color-mix(in srgb, var(--note) 22%, transparent); flex: none; }
  .qr { background: #fff; padding: 0.8rem; border-radius: 10px; width: max-content; margin: 0.6rem 0; }
  .qr :global(svg) { display: block; width: 220px; height: 220px; }

  .draft { background: var(--bg); border: 1px solid var(--line); border-radius: 8px; padding: 0.7rem 0.8rem; margin: 0.6rem 0 0; max-height: 20rem; overflow: auto; font-size: 0.78rem; line-height: 1.45; white-space: pre; }
  .draft-actions { margin-top: 0.5rem; justify-content: flex-end; width: 100%; }

  .ref-inline { border: 1px solid var(--line); border-radius: 10px; padding: 0.6rem 0.8rem; margin-top: 0.5rem; background: color-mix(in srgb, var(--accent) 6%, transparent); }
  .ref-inline > strong { font-size: 0.95rem; }
  .attribution { font-size: 0.75rem; margin: 0.4rem 0 0; }
  .warn-text { color: var(--caution); margin-top: 0.3rem; }
  .dose-class { display: inline-block; font-size: 0.85rem; padding: 0.15rem 0.55rem; border-radius: 999px; border: 1px solid currentColor; margin-bottom: 0.4rem; }
  .dose-class.ok { color: var(--note); }
  .dose-class.caution { color: var(--caution); }
  .dose-class.danger { color: var(--danger); font-weight: 600; }
  .dose-class.muted { color: var(--muted); }
  .update-banner { border: 1px solid var(--accent); background: color-mix(in srgb, var(--accent) 12%, transparent); border-radius: 10px; padding: 0.6rem 0.9rem; margin-bottom: 1rem; font-size: 0.9rem; }
  .update-head { display: flex; flex-wrap: wrap; align-items: center; gap: 0.8rem; justify-content: space-between; }
  .update-body { margin: 0.6rem 0 0.1rem; padding-top: 0.5rem; border-top: 1px solid color-mix(in srgb, var(--accent) 30%, transparent); color: var(--muted); font-size: 0.83rem; line-height: 1.5; white-space: pre-wrap; max-height: 11rem; overflow-y: auto; }
  .ai-log { max-height: 150px; overflow: auto; background: color-mix(in srgb, var(--ink) 6%, transparent); border-radius: 8px; padding: 0.6rem; font-size: 0.75rem; line-height: 1.4; white-space: pre-wrap; word-break: break-word; color: var(--muted); margin: 0.2rem 0 0; }
  .import-panel { border: 1px solid var(--line); border-radius: 12px; padding: 1rem; margin: 0.6rem 0 1rem; display: flex; flex-direction: column; gap: 0.6rem; }
  .import-text { font: inherit; background: var(--bg); color: var(--ink); border: 1px solid var(--line); border-radius: 8px; padding: 0.6rem 0.7rem; resize: vertical; width: 100%; box-sizing: border-box; }
  .import-tip { margin-top: -0.2rem; }
  .import-text::placeholder { opacity: 0.55; }
  .import-summary { margin: 0.4rem 0 0.2rem; display: flex; flex-direction: column; gap: 0.4rem; }
  .import-summary div { display: flex; gap: 0.6rem; }
  .import-summary dt { flex: none; width: 5.5rem; color: var(--muted); font-size: 0.8rem; text-transform: uppercase; letter-spacing: 0.03em; padding-top: 0.05rem; }
  .import-summary dd { margin: 0; font-size: 0.9rem; }
  .notice p { margin: 0 0 0.4rem; }
  .notice p:last-child { margin-bottom: 0; }

  .notice { border: 1px solid var(--caution); background: color-mix(in srgb, var(--caution) 12%, transparent); border-radius: 10px; padding: 0.8rem 1rem; line-height: 1.5; }
  .notice.bad-notice { border-color: var(--danger); background: color-mix(in srgb, var(--danger) 12%, transparent); }
  .notice.good-notice { border-color: var(--note); background: color-mix(in srgb, var(--note) 12%, transparent); }
  .notice.warn-notice { border-color: var(--caution); background: color-mix(in srgb, var(--caution) 14%, transparent); }

  .remote-form { display: flex; gap: 0.5rem; align-items: center; margin: 0.4rem 0; }
  .remote-form input { flex: 1; min-width: 0; padding: 0.45rem 0.6rem; border-radius: 8px; border: 1px solid var(--line); background: var(--bg); color: var(--ink); }
  .person-tag { font-size: 0.75rem; border: 1px solid var(--line); border-radius: 6px; padding: 0 0.35rem; margin-left: 0.3rem; color: var(--muted); font-weight: 500; }
  .device-list li.person-remove { display: block; border-color: var(--danger); }
  .person-remove form { display: flex; flex-direction: column; gap: 0.4rem; }
  .device-list { list-style: none; padding: 0; margin: 0.4rem 0; display: flex; flex-direction: column; gap: 0.4rem; }
  .device-list li { display: flex; justify-content: space-between; align-items: center; gap: 0.8rem; padding: 0.45rem 0.6rem; border: 1px solid var(--line); border-radius: 8px; }
  .pair-ways { margin: 0.2rem 0 0.5rem; padding-left: 1.2rem; }
  .remote-line { display: flex; align-items: baseline; gap: 0.45rem; }
  .remote-dot { display: inline-block; width: 0.55rem; height: 0.55rem; border-radius: 50%; background: #3fa46a; flex: none; }
  .remote-dot.off { background: #d08a2e; }
  .remote-pill { display: inline-flex; align-items: center; gap: 0.4rem; font-size: 0.8rem; padding: 0.2rem 0.6rem; border-radius: 999px; border: 1px solid var(--line); background: transparent; color: var(--ink); cursor: pointer; }
  .remote-pill.off { border-color: #d08a2e; }
  .unlock-form { display: flex; flex-direction: column; gap: 0.7rem; margin: 1.2rem 0 0.8rem; }
  .unlock-form input { padding: 0.6rem 0.7rem; border-radius: 10px; border: 1px solid var(--line); background: var(--bg); color: var(--ink); font-size: 1rem; }
  .dont-show { display: flex; align-items: center; gap: 0.5rem; color: var(--muted); font-size: 0.9rem; margin: 1rem 0; cursor: pointer; }
  .dont-show input { width: auto; }
  .companion-optin { display: flex; align-items: flex-start; gap: 0.5rem; font-size: 0.9rem; margin: 1rem 0 0; cursor: pointer; text-align: left; }
  .companion-optin input { width: auto; margin-top: 0.2rem; flex: none; }
  .companion-optin span { color: var(--muted); }
  .companion-optin strong { color: var(--fg, inherit); }
  .sec-block { border-top: 1px solid var(--line); margin-top: 1.1rem; padding-top: 1.1rem; display: flex; flex-direction: column; gap: 0.6rem; align-items: flex-start; }
  .sec-block h3 { margin: 0; font-size: 0.98rem; }
  .sec-block input { padding: 0.5rem 0.65rem; border-radius: 9px; border: 1px solid var(--line); background: var(--bg); color: var(--ink); min-width: 16rem; max-width: 24rem; }
  /* That sizing is for password fields; a checkbox in the same block stays a checkbox. */
  .sec-block input[type="checkbox"] { min-width: 0; padding: 0; }
  .vault-pick { display: flex; gap: 0.6rem; align-items: center; margin: 0.9rem 0; flex-wrap: wrap; }
  .vault-pick input { flex: 1; min-width: 14rem; padding: 0.5rem 0.65rem; border-radius: 9px; border: 1px solid var(--line); background: var(--bg); color: var(--muted); }
  .model-sel { font: inherit; background: var(--bg); color: var(--ink); border: 1px solid var(--line); border-radius: 8px; padding: 0.4rem 0.6rem; max-width: 55%; }
  .disclaimer { margin-top: 0; }
  .share { display: flex; align-items: center; gap: 0.4rem; font-size: 0.85rem; color: var(--muted); margin: 0.4rem 0 0.3rem; }
  .share-pick { display: flex; align-items: center; flex-wrap: wrap; gap: 0.5rem; margin: 0 0 0.8rem 1.5rem; }
  .share input { width: auto; }
  .chat { display: flex; flex-direction: column; gap: 0.5rem; min-height: 220px; max-height: 46vh; overflow-y: auto; padding: 0.4rem; border: 1px solid var(--line); border-radius: 12px; background: var(--bg); }
  .chat-empty { margin: auto; text-align: center; max-width: 40ch; }
  .warm-hint { text-align: center; margin: 0.3rem auto 0; max-width: 34ch; opacity: 0.75; }
  .bubble { padding: 0.55rem 0.8rem; border-radius: 12px; max-width: 82%; line-height: 1.45; font-size: 0.92rem; white-space: pre-wrap; word-break: break-word; }
  .bubble.user { align-self: flex-end; background: var(--accent); color: var(--accent-ink); border-bottom-right-radius: 4px; }
  .bubble.assistant { align-self: flex-start; background: var(--card); border: 1px solid var(--line); border-bottom-left-radius: 4px; }
  .chat-input { display: flex; gap: 0.5rem; margin-top: 0.7rem; }
  .chat-input input { flex: 1; }

  .compute-notice {
    display: flex; gap: 0.5rem; align-items: flex-start;
    padding: 0.6rem 0.75rem; border-radius: 8px; margin: 0.2rem 0 0.8rem;
    font-size: 0.85rem; line-height: 1.35;
    border: 1px solid var(--line); background: color-mix(in srgb, var(--muted) 10%, transparent);
  }
  .compute-notice.insufficient { border-color: var(--danger); background: color-mix(in srgb, var(--danger) 14%, transparent); }
  .compute-notice.tight { border-color: var(--caution); background: color-mix(in srgb, var(--caution) 14%, transparent); }
  .compute-icon { flex: none; }

  .fb-kind { display: flex; gap: 1.2rem; margin: 0.4rem 0 0.7rem; flex-wrap: wrap; }
  .fb-kind label { display: flex; align-items: center; gap: 0.35rem; font-size: 0.9rem; cursor: pointer; }
  .fb-field { display: block; width: 100%; margin-bottom: 0.55rem; }
  textarea.fb-field { resize: vertical; }

  footer { margin-top: 1.6rem; text-align: center; color: var(--muted); font-size: 0.8rem; }
  .footer-sub { margin-top: 0.2rem; font-size: 0.72rem; opacity: 0.75; }

  /* ---- crisis banner + emergency resources ---- */
  .help-btn { background: var(--help-bg); color: var(--help-ink); border: none; border-radius: 999px; padding: 0.45rem 1rem; font-weight: 700; cursor: pointer; }
  nav :global(.discreet-eye) { align-self: center; }
  nav button.nav-help { background: var(--help-bg); color: var(--help-ink); font-weight: 700; white-space: nowrap; }
  .help-btn:hover { filter: brightness(1.08); }
  .danger-card { border-color: color-mix(in srgb, var(--danger) 45%, var(--line)); }
  .danger-btn { background: var(--danger); color: #fff; border: none; border-radius: 8px; padding: 0.45rem 0.9rem; font-weight: 600; cursor: pointer; }
  .danger-btn:hover:not(:disabled) { filter: brightness(1.08); }
  .danger-btn:disabled { opacity: 0.55; cursor: default; }
  .crisis-banner { border-radius: 12px; padding: 1rem 1.2rem; margin-bottom: 1rem; border: 1px solid var(--caution); background: color-mix(in srgb, var(--caution) 14%, var(--card)); }
  .crisis-offer { display: flex; gap: 0.6rem; flex-wrap: wrap; align-items: center; }
  .crisis-offer button { margin-top: 0.4rem; }
  /* Calm at every level: what someone wrote is a reason to offer help, not to sound an alarm. */
  .crisis-banner.psychiatric, .crisis-banner.medical { border-color: var(--note); background: color-mix(in srgb, var(--note) 12%, var(--card)); }
  .crisis-head { display: flex; justify-content: space-between; align-items: flex-start; gap: 1rem; }
  .crisis-res { list-style: none; padding: 0; margin: 0.7rem 0; display: flex; flex-direction: column; gap: 0.6rem; }
  .crisis-res .contact { font-variant-numeric: tabular-nums; }
  .crisis-res li { line-height: 1.4; }

  .modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.6); display: grid; place-items: center; padding: 1.5rem; z-index: 50; }
  .help-modal { background: var(--card); border: 1px solid var(--help-bg); border-radius: 16px; padding: 1.6rem; max-width: 520px; width: 100%; }
  .help-modal h2 { margin-top: 0; }
  .help-first { font-size: 1.05rem; margin: 0 0 0.8rem; }

  /* ---- support style intake ---- */
  .models-panel { border: 1px solid var(--line); border-radius: 12px; padding: 1rem; margin: 0.6rem 0 0.9rem; display: flex; flex-direction: column; gap: 0.6rem; }
  .upgrade-notice { border: 1px solid var(--note); border-radius: 12px; padding: 1rem; margin: 0.6rem 0 0.9rem; display: flex; flex-direction: column; gap: 0.6rem; }
  .upgrade-notice p { margin: 0; }
  .upgrade-notice code { font-size: 0.9em; }
  .upgrade-actions { display: flex; gap: 0.5rem; flex-wrap: wrap; }
  .pull-row { display: flex; gap: 0.5rem; flex-wrap: wrap; }
  .pull-row input { flex: 1; min-width: 14rem; padding: 0.5rem 0.65rem; border-radius: 9px; border: 1px solid var(--line); background: var(--bg); color: var(--ink); }
  .support-style { margin: 0.8rem 0; display: flex; flex-direction: column; gap: 0.5rem; }
  .style-chips { display: flex; flex-wrap: wrap; gap: 0.4rem; }
  .actions-note { display: flex; flex-wrap: wrap; gap: 0.4rem; margin-top: 0.6rem; }
  .action-chip { font-size: 0.8rem; color: var(--note); border: 1px solid var(--note); border-radius: 999px; padding: 0.15rem 0.6rem; }

  /* ---- live session ---- */
  .live-alerts { margin: 0.8rem 0 0; }
  .live-alerts:empty { display: none; }
  .live { position: fixed; inset: 0; background: var(--bg); z-index: 40; display: flex; flex-direction: column; padding: 1.2rem clamp(1rem, 4vw, 3rem); overflow-y: auto; }
  .live-bar { display: flex; justify-content: space-between; align-items: flex-start; gap: 1rem; border-bottom: 1px solid var(--line); padding-bottom: 1rem; }
  .live-title { font-size: 1.5rem; font-weight: 700; }
  .live-last { font-size: 1.25rem; margin-top: 0.35rem; font-variant-numeric: tabular-nums; }
  .live-saved { margin: 0.4rem 0 0; color: var(--muted); }
  .mood-row { display: flex; flex-wrap: wrap; gap: 0.4rem; margin-top: 0.5rem; }
  .chip-btn { background: var(--card); color: var(--ink); border: 1px solid var(--line); border-radius: 999px; padding: 0.4rem 0.8rem; font: inherit; font-size: 0.9rem; cursor: pointer; }
  .chip-btn:hover { border-color: var(--accent); }
  .live-foot { margin-top: 2rem; padding-top: 1rem; border-top: 1px solid var(--line); }
  .live-body { display: grid; grid-template-columns: 1fr 1fr; gap: 1.5rem; margin-top: 1.2rem; align-items: start; }
  @media (max-width: 780px) { .live-body { grid-template-columns: 1fr; } }
  .live-timeline h3, .live-companion h3 { margin: 1.1rem 0 0.5rem; }
  .live-companion { border-left: 1px solid var(--line); padding-left: 1.5rem; align-self: stretch; }
  .live-doses, .live-events { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 0.35rem; font-size: 1.05rem; }
  .quick-log { display: flex; flex-wrap: wrap; gap: 0.5rem; }
  .quick-log input { flex: 1; min-width: 6rem; padding: 0.55rem 0.7rem; border-radius: 9px; border: 1px solid var(--line); background: var(--card); color: var(--ink); font-size: 1rem; }
  .live-chat { min-height: 200px; max-height: 42vh; }
  .pdf-choices { display: flex; flex-direction: column; gap: 0.35rem; margin: 0.4rem 0 0.8rem; }
  .pdf-choices label { display: flex; gap: 0.5rem; align-items: center; }
</style>
