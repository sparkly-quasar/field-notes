<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  How long something lasts, in the person's own words, for a substance the dose
  reference has no timings for. Saved in their journal (`my_refs`); the lookup
  fills only the timings the reference leaves empty, so its own always win.
-->
<script lang="ts">
  import { myRefDelete, myRefGet, myRefSet, type MyTimings } from "$lib/api";
  import { parseSpan } from "$lib/arcs";

  let {
    name,
    label = name,
    route = "oral",
    onsaved,
    oncancel,
  }: {
    /** The substance, as the reference (or the journal) names it. */
    name: string;
    /** The name to show: a stand-in in discreet mode. */
    label?: string;
    route?: string;
    onsaved: () => void;
    oncancel: () => void;
  } = $props();

  const ROUTES = ["oral", "sublingual", "buccal", "insufflated", "smoked", "vaporized", "intramuscular", "intravenous", "rectal", "transdermal"];
  const STAGES = [
    { key: "onset", label: "Starts working within", eg: "30–60 minutes" },
    { key: "come_up", label: "Builds up over", eg: "30 minutes" },
    { key: "peak", label: "Strongest for", eg: "1–2 hours" },
    { key: "offset", label: "Wears off over", eg: "2–4 hours" },
    { key: "total", label: "Lasts in total", eg: "6–8 hours" },
    { key: "after_effects", label: "After-effects for", eg: "" },
  ] as const;
  type StageKey = (typeof STAGES)[number]["key"];

  let how = $state("");
  let times = $state<Record<StageKey, string>>({ onset: "", come_up: "", peak: "", offset: "", total: "", after_effects: "" });
  let existing = $state(false);
  let loading = $state(true);
  let saving = $state(false);
  let err = $state("");

  $effect(() => {
    how = route || "oral";
    myRefGet(name)
      .then((t) => {
        if (!t) return;
        existing = true;
        how = t.route;
        for (const s of STAGES) times[s.key] = t[s.key] ?? "";
      })
      .catch(() => {})
      .finally(() => (loading = false));
  });

  const unreadable = $derived(STAGES.filter((s) => times[s.key].trim() && !parseSpan(times[s.key])));
  const anything = $derived(STAGES.some((s) => times[s.key].trim()));

  async function save() {
    err = "";
    if (unreadable.length) {
      err = `Write ${unreadable.map((s) => `"${s.label.toLowerCase()}"`).join(" and ")} as a time, like "30–60 minutes" or "2 hours".`;
      return;
    }
    if (!anything) {
      err = "Fill in at least one timing.";
      return;
    }
    if (!how.trim()) {
      err = "Say how it's taken.";
      return;
    }
    const t = (k: StageKey) => times[k].trim() || null;
    const timings: MyTimings = {
      route: how.trim().toLowerCase(),
      onset: t("onset"),
      come_up: t("come_up"),
      peak: t("peak"),
      offset: t("offset"),
      after_effects: t("after_effects"),
      total: t("total"),
    };
    saving = true;
    try {
      await myRefSet(name, timings);
      onsaved();
    } catch (e) {
      err = e instanceof Error ? e.message : String(e);
    } finally {
      saving = false;
    }
  }

  async function remove() {
    err = "";
    saving = true;
    try {
      await myRefDelete(name);
      onsaved();
    } catch (e) {
      err = e instanceof Error ? e.message : String(e);
    } finally {
      saving = false;
    }
  }
</script>

<form class="mine" onsubmit={(e) => { e.preventDefault(); save(); }}>
  <strong>How long {label} lasts</strong>
  <p class="hint">
    Saved in your journal. These fill gaps in the dose reference and never replace its timings. Nobody checks
    them, so take them from the leaflet or your own experience. Leave out anything you don't know.
  </p>
  {#if loading}
    <p class="hint">Loading…</p>
  {:else}
    <label>How you take it
      <input list="mine-routes" bind:value={how} autocomplete="off" />
    </label>
    <datalist id="mine-routes">{#each ROUTES as r}<option value={r}></option>{/each}</datalist>

    {#each STAGES as s}
      <label>{s.label}
        <input bind:value={times[s.key]} placeholder={s.eg} aria-invalid={unreadable.includes(s)} />
      </label>
    {/each}
    <p class="hint">
      With "builds up", "strongest" and "wears off", the chart draws a curve. With only "lasts in total", it
      draws a block.
    </p>

    {#if err}<p class="err" role="alert">{err}</p>{/if}
    <div class="actions">
      <button type="submit" disabled={saving}>{saving ? "Saving…" : "Save"}</button>
      <button type="button" class="ghost" onclick={oncancel} disabled={saving}>Cancel</button>
      {#if existing}<button type="button" class="ghost" onclick={remove} disabled={saving}>Remove my timings</button>{/if}
    </div>
  {/if}
</form>

<style>
  /* Styled from whichever page's tokens are present: the phone's (--field,
     --text, --field-border) or the desktop's (--bg, --ink, --line). */
  .mine { display: grid; gap: 0.5rem; margin-top: 0.6rem; padding-top: 0.6rem; border-top: 1px solid var(--field-border, var(--line)); }
  .hint { margin: 0; font-size: 0.85em; opacity: 0.8; }
  label { display: grid; gap: 0.2rem; font-size: 0.9em; }
  input {
    font: inherit;
    background: var(--field, var(--bg));
    color: var(--text, var(--ink));
    border: 1px solid var(--field-border, var(--line));
    border-radius: 8px;
    padding: 0.45rem 0.6rem;
    min-width: 0;
    box-sizing: border-box;
  }
  input[aria-invalid="true"] { border-color: var(--bad, #c0392b); }
  .err { margin: 0; color: var(--bad, #c0392b); }
  .actions { display: flex; flex-wrap: wrap; gap: 0.5rem; }
</style>
