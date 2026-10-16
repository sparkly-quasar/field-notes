<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  What kind of drug a substance is, picked when it's added to the catalogue.
  Writes the combination checker's own classes (interactions.rs CLASSES), which
  are also what Stats sorts into drug classes (stats.rs family_of_class), under
  the names people know them by. The ones that only matter for combinations
  (MAOI, SSRI, lithium...) sit apart, so the main choice stays short.
-->
<script module lang="ts">
  /** The drug classes, in the order Stats shows them. */
  export const KIND_CLASSES: [string, string][] = [
    ["psychedelic", "Psychedelic"],
    ["entactogen", "Entactogen"],
    ["dissociative", "Dissociative"],
    ["stimulant", "Stimulant"],
    ["depressant", "Depressant"],
    ["benzodiazepine", "Benzodiazepine"],
    ["opioid", "Opioid"],
    ["cannabinoid", "Cannabinoid"],
    ["deliriant", "Deliriant"],
  ];
  /** Classes that matter for combinations only. */
  export const COMBO_CLASSES: [string, string][] = [
    ["serotonin_releaser", "Releases serotonin (like MDMA)"],
    ["serotonergic", "Serotonergic"],
    ["maoi", "MAOI"],
    ["ssri", "SSRI"],
    ["lithium", "Lithium"],
  ];
  const LABEL = new Map([...KIND_CLASSES, ...COMBO_CLASSES]);
  /** A class as people read it: "serotonin_releaser" is "Releases serotonin (like MDMA)". */
  export const classLabel = (c: string) => LABEL.get(c) ?? c.replace(/_/g, " ");
</script>

<script lang="ts">
  let {
    value = $bindable([]),
    idPrefix = "cls",
    optional = true,
    hint = "Pick more than one if it fits (MDMA: entactogen and stimulant). Leave it empty and Field Notes classes well-known names itself.",
  }: { value?: string[]; idPrefix?: string; optional?: boolean; hint?: string } = $props();

  const toggle = (c: string) => (value = value.includes(c) ? value.filter((x) => x !== c) : [...value, c]);
  let more = $state(false);
  // Stays open while any of them is picked, so a choice is never hidden.
  const open = $derived(more || COMBO_CLASSES.some(([c]) => value.includes(c)));
</script>

<fieldset class="cp">
  <legend>Drug class{#if optional} <span class="opt">(optional)</span>{/if}</legend>
  <div class="row" role="group" aria-label="Drug class">
    {#each KIND_CLASSES as [c, l]}
      <button type="button" id="{idPrefix}-{c}" class:on={value.includes(c)} aria-pressed={value.includes(c)} onclick={() => toggle(c)}>{l}</button>
    {/each}
  </div>
  {#if !open || more}
    <button type="button" class="link" aria-expanded={open} onclick={() => (more = !more)}>
      {open ? "Hide" : "Show"} classes for combination checks
    </button>
  {/if}
  {#if open}
    <div class="row" role="group" aria-label="For combination checks">
      {#each COMBO_CLASSES as [c, l]}
        <button type="button" id="{idPrefix}-{c}" class:on={value.includes(c)} aria-pressed={value.includes(c)} onclick={() => toggle(c)}>{l}</button>
      {/each}
    </div>
  {/if}
  <p class="hint">{hint}</p>
</fieldset>

<style>
  .cp {
    --cp-line: var(--divider, var(--line, #322c25));
    --cp-text: var(--text, var(--ink, #eee7dc));
    --cp-muted: var(--text-2, var(--muted, #b8ad9c));
    --cp-accent: var(--accent, #b9a3c9);
    --cp-surface: var(--surface, var(--card, transparent));
    border: 0;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 0.45rem;
    min-width: 0;
  }
  legend { padding: 0; margin-bottom: 0.35rem; font-weight: 600; color: var(--cp-text); }
  .opt, .hint { color: var(--cp-muted); font-weight: 400; }
  .hint { font-size: 0.82rem; margin: 0; line-height: 1.4; }
  .row { display: flex; flex-wrap: wrap; gap: 0.4rem; }
  .row button {
    font: inherit; font-size: 0.88rem; min-height: 36px; padding: 0 0.8rem; border-radius: 999px; cursor: pointer;
    border: 1px solid var(--cp-line); background: var(--cp-surface); color: var(--cp-text);
  }
  .row button.on { border-color: var(--cp-accent); box-shadow: inset 0 0 0 1px var(--cp-accent); font-weight: 600; }
  .link { justify-self: start; background: none; border: 0; padding: 0.2rem 0; color: var(--cp-accent); font: inherit; font-size: 0.86rem; font-weight: 600; cursor: pointer; }
  button:focus-visible { outline: 2px solid var(--focus, var(--cp-accent)); outline-offset: 2px; }
</style>
