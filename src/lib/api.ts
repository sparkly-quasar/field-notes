// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// Typed wrappers around the Tauri command surface (see src-tauri/src/commands.rs).

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { inTauri } from "./portal";
import { phoneInvoke } from "./offline";

/**
 * The one seam between this app and its backend.
 *
 * On the desktop it's Tauri's IPC. On a phone reaching the same frontend through the
 * portal there is no IPC, so it's an HTTP call to the desktop instead. Everything
 * below this line — and the whole UI above it — is written once and doesn't care
 * which. Keep it that way: this is the only file that may import `invoke`.
 */
function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  // A phone: through the portal, or answered on the phone when the computer is
  // out of reach (offline.ts).
  if (!inTauri()) return phoneInvoke<T>(cmd, args);
  // A desktop whose journal lives on another computer: journal commands go to the
  // backend's `remote_call`, which sends them to the server — or, if it's out of
  // reach, queues new entries and answers from what this computer knows.
  if (remoteMode && ROUTED.has(cmd)) return tauriInvoke<T>("remote_call", { cmd, args: args ?? {} });
  return tauriInvoke<T>(cmd, args);
}

/**
 * What a connected desktop sends to its server. Must match `ROUTED` in
 * `src-tauri/src/remote.rs` — a Rust test reads this file to check.
 */
const ROUTED = new Set([
  "list_experiences",
  "get_experience",
  "export_experience_markdown",
  "usage_by_substance",
  "usage_stats",
  "list_substances",
  "create_experience",
  "end_experience",
  "log_dose",
  "add_timeline_event",
  "add_substance",
  "update_experience",
  "set_writeup_skipped",
  "update_dose",
  "update_timeline_event",
  "delete_experience",
  "delete_dose",
  "delete_timeline_event",
  "delete_substance",
  "list_unit_kinds",
  "save_unit_kind",
  "delete_unit_kind",
  "set_bedtime",
  "check_combo",
  "canonical_name",
  "crisis_scan",
  "companion_chat",
  "companion_warm",
  "compute_status",
  "ai_status",
  "ollama_up",
  "ollama_models",
  "ai_start",
]);

let remoteMode = false;
/** Switch the transport. Set from `remote_status` once the local journal is open. */
export function setRemoteMode(on: boolean) {
  remoteMode = on;
}
export function isRemoteMode() {
  return remoteMode;
}

export interface Substance {
  id: number;
  name: string;
  aliases: string[];
  category: string;
  classes: string[];
  dose_note: string;
  notes: string;
  user_added: boolean;
  created_at: string;
}

export interface Experience {
  id: number;
  /** "session" (a drug session) or "note" (a plain journal entry). Set at creation. */
  kind: "session" | "note";
  title: string;
  intention: string;
  setting: string;
  notes: string;
  rating: number | null;
  started_at: string;
  ended_at: string | null;
  created_at: string;
  /** Marked as not needing a write-up. Absent from servers older than v0.15. */
  writeup_skipped?: boolean;
}

export type ExperienceSummary = Experience & {
  substances: string[];
  dose_count: number;
  /** Once ended, asks for a write-up by default: it has a psychedelic, empathogen,
   *  dissociative or unknown substance in it (or no doses). Absent from older
   *  servers and from entries waiting on a phone: treat as true. */
  writeup_expected?: boolean;
};

export interface Dose {
  id: number;
  experience_id: number;
  substance_id: number | null;
  substance_name: string;
  amount: number | null;
  unit: string;
  route: string;
  taken_at: string;
  note: string;
  /** What the dose was. Missing from older servers, which reads as "not said". */
  form?: string;
  per_unit?: number | null;
  per_unit_unit?: string;
  unit_label?: string;
  estimate?: number | null;
  estimate_unit?: string;
}

/**
 * What a dose was, beyond amount and unit: `DoseDetail` in db.rs. Every field
 * defaults to "not said".
 */
export interface DoseDetail {
  /** `FORMS` in quicklog.ts: dried, fresh, powdered, edible; leaf, extract, 7-oh. */
  form?: string;
  /** For a counted unit: how much one holds, kept on the dose as it was. */
  per_unit?: number | null;
  per_unit_unit?: string;
  /** The capsule kind's name when logged ("00 caps"). */
  unit_label?: string;
  /** An edible's estimated content for the whole dose: "g" (dried mushroom) or "mg psilocybin". */
  estimate?: number | null;
  estimate_unit?: string;
}

/** A named capsule (pill, tab) for one substance: "00 caps, 0.45 g". */
export interface UnitKind {
  id: number;
  substance: string;
  label: string;
  per_unit: number;
  per_unit_unit: string;
}

export interface TimelineEvent {
  id: number;
  experience_id: number;
  at: string;
  note: string;
  mood: string;
  intensity: number | null;
}

export type ExperienceDetail = Experience & {
  doses: Dose[];
  timeline: TimelineEvent[];
};

export interface Warning {
  severity: "danger" | "caution" | "note";
  a: string;
  b: string;
  message: string;
  /** Harm-reduction notes for the classes involved (older servers send none). */
  advice?: string[];
}

export interface SubstanceUsage {
  substance_name: string;
  times_used: number;
  doses: Dose[];
}

export interface LogDoseResult {
  dose: Dose;
  warnings: Warning[];
}

export interface SubstanceInput {
  name: string;
  aliases?: string[];
  category?: string;
  classes?: string[];
  dose_note?: string;
  notes?: string;
}

export interface ExperienceInput {
  /** Defaults to "session" on the backend. */
  kind?: "session" | "note";
  title?: string;
  intention?: string;
  setting?: string;
  started_at: string;
}

export interface DoseInput extends DoseDetail {
  experience_id: number;
  substance_name: string;
  amount: number | null;
  unit?: string;
  route?: string;
  taken_at: string;
  note?: string;
}

export interface TimelineInput {
  experience_id: number;
  at: string;
  note?: string;
  mood?: string;
  intensity: number | null;
}

export interface ExperienceUpdate {
  title?: string;
  intention?: string;
  setting?: string;
  notes?: string;
  rating: number | null;
  started_at: string;
  ended_at: string | null;
}

export interface TimelineUpdate {
  at: string;
  note?: string;
  mood?: string;
  intensity: number | null;
}

export interface DoseUpdate {
  substance_name: string;
  amount: number | null;
  unit?: string;
  route?: string;
  taken_at: string;
  note?: string;
  /** What the form shows now, replacing what the dose had. Nested, unlike a
   *  new dose: an edit without it (an older phone) leaves the dose's detail be. */
  detail?: DoseDetail;
}

export const updateExperience = (id: number, update: ExperienceUpdate) =>
  invoke<Experience>("update_experience", { id, update });
/** "Doesn't need a write-up", or undo it. A reflection is never owed. */
export const setWriteupSkipped = (id: number, skipped: boolean) =>
  invoke<Experience>("set_writeup_skipped", { id, skipped });
export const updateDose = (id: number, update: DoseUpdate) => invoke<Dose>("update_dose", { id, update });
export const updateTimelineEvent = (id: number, update: TimelineUpdate) =>
  invoke<TimelineEvent>("update_timeline_event", { id, update });
export const deleteExperience = (id: number) => invoke<void>("delete_experience", { id });
export const deleteDose = (id: number) => invoke<void>("delete_dose", { id });
export const listUnitKinds = (substance: string) => invoke<UnitKind[]>("list_unit_kinds", { substance });
export const saveUnitKind = (substance: string, kind: Omit<UnitKind, "id" | "substance">) =>
  invoke<UnitKind>("save_unit_kind", { substance, kind });
export const deleteUnitKind = (id: number) => invoke<void>("delete_unit_kind", { id });
/** "23:00", "varies", "skip", or null to forget it (the time-of-day card asks again). */
export const setBedtime = (value: string | null) => invoke<void>("set_bedtime", { value });
export const deleteTimelineEvent = (id: number) => invoke<void>("delete_timeline_event", { id });
export const deleteSubstance = (id: number) => invoke<void>("delete_substance", { id });

// ---- DoseWiki reference cache ----
export interface PwRange {
  min: number | null;
  max: number | null;
}
export interface PwRoa {
  name: string;
  units: string | null;
  threshold: number | null;
  light: PwRange;
  common: PwRange;
  strong: PwRange;
  heavy: number | null;
  /** Top of the heavy band, when a source gives one. */
  heavy_max?: number | null;
  onset: string | null;
  come_up: string | null;
  peak: string | null;
  offset: string | null;
  after_effects: string | null;
  total: string | null;
  half_life: string | null;
}
export interface PwInteraction {
  name: string;
  reason: string | null;
  severity: "danger" | "caution" | "note";
}
export interface PwInfo {
  name: string;
  common_names: string[];
  psychoactive: string[];
  chemical: string[];
  roas: PwRoa[];
  interactions: PwInteraction[];
  /** Where the dose figures come from, when it isn't DoseWiki. */
  dose_note?: string | null;
}
export interface PwStatus {
  count: number;
  snapshot: string;
}
export const pwUpdate = () => invoke<number>("pw_update");
export const pwStatus = () => invoke<PwStatus>("pw_status");
export const pwLookup = (name: string) => invoke<PwInfo | null>("pw_lookup", { name });
/** Every dose-reference substance with its street names — for matching pasted logs. */
/** The substance a typed name clearly means ("acid" → LSD), or null to keep it as written. */
export const canonicalName = (name: string) => invoke<string | null>("canonical_name", { name });
export const pwNames = () => invoke<{ name: string; aliases: string[] }[]>("pw_names");

// ---- Knowledge corpus (DoseWiki prose, searched offline with BM25) ----
// Reference prose only. Doses and interactions come from pwLookup/checkCombo,
// which are deterministic; never read a dose or a combo verdict out of a Hit.
export interface KnowledgeHit {
  title: string;
  slug: string;
  section: string;
  text: string;
  /** DoseWiki's entry for this substance is sparse — say so rather than lean on it. */
  thin: boolean;
  /** DoseWiki's own editors have signed off on this entry. Almost none are. */
  reviewed: boolean;
  score: number;
}
export interface KnowledgeStatus {
  available: boolean;
  chunks: number;
}
/** A substance in the corpus, for browsing by name instead of by query. */
export interface KnowledgeEntry {
  title: string;
  slug: string;
  thin: boolean;
  reviewed: boolean;
  /** Number of passages — the honest measure of how much is actually written. */
  sections: number;
}
export const knowledgeSearch = (query: string, limit?: number) =>
  invoke<KnowledgeHit[]>("knowledge_search", { query, limit });
/** Every passage of one substance's entry, in reading order. */
export const knowledgeEntry = (slug: string) =>
  invoke<KnowledgeHit[]>("knowledge_entry", { slug });
export const knowledgeEntries = () => invoke<KnowledgeEntry[]>("knowledge_entries");
export const knowledgeStatus = () => invoke<KnowledgeStatus>("knowledge_status");

// ---- Upstream contribution drafts (DoseWiki is CC0) ----
// Nothing here touches the network. `contributionSave` writes a file to a path the
// user picked; submitting it upstream is something they do by hand, or not at all.
export interface ContributionCandidate {
  id: number;
  name: string;
  /** DoseWiki already covers it — nothing to contribute. */
  in_dosewiki: boolean;
  /** A draft has already been exported locally. Does not mean anything was sent. */
  contributed: boolean;
}
export interface ContributionDraft {
  name: string;
  slug: string;
  json: string;
  upstream_url: string;
}
export const contributionCandidates = () =>
  invoke<ContributionCandidate[]>("contribution_candidates");
export const contributionDraft = (id: number) =>
  invoke<ContributionDraft>("contribution_draft", { id });
export const contributionSave = (id: number, path: string) =>
  invoke<void>("contribution_save", { id, path });

export const interactionClasses = () => invoke<string[]>("interaction_classes");
export const listSubstances = () => invoke<Substance[]>("list_substances");
export const addSubstance = (input: SubstanceInput) => invoke<Substance>("add_substance", { input });
/** A dose for a timed check: when it was taken, in minutes since the epoch. */
export interface TimedDose {
  substance_name: string;
  route: string;
  at_min: number | null;
}
/** With `doses`, pairs are checked by when they were taken: ones that never
 *  overlapped drop out, ones that met past a peak are softened. */
export const checkCombo = (names: string[], doses?: TimedDose[]) =>
  invoke<Warning[]>("check_combo", doses ? { names, doses } : { names });
export const createExperience = (input: ExperienceInput) =>
  invoke<Experience>("create_experience", { input });
export const listExperiences = () => invoke<ExperienceSummary[]>("list_experiences");
export const getExperience = (id: number) => invoke<ExperienceDetail>("get_experience", { id });
export const endExperience = (id: number, ended_at: string, rating: number | null, notes: string) =>
  invoke<Experience>("end_experience", { id, endedAt: ended_at, rating, notes });
export const logDose = (input: DoseInput) => invoke<LogDoseResult>("log_dose", { input });
export const addTimelineEvent = (input: TimelineInput) =>
  invoke<TimelineEvent>("add_timeline_event", { input });
export const usageBySubstance = () => invoke<SubstanceUsage[]>("usage_by_substance");

// ---- usage stats (roadmap #2; shape mirrors src-tauri/src/stats.rs) ----
export interface StatsRange { min: number | null; max: number | null }
export interface StatsBands {
  route: string;
  threshold: number | null;
  light: StatsRange;
  common: StatsRange;
  strong: StatsRange;
  heavy: number | null;
}
export interface StatsDosePoint {
  dose_id: number;
  experience_id: number;
  taken_at: string;
  /** In the series' unit. */
  amount: number | null;
  route: string;
  /** What was written, when the amount shown differs from it (600 mg in a g
   *  series, "3 g fresh", "× 00 caps, 0.45 g each"). Missing from older servers. */
  logged_amount?: number | null;
  logged_unit?: string | null;
  /** An estimate (fresh mushrooms as dried, an edible's guess): shown as "about". */
  approx?: boolean;
}
export interface StatsUnitSeries {
  unit: string;
  points: StatsDosePoint[];
  without_amount: number;
  bands: StatsBands | null;
}
export interface StatsSubstance {
  key: string;
  name: string;
  sessions: number;
  doses: number;
  last_used: string;
  gaps_days: number[];
  series: StatsUnitSeries[];
  routes: [string, number][];
  /** Drug families it counts toward: psychedelics, entactogens, dissociatives,
   *  stimulants, depressants, opioids, cannabinoids, other (stats.rs). */
  families: string[];
  /** The dose reference's half-life, when it has one. Missing from older servers. */
  half_life?: { text: string; route: string; low_hours: number; high_hours: number } | null;
}
export interface StatsSession {
  experience_id: number;
  title: string;
  started_at: string;
  rating: number | null;
  substances: string[];
}
export interface UsageStats {
  sessions: StatsSession[];
  substances: StatsSubstance[];
  pairs: { a: string; b: string; sessions: number }[];
  total_sessions: number;
  total_doses: number;
  /** "23:00" (local), "varies", "skip" (asked, not answered), or null (not asked
   *  yet). Missing from older servers, which can't store one. */
  bedtime?: string | null;
}
/** Read-only. `since` is an ISO time; omit for all time. */
export const usageStats = (since: string | null) => invoke<UsageStats>("usage_stats", { since });

export interface ChatMsg {
  role: "user" | "assistant" | "system";
  content: string;
}

export interface ParsedDose {
  substance: string;
  amount: number | null;
  unit: string;
  route: string;
  taken_at: string | null;
  note: string;
}
export interface ParsedTimeline {
  at: string | null;
  note: string;
  mood: string;
  intensity: number | null;
}
export interface ParsedExperience {
  title: string;
  started_at: string | null;
  intention: string;
  setting: string;
  notes: string;
  doses: ParsedDose[];
  timeline: ParsedTimeline[];
}
export const parseExperience = (model: string, text: string) =>
  invoke<ParsedExperience>("parse_experience", { model, text });
/** `start` is the start the user confirmed. Pass `parsed` as the model returned it
 *  (its own `started_at` included): the backend decides from that whether the
 *  times need rebasing onto `start`. */
export const importExperience = (parsed: ParsedExperience, start: string | null) =>
  invoke<Experience>("import_experience", { parsed, start });

export interface AiStatus {
  installed: boolean;
  running: boolean;
  models: string[];
}
export const aiStatus = () => invoke<AiStatus>("ai_status");
export const aiRecommendedModels = () => invoke<[string, string][]>("ai_recommended_models");
export const aiInstall = () => invoke<void>("ai_install");
export const aiStart = () => invoke<void>("ai_start");
export const aiPull = (tag: string) => invoke<void>("ai_pull", { tag });
/** Read the desktop's companion-off preference. Exposed over the portal. */
export const companionEnabled = () => invoke<boolean>("companion_enabled");
export const setCompanionEnabled = (enabled: boolean) =>
  invoke<void>("set_companion_enabled", { enabled });
export const aiRemove = (tag: string) => invoke<void>("ai_remove", { tag });
export const aiPreferredModel = () => invoke<string>("ai_preferred_model");
/** Downloads the recommended model, then removes `from`. Resolves to the new tag. */
export const aiSwitchModel = (from: string) => invoke<string>("ai_switch_model", { from });

export const ollamaUp = () => invoke<boolean>("ollama_up");
export const ollamaModels = () => invoke<string[]>("ollama_models");

export interface CompanionReply {
  reply: string;
  actions: string[];
  journal_changed: boolean;
  /** Measured generation speed of this reply, tokens/sec; absent for a tool-only turn. */
  tokens_per_sec?: number | null;
}
export const companionChat = (
  model: string,
  history: ChatMsg[],
  experienceId: number | null,
  supportStyle: string | null,
) => invoke<CompanionReply>("companion_chat", { model, history, experienceId, supportStyle });

/** Pre-load the model into memory so the first message isn't slow. Best-effort —
 * fire it when the Companion comes into view and ignore any error. */
export const companionWarm = (model: string) => invoke<void>("companion_warm", { model });

export type ComputeVerdict = "ample" | "tight" | "insufficient" | "unknown";
export interface LoadedModel {
  resident_gb: number;
  gpu_gb: number;
  cpu_gb: number;
}
export interface ComputeStatus {
  verdict: ComputeVerdict;
  total_ram_gb: number;
  available_ram_gb: number;
  required_gb: number;
  cpu_cores: number;
  arch: string;
  os: string;
  loaded: LoadedModel | null;
  tokens_per_sec: number | null;
  message: string;
}
/** Does this machine have the memory (and, once a reply lands, the speed) to run
 * `model` comfortably? A read-only preflight — advisory, never a gate.
 * `measuredTps` is the last reply's tokens/sec, or null before the first message. */
export const computeStatus = (model: string, measuredTps: number | null = null) =>
  invoke<ComputeStatus>("compute_status", { model, measuredTps });

// ---- Companion as a background job (PHONE-PORTAL-ONLY) ----
// A slow local model can take minutes per turn; mobile Safari kills a silent
// request at ~60s, and a locked screen kills it instantly. So the phone starts
// the turn as a job on the desktop and polls for the result with fast, cheap
// requests. These two commands exist only in portal.rs's dispatch — they are
// NOT Tauri commands, so calling them through desktop IPC will fail. On the
// desktop, keep using companionChat.
export const companionChatStart = (
  model: string,
  history: ChatMsg[],
  experienceId: number | null,
  supportStyle: string | null,
) => invoke<{ job: number }>("companion_chat_start", { model, history, experienceId, supportStyle });

export type CompanionPoll =
  | { status: "running" }
  | { status: "done"; reply: CompanionReply }
  | { status: "error"; error: string };
export const companionChatPoll = (id: number) =>
  invoke<CompanionPoll>("companion_chat_poll", { id });

// ---- deterministic crisis escalation ----
export type CrisisLevel = "none" | "peer" | "psychiatric" | "medical";
export interface CrisisResource {
  label: string;
  contact: string;
  detail: string;
}
/**
 * Whether to ask before showing resources. `offer` is used for ordinary hard
 * moments, where flashing hotlines pathologises a difficult experience; `direct`
 * for psychiatric and medical signals, where asking someone to triage themselves
 * is the wrong thing to ask of them.
 */
export type CrisisPresentation = "offer" | "direct";
export interface CrisisResult {
  level: CrisisLevel;
  headline: string;
  matched: string[];
  resources: CrisisResource[];
  presentation: CrisisPresentation;
}
/**
 * `recent` is the person's earlier messages this conversation, oldest first.
 * Expressive distress ("I want it to stop") is judged on repetition, so without
 * it a hard moment named once is correctly treated as just that.
 */
export const crisisScan = (text: string, experienceId: number | null, recent: string[] = []) =>
  invoke<CrisisResult>("crisis_scan", { text, experienceId, recent });
export const emergencyResources = () => invoke<CrisisResource[]>("emergency_resources");

// ---- encryption at rest & backups ----
export interface DbStatus {
  /** Is the journal file on disk encrypted (SQLCipher)? */
  encrypted: boolean;
  /** Is there a live, usable connection this session? */
  unlocked: boolean;
  /** Still opening at launch; ask again shortly. */
  opening?: boolean;
  /** Opening failed, and why. */
  error?: string | null;
  // Only on another person's device (src-tauri/src/people.rs); absent for the owner.
  /** Whose journal this device opens. Present means "not the owner". */
  person_name?: string;
  /** They haven't chosen a password yet, so there's no journal: ask for one. */
  new_journal?: boolean;
  /** Their password is kept on the server so the journal reopens after a restart. */
  remembered?: boolean;
}
export const dbStatus = () => invoke<DbStatus>("db_status");
export const unlockDb = (passphrase: string) => invoke<void>("unlock_db", { passphrase });
export const enableEncryption = (passphrase: string) => invoke<void>("enable_encryption", { passphrase });
export const disableEncryption = (passphrase: string) => invoke<void>("disable_encryption", { passphrase });
export const changePassphrase = (current: string, newPassphrase: string) =>
  invoke<void>("change_passphrase", { current, newPassphrase });
export const exportBackup = (path: string, password: string | null = null) =>
  invoke<void>("export_backup", { path, password });
export const importBackup = (path: string) => invoke<void>("import_backup", { path });

// ---- Obsidian vault sync ----
export interface ObsidianExportResult {
  written: number;
}
export interface ObsidianImportResult {
  created: number;
  updated: number;
  skipped: number;
}
export const obsidianExport = (folder: string) =>
  invoke<ObsidianExportResult>("obsidian_export", { folder });
export const obsidianImport = (folder: string) =>
  invoke<ObsidianImportResult>("obsidian_import", { folder });

// ---- single-entry Markdown export ----
// Same rendering + filename convention as the Obsidian vault export.
export interface ExportedNote {
  filename: string;
  markdown: string;
}
/** Pure render — returns the Markdown text. Portal-safe; the phone downloads it. */
export const exportExperienceMarkdown = (id: number) =>
  invoke<ExportedNote>("export_experience_markdown", { id });
/** Desktop-only: writes the note to `dest`, a path from the save dialog. */
export const exportExperienceFile = (id: number, dest: string) =>
  invoke<void>("export_experience_file", { id, dest });

// ---- phone portal (optional; off by default) ----
// Desktop-only by construction: portal.rs does not allowlist these, so the portal
// cannot be used to reconfigure or switch off the portal.
export interface PortalStatus {
  running: boolean;
  port: number | null;
  /** A paired device has made a request since the portal was turned on. Not a
   *  live connection check — it stays true after the device walks away. */
  paired: boolean;
}
export interface DeviceInfo {
  id: number;
  name: string;
  /** Unix seconds. */
  created_at: number;
  last_seen: number | null;
  /** Whose device: 1 is the owner, anyone else is listed in People. */
  person: number;
}
export interface PairResult {
  device: DeviceInfo;
  /** Shown once — as a QR code or a link — and never again. */
  token: string;
}
export interface ServerPrefs {
  serve_on_launch: boolean;
  served_https: number | null;
  /** Published through the Tailscale built into Field Notes. */
  builtin_tailnet: boolean;
  /** Use the Tailscale app on this computer instead of the built-in one. */
  tailscale_app: boolean;
  /** Paired phones may install updates on this computer. Set only at the computer. */
  phone_can_update: boolean;
  /** Discreet mode is offered (its toggle sits next to Help). */
  discreet_available: boolean;
  /** Runs from the menu bar / system tray instead of the Dock / taskbar. */
  menu_bar: boolean;
}
export interface KeychainStatus {
  /** The journal is encrypted, so there's a password to remember at all. */
  applicable: boolean;
  remembered: boolean;
}
export interface RemoteFailed {
  seq: number;
  cmd: string;
  error: string;
}
export interface RemoteStatus {
  connected: boolean;
  server: string | null;
  server_name: string | null;
  online: boolean;
  pending: number;
  failed: RemoteFailed[];
  unpaired: boolean;
  /** Entries in this computer's own journal not yet copied to the server. */
  local_unsynced: number;
}
export interface UploadResult {
  copied: number;
  /** Not sent: the server already had an identical entry. */
  skipped: number;
  /** Duplicate entries removed from the server's journal afterwards. */
  removed: number;
  substances: number;
  status: RemoteStatus;
}
export interface TailscaleStatus {
  installed: boolean;
  host: string | null;
  serving: boolean;
  url: string | null;
  /** HTTPS port we're published on — not always 443, another service may hold it. */
  https_port: number | null;
  serve_command: string | null;
  /** Tailscale is running and signed in on this computer. */
  signed_in: boolean;
  /** HTTPS certificates are enabled for the tailnet, which publishing needs. */
  https_enabled: boolean;
  /** Reached through the Tailscale built into Field Notes, not the Tailscale app.
   *  `installed` then means "this build includes it". */
  builtin: boolean;
  /** The Tailscale app is installed on this computer, so it can be offered. */
  app_available: boolean;
  /** The account this computer is signed in as (built-in only). */
  login: string | null;
  /** Tailscale's sign-in page, while it's waiting for one (built-in only). */
  auth_url: string | null;
  /** The built-in Tailscale's last error, in its own words. */
  problem: string | null;
}
export const portalStatus = () => invoke<PortalStatus>("portal_status");
export const portalEnable = () => invoke<PortalStatus>("portal_enable");
export const portalDisable = () => invoke<PortalStatus>("portal_disable");
export const portalQr = (url: string) => invoke<string>("portal_qr", { url });
export const portalTailscale = () => invoke<TailscaleStatus>("portal_tailscale");
export const portalServe = () => invoke<TailscaleStatus>("portal_serve");
export const portalUnserve = () => invoke<TailscaleStatus>("portal_unserve");
/** Turn on device access and start the built-in Tailscale. */
export const tailnetConnect = () => invoke<TailscaleStatus>("tailnet_connect");
/** Sign this computer out of the built-in Tailscale and forget its keys. */
export const tailnetSignOut = () => invoke<TailscaleStatus>("tailnet_sign_out");
/** Use the Tailscale app on this computer (true) or the built-in one (false). */
export const tailnetUseApp = (useApp: boolean) => invoke<TailscaleStatus>("tailnet_use_app", { useApp });
/** Pair a device. With `person`, it opens that person's journal instead of yours. */
export const portalPair = (name: string, person: number | null = null) =>
  invoke<PairResult>("portal_pair", { name, person });
export const portalDevices = () => invoke<DeviceInfo[]>("portal_devices");
export const portalRevoke = (id: number) => invoke<DeviceInfo[]>("portal_revoke", { id });

// ---- other people on this server (see src-tauri/src/people.rs) ----
/** What the owner may see about someone else: never anything from their journal. */
export interface PersonInfo {
  id: number;
  name: string;
  created_at: number;
  /** They've chosen a password, so their journal exists. */
  has_journal: boolean;
  unlocked: boolean;
  remembered: boolean;
}
/** Desktop only. */
export const peopleList = () => invoke<PersonInfo[]>("people_list");
/** Desktop only. */
export const personAdd = (name: string) => invoke<PersonInfo>("person_add", { name });
/** Desktop only. `confirm` must be their name, typed. Deletes their journal. */
export const personRemove = (id: number, confirm: string) =>
  invoke<PersonInfo[]>("person_remove", { id, confirm });

// From another person's own device. None of these take a person: the server
// knows whose device is asking.
export interface MyDevice extends DeviceInfo {
  /** The device making the request. */
  this: boolean;
}
/** Unlock, or the first time create, your journal with your password. */
export const personUnlock = (password: string) => invoke<DbStatus>("person_unlock", { password });
/** Keep your password on the server (needs it), or stop keeping it. */
export const personRemember = (remember: boolean, password: string | null = null) =>
  invoke<DbStatus>("person_remember", { remember, password });
export const myDevices = () => invoke<MyDevice[]>("my_devices");
/** Pair another device of yours. `origin` is where this phone reached the server,
 *  so the returned QR code points at the same place. */
export const pairOwnDevice = (name: string, origin: string, password: string | null = null) =>
  invoke<PairResult & { qr: string | null }>("pair_own_device", { name, origin, password });
export const unpairMyDevice = (id: number, password: string | null = null) =>
  invoke<MyDevice[]>("unpair_my_device", { id, password });
/** On the owner's phone: what pairing or un-pairing asks for (owner_auth.rs). */
export const ownerDeviceAuth = () => invoke<"password" | "pin" | "off">("owner_device_auth");
export interface PhonePinStatus {
  method: "password" | "pin" | "off";
  has_pin: boolean;
}
/** At the computer: how the owner's phone proves it's them, and the PIN behind it. */
export const phonePinStatus = () => invoke<PhonePinStatus>("phone_pin_status");
export const setPhonePin = (pin: string | null) => invoke<PhonePinStatus>("set_phone_pin", { pin });
/** Re-encrypt your journal under a new password. Needs the current one. */
export const personChangePassword = (current: string, newPassword: string) =>
  invoke<DbStatus>("person_change_password", { current, new: newPassword });
/** Your journal as a file (base64), still encrypted with your password. */
export const exportMyJournal = () => invoke<{ data: string }>("export_my_journal");

// ---- this computer as the server ----
export const serverPrefs = () => invoke<ServerPrefs>("server_prefs");
export const setServerPrefs = (serveOnLaunch: boolean) =>
  invoke<ServerPrefs>("set_server_prefs", { serveOnLaunch });
/** Is discreet mode offered? Readable from the phone. */
export const discreetAvailable = () => invoke<boolean>("discreet_available");
/** Offer discreet mode. Settings on the computer only; not on the portal allowlist. */
export const setDiscreetAvailable = (available: boolean) =>
  invoke<ServerPrefs>("set_discreet_available", { available });
/** Desktop only: run from the menu bar / system tray. Not on the portal allowlist. */
export const setMenuBar = (on: boolean) => invoke<ServerPrefs>("set_menu_bar", { on });
/** Desktop only: let paired phones install updates here. Not on the portal allowlist. */
export const setPhoneCanUpdate = (allowed: boolean) => invoke<ServerPrefs>("set_phone_can_update", { allowed });

// ---- updating the server from a phone (see src-tauri/src/server_update.rs) ----
export interface ServerUpdateStatus {
  current: string;
  available: { version: string; notes: string } | null;
  /** Why this phone can't install it now; null means it can. */
  blocked: string | null;
  /** `blocked` only locks other people's journals: the owner may install anyway. */
  can_override: boolean;
  installing: boolean;
  /** A check is running on the server; ask again shortly. */
  checking: boolean;
  error: string | null;
}
export const serverUpdateStatus = () => invoke<ServerUpdateStatus>("server_update_status");
export const serverUpdateInstall = (anyway = false) => invoke<ServerUpdateStatus>("server_update_install", { anyway });
export const keychainStatus = () => invoke<KeychainStatus>("keychain_status");
export const keychainRemember = (passphrase: string) =>
  invoke<KeychainStatus>("keychain_remember", { passphrase });
export const keychainForget = () => invoke<KeychainStatus>("keychain_forget");

// ---- another computer as the server ----
export const remoteStatus = () => invoke<RemoteStatus>("remote_status");
export const remoteConnect = (link: string) => invoke<RemoteStatus>("remote_connect", { link });
export const remoteDisconnect = (discard: boolean) => invoke<RemoteStatus>("remote_disconnect", { discard });
export const remoteDiscard = (seq: number) => invoke<RemoteStatus>("remote_discard", { seq });
export const remoteFlush = () => invoke<RemoteStatus>("remote_flush");
export const remoteUploadLocal = () => invoke<UploadResult>("remote_upload_local");
export const saveMarkdownFile = (dest: string, markdown: string) =>
  invoke<void>("save_markdown_file", { dest, markdown });
/** Desktop only: write a PDF (from `pdf.ts`) to a path picked in a save dialog. */
export const savePdfFile = (dest: string, data: Uint8Array) => invoke<void>("save_pdf_file", { dest, data: Array.from(data) });

// ---- erase all data / uninstall ----
export const dataDir = () => invoke<string>("data_dir");
export const revealDataDir = () => invoke<void>("reveal_data_dir");
export const wipeAllData = () => invoke<void>("wipe_all_data");
