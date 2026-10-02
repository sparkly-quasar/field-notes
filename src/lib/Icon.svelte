<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  A small set of line icons (1.75px stroke, currentColor), drawn for Field Notes.
  Always paired with a visible text label: an icon alone fails someone who's
  altered. Decorative, so hidden from screen readers.
-->
<script lang="ts">
  type Name = "today" | "journal" | "stats" | "check" | "talk" | "settings" | "help";
  let { name, size = 20 }: { name: Name; size?: number } = $props();
</script>

<svg viewBox="0 0 24 24" width={size} height={size} aria-hidden="true" fill="none" stroke="currentColor"
  stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" class="icon">
  {#if name === "today"}
    <circle cx="12" cy="12" r="4" /><path d="M12 2.5v2M12 19.5v2M4.6 4.6l1.4 1.4M18 18l1.4 1.4M2.5 12h2M19.5 12h2M4.6 19.4L6 18M18 6l1.4-1.4" />
  {:else if name === "journal"}
    <path d="M3 4.5h6a3 3 0 0 1 3 3V20a2.5 2.5 0 0 0-2.5-2.5H3z" /><path d="M21 4.5h-6a3 3 0 0 0-3 3V20a2.5 2.5 0 0 1 2.5-2.5H21z" />
  {:else if name === "stats"}
    <path d="M3 20.5h18" /><path d="M6 17V11M11 17V5M16 17v-8M20 17v-4" />
  {:else if name === "check"}
    <path d="M12 2.8l7.5 2.8v6.1c0 4.6-3.2 8-7.5 9.5-4.3-1.5-7.5-4.9-7.5-9.5V5.6z" /><path d="M8.8 12l2.2 2.2 4.3-4.4" />
  {:else if name === "talk"}
    <path d="M20.5 11.5a8.5 8.5 0 0 1-12.4 7.6L3.5 20.5l1.4-4.4A8.5 8.5 0 1 1 20.5 11.5z" />
  {:else if name === "settings"}
    <path d="M4 7h9M17 7h3M4 17h3M11 17h9" /><circle cx="15" cy="7" r="2" /><circle cx="9" cy="17" r="2" />
  {:else if name === "help"}
    <circle cx="12" cy="12" r="9" /><circle cx="12" cy="12" r="3.5" /><path d="M5.6 5.6l3.9 3.9M14.5 14.5l3.9 3.9M18.4 5.6l-3.9 3.9M9.5 14.5l-3.9 3.9" />
  {/if}
</svg>

<style>
  .icon { display: block; flex: none; }
</style>
