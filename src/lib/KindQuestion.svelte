<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  Right after a dose is logged: the one question the app may have about how that
  substance is taken (kinds.rs). "You take Adderall around 8am most days. Should
  Field Notes treat it as part of your routine?", or, on a second dose of a
  benzodiazepine, Z-drug, gabapentinoid or kratom, "How do you take this?".

  Almost always there's nothing to ask and this shows nothing. The backend
  decides, and remembers answers so nothing is nagged: a No sticks, Not now waits
  a week. `tick` asks again for the same substance after another dose.
-->
<script lang="ts">
  import { answerKindQuestion, kindQuestion, type KindQuestion } from "$lib/api";

  let { substance, tick = 0 }: { substance: string; tick?: number } = $props();

  let q = $state<KindQuestion | null>(null);
  let said = $state("");
  let busy = $state(false);

  $effect(() => {
    const s = substance.trim();
    void tick;
    q = null;
    said = "";
    if (!s) return;
    kindQuestion(s)
      .then((r) => {
        if (substance.trim() === s) q = r;
      })
      .catch(() => {});
  });

  /** What the answer means, in a sentence. Nothing for "not now". */
  function meaning(name: string, value: string): string {
    switch (value) {
      case "yes":
      case "routine":
        return `${name} is part of your routine now. Its doses stay in your journal, your combination checks and its own Stats, but aren't counted as experiences. You can change this on its Stats page.`;
      case "as_needed":
        return `${name} is marked as taken as needed. Its doses stay in your journal, your combination checks and its own Stats, but aren't counted as experiences. You can change this on its Stats page.`;
      case "no":
        return "Got it. Field Notes won't ask again unless the pattern changes.";
      case "neither":
        return `Got it. ${name} stays with your experiences.`;
      default:
        return "";
    }
  }

  async function pick(value: string) {
    if (!q || busy) return;
    busy = true;
    try {
      await answerKindQuestion(q.substance, q.ask, value);
      said = meaning(q.substance, value);
      q = null;
    } catch {
      // Offline on a phone: it'll be asked again after a later dose.
      q = null;
    } finally {
      busy = false;
    }
  }
</script>

{#if q}
  <div class="kq" role="group" aria-label="How you take {q.substance}">
    <p>{q.text}</p>
    <div class="kq-row">
      {#each q.choices as c}
        <button type="button" disabled={busy} onclick={() => pick(c.value)}>{c.label}</button>
      {/each}
    </div>
  </div>
{:else if said}
  <p class="kq-said">{said}</p>
{/if}

<style>
  .kq {
    margin: 0.6rem 0; padding: 0.7rem 0.8rem; border-radius: var(--radius, 10px);
    border: 1px solid var(--line, var(--divider, currentColor));
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }
  .kq p { margin: 0 0 0.5rem; font-size: var(--fs-sm, 0.92rem); }
  .kq-row { display: flex; flex-wrap: wrap; gap: 0.4rem; }
  .kq-row button {
    width: auto; margin: 0; min-height: var(--tap-min, 2rem); padding: 0.3rem 0.8rem; border-radius: 999px;
    border: 1px solid var(--accent); background: transparent; color: var(--accent);
    font: inherit; font-size: var(--fs-sm, 0.9rem); cursor: pointer;
  }
  .kq-row button:first-child { background: var(--accent); color: var(--on-accent, var(--accent-ink, #fff)); }
  .kq-said { margin: 0.5rem 0; font-size: var(--fs-sm, 0.88rem); color: var(--muted, var(--text-2)); }
</style>
