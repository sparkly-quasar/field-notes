// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! One-shot check of the in-app "switch to the recommended model" button.
//!
//! Deliberately **not** a `cargo test`: it downloads several gigabytes and then
//! deletes a model from the machine it runs on. Neither belongs in a suite that
//! people run without thinking. But the thing it exercises is the one piece of
//! this release that reaches into a user's machine and removes something, so it
//! should not ship having only been reasoned about.
//!
//! What it pins is the ordering. `switch_model` must download the new model
//! *before* removing the old one — the reverse would leave someone with no
//! Companion at all if the download failed, possibly mid-session, which is
//! exactly when they'd want it.
//!
//!   cargo run --example switch_model_check -- llama3.1:8b
//!
//! Requires Ollama running. Pass the tag to migrate away from.

use field_notes_lib::ollama;

fn main() {
    let from = std::env::args().nth(1).unwrap_or_else(|| "llama3.1:8b".to_string());
    let to = ollama::PREFERRED_MODEL;

    if !ollama::api_up() {
        eprintln!("Ollama isn't running — start it and try again.");
        std::process::exit(1);
    }

    let before = ollama::list_models();
    println!("before: {before:?}");
    if !before.iter().any(|m| m == &from) {
        eprintln!(
            "{from} isn't installed, so there's no migration to test. \
             `ollama pull {from}` first."
        );
        std::process::exit(1);
    }

    // A mock runtime gives a real AppHandle without standing up a window, so the
    // progress events `switch_model` emits have somewhere to go.
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("failed to build mock app");

    println!("switching {from} -> {to} …");
    match ollama::switch_model(app.handle(), &from) {
        Ok(()) => println!("switch_model returned Ok"),
        Err(e) => {
            eprintln!("switch_model failed: {e}");
            // Not an immediate exit: the whole point is to check what state the
            // machine was left in on failure, which is where the danger lives.
        }
    }

    let after = ollama::list_models();
    println!("after: {after:?}");

    let got_new = after.iter().any(|m| m == to);
    let dropped_old = !after.iter().any(|m| m == &from);

    println!("\n  {to} present: {got_new}");
    println!("  {from} removed: {dropped_old}");

    // The failure that matters is being left with neither. Everything else is
    // recoverable by pulling again.
    if !got_new {
        eprintln!("\nFAIL: {to} is not installed. Did the download fail?");
        if dropped_old {
            eprintln!("WORSE: {from} was removed anyway — this machine now has no Companion.");
        }
        std::process::exit(1);
    }
    if !dropped_old {
        println!("\nPARTIAL: {to} installed but {from} is still here.");
        println!("Non-fatal by design — the Companion works; this is only disk space.");
        return;
    }
    println!("\nPASS: downloaded {to}, then removed {from}.");
}
