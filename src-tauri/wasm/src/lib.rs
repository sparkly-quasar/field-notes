// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! `field_notes_core`, compiled to WebAssembly for the phone. When the phone can't
//! reach the computer, its combination checker, dose lookup and passage search run
//! here, on the same code the computer runs, over the bundled reference files the
//! phone cached while it was connected.
//!
//! The interface is deliberately tiny and dependency-free (no wasm-bindgen): JSON
//! in, JSON out, through linear memory. `src/lib/offline.ts` is the other half.
//!
//! - `alloc(len)` / `dealloc(ptr, len)`: buffers for the host to write into.
//! - `load_reference(ptr, len)`: the bundled `dosewiki.json`; returns the number of
//!   substances, or -1.
//! - `load_corpus(ptr, len)`: the bundled `dosewiki-corpus.json`; returns the
//!   number of passages, or -1.
//! - `call(ptr, len)`: `{"cmd": …, "args": {…}}`; returns a pointer to
//!   `{"ok": …}` or `{"err": "…"}`, `result_len()` bytes long, which the host
//!   frees with `dealloc`.

use field_notes_core::check::{self, MemReference, Reference, TimedDose};
use field_notes_core::knowledge::{self, Index};
use field_notes_core::pw;
use serde::Deserialize;
use serde_json::{json, Value};
use std::cell::RefCell;

thread_local! {
    static REFERENCE: RefCell<Option<MemReference>> = const { RefCell::new(None) };
    static CORPUS: RefCell<Option<Index>> = const { RefCell::new(None) };
    static RESULT_LEN: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Length of the buffer the last [`call`] returned.
#[no_mangle]
pub extern "C" fn result_len() -> usize {
    RESULT_LEN.with(|l| l.get())
}

#[no_mangle]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len.max(1));
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// # Safety
/// `ptr` and `len` must come from [`alloc`] or a [`call`] result, freed once.
#[no_mangle]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, len: usize) {
    drop(Vec::from_raw_parts(ptr, 0, len.max(1)));
}

/// # Safety
/// `ptr..ptr+len` must be a buffer from [`alloc`] holding UTF-8.
unsafe fn read(ptr: *const u8, len: usize) -> String {
    String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned()
}

/// # Safety
/// See [`read`].
#[no_mangle]
pub unsafe extern "C" fn load_reference(ptr: *const u8, len: usize) -> i32 {
    match pw::parse_slim(&read(ptr, len)) {
        Ok(subs) => {
            let n = subs.len() as i32;
            REFERENCE.with(|r| *r.borrow_mut() = Some(MemReference::new(subs)));
            n
        }
        Err(_) => -1,
    }
}

/// # Safety
/// See [`read`].
#[no_mangle]
pub unsafe extern "C" fn load_corpus(ptr: *const u8, len: usize) -> i32 {
    match knowledge::load_str(&read(ptr, len)) {
        Ok(index) => {
            let n = index.len() as i32;
            CORPUS.with(|c| *c.borrow_mut() = Some(index));
            n
        }
        Err(_) => -1,
    }
}

/// # Safety
/// See [`read`].
#[no_mangle]
pub unsafe extern "C" fn call(ptr: *const u8, len: usize) -> *mut u8 {
    let out = match serde_json::from_str::<Value>(&read(ptr, len)) {
        Ok(req) => match run(req["cmd"].as_str().unwrap_or_default(), &req["args"]) {
            Ok(v) => json!({ "ok": v }),
            Err(e) => json!({ "err": e }),
        },
        Err(e) => json!({ "err": e.to_string() }),
    };
    let bytes = out.to_string().into_bytes();
    // `into_boxed_slice` makes capacity == len, so `dealloc(ptr, len)` frees it.
    let bytes = Box::leak(bytes.into_boxed_slice());
    RESULT_LEN.with(|l| l.set(bytes.len()));
    bytes.as_mut_ptr()
}

#[derive(Deserialize)]
struct DoseIn {
    substance_name: String,
    #[serde(default)]
    route: String,
    /// When it was taken, in minutes since the epoch; the host parses the time.
    at_min: Option<f64>,
}

/// Answer one command. Names and shapes match the desktop's, so the phone's UI
/// can't tell which side answered.
pub fn run(cmd: &str, args: &Value) -> Result<Value, String> {
    let names = || -> Vec<String> { serde_json::from_value(args["names"].clone()).unwrap_or_default() };
    let to = |v: Result<Value, serde_json::Error>| v.map_err(|e| e.to_string());
    match cmd {
        // With `doses`, timed like a session; see the desktop's `check_combo`.
        "check_combo" if args["doses"].as_array().is_some_and(|d| !d.is_empty()) => run("session_warnings", args),
        "check_combo" => with_reference(|r| to(serde_json::to_value(check::combo_warnings(r, &names())))),
        "session_warnings" => with_reference(|r| {
            let doses: Vec<DoseIn> = serde_json::from_value(args["doses"].clone()).map_err(|e| e.to_string())?;
            let doses: Vec<TimedDose> = doses
                .into_iter()
                .map(|d| TimedDose { name: d.substance_name, route: d.route, at_min: d.at_min })
                .collect();
            to(serde_json::to_value(check::session_warnings(r, &doses)))
        }),
        "pw_lookup" => with_reference(|r| to(serde_json::to_value(r.lookup(args["name"].as_str().unwrap_or_default())))),
        "pw_names" => with_reference(|r| {
            Ok(Value::Array(r.names().into_iter().map(|(name, aliases)| json!({ "name": name, "aliases": aliases })).collect()))
        }),
        // Names the offline checker knows nothing about: not in the reference and
        // in no built-in class. The phone says so rather than implying "no warnings".
        "unknown_names" => with_reference(|r| {
            Ok(json!(names().into_iter().filter(|n| r.lookup(n).is_none() && r.classes(n).is_empty()).collect::<Vec<_>>()))
        }),
        "knowledge_search" => CORPUS.with(|c| match c.borrow().as_ref() {
            Some(index) => {
                let limit = args["limit"].as_u64().unwrap_or(5) as usize;
                to(serde_json::to_value(index.search(args["query"].as_str().unwrap_or_default(), limit)))
            }
            None => Err("The passages aren't saved on this phone yet.".into()),
        }),
        _ => Err(format!("`{cmd}` isn't available offline.")),
    }
}

fn with_reference(f: impl FnOnce(&MemReference) -> Result<Value, String>) -> Result<Value, String> {
    REFERENCE.with(|r| match r.borrow().as_ref() {
        Some(r) => f(r),
        None => Err("The dose reference isn't saved on this phone yet.".into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loaded() {
        let raw = include_str!("../../resources/dosewiki.json");
        assert!(unsafe { load_reference(raw.as_ptr(), raw.len()) } > 500);
    }

    #[test]
    fn answers_like_the_desktop() {
        loaded();
        let w = run("check_combo", &json!({ "names": ["MDMA", "Tramadol"] })).unwrap();
        assert!(w.as_array().unwrap().iter().any(|w| w["severity"] == "danger"));
        assert_eq!(run("pw_lookup", &json!({ "name": "molly" })).unwrap()["name"], "MDMA");
        assert_eq!(run("unknown_names", &json!({ "names": ["MDMA", "zzz"] })).unwrap(), json!(["zzz"]));
        assert!(run("delete_experience", &json!({})).is_err());
    }

    #[test]
    fn a_call_round_trips_through_memory() {
        loaded();
        let req = br#"{"cmd":"pw_lookup","args":{"name":"LSD"}}"#;
        let p = unsafe { call(req.as_ptr(), req.len()) };
        let l = result_len();
        let out: Value = serde_json::from_str(&unsafe { read(p, l) }).unwrap();
        unsafe { dealloc(p, l) };
        assert_eq!(out["ok"]["name"], "LSD");
    }
}
