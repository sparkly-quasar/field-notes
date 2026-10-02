<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  The discreet-mode switch that sits next to Help. It only appears once discreet
  mode is enabled in Settings on the computer that holds the journal; then it
  turns name-hiding on and off for this device.
-->
<script lang="ts">
  import { discreet, setDiscreet } from "$lib/discreet.svelte";
</script>

{#if discreet.available}
  <button
    type="button"
    class="discreet-eye"
    class:on={discreet.on}
    aria-pressed={discreet.on}
    aria-label={discreet.on ? "Discreet mode is on. Show names" : "Discreet mode: hide substance names and titles"}
    title={discreet.on ? "Discreet mode is on" : "Discreet mode"}
    onclick={() => setDiscreet(!discreet.on)}
  >
    <svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      {#if discreet.on}
        <path d="M3 3l18 18M10.6 10.6a2 2 0 0 0 2.8 2.8M9.9 5.1A9.8 9.8 0 0 1 12 5c6 0 9.5 7 9.5 7a17 17 0 0 1-3 3.8M6.2 6.2A17 17 0 0 0 2.5 12s3.5 7 9.5 7a9.6 9.6 0 0 0 4.2-.9" />
      {:else}
        <path d="M2.5 12s3.5-7 9.5-7 9.5 7 9.5 7-3.5 7-9.5 7-9.5-7-9.5-7z" /><circle cx="12" cy="12" r="3" />
      {/if}
    </svg>
  </button>
{/if}

<style>
  /* Works on either page's tokens (phone: --text-2/--surface-2, desktop: --muted/--card). */
  .discreet-eye {
    width: 44px; min-width: 44px; height: 44px; min-height: 44px; margin: 0; padding: 0;
    display: inline-grid; place-items: center; border-radius: 999px; border: 0; cursor: pointer;
    background: transparent; color: var(--text-2, var(--muted));
  }
  .discreet-eye.on { color: var(--accent); background: var(--surface-2, var(--line)); }
  .discreet-eye:focus-visible { outline: 3px solid var(--focus, var(--accent)); outline-offset: 2px; }
</style>
