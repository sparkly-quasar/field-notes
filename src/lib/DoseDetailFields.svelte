<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  Under a dose's amount and unit: what the dose was (dosedetail.ts). Shows only
  what applies, so most doses get nothing here:

  - mushrooms and truffles: dried, fresh, powdered or edible; kratom: leaf,
    extract or 7-OH. Fresh mushrooms say what they are dried; an edible takes
    an estimate in grams of dried mushroom or mg of psilocybin, or none.
  - a counted unit (capsule, pill, tab, piece): how much is in each, from the
    kinds saved for this substance, a one-off amount, or "not sure".

  `detail` is what gets saved. With `suggest`, a new dose starts on the capsule
  kind used last for this substance; an edit (`suggest={false}`) never changes
  what a dose already says. Used by every dose form, on the computer and phone.
-->
<script lang="ts">
  import { listUnitKinds, saveUnitKind, type DoseDetail, type UnitKind } from "$lib/api";
  import { COUNTED_UNITS, ESTIMATE_UNITS, formsFor, isMushroom, measure } from "$lib/dosedetail";

  let {
    substance,
    unit,
    amount = "",
    detail = $bindable({}),
    suggest = true,
  }: { substance: string; unit: string; amount?: string | number | null; detail?: DoseDetail; suggest?: boolean } =
    $props();

  const MASS = ["mg", "g", "µg"];
  const LAST_KIND = "fn.capsuleKind.";
  const EDIBLE_NOTE = "fn.edibleNoteSeen";

  const forms = $derived(formsFor(substance));
  const counted = $derived(COUNTED_UNITS.includes(unit.trim().toLowerCase()));
  const form = $derived(detail.form ?? "");
  /** Not said reads as the first form: dried, or leaf. */
  const shownForm = $derived(form || forms[0]?.value || "");

  // A form or capsule amount that no longer applies (the substance or unit
  // changed) isn't saved with the dose.
  $effect(() => {
    if (form && !forms.some((f) => f.value === form)) {
      detail = { ...detail, form: "", estimate: null, estimate_unit: "" };
    }
  });
  $effect(() => {
    if (!counted && (detail.per_unit != null || detail.unit_label)) {
      detail = { ...detail, per_unit: null, per_unit_unit: "", unit_label: "" };
    }
  });

  function pickForm(value: string) {
    detail =
      value === "edible"
        ? { ...detail, form: value, estimate_unit: detail.estimate_unit || "g" }
        : { ...detail, form: value, estimate: null, estimate_unit: "" };
  }

  // ---- how much is in each ----
  let kinds = $state<UnitKind[]>([]);
  /** Once someone picks here, a suggestion never overrides them. */
  let touched = $state(false);
  let custom = $state(false);
  let saveMsg = $state("");
  const lastKey = $derived(LAST_KIND + substance.trim().toLowerCase());

  $effect(() => {
    const s = substance.trim();
    touched = false;
    if (!counted || !s) {
      kinds = [];
      return;
    }
    const t = setTimeout(async () => {
      const got = await listUnitKinds(s).catch(() => [] as UnitKind[]);
      if (substance.trim() !== s) return;
      kinds = got;
      if (suggest && !touched && detail.per_unit == null && !detail.unit_label) {
        let last = "";
        try { last = localStorage.getItem(LAST_KIND + s.toLowerCase()) ?? ""; } catch {}
        const k = got.find((x) => x.label === last);
        if (k) useKind(k, false);
      }
    }, 250);
    return () => clearTimeout(t);
  });

  // The form was cleared (a dose was just logged): close "another amount".
  $effect(() => {
    if (detail.per_unit == null && !detail.unit_label && !detail.per_unit_unit) {
      custom = false;
      saveMsg = "";
    }
  });

  const pick = $derived.by(() => {
    if (custom) return "custom";
    const k = kinds.find((x) => x.label === detail.unit_label);
    if (k) return `k${k.id}`;
    return detail.per_unit != null ? "custom" : "none";
  });

  function useKind(k: UnitKind, byHand = true) {
    detail = { ...detail, per_unit: k.per_unit, per_unit_unit: k.per_unit_unit, unit_label: k.label };
    custom = false;
    if (byHand) {
      touched = true;
      try { localStorage.setItem(lastKey, k.label); } catch {}
    }
  }

  function onPick(v: string) {
    touched = true;
    saveMsg = "";
    if (v === "none") {
      custom = false;
      detail = { ...detail, per_unit: null, per_unit_unit: "", unit_label: "" };
      try { localStorage.removeItem(lastKey); } catch {}
    } else if (v === "custom") {
      custom = true;
      detail = { ...detail, per_unit_unit: detail.per_unit_unit || "g", unit_label: kinds.some((k) => k.label === detail.unit_label) ? "" : (detail.unit_label ?? "") };
    } else {
      const k = kinds.find((x) => `k${x.id}` === v);
      if (k) useKind(k);
    }
  }

  async function saveKind() {
    const label = (detail.unit_label ?? "").trim();
    if (!label || detail.per_unit == null) {
      saveMsg = "Give it a name and an amount to save it.";
      return;
    }
    try {
      const k = await saveUnitKind(substance.trim(), { label, per_unit: detail.per_unit, per_unit_unit: detail.per_unit_unit || "g" });
      kinds = await listUnitKinds(substance.trim()).catch(() => kinds);
      useKind(k);
      saveMsg = `Saved “${k.label}”.`;
    } catch (e) {
      // Offline on a phone: this dose still keeps the amount.
      saveMsg = `Couldn't save it for next time (${String(e).replace(/^Error: /, "")}). This dose still keeps the amount.`;
    }
  }

  // ---- what it works out to ----
  const amountNum = $derived(amount === "" || amount == null ? null : Number(amount));
  const freshAs = $derived(
    form === "fresh" && amountNum != null && Number.isFinite(amountNum) ? measure(substance, amountNum, unit, detail) : null,
  );

  let edibleNoteSeen = $state(false);
  try { edibleNoteSeen = localStorage.getItem(EDIBLE_NOTE) === "1"; } catch {}
  function dismissEdibleNote() {
    edibleNoteSeen = true;
    try { localStorage.setItem(EDIBLE_NOTE, "1"); } catch {}
  }
</script>

{#if forms.length}
  <div class="dd-row" role="group" aria-label="Form">
    <span class="dd-label">Form</span>
    {#each forms as f}
      <button type="button" class="dd-chip" class:on={shownForm === f.value} aria-pressed={shownForm === f.value} onclick={() => pickForm(f.value)}>{f.label}</button>
    {/each}
  </div>
  {#if form === "fresh"}
    {#if !isMushroom(substance)}
      <p class="dd-note">Truffles hold less water than mushrooms, so a fresh weight isn't turned into a dried one.</p>
    {:else if freshAs}
      <p class="dd-note">About <strong>{+freshAs.amount.toFixed(3)} g dried</strong>. Dose ranges are for dried mushrooms, and fresh ones are mostly water, so this is an estimate.</p>
    {:else}
      <p class="dd-note">Dose ranges are for dried mushrooms. Fresh ones are about ten times lighter once dried.</p>
    {/if}
  {:else if form === "edible"}
    <div class="dd-row">
      <label class="dd-label" for="dd-est">How much is in it?</label>
      <input id="dd-est" class="dd-num" type="number" step="any" min="0" placeholder="If you know" bind:value={detail.estimate} />
      <select aria-label="Estimate in" bind:value={detail.estimate_unit}>
        {#each ESTIMATE_UNITS as u}<option value={u.value}>{u.label}</option>{/each}
      </select>
    </div>
    <p class="dd-note">For the whole dose. Leave it blank if you don't know; a weight of chocolate or gummies isn't compared with mushroom ranges.</p>
    {#if !edibleNoteSeen}
      <p class="dd-note">
        Shop-bought “mushroom” edibles often aren't what the label says. Some contain Amanita muscaria (muscimol, a different drug with different interactions) or unlisted compounds; in 2024 one brand was recalled after people became ill and were hospitalized.
        <button type="button" class="dd-link" onclick={dismissEdibleNote}>Got it</button>
      </p>
    {/if}
  {:else if form === "7-oh"}
    <p class="dd-note">7-OH products are far stronger than kratom leaf and act like an opioid at any amount, so they aren't compared with leaf's ranges.</p>
  {:else if form === "extract"}
    <p class="dd-note">Extracts are stronger than leaf by weight, so they aren't compared with leaf's ranges.</p>
  {/if}
{/if}

{#if counted && form !== "edible"}
  <div class="dd-row">
    <label class="dd-label" for="dd-each">In each {unit}</label>
    <select id="dd-each" value={pick} onchange={(e) => onPick(e.currentTarget.value)}>
      <option value="none">Not sure</option>
      {#each kinds as k}<option value={`k${k.id}`}>{k.label}: {k.per_unit} {k.per_unit_unit}</option>{/each}
      <option value="custom">Another amount…</option>
    </select>
  </div>
  {#if pick === "custom"}
    <div class="dd-row">
      <input class="dd-num" type="number" step="any" min="0" placeholder="Amount" aria-label="Amount in each" bind:value={detail.per_unit} />
      <select aria-label="Unit" bind:value={detail.per_unit_unit}>
        {#each MASS as u}<option>{u}</option>{/each}
      </select>
      <input class="dd-name" placeholder="Name, e.g. 00 caps" aria-label="Name these" bind:value={detail.unit_label} />
      <button type="button" class="dd-link" onclick={saveKind}>Save for next time</button>
    </div>
  {/if}
  {#if saveMsg}<p class="dd-note">{saveMsg}</p>{/if}
  {#if unit.trim().toLowerCase() === "pill"}
    <p class="dd-note">What a pressed pill claims and what it holds can differ, so its amount is “logged as”.</p>
  {/if}
{/if}

<style>
  .dd-row { display: flex; flex-wrap: wrap; align-items: center; gap: 0.4rem; margin: 0.35rem 0; }
  .dd-label { font-size: var(--fs-sm, 0.85rem); color: var(--muted, var(--text-2)); margin-right: 0.2rem; }
  .dd-chip {
    width: auto; min-height: var(--tap-min, 0); margin: 0; padding: 0.25rem 0.7rem; border-radius: 999px;
    border: 1px solid var(--line, var(--field-border, currentColor)); background: transparent;
    color: var(--muted, var(--text-2)); font: inherit; font-size: var(--fs-sm, 0.85rem); cursor: pointer;
  }
  .dd-chip.on {
    background: color-mix(in srgb, var(--accent) 22%, transparent); color: var(--accent);
    border-color: var(--accent); font-weight: 600;
  }
  /* Each screen styles its own fields, and those styles don't reach in here:
     match them from the shared colour tokens (phone first, then desktop). */
  .dd-row select, .dd-row input {
    width: auto; margin: 0; font: inherit; font-size: var(--fs-sm, 0.9rem);
    min-height: var(--tap-min, 2rem); padding: 0.3rem 0.6rem;
    border: 1px solid var(--field-border, var(--line, currentColor)); border-radius: var(--radius, 6px);
    background: var(--field, var(--card, transparent)); color: var(--text, var(--ink, inherit));
  }
  .dd-num { width: 6.5rem; flex: none; }
  .dd-name { width: 10rem; flex: 1 1 8rem; }
  .dd-row select { width: auto; flex: none; }
  .dd-note { margin: 0.2rem 0 0.5rem; font-size: var(--fs-sm, 0.85rem); color: var(--muted, var(--text-2)); }
  .dd-link {
    display: inline; width: auto; min-height: 0; margin: 0 0 0 0.25rem; padding: 0; border: 0; background: none;
    color: var(--accent); font: inherit; text-decoration: underline; cursor: pointer;
  }
</style>
