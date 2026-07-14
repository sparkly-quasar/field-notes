// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// Typed wrappers around the Tauri command surface (see src-tauri/src/commands.rs).

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { inTauri, portalInvoke } from "./portal";

/**
 * The one seam between this app and its backend.
 *
 * On the desktop it's Tauri's IPC. On a phone reaching the same frontend through the
 * portal there is no IPC, so it's an HTTP call to the desktop instead. Everything
 * below this line — and the whole UI above it — is written once and doesn't care
 * which. Keep it that way: this is the only file that may import `invoke`.
 */
function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return inTauri() ? tauriInvoke<T>(cmd, args) : portalInvoke<T>(cmd, args);
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
  title: string;
  intention: string;
  setting: string;
  notes: string;
  rating: number | null;
  started_at: string;
  ended_at: string | null;
  created_at: string;
}

export type ExperienceSummary = Experience & {
  substances: string[];
  dose_count: number;
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
  title?: string;
  intention?: string;
  setting?: string;
  started_at: string;
}

export interface DoseInput {
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

export interface DoseUpdate {
  substance_name: string;
  amount: number | null;
  unit?: string;
  route?: string;
  taken_at: string;
  note?: string;
}

export const updateExperience = (id: number, update: ExperienceUpdate) =>
  invoke<Experience>("update_experience", { id, update });
export const updateDose = (id: number, update: DoseUpdate) => invoke<Dose>("update_dose", { id, update });
export const deleteExperience = (id: number) => invoke<void>("delete_experience", { id });
export const deleteDose = (id: number) => invoke<void>("delete_dose", { id });
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
}
export interface PwStatus {
  count: number;
  snapshot: string;
}
export const pwUpdate = () => invoke<number>("pw_update");
export const pwStatus = () => invoke<PwStatus>("pw_status");
export const pwLookup = (name: string) => invoke<PwInfo | null>("pw_lookup", { name });

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
export const knowledgeSearch = (query: string, limit?: number) =>
  invoke<KnowledgeHit[]>("knowledge_search", { query, limit });
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
export const checkCombo = (names: string[]) => invoke<Warning[]>("check_combo", { names });
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
export const importExperience = (parsed: ParsedExperience) =>
  invoke<Experience>("import_experience", { parsed });

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

export const ollamaUp = () => invoke<boolean>("ollama_up");
export const ollamaModels = () => invoke<string[]>("ollama_models");

export interface CompanionReply {
  reply: string;
  actions: string[];
  journal_changed: boolean;
}
export const companionChat = (
  model: string,
  history: ChatMsg[],
  experienceId: number | null,
  supportStyle: string | null,
) => invoke<CompanionReply>("companion_chat", { model, history, experienceId, supportStyle });

// ---- deterministic crisis escalation ----
export type CrisisLevel = "none" | "peer" | "psychiatric" | "medical";
export interface CrisisResource {
  label: string;
  contact: string;
  detail: string;
}
export interface CrisisResult {
  level: CrisisLevel;
  headline: string;
  matched: string[];
  resources: CrisisResource[];
}
export const crisisScan = (text: string, experienceId: number | null) =>
  invoke<CrisisResult>("crisis_scan", { text, experienceId });
export const emergencyResources = () => invoke<CrisisResource[]>("emergency_resources");

// ---- encryption at rest & backups ----
export interface DbStatus {
  /** Is the journal file on disk encrypted (SQLCipher)? */
  encrypted: boolean;
  /** Is there a live, usable connection this session? */
  unlocked: boolean;
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

// ---- phone portal (optional; off by default) ----
// Desktop-only by construction: portal.rs does not allowlist these, so the portal
// cannot be used to reconfigure or switch off the portal.
export interface PortalStatus {
  running: boolean;
  port: number | null;
  /** Contains the bearer token. Only ever rendered on the desktop's own screen. */
  pair_url: string | null;
}
export interface TailscaleStatus {
  installed: boolean;
  host: string | null;
  serving: boolean;
  url: string | null;
  serve_command: string | null;
}
export const portalStatus = () => invoke<PortalStatus>("portal_status");
export const portalEnable = () => invoke<PortalStatus>("portal_enable");
export const portalDisable = () => invoke<PortalStatus>("portal_disable");
export const portalQr = (url?: string) => invoke<string>("portal_qr", { url });
export const portalTailscale = () => invoke<TailscaleStatus>("portal_tailscale");
export const portalServe = () => invoke<TailscaleStatus>("portal_serve");
export const portalUnserve = () => invoke<TailscaleStatus>("portal_unserve");

// ---- erase all data / uninstall ----
export const dataDir = () => invoke<string>("data_dir");
export const revealDataDir = () => invoke<void>("reveal_data_dir");
export const wipeAllData = () => invoke<void>("wipe_all_data");
