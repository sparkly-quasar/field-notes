<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  Interaction warnings as quiet, tappable notes rather than banners: one line per
  thing worth knowing (an icon, how much care it needs, which pairs), and behind a
  tap what it's about and what lowers the risk. The point is an informed decision,
  not an alarm: a wall of red over a dose list teaches people to stop logging.

  Used by the desktop and the phone, so the two can't drift. It styles itself from
  whichever page hosts it, falling back between the two pages' variable names.
-->
<script lang="ts">
  import type { Warning } from "$lib/api";
  import { groupWarnings } from "$lib/quicklog";

  let { warnings }: { warnings: Warning[] } = $props();

  const groups = $derived(groupWarnings(warnings));
  const label = (s: string) => (s === "danger" ? "Higher risk" : s === "caution" ? "Use care" : "Good to know");
</script>

{#if groups.length}
  <ul class="risk-notes" aria-label="Combination notes">
    {#each groups as g}
      <li class={g.severity}>
        <details>
          <summary>
            <span class="icon" aria-hidden="true">{g.severity === "note" ? "i" : "!"}</span>
            <span class="what"><strong>{label(g.severity)}</strong> · {g.pairs.join(", ")}</span>
          </summary>
          <p>{g.message}</p>
          {#if g.advice.length}
            <p class="lead">What lowers the risk</p>
            <ul class="advice">
              {#each g.advice as a}<li>{a}</li>{/each}
            </ul>
          {/if}
        </details>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .risk-notes {
    --rn-border: var(--field-border, var(--line, var(--border, #555)));
    --rn-muted: var(--text-2, var(--muted, #999));
    --rn-danger: var(--danger, #c0392b);
    --rn-caution: var(--caution, #b7791f);
    --rn-note: var(--note, var(--accent, #6ea8fe));
    list-style: none;
    padding: 0;
    margin: 0.5rem 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }
  .risk-notes > li { border: 1px solid var(--rn-border); border-radius: 10px; font-size: 0.92em; line-height: 1.45; }
  summary { display: flex; align-items: center; gap: 0.55rem; padding: 0.5rem 0.7rem; cursor: pointer; min-height: 44px; box-sizing: border-box; list-style: none; }
  summary::-webkit-details-marker { display: none; }
  summary::after { content: "›"; margin-left: auto; color: var(--rn-muted); transition: transform 0.15s; }
  details[open] summary::after { transform: rotate(90deg); }
  .icon {
    flex: none; width: 1.4rem; height: 1.4rem; border-radius: 50%; display: grid; place-items: center;
    font-weight: 700; font-size: 0.85em; color: var(--rn-note); border: 1.5px solid currentColor;
  }
  .caution .icon { color: var(--rn-caution); }
  .danger .icon { color: var(--rn-danger); }
  .what { overflow-wrap: anywhere; }
  details > p, .advice { margin: 0 0.7rem 0.5rem 2.65rem; }
  .lead { font-weight: 600; margin-top: 0.4rem; }
  .advice { padding-left: 1rem; }
  .advice li { margin-bottom: 0.25rem; }
  @media (prefers-reduced-motion: reduce) { summary::after { transition: none; } }
</style>
