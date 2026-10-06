// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! The parts of Field Notes that must answer the same wherever they run: the
//! bundled dose reference, the combination checker, and the corpus search. The app
//! uses them directly; `../wasm` compiles them for the phone, so offline answers
//! come from this same code. Nothing here may depend on Tauri, the database or the
//! clock.

pub mod check;
pub mod interactions;
pub mod knowledge;
pub mod names;
pub mod profiles;
pub mod pw;
