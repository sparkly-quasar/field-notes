# DoseWiki data pack

`SubstanceIndex.json` is a static snapshot of the **DoseWiki** substance
encyclopedia (<https://dose.wiki>), dedicated to the **public domain under CC0**
(the DoseWiki site code is MIT). No attribution is legally required; we credit
DoseWiki in-app anyway as a courtesy.

- Source: `https://dosewiki-admin.vercel.app/SubstanceIndex.json` (the "Open Data"
  export). Re-download to refresh; it's a single ~17 MB file.
- Snapshot taken: 2026-07-08. **577 substances** (547 with dose data, 546 with
  durations, 236 with interaction notes).

This replaces the live PsychonautWiki GraphQL scrape (`src-tauri/src/pw.rs`). Because
it's a single CC0 file, we can **bundle it offline** instead of fetching per-install —
no network dependency at all.

> Not yet wired into the app. This directory is staged for the migration; the code
> in `pw.rs` / `db.rs` / the Substances UI still reads PsychonautWiki. See the
> migration plan below and in `ROADMAP.md`.

## Schema (per substance)

```
title                         -> display name
slug, id                      -> stable keys
identification.alternative_names[]      -> common names / aliases
classification.chemical_class[]         -> chemical classes
classification.psychoactive_class[]     -> psychoactive classes
dosage.routes[]:
  route                                 -> ROA name (oral, insufflated, ...)
  dose_ranges.{threshold,light,moderate,strong,heavy}.{min,max,unit}
duration.routes[] (join to dosage by `route`):
  half_life (+ half_life_notes)
  stages.{onset,come_up,peak,offset,after_effects,total_duration}.{min,max,unit}
interactions.{dangerous,unsafe,caution}[]   -> graded lists of strings;
      each string is a substance/class name, often with a "(parenthetical reason)"
```

## Mapping to the current `PwInfo` / `PwRoa` structs (`src-tauri/src/pw.rs`)

| Our field            | DoseWiki source                                             |
|----------------------|-------------------------------------------------------------|
| `name`               | `title`                                                     |
| `common_names`       | `identification.alternative_names`                          |
| `chemical`           | `classification.chemical_class`                             |
| `psychoactive`       | `classification.psychoactive_class`                         |
| `roas[].name`        | `dosage.routes[].route`                                     |
| `roas[].units`       | `dose_ranges.*.unit`                                         |
| `roas[].threshold`   | `dose_ranges.threshold.min`                                 |
| `roas[].light`       | `dose_ranges.light` `{min,max}`                             |
| `roas[].common`      | `dose_ranges.moderate` `{min,max}`  ← DoseWiki says "moderate" |
| `roas[].strong`      | `dose_ranges.strong` `{min,max}`                            |
| `roas[].heavy`       | `dose_ranges.heavy.min` (`.max` is usually null / open-ended) |
| `roas[].onset`       | duration `stages.onset`                                     |
| `roas[].total`       | duration `stages.total_duration`                            |
| `interactions`       | flatten `interactions.dangerous+unsafe+caution`             |

New fields DoseWiki gives us that our model doesn't use yet (worth adding):
`come_up`, `peak`, `offset`, `after_effects`, `half_life`, and **graded**
interaction severity (dangerous / unsafe / caution).

## Integration decisions for next session

1. **Bundle vs. download.** It's CC0 and static, so prefer **bundling** it as a
   Tauri resource (fully offline, instant first run) over keeping the "update from
   the internet" button. Keeping a manual refresh is optional since the file is small.
2. **Graded interactions.** Map DoseWiki `dangerous`/`unsafe`/`caution` to our
   `danger`/`caution`/`note` severities instead of PW's single bucket — richer,
   deterministic warnings. Reword `interactions.rs` PW_INTERACTION message.
3. **Interaction parsing.** Split each entry on the first `(` — the head is the
   substance/class name to match on, the parenthetical is the reason to display.
4. **Symbol churn.** Cheapest path: keep the internal `pw_*` names/tables, just
   swap the fetch/parse source and all user-facing strings + the URL. Optional
   later cleanup: rename to `dw_*` / a neutral "dose reference".
5. **NOTICE / attribution.** Switch third-party-data note to DoseWiki CC0 (drop the
   CC-BY-SA share-alike constraint on the dose pack). Keep an in-app credit line.
