// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
// `npm test`: the phone's outbox (outbox.ts) and its offline checker
// (static/offline/field-notes.wasm, built by scripts/build-wasm.mjs), run the way
// the phone runs them.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import * as ob from "./outbox.ts";

const noCheck = () => [];

test("an experience started offline can be logged into and ended offline", () => {
  const s = ob.emptyState();
  const exp = ob.queue(s, "create_experience", { input: { title: "", started_at: "2026-10-03T20:00:00Z" } }, noCheck);
  assert.ok(exp.id < 0);
  const r = ob.queue(s, "log_dose", { input: { experience_id: exp.id, substance_name: "MDMA", amount: 100, unit: "mg", route: "oral", taken_at: "2026-10-03T20:00:00Z" } }, noCheck);
  assert.ok(r.dose.id < exp.id, "temporary IDs are never reused");
  ob.queue(s, "add_timeline_event", { input: { experience_id: exp.id, at: "2026-10-03T20:30:00Z", note: "coming up" } }, noCheck);
  ob.queue(s, "end_experience", { id: exp.id, endedAt: "2026-10-04T02:00:00Z", rating: 4, notes: "" }, noCheck);

  const d = ob.overlayDetail(s, exp.id);
  assert.equal(d.title, "MDMA", "named after its first dose, as the computer does");
  assert.equal(d.doses.length, 1);
  assert.equal(d.timeline.length, 1);
  assert.equal(d.ended_at, "2026-10-04T02:00:00Z");
  const list = ob.overlayList(s);
  assert.equal(list[0].id, exp.id);
  assert.deepEqual(list[0].substances, ["MDMA"]);
  assert.equal(ob.pending(s).length, 4);
});

test("a queued dose is checked against the experience as last seen, plus what's queued", () => {
  const s = ob.emptyState();
  ob.keepList(s, [{ id: 7, kind: "session", ended_at: null, started_at: new Date().toISOString(), substances: ["LSD"], dose_count: 1 }]);
  ob.keepDetail(s, { id: 7, kind: "session", ended_at: null, title: "LSD", doses: [{ id: 1, substance_name: "LSD", route: "oral", taken_at: "2026-10-03T19:00:00Z" }], timeline: [] });
  let seen = null;
  const r = ob.queue(s, "log_dose", { input: { experience_id: 7, substance_name: "MDMA", route: "oral", taken_at: "2026-10-03T21:00:00Z" } }, (doses) => {
    seen = doses.map((d) => d.substance_name);
    return [{ severity: "note", a: "LSD", b: "MDMA", message: "m", advice: [] }];
  });
  assert.deepEqual(seen, ["LSD", "MDMA"]);
  assert.equal(r.warnings.length, 1);
  assert.equal(ob.overlayList(s)[0].dose_count, 2);
});

test("temporary IDs are translated once the computer has assigned one", () => {
  const s = ob.emptyState();
  const exp = ob.queue(s, "create_experience", { input: { title: "x" } }, noCheck);
  ob.queue(s, "log_dose", { input: { experience_id: exp.id, substance_name: "LSD" } }, noCheck);
  const [first, second] = ob.pending(s);
  assert.equal(ob.translate(s, structuredClone(second.args)), false, "waits for its experience");
  ob.sent(s, first.seq, { id: 42 });
  const args = structuredClone(second.args);
  assert.equal(ob.translate(s, args), true);
  assert.equal(args.input.experience_id, 42);
  // The page may still hold the temporary ID: it still finds the entry.
  assert.deepEqual(ob.aliases(s, exp.id).sort(), [exp.id, 42].sort());
});

test("what the computer refuses is kept, visibly, until discarded", () => {
  const s = ob.emptyState();
  ob.queue(s, "add_timeline_event", { input: { experience_id: 3, note: "hi" } }, noCheck);
  const [item] = ob.pending(s);
  ob.markFailed(s, item.seq, "No such experience.");
  assert.equal(ob.pending(s).length, 0);
  assert.equal(ob.failed(s).length, 1);
  ob.discard(s, item.seq);
  assert.equal(s.items.length, 0);
});

test("edits and deletes never wait in the outbox", () => {
  for (const cmd of ["update_experience", "update_dose", "delete_experience", "delete_dose", "add_substance"]) {
    assert.throws(() => ob.queue(ob.emptyState(), cmd, {}, noCheck));
  }
  assert.ok(!ob.QUEUEABLE.some((c) => c.startsWith("update_") || c.startsWith("delete_")));
});

test("a dose can't be queued into a plain note", () => {
  const s = ob.emptyState();
  const n = ob.queue(s, "create_experience", { input: { kind: "note", title: "t" } }, noCheck);
  assert.throws(() => ob.queue(s, "log_dose", { input: { experience_id: n.id, substance_name: "LSD" } }, noCheck));
});

test("the phone keeps only the last day's entries and those in progress", () => {
  const s = ob.emptyState();
  const now = Date.parse("2026-10-03T12:00:00Z");
  ob.keepDetail(s, { id: 1, kind: "session", ended_at: null, doses: [], timeline: [] });
  ob.keepDetail(s, { id: 2, kind: "session", ended_at: null, doses: [], timeline: [] });
  ob.keepList(s, [
    { id: 1, kind: "session", ended_at: null, started_at: "2026-09-01T00:00:00Z" },
    { id: 2, kind: "session", ended_at: "2026-10-03T10:00:00Z", started_at: "2026-10-03T08:00:00Z" },
    { id: 3, kind: "session", ended_at: "2026-09-02T00:00:00Z", started_at: "2026-09-01T20:00:00Z" },
  ], now);
  assert.deepEqual(s.list.map((e) => e.id), [1, 2]);
  assert.deepEqual(Object.keys(s.details), ["1"], "an ended experience's doses aren't kept");
});

// ---------- the offline checker itself ----------

function loadEngine() {
  const bytes = readFileSync(new URL("../../static/offline/field-notes.wasm", import.meta.url));
  const ex = new WebAssembly.Instance(new WebAssembly.Module(bytes), {}).exports;
  const withBuffer = (text, f) => {
    const b = new TextEncoder().encode(text);
    const p = ex.alloc(b.length);
    new Uint8Array(ex.memory.buffer, p, b.length).set(b);
    try {
      return f(p, b.length);
    } finally {
      ex.dealloc(p, b.length);
    }
  };
  const ref = readFileSync(new URL("../../src-tauri/resources/dosewiki.json", import.meta.url), "utf8");
  assert.ok(withBuffer(ref, ex.load_reference) > 500);
  return (cmd, args) => {
    const at = withBuffer(JSON.stringify({ cmd, args }), ex.call);
    const len = ex.result_len();
    const out = JSON.parse(new TextDecoder().decode(new Uint8Array(ex.memory.buffer, at, len)));
    ex.dealloc(at, len);
    return out;
  };
}

test("the phone's checker flags what the computer flags", () => {
  const call = loadEngine();
  const w = call("check_combo", { names: ["MDMA", "Tramadol"] }).ok;
  assert.ok(w.some((x) => x.severity === "danger"));
  const lsd = call("check_combo", { names: ["MDMA", "LSD"] }).ok;
  assert.ok(lsd.some((x) => x.message.startsWith("Empathogen + psychedelic")));
  assert.equal(call("pw_lookup", { name: "molly" }).ok.name, "MDMA");
  assert.deepEqual(call("unknown_names", { names: ["MDMA", "my own blend"] }).ok, ["my own blend"]);
  assert.ok(call("update_dose", {}).err);
});

test("the phone's checker only combines doses that overlapped", () => {
  const call = loadEngine();
  const h = (n) => Date.parse("2026-10-03T18:00:00Z") / 60_000 + n * 60;
  const near = call("session_warnings", { doses: [{ substance_name: "MDMA", route: "oral", at_min: h(0) }, { substance_name: "Tramadol", route: "oral", at_min: h(1) }] }).ok;
  const apart = call("session_warnings", { doses: [{ substance_name: "MDMA", route: "oral", at_min: h(0) }, { substance_name: "Tramadol", route: "oral", at_min: h(72) }] }).ok;
  assert.ok(near.length > 0);
  assert.equal(apart.length, 0);
});
