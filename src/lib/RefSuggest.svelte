<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  Suggesting what the person added themselves (their own timings, substances the
  reference lacks) back to Field Notes, as a prefilled GitHub issue they open in
  their browser and submit there, or don't. Nothing is sent from the app
  (`suggest.rs`).

  `prompt`: the monthly question on Today. Only when there's something new, at
  most once every 30 days, never during a live trip report or with names hidden,
  and "Not now" counts as asked. `list`: the same, any time, in Settings, with
  the switch for the reminder.
-->
<script lang="ts">
  import {
    refSuggestionIssues,
    refSuggestionLater,
    refSuggestionMark,
    refSuggestions,
    setRefSuggestionReminder,
    type RefIssue,
    type RefSuggestionItem,
  } from "$lib/api";
  import { hiding } from "$lib/discreet.svelte";
  import { inTauri } from "$lib/portal";

  let {
    mode = "prompt",
    quiet = false,
  }: {
    mode?: "prompt" | "list";
    /** A live trip report is going: the prompt waits. */
    quiet?: boolean;
  } = $props();

  function localDate(d = new Date()): string {
    const p = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
  }
  const today = localDate();

  let items = $state<RefSuggestionItem[]>([]);
  let due = $state(false);
  let reminder = $state(true);
  let loaded = $state(false);
  let open = $state(false);
  let chosen = $state<Record<string, boolean>>({});
  let issues = $state<RefIssue[] | null>(null);
  let opened = $state<Record<string, boolean>>({});
  let done = $state("");
  let err = $state("");

  async function load() {
    try {
      const s = await refSuggestions(mode === "prompt" ? today : null);
      items = s.items;
      due = s.due;
      reminder = s.reminder;
      chosen = Object.fromEntries(s.items.map((i) => [i.key, true]));
    } catch {
      // Locked, or a phone without its computer: nothing to ask about right now.
      items = [];
      due = false;
    }
    loaded = true;
  }
  load();

  const show = $derived(
    loaded && (mode === "list" || (due && !quiet && !hiding()) || open || !!done),
  );
  const picked = $derived(items.filter((i) => chosen[i.key]).map((i) => i.key));

  async function notNow() {
    due = false;
    open = false;
    await refSuggestionLater(today).catch(() => {});
  }
  async function stopAsking() {
    await setRefSuggestionReminder(false).catch(() => {});
    reminder = false;
    await notNow();
  }
  async function toggleReminder() {
    const on = !reminder;
    try {
      await setRefSuggestionReminder(on);
      reminder = on;
    } catch (e) {
      err = e instanceof Error ? e.message : String(e);
    }
  }
  async function keepPrivate(key: string) {
    try {
      await refSuggestionMark([key], true, today);
      items = items.filter((i) => i.key !== key);
      issues = null;
      if (!items.length) finish("Kept private. Nothing left to suggest.");
    } catch (e) {
      err = e instanceof Error ? e.message : String(e);
    }
  }
  async function prepare() {
    err = "";
    try {
      issues = await refSuggestionIssues(picked);
      opened = {};
    } catch (e) {
      err = e instanceof Error ? e.message : String(e);
    }
  }
  /** Opening the page is the person's own action; remember it so the same thing
   *  isn't offered again. It doesn't mean they submitted it. */
  async function openIssue(ev: MouseEvent, issue: RefIssue) {
    if (inTauri()) {
      ev.preventDefault();
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(issue.url).catch(() => {});
    }
    opened[issue.url] = true;
    await refSuggestionMark(issue.keys, false, today).catch(() => {});
    if (issues && issues.every((i) => opened[i.url])) {
      const left = items.filter((i) => !issues!.some((x) => x.keys.includes(i.key)));
      items = left;
      issues = null;
      finish("Thank you. Submit it on GitHub, or close the page if you changed your mind.");
    }
  }
  function finish(msg: string) {
    done = msg;
    open = false;
    due = false;
  }
  const KIND = { timings: "your timings", substance: "not in the reference" } as const;
</script>

{#if show}
  <section class="refs" class:card-like={mode === "prompt"} aria-label="Suggest to the reference">
    {#if mode === "list"}
      <p class="muted small">
        Timings you entered yourself, and substances you added that the reference doesn't have, can be suggested back
        to Field Notes so the next person doesn't have to type them. Each one is reviewed by hand before anything is
        added.
      </p>
    {/if}

    {#if done && mode === "prompt" && !open}
      <p role="status">{done}</p>
    {:else if mode === "prompt" && !open}
      <p>
        <strong>You've added {items.length === 1 ? "something" : `${items.length} things`} the dose reference doesn't
          have.</strong>
        Want to suggest {items.length === 1 ? "it" : "them"} for review, so {items.length === 1 ? "it's" : "they're"} there
        for everyone?
      </p>
      <div class="pair">
        <button class="primary" onclick={() => (open = true)}>Review…</button>
        <button onclick={notNow}>Not now</button>
      </div>
      <button class="link" onclick={stopAsking}>Don't ask me again</button>
    {/if}

    {#if open || mode === "list"}
      {#if !items.length}
        <p class="muted small">{done || "Nothing to suggest right now."}</p>
      {:else}
        <ul>
          {#each items as i (i.key)}
            <li>
              <label class="head">
                <input type="checkbox" bind:checked={chosen[i.key]} onchange={() => (issues = null)} />
                <strong>{i.name}</strong>
                <span class="tag">{i.changed ? "changed since you shared it" : KIND[i.kind]}</span>
              </label>
              {#if i.fields.length}
                <dl>
                  {#each i.fields as [label, value]}
                    <dt>{label}</dt>
                    <dd>{value}</dd>
                  {/each}
                </dl>
              {/if}
              <button class="link" onclick={() => keepPrivate(i.key)}>Keep this private</button>
            </li>
          {/each}
        </ul>

        <p class="muted small">
          <strong>This is everything that would be shared.</strong> Never your journal, a dose you logged, or when. It
          opens prefilled in your browser as a public GitHub issue, and you submit it there under your GitHub account
          (any account works, including a new one), or close the page. Nothing is sent from Field Notes.
        </p>

        {#if issues}
          <div class="pair">
            {#each issues as issue, n (issue.url)}
              <a
                class="btn primary"
                href={issue.url}
                target="_blank"
                rel="noopener noreferrer"
                onclick={(ev) => openIssue(ev, issue)}
              >
                {opened[issue.url] ? "Opened" : "Open on GitHub"}{issues.length > 1 ? ` (${n + 1} of ${issues.length})` : ""} →
              </a>
            {/each}
          </div>
          {#if issues.length > 1}
            <p class="muted small">That's too much for one page, so it's split. Open each one.</p>
          {/if}
        {:else}
          <div class="pair">
            <button class="primary" disabled={!picked.length} onclick={prepare}>
              Suggest {picked.length === items.length ? (items.length === 1 ? "it" : "all") : picked.length}…
            </button>
            {#if mode === "prompt"}<button onclick={notNow}>Not now</button>{/if}
          </div>
        {/if}
        {#if done && mode === "list"}<p class="muted small" role="status">{done}</p>{/if}
      {/if}
      {#if err}<p class="err" role="alert">{err}</p>{/if}
    {/if}

    {#if mode === "list"}
      <label class="remind">
        <input type="checkbox" checked={reminder} onchange={toggleReminder} />
        Ask me about new ones once a month
      </label>
    {/if}
  </section>
{/if}

<style>
  .refs { margin: 0 0 0.8rem; }
  .card-like {
    padding: 0.7rem 0.9rem; border-radius: var(--radius, 10px);
    border: 1px solid var(--line, var(--divider, currentColor)); background: var(--surface, var(--card, transparent));
  }
  .refs p { margin: 0 0 0.5rem; }
  ul { list-style: none; margin: 0 0 0.6rem; padding: 0; }
  li { padding: 0.5rem 0; border-bottom: 1px solid var(--line, var(--divider, transparent)); }
  .head { display: flex; flex-wrap: wrap; align-items: center; gap: 0.4rem; margin: 0; }
  .head input { width: auto; margin: 0; }
  .tag { font-size: 0.75rem; color: var(--muted, var(--text-2)); }
  dl { display: grid; grid-template-columns: auto 1fr; gap: 0.1rem 0.6rem; margin: 0.3rem 0 0.2rem 1.6rem; font-size: 0.85rem; }
  dt { color: var(--muted, var(--text-2)); }
  dd { margin: 0; overflow-wrap: anywhere; }
  .pair { display: flex; flex-wrap: wrap; gap: 0.5rem; margin: 0 0 0.4rem; }
  .pair > * { width: auto; margin: 0; }
  .btn {
    display: inline-flex; align-items: center; min-height: var(--tap, 2.4rem); padding: 0.3rem 0.9rem;
    border-radius: 10px; text-decoration: none; background: var(--accent); color: var(--on-accent, #fff);
  }
  .link {
    width: auto; min-height: 0; margin: 0.2rem 0 0; padding: 0.1rem 0; border: 0; background: none;
    color: var(--muted, var(--text-2)); font: inherit; font-size: 0.85rem; text-decoration: underline; cursor: pointer;
  }
  li .link { margin-left: 1.6rem; }
  .remind { display: flex; align-items: center; gap: 0.4rem; margin: 0.5rem 0 0; font-size: 0.9rem; }
  .remind input { width: auto; margin: 0; }
  .err { color: var(--danger, crimson); }
  .muted { color: var(--muted, var(--text-2)); }
  .small { font-size: 0.85rem; }
</style>
