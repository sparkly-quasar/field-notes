<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  The morning question (dose-aware Stats, step 6): "How did you sleep last
  night?", 1 to 5, one tap. Only for someone who turned it on (the time-of-day
  card in Stats), only in the morning, only until answered or skipped. No
  streaks, no reminders, nothing owed: a skipped morning is just a morning.
  Offline on a phone it says nothing; it can be answered when the computer's
  back.
-->
<script lang="ts">
  import { logSleep, sleepCheckin } from "$lib/api";
  import { lastNight } from "$lib/stats";

  const SKIP = "fn.sleepSkipped";
  const night = lastNight();
  const hour = new Date().getHours();

  let ask = $state(false);
  let thanks = $state(false);

  if (hour >= 4 && hour < 14) {
    let skipped = "";
    try { skipped = localStorage.getItem(SKIP) ?? ""; } catch {}
    if (skipped !== night) {
      sleepCheckin(night)
        .then((s) => (ask = s.enabled && s.rating == null))
        .catch(() => {});
    }
  }

  async function rate(n: number) {
    try {
      await logSleep(night, n);
      ask = false;
      thanks = true;
      setTimeout(() => (thanks = false), 4000);
    } catch {
      ask = false;
    }
  }
  function skip() {
    ask = false;
    try { localStorage.setItem(SKIP, night); } catch {}
  }

  const WORDS = ["Badly", "", "OK", "", "Well"];
</script>

{#if ask}
  <section class="sleepq" aria-label="How you slept">
    <p>How did you sleep last night?</p>
    <div class="row" role="group" aria-label="1 badly to 5 well">
      {#each [1, 2, 3, 4, 5] as n}
        <button type="button" onclick={() => rate(n)} aria-label={`${n}${WORDS[n - 1] ? `, ${WORDS[n - 1].toLowerCase()}` : ""}`}>
          <span class="n">{n}</span>{#if WORDS[n - 1]}<span class="w">{WORDS[n - 1]}</span>{/if}
        </button>
      {/each}
    </div>
    <button type="button" class="skip" onclick={skip}>Skip</button>
  </section>
{:else if thanks}
  <p class="sleep-thanks" role="status">Noted.</p>
{/if}

<style>
  .sleepq {
    margin: 0 0 0.8rem; padding: 0.7rem 0.9rem; border-radius: var(--radius, 10px);
    border: 1px solid var(--line, var(--divider, currentColor)); background: var(--surface, var(--card, transparent));
  }
  .sleepq p { margin: 0 0 0.5rem; font-weight: 600; }
  .row { display: grid; grid-template-columns: repeat(5, 1fr); gap: 0.4rem; }
  .row button {
    width: 100%; margin: 0; min-height: var(--tap, 2.6rem); padding: 0.3rem 0; border-radius: 10px;
    border: 1px solid var(--accent); background: transparent; color: var(--accent); font: inherit; cursor: pointer;
    display: flex; flex-direction: column; align-items: center; justify-content: center; line-height: 1.1;
  }
  .n { font-weight: 700; font-size: 1.05rem; }
  .w { font-size: 0.7rem; opacity: 0.85; }
  .skip {
    width: auto; min-height: 0; margin: 0.4rem 0 0; padding: 0.2rem 0; border: 0; background: none;
    color: var(--muted, var(--text-2)); font: inherit; font-size: 0.85rem; text-decoration: underline; cursor: pointer;
  }
  .sleep-thanks { margin: 0 0 0.8rem; color: var(--muted, var(--text-2)); font-size: 0.9rem; }
</style>
