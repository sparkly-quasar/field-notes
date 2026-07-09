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
    companionChat,
    parseExperience,
    importExperience,
    pwUpdate,
    pwStatus,
    pwLookup,
    dbStatus,
    unlockDb,
    enableEncryption,
    disableEncryption,
    changePassphrase,
    exportBackup,
    importBackup,
    obsidianExport,
    obsidianImport,
    type DbStatus,
    type ParsedExperience,
    type PwInfo,
    type PwStatus,
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

  // Obsidian vault sync
  const VAULT_KEY = "fieldnotes.vaultFolder";
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

  // dose form
  let dSubstance = $state("");
  let dAmount = $state("");
  let dUnit = $state("mg");
  let dRoute = $state("oral");
  let dTime = $state("");

  // DoseWiki reference data
  let pwStat = $state<PwStatus | null>(null);
  let pwBusy = $state(false);
  let pwErr = $state<string | null>(null);
  let dRef = $state<PwInfo | null>(null); // reference for the dose being logged

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
  let aiLog = $state<string[]>([]);
  let aiBusy = $state(false);
  let aiErr = $state<string | null>(null);
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

  const nowIso = () => new Date().toISOString();
  const fmtDate = (iso: string) => new Date(iso).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
  const fmtTime = (iso: string) => new Date(iso).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });

  // <input type="datetime-local"> <-> ISO helpers (local time)
  const localOffset = (d: Date) => new Date(d.getTime() - d.getTimezoneOffset() * 60000);
  const nowLocalInput = () => localOffset(new Date()).toISOString().slice(0, 16);
  const isoToLocalInput = (iso: string) => localOffset(new Date(iso)).toISOString().slice(0, 16);
  const localInputToIso = (local: string) => (local ? new Date(local).toISOString() : nowIso());

  onMount(() => {
    interactionClasses().then((c) => (classesVocab = c));
    checkForUpdate();
    dontShowDisclaimer = localStorage.getItem(HIDE_DISCLAIMER_KEY) === "1";
    vaultFolder = localStorage.getItem(VAULT_KEY) ?? "";
    loadDbStatus();
    const un = listen<string>("ai-progress", (e) => {
      aiLog = [...aiLog.slice(-200), e.payload];
    });
    return () => un.then((f) => f());
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
    if (encNewPass.length < 1) return (secErr = "Choose a passphrase.");
    if (encNewPass !== encNewPass2) return (secErr = "The passphrases don't match.");
    secBusy = true;
    try {
      await enableEncryption(encNewPass);
      encNewPass = encNewPass2 = "";
      db = await dbStatus();
      secMsg = "Encryption is on. You'll need this passphrase each time you open the app.";
    } catch (e) {
      secErr = typeof e === "string" ? e : String(e);
    } finally {
      secBusy = false;
    }
  }

  async function doDisableEncryption() {
    secReset();
    if (!encDisablePass) return (secErr = "Enter your current passphrase.");
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
    if (!chgCurrent) return (secErr = "Enter your current passphrase.");
    if (chgNew.length < 1) return (secErr = "Choose a new passphrase.");
    if (chgNew !== chgNew2) return (secErr = "The new passphrases don't match.");
    secBusy = true;
    try {
      await changePassphrase(chgCurrent, chgNew);
      chgCurrent = chgNew = chgNew2 = "";
      secMsg = "Passphrase changed.";
    } catch (e) {
      secErr = typeof e === "string" ? e : String(e);
    } finally {
      secBusy = false;
    }
  }

  async function doExportBackup() {
    secReset();
    try {
      const path = await save({
        title: "Save journal backup",
        defaultPath: `field-notes-backup-${new Date().toISOString().slice(0, 10)}.db`,
        filters: [{ name: "Field Notes journal", extensions: ["db"] }],
      });
      if (!path) return;
      secBusy = true;
      await exportBackup(path);
      secMsg = db.encrypted
        ? "Backup written. It is encrypted with your current passphrase."
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
    selected = await getExperience(id);
    dTime = nowLocalInput();
  }

  function openNewExp() {
    showNewExp = !showNewExp;
    if (showNewExp && !neStart) neStart = nowLocalInput();
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

  async function delExp() {
    if (!selected || !confirm("Delete this experience and all its doses? This cannot be undone.")) return;
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
  }

  function toggleClass(c: string) {
    nsClasses = nsClasses.includes(c) ? nsClasses.filter((x) => x !== c) : [...nsClasses, c];
  }

  async function loadAi() {
    ai = await aiStatus();
    if (!aiRecommended.length) aiRecommended = await aiRecommendedModels();
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

  // the experience the companion is aware of (most recent), if sharing is on
  const attachedExp = $derived(cShareSession && experiences.length ? experiences[0] : null);

  async function sendCompanion() {
    if (!cInput.trim() || !aiModel || cSending) return;
    const history: ChatMsg[] = [...cMessages, { role: "user", content: cInput.trim() }];
    cMessages = history;
    cInput = "";
    cSending = true;
    try {
      const reply = await companionChat(aiModel, history, attachedExp ? attachedExp.id : null);
      cMessages = [...cMessages, { role: "assistant", content: reply }];
    } catch (e) {
      cMessages = [...cMessages, { role: "assistant", content: `⚠️ ${typeof e === "string" ? e : String(e)}` }];
    } finally {
      cSending = false;
    }
  }

  async function goTab(t: Tab) {
    tab = t;
    selected = null;
    if (t === "bysub") await loadUsage();
    if (t === "substances") { await loadSubstances(); await loadPwStatus(); }
    if (t === "journal") await loadJournal();
    if (t === "companion") await loadAi();
    if (t === "data") { secReset(); db = await dbStatus(); }
  }

  // ---- DoseWiki reference ----
  async function loadPwStatus() {
    pwStat = await pwStatus();
  }
  async function updatePw() {
    pwBusy = true;
    pwErr = null;
    try {
      await pwUpdate();
      pwStat = await pwStatus();
    } catch (e) {
      pwErr = typeof e === "string" ? e : String(e);
    } finally {
      pwBusy = false;
    }
  }
  async function lookupRef(name: string) {
    dRef = name.trim() ? await pwLookup(name.trim()) : null;
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
      <p class="lead">This journal is encrypted. Enter your passphrase to unlock it.</p>
      <form class="unlock-form" onsubmit={(e) => { e.preventDefault(); doUnlock(); }}>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          type="password"
          autocomplete="current-password"
          autofocus
          placeholder="Passphrase"
          bind:value={unlockPass}
        />
        {#if unlockErr}<p class="notice bad-notice">{unlockErr}</p>{/if}
        <button class="primary" type="submit" disabled={unlockBusy || !unlockPass}>
          {unlockBusy ? "Unlocking…" : "Unlock"}
        </button>
      </form>
      <p class="muted small">There is no recovery — if you lose this passphrase, the journal cannot be opened.</p>
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
      <p class="muted small">A one-time install of Ollama, the local model runner. macOS uses Homebrew; Linux uses the official installer.</p>
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
    <header>
      <h1>Field Notes</h1>
      <nav>
        <button class:active={tab === "journal"} onclick={() => goTab("journal")}>Journal</button>
        <button class:active={tab === "companion"} onclick={() => goTab("companion")}>Companion</button>
        <button class:active={tab === "substances"} onclick={() => goTab("substances")}>Substances</button>
        <button class:active={tab === "bysub"} onclick={() => goTab("bysub")}>By substance</button>
        <button class:active={tab === "data"} onclick={() => goTab("data")}>Data &amp; security</button>
      </nav>
    </header>

    <!-- ============ JOURNAL ============ -->
    {#if tab === "journal"}
      {#if selected}
        <section class="card">
          <button class="link" onclick={() => (selected = null)}>← All experiences</button>
          <div class="exp-head">
            <h2>{selected.title || "Untitled experience"}</h2>
            <span class="row-actions">
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
                    <span class="dtime">{fmtTime(d.taken_at)}</span>
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
                  <span class="dtime">{fmtTime(t.at)}</span>
                  <span class="tl-note">{t.note}{t.intensity != null ? ` (${t.intensity}/10)` : ""}{t.mood ? ` · ${t.mood}` : ""}</span>
                  <button class="icon-btn" title="Delete" onclick={() => delTimeline(t.id)}>✕</button>
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
        </section>
      {:else}
        <section class="card">
          <div class="exp-head">
            <h2>Experiences</h2>
            <span class="row-actions">
              <button class="ghost small-btn" onclick={openImport}>Import from text</button>
              <button class="primary small-btn" onclick={openNewExp}>+ New</button>
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
                    <div>
                      <strong>{e.title || "Untitled"}</strong>
                      <span class="muted small">{fmtDate(e.started_at)}{e.ended_at ? "" : " · ongoing"}</span>
                    </div>
                    <div class="exp-meta">
                      {#each e.substances as s}<span class="pill">{s}</span>{/each}
                      <span class="muted small">{e.dose_count} dose{e.dose_count === 1 ? "" : "s"}</span>
                    </div>
                  </button>
                </li>
              {/each}
            </ul>
          {:else}
            <p class="muted">No experiences yet. Start one to begin logging.</p>
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
            <select bind:value={aiModel} class="model-sel">
              {#each ai?.models ?? [] as m}<option value={m}>{m}</option>{/each}
            </select>
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

          <label class="share">
            <input type="checkbox" bind:checked={cShareSession} />
            Share current session with the companion
            {#if attachedExp}<span class="muted small"> · aware of “{attachedExp.title || "Untitled"}”</span>{/if}
          </label>

          <div class="chat">
            {#if !cMessages.length}
              <p class="muted small chat-empty">Say hello, or ask about how you're feeling. It can see your logged doses if sharing is on.</p>
            {/if}
            {#each cMessages as m}
              <div class="bubble {m.role}">{m.content}</div>
            {/each}
            {#if cSending}<div class="bubble assistant muted">…</div>{/if}
          </div>

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
      <section class="card ref-card">
        <div class="exp-head">
          <h2>Dose reference</h2>
          <button class="primary small-btn" disabled={pwBusy} onclick={updatePw}>{pwBusy ? "Reloading…" : "Reload reference"}</button>
        </div>
        {#if pwStat && pwStat.count > 0}
          <p class="muted small">{pwStat.count} substances bundled offline{pwStat.snapshot ? ` · snapshot ${pwStat.snapshot}` : ""}. Dose ranges, durations &amp; graded interactions show while you log.</p>
        {:else}
          <p class="muted small">Dose ranges, durations, and graded interaction data for hundreds of substances, bundled with the app — fully offline. Nothing is ever sent when you look things up.</p>
        {/if}
        {#if pwBusy}<p class="muted small">Reloading the bundled reference…</p>{/if}
        {#if pwErr}<p class="notice bad-notice">{pwErr}</p>{/if}
        <p class="muted attribution">Dose data from <strong>DoseWiki</strong> (dose.wiki), dedicated to the public domain under CC0. Reference only — not a prescription.</p>
      </section>

      <section class="card">
        <h2>Substances</h2>
        <p class="muted small">Catalogue substances you track. Assign classes so the safety checker can flag interactions — or leave them blank and common ones are auto-classified.</p>

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
    {/if}

    <!-- ============ BY SUBSTANCE ============ -->
    {#if tab === "bysub"}
      <section class="card">
        <h2>By substance</h2>
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
        <h2>Encryption at rest</h2>
        {#if db.encrypted}
          <p class="muted small">
            This journal is <strong>encrypted</strong>. Its contents are unreadable on disk without your
            passphrase, which you enter each time you open the app.
          </p>

          <div class="sec-block">
            <h3>Change passphrase</h3>
            <input type="password" autocomplete="current-password" placeholder="Current passphrase" bind:value={chgCurrent} />
            <input type="password" autocomplete="new-password" placeholder="New passphrase" bind:value={chgNew} />
            <input type="password" autocomplete="new-password" placeholder="Confirm new passphrase" bind:value={chgNew2} />
            <button class="primary small-btn" disabled={secBusy} onclick={doChangePassphrase}>Change passphrase</button>
          </div>

          <div class="sec-block">
            <h3>Turn off encryption</h3>
            <p class="muted small">Returns the journal to plaintext on this device.</p>
            <input type="password" autocomplete="current-password" placeholder="Current passphrase" bind:value={encDisablePass} />
            <button class="ghost small-btn" disabled={secBusy} onclick={doDisableEncryption}>Disable encryption</button>
          </div>
        {:else}
          <p class="muted small">
            The journal is currently stored <strong>unencrypted</strong>. Turn on encryption to protect it with a
            passphrase (AES-256 via SQLCipher). You'll enter the passphrase each time you open the app.
          </p>
          <p class="notice warn-notice">
            There is no recovery. If you forget this passphrase, the journal cannot be opened by anyone — including you.
          </p>
          <div class="sec-block">
            <input type="password" autocomplete="new-password" placeholder="Choose a passphrase" bind:value={encNewPass} />
            <input type="password" autocomplete="new-password" placeholder="Confirm passphrase" bind:value={encNewPass2} />
            <button class="primary small-btn" disabled={secBusy} onclick={doEnableEncryption}>Enable encryption</button>
          </div>
        {/if}
      </section>

      <section class="card">
        <h2>Backup &amp; restore</h2>
        <p class="muted small">
          A backup is a single-file copy of your whole journal. {db.encrypted
            ? "It keeps its encryption — you'll need this passphrase to restore or open it elsewhere."
            : "It is unencrypted, so store it somewhere safe."}
        </p>
        <div class="row-actions">
          <button class="primary small-btn" disabled={secBusy} onclick={doExportBackup}>Export backup…</button>
          <button class="ghost small-btn" disabled={secBusy} onclick={doImportBackup}>Restore from backup…</button>
        </div>
        <p class="muted small">Restoring replaces the journal on this device with the backup's contents.</p>
      </section>

      <section class="card">
        <h2>Obsidian vault sync</h2>
        <p class="muted small">
          Keep a copy of your journal in an Obsidian vault as Markdown notes — one per experience, with a
          readable summary you can annotate. Fully offline; nothing leaves your device.
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
    {/if}

    <footer>Offline · private · harm-reduction. Not medical advice.</footer>
  </main>
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
  .dose-form input:first-child { flex: 1; min-width: 8rem; }

  .doses li, .timeline li { display: flex; gap: 0.7rem; padding: 0.4rem 0; border-bottom: 1px solid var(--line); font-size: 0.92rem; align-items: baseline; }
  .doses li:last-child, .timeline li:last-child { border-bottom: none; }
  .dtime { color: var(--muted); font-variant-numeric: tabular-nums; min-width: 3.4rem; }
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

  .ref-card { margin-bottom: 1rem; }
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
  .share { display: flex; align-items: center; gap: 0.4rem; font-size: 0.85rem; color: var(--muted); margin: 0.4rem 0 0.8rem; }
  .share input { width: auto; }
  .chat { display: flex; flex-direction: column; gap: 0.5rem; min-height: 220px; max-height: 46vh; overflow-y: auto; padding: 0.4rem; border: 1px solid var(--line); border-radius: 12px; background: var(--bg); }
  .chat-empty { margin: auto; text-align: center; max-width: 26ch; }
  .bubble { padding: 0.55rem 0.8rem; border-radius: 12px; max-width: 82%; line-height: 1.45; font-size: 0.92rem; white-space: pre-wrap; word-break: break-word; }
  .bubble.user { align-self: flex-end; background: var(--accent); color: var(--accent-ink); border-bottom-right-radius: 4px; }
  .bubble.assistant { align-self: flex-start; background: var(--card); border: 1px solid var(--line); border-bottom-left-radius: 4px; }
  .chat-input { display: flex; gap: 0.5rem; margin-top: 0.7rem; }
  .chat-input input { flex: 1; }

  footer { margin-top: 1.6rem; text-align: center; color: var(--muted); font-size: 0.8rem; }
</style>
