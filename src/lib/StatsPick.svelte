<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  What a tap on a chart opens: the experiences behind a bar, a day, an hour or a
  dot, each one a button into the journal. Shared by Stats and Trends so a tap
  reads the same everywhere. Titles follow discreet mode the way the journal
  list does ("Experience"), and substance names come through the caller's label.
-->
<script module lang="ts">
  import { hiding } from "$lib/discreet.svelte";

  export type PickExp = { id: number; t: number; title: string; subs: string[]; rating: number | null };
  export type PickItem = { id: number; title: string; meta: string };

  /** One experience as a line: its title, then when, what, and the rating.
   *  `day`: the panel is about one day, so the date would only repeat. */
  export function pickItem(e: PickExp, label: (key: string) => string, day = false): PickItem {
    const subs = e.subs.map(label).join(", ");
    const when = new Date(e.t).toLocaleString(undefined, day
      ? { timeStyle: "short", hour12: true }
      : { weekday: "short", month: "short", day: "numeric", hour: "numeric", minute: "2-digit", hour12: true });
    const title = hiding() ? "Experience" : e.title || subs || "Untitled";
    const meta = [when, !hiding() && subs === e.title ? "" : subs, e.rating != null ? `${e.rating}/10` : ""];
    return { id: e.id, title, meta: meta.filter(Boolean).join(" · ") };
  }
</script>

<script lang="ts">
  let {
    heading,
    items,
    note = "",
    empty = "No experiences here.",
    onOpen,
    onClose,
  }: {
    heading: string;
    items: PickItem[];
    /** A line under the heading: an amount, a gap. */
    note?: string;
    empty?: string;
    onOpen?: (experienceId: number) => void;
    onClose: () => void;
  } = $props();

  /** A busy hour can hold dozens; the first few, then the rest on request. */
  const FIRST = 6;
  let all = $state(false);
  const visible = $derived(all ? items : items.slice(0, FIRST));
</script>

<div class="pick" role="region" aria-label={heading} aria-live="polite">
  <div class="top">
    <strong>{heading}</strong>
    <button class="x" onclick={onClose} aria-label="Close">×</button>
  </div>
  {#if note}<p class="sub">{note}</p>{/if}
  {#if items.length}
    <ul>
      {#each visible as it (it.id)}
        <li>
          {#if onOpen}
            <button class="row" onclick={() => onOpen?.(it.id)}>
              <span class="txt"><span class="t">{it.title}</span><span class="m">{it.meta}</span></span>
              <span class="go">Open ›</span>
            </button>
          {:else}
            <span class="txt row"><span class="t">{it.title}</span><span class="m">{it.meta}</span></span>
          {/if}
        </li>
      {/each}
    </ul>
    {#if items.length > FIRST}
      <button class="more" onclick={() => (all = !all)}>{all ? "Show fewer" : `Show all ${items.length}`}</button>
    {/if}
  {:else}
    <p class="sub">{empty}</p>
  {/if}
</div>

<style>
  .pick {
    --pk-text: var(--text, var(--ink, #e7e9ee));
    --pk-muted: var(--text-2, var(--muted, #9aa0ab));
    --pk-line: var(--divider, var(--line, #2e323b));
    --pk-accent: var(--accent, #6d8fb0);
    margin-top: 0.55rem;
    border: 1px solid var(--pk-line);
    border-radius: 10px;
    padding: 0.45rem 0.7rem 0.35rem;
    font-size: 0.92rem;
  }
  .top { display: flex; justify-content: space-between; align-items: center; gap: 0.5rem; }
  .x { background: none; border: 0; color: var(--pk-muted); font: inherit; font-size: 1.3rem; line-height: 1; min-width: 36px; min-height: 36px; margin-right: -0.4rem; cursor: pointer; }
  .sub { color: var(--pk-muted); font-size: 0.85rem; margin: 0 0 0.3rem; }
  ul { list-style: none; margin: 0; padding: 0; }
  li + li { border-top: 1px solid var(--pk-line); }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 0.6rem; width: 100%; min-height: 44px; padding: 0.35rem 0; background: none; border: 0; color: var(--pk-text); font: inherit; text-align: left; cursor: pointer; }
  .txt { display: flex; flex-direction: column; min-width: 0; }
  .t { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .m { color: var(--pk-muted); font-size: 0.82rem; }
  .more { background: none; border: 0; color: var(--pk-accent); font: inherit; font-size: 0.88rem; font-weight: 600; padding: 0.3rem 0; min-height: 40px; cursor: pointer; }
  .go { color: var(--pk-accent); font-size: 0.88rem; font-weight: 600; white-space: nowrap; }
</style>
