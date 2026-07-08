// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// Typed wrappers around the Tauri command surface (see src-tauri/src/commands.rs).

import { invoke } from "@tauri-apps/api/core";

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

// ---- PsychonautWiki reference cache ----
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
  total: string | null;
}
export interface PwInfo {
  name: string;
  common_names: string[];
  psychoactive: string[];
  chemical: string[];
  roas: PwRoa[];
  interactions: string[];
}
export interface PwStatus {
  count: number;
  last_fetched: string | null;
}
export const pwUpdate = () => invoke<number>("pw_update");
export const pwStatus = () => invoke<PwStatus>("pw_status");
export const pwLookup = (name: string) => invoke<PwInfo | null>("pw_lookup", { name });

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

export const ollamaUp = () => invoke<boolean>("ollama_up");
export const ollamaModels = () => invoke<string[]>("ollama_models");
export const companionChat = (model: string, history: ChatMsg[], experienceId: number | null) =>
  invoke<string>("companion_chat", { model, history, experienceId });
