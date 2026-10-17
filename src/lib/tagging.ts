// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// Tagging someone in a dose you've already logged, from Edit dose
// (DoseTagPicker.svelte, on the phone and the computer). Kept apart from the
// component so `npm test` reaches it.

export type TagState = "none" | "waiting" | "accepted" | "declined";

type Sent = { to: number; dose_id: number; status: "waiting" | "accepted" | "declined" };

/** What became of the newest tag of `doseId` to `person` (`sent` is newest first). */
export function tagState(sent: Sent[], doseId: number, person: number): TagState {
  return sent.find((s) => s.dose_id === doseId && s.to === person)?.status ?? "none";
}

/** Who gets the dose when Edit dose is saved: everyone newly picked, and everyone
 *  whose tag is still waiting, so they get the edited version instead of the old
 *  one. Never someone who already added it. */
export function sendOnSave(sent: Sent[], doseId: number, partners: number[], picked: number[]): number[] {
  return partners.filter((p) => {
    const s = tagState(sent, doseId, p);
    return s === "waiting" || (s !== "accepted" && picked.includes(p));
  });
}
