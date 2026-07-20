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
    updateDose,
    updateTimelineEvent,
    type TimelineEvent,
    deleteExperience,
    deleteDose,
    deleteTimelineEvent,
    deleteSubstance,
    type Dose,
    aiStatus,
    aiRecommendedModels,
    aiInstall,
    aiStart,
    aiPull,
    aiPreferredModel,
    setCompanionEnabled,
    aiSwitchModel,
    companionChat,
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
    type ChatMsg,
    type AiStatus,
  } from "$lib/api";
  import { listen } from "@tauri-apps/api/event";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { save, open as openDialog } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { exit } from "@tauri-apps/plugin-process";

  type Tab = "journal" | "companion" | "substances" | "bysub" | "data";

  const HIDE_DISCLAIMER_KEY = "fieldnotes.hideDisclaimer";

  let acknowledged = $state(false);
  let tab = $state<Tab>("journal");

  // at-rest encryption / unlock gate
  let db = $state<DbStatus>({ encrypted: false, unlocked: true });
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
  let vaultFolder = $state("");
  let obsBusy = $state(false);
  let obsErr = $state<string | null>(null);
  let obsMsg = $state<string | null>(null);

  let experiences = $state<ExperienceSummary[]>([]);
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
  let showNewExp = $state(false);
  // plain note (kind: "note") — title, body, date; nothing else
  let nnTitle = $state("");
  let nnBody = $state("");
  let nnDate = $state("");
  let showNewNote = $state(false);

  // dose form
  let dSubstance = $state("");
  let dAmount = $state("");
  let dUnit = $state("mg");
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

  // Phone portal — off by default, and it stays off until the user says otherwise.
  let portal = $state<PortalStatus>({ running: false, port: null, pair_url: null });
  let portalQrSvg = $state<string | null>(null);
  let ts = $state<TailscaleStatus | null>(null);
  let portalErr = $state<string | null>(null);
  // The pairing QR carries the bearer token, so it is hidden until asked for —
  // it should not be sitting on screen behind you while you're screen-sharing.
  let showQr = $state(false);
  let serving = $state(false);
  let tailscaleUrl = $derived(
    ts?.host && portal.port ? `https://${ts.host}/m` : null,
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
  let companionOff = $state(false);
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

  // live session mode
  let liveSession = $state(false);
  let lsNow = $state(Date.now());
  let lsTimer: ReturnType<typeof setInterval> | null = null;
  // quick-log within the live session
  let qSub = $state("");
  let qAmt = $state("");
  let qUnit = $state("mg");
  let qRoute = $state("oral");
  let qNote = $state("");
  let lsNote = $state("");

  const nowIso = () => new Date().toISOString();
  const fmtDate = (iso: string) => new Date(iso).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
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
    interactionClasses().then((c) => (classesVocab = c));
    checkForUpdate();
    dontShowDisclaimer = localStorage.getItem(HIDE_DISCLAIMER_KEY) === "1";
    companionOff = localStorage.getItem(COMPANION_OFF_KEY) === "1";
    vaultFolder = localStorage.getItem(VAULT_KEY) ?? "";
    loadDbStatus();
    const un = listen<string>("ai-progress", (e) => {
      aiLog = [...aiLog.slice(-200), e.payload];
    });
    return () => {
      un.then((f) => f());
      if (lsTimer) clearInterval(lsTimer);
    };
  });

  // Decide the startup screen: a locked encrypted journal shows the unlock
  // prompt; otherwise the disclaimer splash, unless the user opted out of it.
  async function loadDbStatus() {
    try {
      db = await dbStatus();
    } catch (_) {
      db = { encrypted: false, unlocked: true };
    }
    statusLoaded = true;
    if (db.unlocked && dontShowDisclaimer) {
      await enter();
    }
  }

  async function checkForUpdate() {
    try {
      update = await check();
    } catch (_) {
      // offline, or no published release with an updater manifest yet — ignore
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
    acknowledged = true;
    await Promise.all([loadJournal(), loadSubstances()]);
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
    if (!confirm("Importing pulls experiences from the vault into this journal. For any experience already here, the vault's version wins. Continue?")) return;
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
    if (!confirm("Erase ALL Field Notes data on this device?\n\nThis permanently deletes every experience, dose, note, substance, and setting, and turns off encryption. It cannot be undone.")) return;
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
  }
  async function loadSubstances() {
    substances = await listSubstances();
  }
  async function loadUsage() {
    usage = await usageBySubstance();
  }

  async function openExperience(id: number) {
    lastWarnings = [];
    editExp = false;
    editingDoseId = null;
    dRef = null;
    exportErr = exportMsg = null;
    selected = await getExperience(id);
    dTime = nowLocalInput();
  }

  function openNewExp() {
    showNewExp = !showNewExp;
    if (showNewExp) showNewNote = false;
    if (showNewExp && !neStart) neStart = nowLocalInput();
  }

  function openNewNote() {
    showNewNote = !showNewNote;
    if (showNewNote) showNewExp = false;
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
    const exp = await importExperience({
      ...importParsed,
      title: importTitle,
      started_at: localInputToIso(importStart),
    });
    importParsed = null;
    importText = "";
    showImport = false;
    await loadJournal();
    await openExperience(exp.id);
  }

  async function submitNewExperience() {
    const e = await createExperience({
      title: neTitle || "Untitled experience",
      intention: neIntention,
      setting: neSetting,
      started_at: localInputToIso(neStart),
    });
    neTitle = neIntention = neSetting = neStart = "";
    showNewExp = false;
    await loadJournal();
    await openExperience(e.id);
  }

  async function submitDose() {
    if (!selected || !dSubstance.trim()) return;
    const res = await logDose({
      experience_id: selected.id,
      substance_name: dSubstance.trim(),
      amount: dAmount ? parseFloat(dAmount) : null,
      unit: dUnit,
      route: dRoute,
      taken_at: localInputToIso(dTime),
    });
    lastWarnings = res.warnings;
    dSubstance = dAmount = "";
    dTime = nowLocalInput();
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
      await exportExperienceFile(selected.id, path);
      exportMsg = "Entry exported as Markdown.";
    } catch (e) {
      exportErr = typeof e === "string" ? e : String(e);
    }
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
  }

  async function saveDose() {
    if (!selected || editingDoseId == null) return;
    await updateDose(editingDoseId, {
      substance_name: edSub.trim(),
      amount: edAmt ? parseFloat(edAmt) : null,
      unit: edUnit,
      route: edRoute,
      taken_at: localInputToIso(edTime),
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
    }
    if (!experiences.length) await loadJournal();
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
      if (res.journal_changed) {
        await loadJournal();
        await refreshSelected();
      }
    } catch (e) {
      cMessages = [...cMessages, { role: "assistant", content: `⚠️ ${typeof e === "string" ? e : String(e)}` }];
    } finally {
      cSending = false;
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

  async function quickLog() {
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
    });
    qSub = ""; qAmt = ""; qNote = "";
    if (res.warnings.length) lastWarnings = res.warnings;
    await refreshSelected();
    await loadJournal();
    // A newly dangerous combination should raise the crisis banner deterministically.
    const c = await crisisScan("", selected.id);
    if (c.level !== "none") { crisis = c; crisisResourcesShown = false; }
  }

  async function quickNote() {
    if (!selected || !lsNote.trim()) return;
    await addTimelineEvent({ experience_id: selected.id, at: nowIso(), note: lsNote.trim(), mood: "", intensity: null });
    lsNote = "";
    await refreshSelected();
  }

  async function goTab(t: Tab) {
    tab = t;
    selected = null;
    if (t === "bysub") await loadUsage();
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
  // ---- Phone portal ----
  async function loadPortal() {
    portal = await portalStatus();
    ts = await portalTailscale();
    if (!portal.running) { portalQrSvg = null; showQr = false; }
  }

  /** What the phone should actually open. Over the tailnet if Tailscale can carry
   *  it there; otherwise the loopback URL, which only this machine can reach — so
   *  the QR is honest about the portal being desktop-only until Tailscale is set up. */
  function pairTarget(): string | null {
    if (!portal.pair_url) return null;
    const token = portal.pair_url.match(/#t=([a-f0-9]+)/)?.[1];
    if (ts?.host && token) return `https://${ts.host}/m#t=${token}`;
    return portal.pair_url;
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

  async function revealQr() {
    const target = pairTarget();
    if (!target) return;
    portalQrSvg = await portalQr(target);
    showQr = true;
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
      // The pairing URL changes with it, so any QR on screen is now stale.
      portalQrSvg = null;
      showQr = false;
    } catch (e) {
      portalErr = e instanceof Error ? e.message : String(e);
    } finally {
      serving = false;
    }
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
    if (r.heavy != null) parts.push(`heavy ${r.heavy}+`);
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
    if (r.heavy != null && amount >= r.heavy) return { label: "heavy", level: "danger" };
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
    // Don't classify across mismatched units (e.g. entering g against mg ranges).
    const u = (roa.units ?? "").toLowerCase();
    if (u && dUnit && u !== dUnit.trim().toLowerCase()) return null;
    return classifyDose(amt, roa);
  });

  const sevClass = (s: string) => (s === "danger" ? "danger" : s === "caution" ? "caution" : "note");
</script>

{#if !statusLoaded}
  <div class="gate">
    <div class="gate-card">
      <h1>Field Notes</h1>
      <p class="muted">Loading…</p>
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
      <label class="dont-show">
        <input type="checkbox" bind:checked={dontShowDisclaimer} />
        Don't show this again on startup
      </label>
      <button class="primary" onclick={enter}>I understand — continue</button>
    </div>
  </div>
{:else}
  {#snippet aiSetup()}
    {#if !ai}
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
      <button class="primary small-btn" disabled={aiBusy} onclick={doPull}>{aiBusy ? "Downloading…" : "Download model"}</button>
    {/if}
    {#if aiErr}<p class="notice bad-notice">{aiErr}</p>{/if}
    {#if aiLog.length}<pre class="ai-log">{aiLog.slice(-14).join("\n")}</pre>{/if}
  {/snippet}

  <main>
    {#if update && !updateDismissed}
      <div class="update-banner">
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
    {/if}

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

    <header>
      <h1>Field Notes</h1>
      <nav>
        <button class:active={tab === "journal"} onclick={() => goTab("journal")}>Journal</button>
        {#if !companionOff}
          <button class:active={tab === "companion"} onclick={() => goTab("companion")}>Companion</button>
        {/if}
        <button class:active={tab === "substances"} onclick={() => goTab("substances")}>Substances</button>
        <button class:active={tab === "bysub"} onclick={() => goTab("bysub")}>Substance Log</button>
        <button class:active={tab === "data"} onclick={() => goTab("data")}>Settings</button>
        <button title="Emergency &amp; support resources" onclick={openHelp}>Emergency Resources</button>
      </nav>
    </header>

    <!-- ============ JOURNAL ============ -->
    {#if tab === "journal"}
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
              <label>Date<input type="datetime-local" bind:value={eStart} /></label>
              <label>Entry<textarea bind:value={eNotes} rows="10"></textarea></label>
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
          <button class="link" onclick={() => (selected = null)}>← All experiences</button>
          <div class="exp-head">
            <h2>{selected.title || "Untitled experience"}</h2>
            <span class="row-actions">
              {#if !selected.ended_at}<button class="primary small-btn" onclick={startLiveSession}>Live session</button>{/if}
              {#if !editExp}<button class="link" onclick={startEditExp}>Edit</button>{/if}
              <button class="link danger-link" onclick={delExp}>Delete</button>
            </span>
          </div>
          <span class="muted small">{fmtDate(selected.started_at)} · {fmtTime(selected.started_at)}{selected.ended_at ? " → " + fmtTime(selected.ended_at) : " · ongoing"}</span>

          {#if editExp}
            <div class="edit-form">
              <label>Title<input bind:value={eTitle} /></label>
              <label>Started<input type="datetime-local" bind:value={eStart} /></label>
              <label>Intention<input bind:value={eIntention} /></label>
              <label>Setting<input bind:value={eSetting} /></label>
              <label>Notes<textarea bind:value={eNotes} rows="3"></textarea></label>
              <label>Rating (0–10)<input type="number" min="0" max="10" bind:value={eRating} /></label>
              <div class="row-actions">
                <button class="primary small-btn" onclick={saveExp}>Save</button>
                <button class="ghost small-btn" onclick={() => (editExp = false)}>Cancel</button>
              </div>
            </div>
          {:else}
            {#if selected.intention}<p><strong>Intention:</strong> {selected.intention}</p>{/if}
            {#if selected.setting}<p><strong>Setting:</strong> {selected.setting}</p>{/if}
            {#if selected.notes}<p><strong>Notes:</strong> {selected.notes}</p>{/if}
            {#if selected.rating != null}<p class="muted small">Rating: {selected.rating}/10</p>{/if}
          {/if}

          {#if lastWarnings.length}
            <div class="warnings">
              {#each lastWarnings as w}
                <div class="warn {sevClass(w.severity)}">
                  <strong>{w.severity.toUpperCase()}</strong> · {w.a} + {w.b}
                  <div>{w.message}</div>
                </div>
              {/each}
            </div>
          {/if}

          <h3>Doses</h3>
          {#if selected.doses.length}
            <ul class="doses">
              {#each selected.doses as d}
                <li>
                  {#if editingDoseId === d.id}
                    <div class="dose-form inline">
                      <input list="subnames" bind:value={edSub} />
                      <input type="number" step="any" bind:value={edAmt} class="narrow" />
                      <input bind:value={edUnit} class="narrow" />
                      <input bind:value={edRoute} class="narrow" />
                      <input type="datetime-local" bind:value={edTime} />
                      <button class="primary small-btn" onclick={saveDose}>Save</button>
                      <button class="ghost small-btn" onclick={() => (editingDoseId = null)}>Cancel</button>
                    </div>
                  {:else}
                    <span class="dtime">{fmtTime(d.taken_at)}{#if sessionT0}<span class="rel"> ({relTime(d.taken_at, sessionT0)})</span>{/if}</span>
                    <span class="dname">{d.substance_name}</span>
                    <span class="damt">{d.amount ?? "?"} {d.unit}{d.route ? " · " + d.route : ""}</span>
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
            <input list="subnames" placeholder="Substance" bind:value={dSubstance} onchange={() => lookupRef(dSubstance)} />
            <input type="number" step="any" placeholder="Amount" bind:value={dAmount} />
            <input placeholder="unit" bind:value={dUnit} class="narrow" />
            <input placeholder="route" bind:value={dRoute} class="narrow" />
            <input type="datetime-local" bind:value={dTime} title="Time taken" />
            <button class="primary small-btn" onclick={submitDose}>Log dose</button>
          </div>
          {#if dRef}
            <div class="ref-inline">
              {#if doseClass}
                <div class="dose-class {doseClass.level}">{dAmount}{dUnit} · <strong>{doseClass.label}</strong> dose{doseClass.level === "danger" ? " ⚠" : ""}</div>
              {/if}
              <strong>{dRef.name}</strong> — reference doses
              {#each dRef.roas as r}
                {#if roaSummary(r)}<div class="muted small">{r.name}: {roaSummary(r)}{durationSummary(r) ? ` · ${durationSummary(r)}` : ""}</div>{/if}
              {/each}
              {#if refInteractions(dRef, "danger").length}
                <div class="small warn-text">⚠ dangerous with: {refInteractions(dRef, "danger").map((i) => i.name).join(", ")}</div>
              {/if}
              {#if refInteractions(dRef, "caution").length}
                <div class="small warn-text muted">unsafe with: {refInteractions(dRef, "caution").map((i) => i.name).join(", ")}</div>
              {/if}
              <div class="muted attribution">via DoseWiki · CC0 public domain · reference only, verify before dosing</div>
            </div>
          {/if}
          <datalist id="subnames">
            {#each substances as s}<option value={s.name}></option>{/each}
          </datalist>

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
                      <input type="datetime-local" bind:value={etTime} />
                      <button class="primary small-btn" onclick={saveTimeline}>Save</button>
                      <button class="ghost small-btn" onclick={() => (editingTimelineId = null)}>Cancel</button>
                    </div>
                  {:else}
                    <span class="dtime">{fmtTime(t.at)}{#if sessionT0}<span class="rel"> ({relTime(t.at, sessionT0)})</span>{/if}</span>
                    <span class="tl-note">{t.note}{t.intensity != null ? ` (${t.intensity}/10)` : ""}{t.mood ? ` · ${t.mood}` : ""}</span>
                    <span class="row-actions">
                      <button class="icon-btn" title="Edit note" onclick={() => startEditTimeline(t)}>✎</button>
                      <button class="icon-btn" title="Delete" onclick={() => delTimeline(t.id)}>✕</button>
                    </span>
                  {/if}
                </li>
              {/each}
            </ul>
          {:else}
            <p class="muted small">No timeline notes yet.</p>
          {/if}
          <div class="dose-form">
            <input placeholder="How are you feeling?" bind:value={tNote} />
            <input placeholder="mood" bind:value={tMood} class="narrow" />
            <input type="number" min="0" max="10" placeholder="0-10" bind:value={tIntensity} class="narrow" />
            <button class="ghost small-btn" onclick={submitTimeline}>Add note</button>
          </div>

          {#if !selected.ended_at}
            <button class="ghost" onclick={finishExperience}>End experience</button>
          {/if}
          <button class="ghost" onclick={exportEntry}>Export this entry</button>
          {#if exportErr}<p class="notice bad-notice">{exportErr}</p>{/if}
          {#if exportMsg}<p class="notice good-notice">{exportMsg}</p>{/if}
        </section>
      {:else}
        <section class="card">
          <div class="exp-head">
            <h2>Journal</h2>
            <span class="row-actions">
              <button class="ghost small-btn" onclick={openImport}>Import from text</button>
              <button class="ghost small-btn" onclick={openNewNote}>+ Note</button>
              <button class="primary small-btn" onclick={openNewExp}>+ Session</button>
            </span>
          </div>

          {#if showNewExp}
            <div class="new-exp">
              <input placeholder="Title" bind:value={neTitle} />
              <input type="datetime-local" bind:value={neStart} title="Start time" />
              <input placeholder="Intention (optional)" bind:value={neIntention} />
              <input placeholder="Set & setting (optional)" bind:value={neSetting} />
              <button class="primary small-btn" onclick={submitNewExperience}>Start</button>
            </div>
          {/if}

          {#if showNewNote}
            <!-- A plain entry — not a session. Title, words, date. -->
            <div class="new-note">
              <input placeholder="Title (optional)" bind:value={nnTitle} />
              <textarea rows="6" placeholder="Write anything." bind:value={nnBody}></textarea>
              <div class="row-actions">
                <input type="datetime-local" bind:value={nnDate} title="Date" />
                <button class="primary small-btn" onclick={submitNewNote}>Save note</button>
              </div>
            </div>
          {/if}

          {#if showImport}
            <div class="import-panel">
              {#if !aiReady}
                <p class="muted small">Import uses a local model to read your text — nothing leaves this computer. Let's get it set up:</p>
                {@render aiSetup()}
                <button class="ghost small-btn" onclick={() => (showImport = false)}>Cancel</button>
              {:else if !importParsed}
                <p class="muted small">Paste a past experience in your own words. The local model extracts the substances, doses, and timeline for you to review before saving.</p>
                <select bind:value={aiModel} class="model-sel">
                  {#each ai?.models ?? [] as m}<option value={m}>{m}</option>{/each}
                </select>
                <textarea class="import-text" rows="6" placeholder="e.g. Last Saturday around 9pm I took 100mg of MDMA at a friend's place. About an hour later I redosed 50mg…" bind:value={importText}></textarea>
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
                  <input type="datetime-local" bind:value={importStart} title="Start time" />
                </div>
                {#if importParsed.doses.length}
                  <h3>Doses found</h3>
                  <ul class="doses">
                    {#each importParsed.doses as d}
                      <li><span class="dname">{d.substance}</span><span class="damt">{d.amount ?? "?"} {d.unit}{d.route ? " · " + d.route : ""}</span></li>
                    {/each}
                  </ul>
                {:else}
                  <p class="notice">No doses were detected — you can still import and add them by hand.</p>
                {/if}
                {#if importParsed.timeline.length}
                  <h3>Timeline</h3>
                  <ul class="timeline">
                    {#each importParsed.timeline as t}<li><span class="tl-note">{t.note}{t.intensity != null ? ` (${t.intensity}/10)` : ""}</span></li>{/each}
                  </ul>
                {/if}
                <div class="row-actions">
                  <button class="primary small-btn" onclick={confirmImport}>Import</button>
                  <button class="ghost small-btn" onclick={() => (importParsed = null)}>Back</button>
                </div>
              {/if}
            </div>
          {/if}

          {#if experiences.length}
            <ul class="exp-list">
              {#each experiences as e}
                <li>
                  <button class="exp-row" onclick={() => openExperience(e.id)}>
                    {#if e.kind === "note"}
                      <div>
                        <strong>{e.title || "Untitled note"}</strong>
                        <span class="muted small">{fmtDate(e.started_at)}</span>
                      </div>
                      <div class="exp-meta">
                        <span class="pill note-pill">note</span>
                      </div>
                    {:else}
                      <div>
                        <strong>{e.title || "Untitled"}</strong>
                        <span class="muted small">{fmtDate(e.started_at)}{e.ended_at ? "" : " · ongoing"}</span>
                      </div>
                      <div class="exp-meta">
                        {#each e.substances as s}<span class="pill">{s}</span>{/each}
                        <span class="muted small">{e.dose_count} dose{e.dose_count === 1 ? "" : "s"}</span>
                      </div>
                    {/if}
                  </button>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="muted">Nothing here yet. Start a session, or just write a note.</p>
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

          {#if upgradeAvailable}
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
                <button class="small-btn" disabled={aiBusy} onclick={() => (upgradeDismissed = true)}>
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
              <p class="muted small">
                These models run on this computer, so your hardware sets the ceiling. On an older or
                lower-memory machine the Companion will be slower and less capable. You can turn it
                off in Settings.
              </p>
              {#if aiErr}<p class="notice bad-notice">{aiErr}</p>{/if}
              {#if aiLog.length}<pre class="ai-log">{aiLog.slice(-14).join("\n")}</pre>{/if}
            </div>
          {/if}

          <label class="share">
            <input type="checkbox" bind:checked={cShareSession} />
            Share a session with the companion
          </label>
          {#if cShareSession}
            <div class="share-pick">
              <select bind:value={cSessionChoice} class="model-sel" aria-label="Which session to share">
                <option value={null}>Current session (most recent)</option>
                {#each shareableSessions as s}
                  <option value={s.id}>{s.title || "Untitled"} · {fmtDate(s.started_at)}</option>
                {/each}
              </select>
              <span class="muted small">
                {#if attachedExp}Sharing “{attachedExp.title || "Untitled"}”{:else}No sessions yet — nothing to share.{/if}
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
              <p class="muted small chat-empty">Start chatting about an experience you're planning, having currently, or want to discuss for integration. If you share a session with me, I can see what doses you've taken and, at your request, I can log doses or take notes for you while we chat.</p>
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
                {#if refInteractions(kbDose, "danger").length}
                  <div class="small warn-text">⚠ dangerous with: {refInteractions(kbDose, "danger").map((i) => i.name).join(", ")}</div>
                {/if}
                {#if refInteractions(kbDose, "caution").length}
                  <div class="small warn-text muted">unsafe with: {refInteractions(kbDose, "caution").map((i) => i.name).join(", ")}</div>
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

      {#if contribCands.some((c) => !c.in_dosewiki)}
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
                  <button class="small-btn" onclick={() => previewDraft(c.id)}>Review draft…</button>
                </div>

                {#if contribDraft && contribDraft.name === c.name}
                  <pre class="draft">{contribDraft.json}</pre>
                  <div class="row-actions draft-actions">
                    <button class="link" onclick={() => (contribDraft = null)}>Cancel</button>
                    <button class="small-btn" onclick={() => openUrl(contribDraft!.upstream_url)}>Open DoseWiki</button>
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

    <!-- ============ BY SUBSTANCE ============ -->
    {#if tab === "bysub"}
      <section class="card">
        <h2>Substance Log</h2>
        {#if usage.length}
          {#each usage as u}
            <div class="usage">
              <div class="usage-head">
                <strong>{u.substance_name}</strong>
                <span class="muted small">{u.times_used} dose{u.times_used === 1 ? "" : "s"}</span>
              </div>
              <ul class="doses">
                {#each u.doses as d}
                  <li>
                    <span class="dtime">{fmtDate(d.taken_at)}</span>
                    <span class="damt">{d.amount ?? "?"} {d.unit}{d.route ? " · " + d.route : ""}</span>
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

    <!-- ============ DATA & SECURITY ============ -->
    {#if tab === "data"}
      {#if secErr}<p class="notice bad-notice">{secErr}</p>{/if}
      {#if secMsg}<p class="notice good-notice">{secMsg}</p>{/if}

      <section class="card">
        <h2>Companion <span class="off-badge" class:on={!companionOff}>{companionOff ? "off" : "on"}</span></h2>
        <label class="share">
          <input type="checkbox" bind:checked={companionOff} />
          Use Field Notes without a Companion
        </label>
        <p class="muted small">
          Hides the Companion tab and stops any local model from loading. The journal, timeline, dose
          reference, interaction checker and crisis resources don't use a model and are unaffected.
        </p>
      </section>


      <section class="card">
        <h2>Phone access <span class="off-badge" class:on={portal.running}>{portal.running ? "on" : "off"}</span></h2>
        <p class="muted small">
          Optional. Field Notes is an offline, on-device app and it stays that way unless you turn this
          on: it lets a phone on your <strong>tailnet</strong> reach this journal while the app is
          running here — the desktop is the workstation, the phone is what's actually in your hand.
        </p>
        <p class="muted small">
          The server listens on <strong>127.0.0.1 only</strong>, so nothing is exposed to your local
          network; Tailscale is what carries it to your phone, encrypted. Every request needs the
          paired token, and the portal won't serve a locked journal. Your data still never reaches a
          third party.
        </p>

        <div class="prereq" class:missing={ts != null && !ts.installed}>
          <p class="small">
            <strong>Before this is useful, you need <button class="link-inline" onclick={() => openUrl("https://tailscale.com/download")}>Tailscale</button></strong>
            — a free app that privately links your own devices — installed and signed into the same
            account on <em>both</em> this computer and your phone. It's what carries the connection.
            Without it the portal still turns on, but only this machine can reach it.
          </p>
          {#if ts != null}
            <p class="small prereq-status">
              {#if !ts.installed}
                Not detected on this computer yet — <button class="link-inline" onclick={() => openUrl("https://tailscale.com/download")}>install Tailscale</button>, then reopen this tab.
              {:else if !tailscaleUrl}
                Installed here, but not signed in yet — sign in, then reopen this tab.
              {:else}
                ✓ Tailscale is ready on this computer. Set it up on your phone too if you haven't.
              {/if}
            </p>
          {/if}
        </div>

        {#if portalErr}<p class="notice bad-notice">{portalErr}</p>{/if}

        <button class="primary small-btn" onclick={togglePortal}>
          {portal.running ? "Turn off phone access" : "Turn on phone access"}
        </button>

        {#if portal.running}
          <div class="sec-block">
            <h3>Pair a phone</h3>
            {#if !ts?.installed}
              <p class="muted small">
                ⚠️ Tailscale isn't installed, so the portal is only reachable from this machine.
                Install Tailscale on this computer and your phone, then come back.
              </p>
            {:else if !tailscaleUrl}
              <p class="muted small">⚠️ Tailscale is installed but isn't logged in — sign in, then reopen this tab.</p>
            {:else if ts.serving}
              <p class="muted small">
                Published to your tailnet. Your phone can reach <strong>{ts.url ?? tailscaleUrl}</strong> —
                and nothing else can: it's your tailnet, encrypted end to end, and every request still
                needs the paired token.
              </p>
              <button class="small-btn" disabled={serving} onclick={toggleServe}>
                {serving ? "Working…" : "Stop publishing to my tailnet"}
              </button>
            {:else}
              <p class="muted small">
                One more step: publish the portal to your tailnet, so your phone can reach it.
                Tailscale carries it, encrypted — this does not open anything to the internet or to
                your local network. You can undo it here at any time.
              </p>
              <button class="primary small-btn" disabled={serving} onclick={toggleServe}>
                {serving ? "Publishing…" : "Publish to my tailnet"}
              </button>
              <p class="muted small">
                That runs <code>{ts.serve_command}</code>, if you'd rather do it yourself.
              </p>
            {/if}

            {#if showQr && portalQrSvg}
              <div class="qr">{@html portalQrSvg}</div>
              <p class="muted small">
                Scan it with the phone's camera. <strong>This code is a key</strong> — it pairs whoever
                scans it. Don't leave it on screen, and don't photograph it.
              </p>
              <button class="small-btn" onclick={() => (showQr = false)}>Hide</button>
            {:else}
              <button class="small-btn" onclick={revealQr}>Show pairing QR code…</button>
            {/if}
          </div>
        {/if}
      </section>

      <section class="card">
        <h2>Encryption at rest</h2>
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
      </section>

      <section class="card">
        <h2>Obsidian vault sync</h2>
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

    <footer>
      “The greatest intention is to be open to learning.”
      <div class="footer-sub">for mindful exploration and contemplation</div>
    </footer>
  </main>

  <!-- ============ EMERGENCY / PANIC RESOURCES ============ -->
  {#if showHelp}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="modal-overlay" role="presentation" onclick={() => (showHelp = false)}>
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div class="help-modal" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()}>
        <h2>Get help now</h2>
        <p class="muted small">
          You're not alone. These lines are staffed by people who want to help, and calling is always okay.
          Field Notes is not an emergency service.
        </p>
        <ul class="crisis-res">
          {#each helpResources as r}
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

  <!-- ============ LIVE SESSION ============ -->
  {#if liveSession && selected}
    <div class="live">
      <div class="live-bar">
        <div>
          <div class="live-title">{selected.title || "Live session"}</div>
          <div class="muted">
            Started {fmtTime(selected.started_at)} · {elapsedSince(selected.started_at)} in
            <!-- Ticks with lsNow: the current t+ is the number a sitter actually
                 wants at a glance, against which every row below is read. -->
            {#if sessionT0}<span class="rel live-rel">· now {relTime(new Date(lsNow).toISOString(), sessionT0)}</span>{/if}
          </div>
        </div>
        <div class="row-actions">
          <button class="help-btn" onclick={openHelp}>Get help now</button>
          <button class="ghost" onclick={endLiveSession}>Exit</button>
        </div>
      </div>

      <div class="live-body">
        <section class="live-timeline">
          <h3>Doses</h3>
          {#if selected.doses.length}
            <ul class="live-doses">
              {#each selected.doses as d}
                <li><span class="muted">{fmtTime(d.taken_at)}{#if sessionT0}<span class="rel"> ({relTime(d.taken_at, sessionT0)})</span>{/if}</span> — {d.substance_name} {d.amount ?? "?"} {d.unit}{d.route ? " · " + d.route : ""}</li>
              {/each}
            </ul>
          {:else}
            <p class="muted">Nothing logged yet.</p>
          {/if}

          <h3>Quick log</h3>
          <div class="quick-log">
            <input placeholder="Substance" bind:value={qSub} />
            <input placeholder="Amount" inputmode="decimal" bind:value={qAmt} />
            <input placeholder="Unit" bind:value={qUnit} />
            <input placeholder="Route" bind:value={qRoute} />
            <button class="primary" disabled={!qSub.trim()} onclick={quickLog}>Log dose</button>
          </div>

          <h3>Timeline</h3>
          <div class="quick-log">
            <input placeholder="How are you feeling right now?" bind:value={lsNote} onkeydown={(e) => e.key === "Enter" && quickNote()} />
            <button class="primary" disabled={!lsNote.trim()} onclick={quickNote}>Add note</button>
          </div>
          {#if selected.timeline.length}
            <ul class="live-events">
              {#each selected.timeline as t}
                <li><span class="muted">{fmtTime(t.at)}{#if sessionT0}<span class="rel"> ({relTime(t.at, sessionT0)})</span>{/if}</span> {t.note}</li>
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
    </div>
  {/if}
{/if}

<style>
  :global(:root) {
    --bg: #16181d;
    --card: #1e2127;
    --ink: #e7e9ee;
    --muted: #9aa0ab;
    --line: #2e323b;
    --accent: #6d8fb0;
    --accent-ink: #0c0e12;
    --danger: #e06b6b;
    --caution: #d6a24e;
    --note: #6fae8f;
  }
  :global(body) {
    margin: 0;
    background: var(--bg);
    color: var(--ink);
    font-family: Inter, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
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
  nav { display: inline-flex; gap: 4px; background: var(--card); border: 1px solid var(--line); border-radius: 999px; padding: 4px; }
  nav button { border: none; background: transparent; color: var(--muted); font: inherit; font-weight: 600; padding: 0.4rem 0.9rem; border-radius: 999px; cursor: pointer; }
  nav button.active { background: var(--accent); color: var(--accent-ink); }

  .card { background: var(--card); border: 1px solid var(--line); border-radius: 16px; padding: 1.4rem; }
  h2 { margin: 0 0 0.6rem; font-size: 1.15rem; }
  h3 { margin: 1.2rem 0 0.4rem; font-size: 0.95rem; color: var(--muted); text-transform: uppercase; letter-spacing: 0.04em; }
  p { line-height: 1.5; }
  .muted { color: var(--muted); }
  .small { font-size: 0.85rem; }

  button { font: inherit; cursor: pointer; border-radius: 9px; border: 1px solid transparent; }
  .primary { background: var(--accent); color: var(--accent-ink); font-weight: 600; padding: 0.7rem 1rem; }
  .primary:hover { filter: brightness(1.08); }
  .ghost { background: transparent; color: var(--muted); border-color: var(--line); padding: 0.7rem 1rem; margin-top: 0.8rem; }
  .small-btn { padding: 0.5rem 0.85rem; margin: 0; }
  .link { background: none; border: none; color: var(--accent); padding: 0; font-weight: 600; cursor: pointer; margin-bottom: 0.6rem; }
  .link-inline { background: none; border: none; color: var(--accent); padding: 0; font: inherit; font-weight: 600; cursor: pointer; text-decoration: underline; }
  .prereq { border: 1px solid var(--line); border-radius: 10px; padding: 0.5rem 0.8rem; margin: 0.2rem 0 0.9rem; }
  .prereq.missing { border-color: var(--caution); }
  .prereq p { color: var(--muted); margin: 0.3rem 0; }
  .prereq-status { font-weight: 600; }

  input { font: inherit; background: var(--bg); color: var(--ink); border: 1px solid var(--line); border-radius: 8px; padding: 0.55rem 0.7rem; }
  input.narrow { width: 5.5rem; }

  .exp-head { display: flex; justify-content: space-between; align-items: center; gap: 0.8rem; }
  .exp-list, .sub-list, .doses, .timeline { list-style: none; padding: 0; margin: 0.6rem 0 0; }
  .exp-row { width: 100%; text-align: left; background: transparent; border: 1px solid var(--line); border-radius: 10px; padding: 0.8rem; margin-bottom: 0.5rem; display: flex; justify-content: space-between; align-items: center; gap: 0.8rem; color: var(--ink); }
  .exp-row:hover { border-color: var(--accent); }
  .exp-row strong { display: block; }
  .exp-meta { display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap; justify-content: flex-end; }
  .pill { font-size: 0.72rem; border: 1px solid var(--line); border-radius: 999px; padding: 0.1rem 0.5rem; color: var(--muted); }

  .new-exp, .dose-form, .new-sub { display: flex; flex-wrap: wrap; gap: 0.5rem; margin: 0.8rem 0; align-items: center; }
  .new-exp input, .new-sub input { flex: 1; min-width: 8rem; }
  .new-note { display: flex; flex-direction: column; gap: 0.5rem; margin: 0.8rem 0; }
  .new-note textarea { width: 100%; resize: vertical; }
  .note-pill { font-style: italic; }
  .note-body { white-space: pre-wrap; margin-top: 0.6rem; line-height: 1.55; }
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
  .chip { font-size: 0.75rem; border: 1px solid var(--line); border-radius: 999px; padding: 0.2rem 0.6rem; background: transparent; color: var(--muted); cursor: pointer; }
  .chip.on { background: var(--accent); color: var(--accent-ink); border-color: var(--accent); }

  .sub-list li { border: 1px solid var(--line); border-radius: 10px; padding: 0.7rem 0.9rem; margin-bottom: 0.5rem; }
  .usage { border: 1px solid var(--line); border-radius: 10px; padding: 0.8rem 1rem; margin-bottom: 0.6rem; }
  .usage-head { display: flex; justify-content: space-between; align-items: baseline; }

  .row-actions { display: inline-flex; gap: 0.5rem; align-items: center; margin-left: auto; }
  .icon-btn { background: transparent; border: 1px solid transparent; color: var(--muted); padding: 0.15rem 0.35rem; border-radius: 6px; font-size: 0.85rem; line-height: 1; }
  .icon-btn:hover { color: var(--ink); border-color: var(--line); }
  .link.danger-link { color: var(--danger); }
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
  .update-banner { display: flex; flex-wrap: wrap; align-items: center; gap: 0.8rem; justify-content: space-between; border: 1px solid var(--accent); background: color-mix(in srgb, var(--accent) 12%, transparent); border-radius: 10px; padding: 0.6rem 0.9rem; margin-bottom: 1rem; font-size: 0.9rem; }
  .ai-log { max-height: 150px; overflow: auto; background: color-mix(in srgb, var(--ink) 6%, transparent); border-radius: 8px; padding: 0.6rem; font-size: 0.75rem; line-height: 1.4; white-space: pre-wrap; word-break: break-word; color: var(--muted); margin: 0.2rem 0 0; }
  .import-panel { border: 1px solid var(--line); border-radius: 12px; padding: 1rem; margin: 0.6rem 0 1rem; display: flex; flex-direction: column; gap: 0.6rem; }
  .import-text { font: inherit; background: var(--bg); color: var(--ink); border: 1px solid var(--line); border-radius: 8px; padding: 0.6rem 0.7rem; resize: vertical; width: 100%; box-sizing: border-box; }

  .notice { border: 1px solid var(--caution); background: color-mix(in srgb, var(--caution) 12%, transparent); border-radius: 10px; padding: 0.8rem 1rem; line-height: 1.5; }
  .notice.bad-notice { border-color: var(--danger); background: color-mix(in srgb, var(--danger) 12%, transparent); }
  .notice.good-notice { border-color: var(--note); background: color-mix(in srgb, var(--note) 12%, transparent); }
  .notice.warn-notice { border-color: var(--caution); background: color-mix(in srgb, var(--caution) 14%, transparent); }

  .unlock-form { display: flex; flex-direction: column; gap: 0.7rem; margin: 1.2rem 0 0.8rem; }
  .unlock-form input { padding: 0.6rem 0.7rem; border-radius: 10px; border: 1px solid var(--line); background: var(--bg); color: var(--ink); font-size: 1rem; }
  .dont-show { display: flex; align-items: center; gap: 0.5rem; color: var(--muted); font-size: 0.9rem; margin: 1rem 0; cursor: pointer; }
  .dont-show input { width: auto; }
  .sec-block { border-top: 1px solid var(--line); margin-top: 1.1rem; padding-top: 1.1rem; display: flex; flex-direction: column; gap: 0.6rem; align-items: flex-start; }
  .sec-block h3 { margin: 0; font-size: 0.98rem; }
  .sec-block input { padding: 0.5rem 0.65rem; border-radius: 9px; border: 1px solid var(--line); background: var(--bg); color: var(--ink); min-width: 16rem; max-width: 24rem; }
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

  footer { margin-top: 1.6rem; text-align: center; color: var(--muted); font-size: 0.8rem; }
  .footer-sub { margin-top: 0.2rem; font-size: 0.72rem; opacity: 0.75; }

  /* ---- crisis banner + emergency resources ---- */
  .help-btn { background: var(--danger); color: #fff; border: none; border-radius: 8px; padding: 0.4rem 0.8rem; font-weight: 600; cursor: pointer; }
  .help-btn:hover { filter: brightness(1.08); }
  .danger-card { border-color: color-mix(in srgb, var(--danger) 45%, var(--line)); }
  .danger-btn { background: var(--danger); color: #fff; border: none; border-radius: 8px; padding: 0.45rem 0.9rem; font-weight: 600; cursor: pointer; }
  .danger-btn:hover:not(:disabled) { filter: brightness(1.08); }
  .danger-btn:disabled { opacity: 0.55; cursor: default; }
  .crisis-banner { border-radius: 12px; padding: 1rem 1.2rem; margin-bottom: 1rem; border: 1px solid var(--caution); background: color-mix(in srgb, var(--caution) 14%, var(--card)); }
  .crisis-offer { display: flex; gap: 0.6rem; flex-wrap: wrap; align-items: center; }
  .crisis-offer button { margin-top: 0.4rem; }
  .crisis-banner.psychiatric, .crisis-banner.medical { border-color: var(--danger); background: color-mix(in srgb, var(--danger) 14%, var(--card)); }
  .crisis-head { display: flex; justify-content: space-between; align-items: flex-start; gap: 1rem; }
  .crisis-res { list-style: none; padding: 0; margin: 0.7rem 0; display: flex; flex-direction: column; gap: 0.6rem; }
  .crisis-res .contact { font-variant-numeric: tabular-nums; }
  .crisis-res li { line-height: 1.4; }

  .modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.6); display: grid; place-items: center; padding: 1.5rem; z-index: 50; }
  .help-modal { background: var(--card); border: 1px solid var(--danger); border-radius: 16px; padding: 1.6rem; max-width: 520px; width: 100%; }
  .help-modal h2 { margin-top: 0; }

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
  .live { position: fixed; inset: 0; background: var(--bg); z-index: 40; display: flex; flex-direction: column; padding: 1.2rem clamp(1rem, 4vw, 3rem); overflow-y: auto; }
  .live-bar { display: flex; justify-content: space-between; align-items: flex-start; gap: 1rem; border-bottom: 1px solid var(--line); padding-bottom: 1rem; }
  .live-title { font-size: 1.5rem; font-weight: 700; }
  .live-body { display: grid; grid-template-columns: 1fr 1fr; gap: 1.5rem; margin-top: 1.2rem; align-items: start; }
  @media (max-width: 780px) { .live-body { grid-template-columns: 1fr; } }
  .live-timeline h3, .live-companion h3 { margin: 1.1rem 0 0.5rem; }
  .live-doses, .live-events { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 0.35rem; font-size: 1.05rem; }
  .quick-log { display: flex; flex-wrap: wrap; gap: 0.5rem; }
  .quick-log input { flex: 1; min-width: 6rem; padding: 0.55rem 0.7rem; border-radius: 9px; border: 1px solid var(--line); background: var(--card); color: var(--ink); font-size: 1rem; }
  .live-chat { min-height: 200px; max-height: 42vh; }
</style>
