<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  A substance field that suggests names as you type: your own substances first,
  then the dose reference's names and street names ("acid" offers LSD). Picking
  one fills the field; typing anything else is still saved as written. Works the
  same on a phone and the desktop (a native datalist barely shows on iOS).
-->
<script module lang="ts">
  import { listSubstances, pwNames } from "$lib/api";

  type Name = { name: string; aliases: string[]; mine: boolean };
  let cache: Promise<Name[]> | null = null;

  /** Loaded once per page, and again after `forgetSubstanceNames`. */
  function names(): Promise<Name[]> {
    cache ??= Promise.all([listSubstances().catch(() => []), pwNames().catch(() => [])]).then(([mine, ref]) => {
      const seen = new Set<string>();
      const out: Name[] = [];
      for (const n of [
        ...mine.map((s) => ({ name: s.name, aliases: s.aliases ?? [], mine: true })),
        ...ref.map((r) => ({ name: r.name, aliases: r.aliases ?? [], mine: false })),
      ]) {
        const k = n.name.toLowerCase();
        if (seen.has(k)) continue;
        seen.add(k);
        out.push(n);
      }
      if (!out.length) cache = null; // offline on first load: try again next time
      return out;
    });
    return cache;
  }

  /** After a substance is added or removed, so the next suggestions include it. */
  export function forgetSubstanceNames() {
    cache = null;
  }

  export type Suggestion = { name: string; via: string | null };

  /** Best few matches for what's typed: start of a name, then start of a word or
   *  street name, then anywhere. Your own substances first within each. */
  export function suggest(all: Name[], typed: string, limit = 6): Suggestion[] {
    const q = typed.trim().toLowerCase();
    if (!q) return [];
    const scored: { s: Suggestion; score: number }[] = [];
    for (const n of all) {
      const name = n.name.toLowerCase();
      if (name === q) continue; // already typed in full
      let score = -1;
      let via: string | null = null;
      if (name.startsWith(q)) score = 0;
      else if (name.split(/[\s\-(/]+/).some((w) => w.startsWith(q))) score = 1;
      else {
        const a = n.aliases.find((a) => a.toLowerCase().startsWith(q));
        if (a && q.length >= 2) {
          score = 2;
          via = a;
        } else if (q.length >= 3 && name.includes(q)) score = 3;
      }
      if (score < 0) continue;
      scored.push({ s: { name: n.name, via }, score: score * 2 + (n.mine ? 0 : 1) });
    }
    scored.sort((a, b) => a.score - b.score || a.s.name.length - b.s.name.length || a.s.name.localeCompare(b.s.name));
    return scored.slice(0, limit).map((x) => x.s);
  }

  let uid = 0;
</script>

<script lang="ts">
  let {
    value = $bindable(""),
    id = undefined,
    placeholder = undefined,
    onpick = undefined,
    onblur = undefined,
    onchange = undefined,
    enterkeyhint = undefined,
  }: {
    value?: string;
    id?: string;
    placeholder?: string;
    /** After a suggestion fills the field. */
    onpick?: (name: string) => void;
    onblur?: () => void;
    onchange?: () => void;
    enterkeyhint?: "next" | "done" | "go" | "send" | "search" | "enter" | "previous";
  } = $props();

  const listId = `subsuggest-${++uid}`;
  let all = $state<Name[]>([]);
  let focused = $state(false);
  /** Closed by Escape or a pick until the next keystroke. */
  let dismissed = $state(false);
  let active = $state(-1);

  const shown = $derived(focused && !dismissed ? suggest(all, value) : []);

  async function focus() {
    focused = true;
    all = await names();
  }

  function pick(name: string) {
    value = name;
    dismissed = true;
    active = -1;
    onpick?.(name);
    onchange?.();
  }

  function key(e: KeyboardEvent) {
    if (!shown.length) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      active = (active + 1) % shown.length;
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      active = active <= 0 ? shown.length - 1 : active - 1;
    } else if (e.key === "Enter" && active >= 0) {
      e.preventDefault();
      pick(shown[active].name);
    } else if (e.key === "Escape") {
      dismissed = true;
    }
  }
</script>

<div class="sub-suggest">
  <input
    {id}
    {placeholder}
    {enterkeyhint}
    bind:value
    role="combobox"
    aria-expanded={shown.length > 0}
    aria-controls={listId}
    aria-autocomplete="list"
    aria-activedescendant={active >= 0 && shown.length ? `${listId}-${active}` : undefined}
    autocapitalize="none"
    autocomplete="off"
    spellcheck="false"
    onfocus={focus}
    oninput={() => {
      dismissed = false;
      active = -1;
    }}
    onkeydown={key}
    onblur={() => {
      focused = false;
      active = -1;
      onblur?.();
    }}
    {onchange}
  />
  {#if shown.length}
    <ul class="menu" id={listId} role="listbox">
      {#each shown as s, i (s.name)}
        <!-- pointerdown, not click: picking mustn't blur the field first. -->
        <li
          id="{listId}-{i}"
          role="option"
          aria-selected={i === active}
          class:active={i === active}
          onpointerdown={(e) => {
            e.preventDefault();
            pick(s.name);
          }}
        >
          {s.name}{#if s.via}<span class="via"> · “{s.via}”</span>{/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .sub-suggest { position: relative; }
  .sub-suggest input { width: 100%; }
  .menu {
    position: absolute;
    z-index: 30;
    left: 0;
    right: 0;
    top: calc(100% + 2px);
    margin: 0;
    padding: 0.25rem 0;
    list-style: none;
    background: var(--surface-2, var(--surface, #222));
    border: 1px solid var(--divider, #444);
    border-radius: 10px;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
    max-height: 16rem;
    overflow-y: auto;
  }
  .menu li {
    padding: 0.6rem 0.85rem;
    min-height: var(--tap-min, 44px);
    display: flex;
    align-items: center;
    cursor: pointer;
    color: var(--text, inherit);
  }
  .menu li.active,
  .menu li:hover { background: color-mix(in srgb, var(--accent, #a9c) 18%, transparent); }
  .via { color: var(--text-2, #999); font-size: var(--fs-sm, 0.9rem); margin-left: 0.25rem; }
</style>
