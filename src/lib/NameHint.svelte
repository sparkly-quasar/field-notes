<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  Under a substance field: when what's typed is a street name for one substance
  ("acid"), say what it will be saved as ("LSD") and offer to keep the wording.
  `saveAs` is the name to save. The matching rule is `names.rs` (conservative:
  whole names only, never a one- or two-letter street name, and the person's own
  catalogue first). Offline on a phone it says nothing and keeps the wording;
  Stats groups both spellings either way.
-->
<script lang="ts">
  import { canonicalName } from "$lib/api";

  let { name, saveAs = $bindable("") }: { name: string; saveAs?: string } = $props();

  let canon = $state<string | null>(null);
  let keep = $state(false);

  $effect(() => {
    const typed = name.trim();
    keep = false;
    // Never save under an answer for what was typed before.
    canon = null;
    if (!typed) return;
    const t = setTimeout(async () => {
      const answer = await canonicalName(typed).catch(() => null);
      // Only if it's still what's in the field.
      if (name.trim() === typed) canon = answer;
    }, 250);
    return () => clearTimeout(t);
  });

  const typed = $derived(name.trim());
  // A different name, not just different capitals: worth saying out loud.
  const renames = $derived(!!canon && canon.toLowerCase() !== typed.toLowerCase());

  $effect(() => {
    saveAs = !canon || (renames && keep) ? typed : canon;
  });
</script>

{#if renames}
  <p class="name-hint">
    {#if keep}
      Saved as “{typed}”, as written. <button type="button" class="link" onclick={() => (keep = false)}>Save as {canon}</button>
    {:else}
      “{typed}” is {canon}, so it's saved as {canon}. <button type="button" class="link" onclick={() => (keep = true)}>Keep “{typed}”</button>
    {/if}
  </p>
{/if}

<style>
  .name-hint { margin: 0.25rem 0 0.5rem; font-size: var(--fs-sm, 0.9rem); color: var(--text-2, var(--muted)); }
  .link {
    display: inline; width: auto; min-height: 0; margin: 0 0 0 0.25rem; padding: 0; border: 0; background: none;
    color: var(--accent); font: inherit; text-decoration: underline; cursor: pointer;
  }
</style>
