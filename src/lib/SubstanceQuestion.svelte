<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  Right after a dose of something Field Notes knows little about (unknowns.rs):
  what kind of drug is it, and how long does it last? Sits under the receipt
  beside KindQuestion. Asked once per substance: any answer, or Skip, ends it.
  The backend decides; usually there's nothing to ask and this shows nothing.
-->
<script lang="ts">
  import { setSubstanceClasses, substanceQuestion, substanceQuestionDone, type SubstanceQuestion } from "$lib/api";
  import ClassPicker, { classLabel } from "$lib/ClassPicker.svelte";
  import MyTimings from "$lib/MyTimings.svelte";

  let { substance, tick = 0, label = substance }: { substance: string; tick?: number; label?: string } = $props();

  let q = $state<SubstanceQuestion | null>(null);
  /** class: what kind of drug; offer: ask about timings; timings: the form. */
  let step = $state<"class" | "offer" | "timings" | "">("");
  let classes = $state<string[]>([]);
  let said = $state("");
  let busy = $state(false);

  $effect(() => {
    const s = substance.trim();
    void tick;
    q = null;
    step = "";
    said = "";
    classes = [];
    if (!s) return;
    substanceQuestion(s)
      .then((r) => {
        if (substance.trim() !== s || !r) return;
        q = r;
        step = r.ask_class ? "class" : "offer";
      })
      .catch(() => {});
  });

  /** Remembered on the first answer or skip, so it's never asked twice. */
  function asked() {
    if (q) substanceQuestionDone(q.substance).catch(() => {});
  }

  function next(msg: string) {
    said = msg;
    step = step === "class" && q?.ask_timings ? "offer" : "";
  }

  async function saveClass() {
    if (!q || !classes.length || busy) return;
    busy = true;
    try {
      await setSubstanceClasses(q.substance, classes);
      asked();
      next(`Saved: ${classes.map(classLabel).join(", ")}. Stats and combination checks use it now.`);
    } catch {
      said = "Couldn't save that. It'll be asked again after a later dose.";
      step = "";
    } finally {
      busy = false;
    }
  }

  function skip() {
    asked();
    if (step === "class" && q?.ask_timings) {
      said = "";
      step = "offer";
    } else {
      said = "Skipped. You can add it any time under Check, in Your substances.";
      step = "";
    }
  }
</script>

{#if q && step === "class"}
  <div class="sq" role="group" aria-label="What kind of drug {label} is">
    <p><strong>Field Notes doesn't know {label} yet.</strong> What kind of drug is it? Stats files it under that class, and combination checks can warn about it.</p>
    <ClassPicker bind:value={classes} idPrefix="sq-{tick}" optional={false}
      hint="Pick more than one if it fits (MDMA: entactogen and stimulant). Not sure, or none of these? Skip it." />
    <div class="sq-row">
      <button type="button" class="sq-main" disabled={busy || !classes.length} onclick={saveClass}>Save</button>
      <button type="button" disabled={busy} onclick={skip}>Skip</button>
    </div>
  </div>
{:else if q && step === "offer"}
  <div class="sq" role="group" aria-label="How long {label} lasts">
    {#if said}<p class="sq-said">{said}</p>{/if}
    <p>Nothing gives timings for {label} yet. Add how long it lasts and Today draws its curve.</p>
    <div class="sq-row">
      <button type="button" class="sq-main" onclick={() => { asked(); step = "timings"; }}>Add timings</button>
      <button type="button" onclick={skip}>Skip</button>
    </div>
  </div>
{:else if q && step === "timings"}
  <div class="sq">
    <MyTimings name={q.substance} {label} cancelLabel="Skip"
      onsaved={() => { said = `Saved. Today draws ${label}'s curve from your timings.`; step = ""; }}
      oncancel={() => { said = "Skipped. You can add timings any time from its line on Today."; step = ""; }} />
  </div>
{:else if said}
  <p class="sq-said">{said}</p>
{/if}

<style>
  .sq {
    margin: 0.6rem 0; padding: 0.7rem 0.8rem; border-radius: var(--radius, 10px); display: grid; gap: 0.55rem;
    border: 1px solid var(--line, var(--divider, currentColor));
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }
  .sq p { margin: 0; font-size: var(--fs-sm, 0.92rem); line-height: 1.45; }
  .sq-row { display: flex; flex-wrap: wrap; gap: 0.4rem; }
  .sq-row button {
    width: auto; margin: 0; min-height: var(--tap-min, 2rem); padding: 0.3rem 0.9rem; border-radius: 999px;
    border: 1px solid var(--accent); background: transparent; color: var(--accent);
    font: inherit; font-size: var(--fs-sm, 0.9rem); cursor: pointer;
  }
  .sq-row .sq-main { background: var(--accent); color: var(--on-accent, var(--accent-ink, #fff)); }
  .sq-row button:disabled { opacity: 0.5; cursor: default; }
  .sq-said { margin: 0.5rem 0; font-size: var(--fs-sm, 0.88rem); color: var(--muted, var(--text-2)); }
</style>
