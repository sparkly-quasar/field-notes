#!/usr/bin/env python3
"""Slim the DoseWiki Open Data export down to the fields Field Notes actually uses.

DoseWiki's `SubstanceIndex.json` is ~17 MB and carries a lot Field Notes doesn't
touch (citations, subjective effects, pharmacology prose, legality, ...). This
extracts just the dose ranges, durations, classification, aliases, and graded
interactions into a minified `dosewiki.json` (~0.9 MB) that we bundle offline as a
Tauri resource.

DoseWiki content is dedicated to the public domain under CC0, so bundling it needs
no attribution (we credit DoseWiki in-app as a courtesy).

Usage (run from the data/dosewiki/ directory):
    python3 slim.py            # SubstanceIndex.json -> ../../src-tauri/resources/dosewiki.json

To refresh the snapshot, re-download SubstanceIndex.json from
https://dosewiki-admin.vercel.app/SubstanceIndex.json first, then rerun this and
bump DOSEWIKI_SNAPSHOT in src-tauri/src/pw.rs.
"""

import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(HERE, "SubstanceIndex.json")
OUT = os.path.normpath(os.path.join(HERE, "..", "..", "src-tauri", "resources", "dosewiki.json"))


def slim(s):
    ident = s.get("identification") or {}
    cls = s.get("classification") or {}
    # duration is keyed separately from dosage; join on route name.
    dur_by_route = {r["route"]: r for r in ((s.get("duration") or {}).get("routes") or []) if r.get("route")}
    routes = []
    for r in ((s.get("dosage") or {}).get("routes") or []):
        name = r.get("route")
        du = dur_by_route.get(name) or {}
        routes.append({
            "route": name,
            "dose_ranges": r.get("dose_ranges") or {},
            "stages": (du.get("stages") or {}),
            "half_life": du.get("half_life") or None,
        })
    return {
        "title": s.get("title"),
        "alternative_names": ident.get("alternative_names") or [],
        "chemical_class": cls.get("chemical_class") or [],
        "psychoactive_class": cls.get("psychoactive_class") or [],
        "routes": routes,
        "interactions": {
            "dangerous": (s.get("interactions") or {}).get("dangerous") or [],
            "unsafe": (s.get("interactions") or {}).get("unsafe") or [],
            "caution": (s.get("interactions") or {}).get("caution") or [],
        },
    }


def main():
    with open(SRC) as f:
        data = json.load(f)
    slimmed = [slim(s) for s in data if s.get("title")]
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w") as f:
        json.dump(slimmed, f, separators=(",", ":"), ensure_ascii=False)
    print(f"{len(slimmed)} substances -> {OUT} ({os.path.getsize(OUT)} bytes)")


if __name__ == "__main__":
    main()
