// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! The owner pairing and un-pairing their own devices from a phone (owner's
//! request, 2026-10-03). A paired phone already opens the journal, so what this
//! guards is *more* access: a new device, or locking out an old one. Someone who
//! picks up an unlocked phone shouldn't be able to give themselves a key of their
//! own, so each of those asks for something only the owner knows:
//!
//! - **The journal's password**, if the journal is encrypted. It's checked by
//!   opening the journal file afresh with it; it's never stored.
//! - Otherwise **a phone PIN** the owner sets in Settings on the computer, kept
//!   as a salted, stretched hash in `phone_pin.json` next to the journal. With
//!   neither, pairing from a phone stays off.
//!
//! Wrong answers are slowed down: after [`MAX_TRIES`] in a row, nothing is
//! checked for [`LOCKOUT`]. That holds across devices, since it's the computer
//! counting.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const MAX_TRIES: u32 = 5;
pub const LOCKOUT: Duration = Duration::from_secs(15 * 60);
pub const MIN_PIN_LEN: usize = 6;
/// Rounds of SHA-256 over the PIN. A PIN is short, so the hash is made slow.
const ROUNDS: u32 = if cfg!(test) { 1_000 } else { 200_000 };

/// What the phone should ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    /// The journal's encryption password.
    Password,
    /// The phone PIN set at the computer.
    Pin,
    /// Neither exists: pairing from a phone is off.
    Off,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    salt: String,
    hash: String,
}

pub struct OwnerAuth {
    path: PathBuf,
    tries: Mutex<(u32, Option<Instant>)>,
}

pub(crate) fn stretch(salt: &str, pin: &str) -> String {
    let mut h: [u8; 32] = Sha256::digest(format!("{salt}:{pin}").as_bytes()).into();
    for _ in 0..ROUNDS {
        h = Sha256::digest(h).into();
    }
    h.iter().map(|b| format!("{b:02x}")).collect()
}

/// Does `password` open the journal at `path`? A fresh connection, so the open
/// journal is untouched.
fn opens(path: &Path, password: &str) -> bool {
    let Ok(conn) = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) else {
        return false;
    };
    conn.pragma_update(None, "key", password).is_ok()
        && conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)).is_ok()
}

impl OwnerAuth {
    pub fn new(path: PathBuf) -> Self {
        OwnerAuth { path, tries: Mutex::new((0, None)) }
    }

    fn stored(&self) -> Option<Stored> {
        std::fs::read_to_string(&self.path).ok().and_then(|s| serde_json::from_str(&s).ok())
    }

    pub fn has_pin(&self) -> bool {
        self.stored().is_some()
    }

    pub fn method(&self, journal: &Path) -> Method {
        if crate::db::is_encrypted(journal) {
            Method::Password
        } else if self.has_pin() {
            Method::Pin
        } else {
            Method::Off
        }
    }

    /// Set the phone PIN, or remove it with `None`. At the computer only.
    pub fn set_pin(&self, pin: Option<&str>) -> Result<(), String> {
        match pin {
            None => match std::fs::remove_file(&self.path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            },
            Some(pin) => {
                let pin = pin.trim();
                if pin.chars().count() < MIN_PIN_LEN {
                    return Err(format!("Use at least {MIN_PIN_LEN} characters."));
                }
                let salt = crate::devices::new_token()?;
                let stored = Stored { hash: stretch(&salt, pin), salt };
                if let Some(dir) = self.path.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
                }
                std::fs::write(&self.path, serde_json::to_vec(&stored).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())
            }
        }
    }

    /// Check what the phone sent against the journal's password or the PIN.
    pub fn verify(&self, journal: &Path, given: &str) -> Result<(), String> {
        let mut tries = self.tries.lock().unwrap();
        if let Some(until) = tries.1 {
            if Instant::now() < until {
                let mins = (until - Instant::now()).as_secs().div_ceil(60);
                return Err(format!("Too many wrong tries. Try again in {mins} minute{}.", if mins == 1 { "" } else { "s" }));
            }
            *tries = (0, None);
        }
        let ok = match self.method(journal) {
            Method::Password => opens(journal, given),
            Method::Pin => self
                .stored()
                .is_some_and(|s| crate::devices::constant_time_eq(&s.hash, &stretch(&s.salt, given.trim()))),
            Method::Off => {
                return Err(
                    "Pairing from a phone is off. Set a phone PIN in Settings on your computer, or encrypt your journal, to turn it on."
                        .into(),
                )
            }
        };
        if ok {
            *tries = (0, None);
            return Ok(());
        }
        tries.0 += 1;
        if tries.0 >= MAX_TRIES {
            tries.1 = Some(Instant::now() + LOCKOUT);
            return Err("That's not it, and that was the last try for now. Try again in 15 minutes.".into());
        }
        let what = if self.method(journal) == Method::Password { "password" } else { "PIN" };
        Err(format!("That's not your {what}."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("fn-owner-auth-{}", crate::devices::new_token().unwrap()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn without_a_password_or_pin_it_is_off() {
        let d = dir();
        let auth = OwnerAuth::new(d.join("phone_pin.json"));
        assert_eq!(auth.method(&d.join("journal.db")), Method::Off);
        assert!(auth.verify(&d.join("journal.db"), "anything").is_err());
    }

    #[test]
    fn a_pin_is_checked_and_never_stored_as_is() {
        let d = dir();
        let auth = OwnerAuth::new(d.join("phone_pin.json"));
        assert!(auth.set_pin(Some("123")).is_err(), "too short");
        auth.set_pin(Some("482913")).unwrap();
        assert!(!std::fs::read_to_string(d.join("phone_pin.json")).unwrap().contains("482913"));
        assert_eq!(auth.method(&d.join("journal.db")), Method::Pin);
        assert!(auth.verify(&d.join("journal.db"), "000000").is_err());
        assert!(auth.verify(&d.join("journal.db"), "482913").is_ok());
        auth.set_pin(None).unwrap();
        assert_eq!(auth.method(&d.join("journal.db")), Method::Off);
    }

    #[test]
    fn an_encrypted_journal_asks_for_its_password() {
        let d = dir();
        let path = d.join("journal.db");
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.pragma_update(None, "key", "correct horse battery").unwrap();
        conn.execute_batch("CREATE TABLE t (x INTEGER);").unwrap();
        let auth = OwnerAuth::new(d.join("phone_pin.json"));
        assert_eq!(auth.method(&path), Method::Password);
        assert!(auth.verify(&path, "wrong").is_err());
        assert!(auth.verify(&path, "correct horse battery").is_ok());
        drop(conn);
    }

    #[test]
    fn wrong_answers_lock_it_for_a_while() {
        let d = dir();
        let auth = OwnerAuth::new(d.join("phone_pin.json"));
        auth.set_pin(Some("482913")).unwrap();
        let j = d.join("journal.db");
        for _ in 0..MAX_TRIES {
            assert!(auth.verify(&j, "000000").is_err());
        }
        let e = auth.verify(&j, "482913").unwrap_err();
        assert!(e.contains("Too many"), "{e}");
    }
}
