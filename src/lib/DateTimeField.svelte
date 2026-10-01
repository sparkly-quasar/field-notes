<!-- SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0 -->
<!--
  A date and a time as two fields, standing in for <input type="datetime-local">.
  With the combined input, tapping anywhere (the time included) opens one picker
  with a calendar on top on iOS and in Safari on macOS, so changing the time
  meant going through the calendar. Split, tapping the time opens only a time
  picker, and tapping the date opens only the calendar.

  `value` keeps the datetime-local format ("YYYY-MM-DDTHH:MM"), so every caller's
  conversion helpers work unchanged.
-->
<script lang="ts">
  let {
    value = $bindable(""),
    id,
    title = "",
    variant = "desk",
  }: {
    value?: string;
    /** Goes on the date field, so an existing `<label for>` still points at it. */
    id?: string;
    title?: string;
    /** "phone" matches the /m page's full-width, thumb-sized fields. */
    variant?: "desk" | "phone";
  } = $props();

  const date = $derived(value ? value.slice(0, 10) : "");
  const time = $derived(value ? value.slice(11, 16) : "");

  function today(): string {
    const d = new Date();
    const p = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
  }

  // An empty field mid-edit (a half-typed date) leaves the value alone rather than
  // wiping the other half.
  function setDate(d: string) {
    if (d) value = `${d}T${time || "00:00"}`;
  }
  function setTime(t: string) {
    if (t) value = `${date || today()}T${t.slice(0, 5)}`;
  }
</script>

<span class="dtf {variant}" {title}>
  <input
    {id}
    type="date"
    value={date}
    aria-label={title ? `${title}: date` : "Date"}
    oninput={(e) => setDate(e.currentTarget.value)}
  />
  <input
    type="time"
    value={time}
    aria-label={title ? `${title}: time` : "Time"}
    oninput={(e) => setTime(e.currentTarget.value)}
  />
</span>

<style>
  /* Styled from whichever page's tokens are present: the phone's (--field,
     --text, --field-border) or the desktop's (--bg, --ink, --line). */
  .dtf { display: inline-flex; gap: 0.4rem; align-items: center; max-width: 100%; }
  input {
    font: inherit;
    background: var(--field, var(--bg));
    color: var(--text, var(--ink));
    border: 1px solid var(--field-border, var(--line));
    border-radius: 8px;
    padding: 0.55rem 0.7rem;
    min-width: 0;
    box-sizing: border-box;
  }
  /* The desktop is dark-only; without this the native calendar/clock icons are
     dark on dark. The phone page sets its own color-scheme. */
  .desk input { color-scheme: dark; }
  .phone { display: flex; width: 100%; gap: 0.5rem; margin-bottom: 0.6rem; }
  .phone input {
    font-size: var(--fs-body, 1rem);
    border-radius: 10px;
    padding: 0.7rem 0.75rem;
    min-height: var(--tap, 48px);
  }
  .phone input[type="date"] { flex: 1.4; }
  .phone input[type="time"] { flex: 1; }
</style>
