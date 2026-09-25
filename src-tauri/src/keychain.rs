// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Remembering the journal passphrase in the operating system's keychain — **opt-in**,
//! for a computer that serves the journal to other devices and has to come back
//! on its own after a reboot.
//!
//! This is a real trade, and the Settings copy says so: with the passphrase in the
//! keychain, anyone who can log in to this computer's user account can open the
//! journal. Encryption at rest still protects a pulled disk or a copied file; it no
//! longer protects against the logged-in account. Off unless the user turns it on.
//!
//! macOS Keychain, Windows Credential Manager, or the Secret Service on Linux.
//! Nothing here is ever reachable from the portal (see `portal.rs` rule 4).

use keyring::Entry;

const SERVICE: &str = "Field Notes";
const ACCOUNT: &str = "journal-passphrase";

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE, ACCOUNT).map_err(|e| format!("The system keychain isn't available: {e}"))
}

/// The remembered passphrase, if there is one. Any keychain error reads as "none" —
/// the fallback is the ordinary unlock screen, which is always safe.
pub fn get() -> Option<String> {
    entry().ok()?.get_password().ok().filter(|p| !p.is_empty())
}

pub fn set(passphrase: &str) -> Result<(), String> {
    entry()?.set_password(passphrase).map_err(|e| format!("Couldn't save to the keychain: {e}"))
}

/// Forget it. Forgetting something that was never remembered is fine.
pub fn forget() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("Couldn't remove it from the keychain: {e}")),
    }
}

pub fn remembered() -> bool {
    get().is_some()
}
