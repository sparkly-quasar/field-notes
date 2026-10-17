<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  Tag someone in a dose you've already logged, from Edit dose, on the phone and
  the computer (tagging.rs). Picking someone only marks them: the page sends the
  dose when it's saved (`sendOnSave` in tagging.ts), so they get it with your
  edits. Someone already tagged shows what became of it.
-->
<script lang="ts">
  import type { SentTag, TagPerson } from "$lib/api";
  import { tagState } from "$lib/tagging";

  let {
    doseId,
    partners,
    sent,
    picked = $bindable([]),
    phone = false,
  }: {
    doseId: number;
    /** People who can be tagged now: both have allowed it. */
    partners: TagPerson[];
    /** Tags you sent, newest first. */
    sent: SentTag[];
    picked?: number[];
    phone?: boolean;
  } = $props();

  function toggle(id: number) {
    picked = picked.includes(id) ? picked.filter((p) => p !== id) : [...picked, id];
  }
  const waiting = $derived(partners.some((p) => tagState(sent, doseId, p.id) === "waiting"));
</script>

{#if partners.length}
  <div class="tagpick" class:phone>
    <p class="label">Tag someone in this dose</p>
    <div class="chips" role="group" aria-label="Tag someone in this dose">
      {#each partners as p (p.id)}
        {@const state = tagState(sent, doseId, p.id)}
        {#if state === "accepted"}
          <span class="chip done">{p.name} added it</span>
        {:else if state === "waiting"}
          <span class="chip on">Tagged {p.name}</span>
        {:else}
          <button type="button" class="chip" class:on={picked.includes(p.id)} aria-pressed={picked.includes(p.id)} onclick={() => toggle(p.id)}>
            {picked.includes(p.id) ? "✓ " : ""}{state === "declined" ? `Tag ${p.name} again` : `Tag ${p.name}`}
          </button>
        {/if}
      {/each}
    </div>
    <p class="hint">
      {#if picked.length || waiting}Sent when you save, with your changes. Only the dose goes, never the rest of the entry.{:else}They get a copy of this dose to add to their own journal, or decline.{/if}
    </p>
  </div>
{/if}

<style>
  .tagpick { margin: 0.5rem 0; }
  .label { margin: 0 0 0.3rem; font-size: 0.85rem; color: var(--muted, var(--text-2)); }
  .chips { display: flex; flex-wrap: wrap; gap: 0.4rem; }
  .chip {
    width: auto; margin: 0; font: inherit; font-size: 0.8rem; border: 1px solid var(--line, var(--divider, currentColor));
    border-radius: 999px; padding: 0.2rem 0.7rem; background: transparent; color: var(--muted, var(--text-2)); cursor: pointer;
  }
  .chip.on { background: color-mix(in srgb, var(--accent) 22%, transparent); color: var(--accent); border-color: transparent; font-weight: 600; }
  .chip.done { cursor: default; }
  span.chip { cursor: default; }
  .phone .chip { min-height: var(--tap-min, 2.4rem); display: inline-flex; align-items: center; padding: 0 0.9rem; font-size: var(--fs-sm, 0.9rem); }
  .hint { margin: 0.3rem 0 0; font-size: 0.8rem; color: var(--muted, var(--text-2)); }
</style>
