// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! The devices allowed to reach this journal through the portal — a phone, a
//! laptop using this computer as its server — each with its own token.
//!
//! Before this, the portal had one token that lived in memory and died with the
//! portal, so every restart meant re-pairing everything. A computer that is meant
//! to *be* the server can't work like that: it reboots, and the laptop that stores
//! its journal there has to come back on its own.
//!
//! ## What is stored, and why here
//!
//! `devices.json`, next to the journal. It holds a **SHA-256 of each token, never
//! the token itself** — the token is shown once, as a QR code or a link, and then
//! only the device has it. A copy of this file is therefore not a key to anything.
//!
//! It deliberately lives *outside* the encrypted journal: rule 2 in `portal.rs` says
//! the token is checked before anything else, including whether the journal is
//! locked. With the registry inside SQLCipher, a locked journal couldn't tell a
//! paired device "locked" from a stranger — and would have to answer strangers
//! first. A list of device names and hashes is not journal data.
//!
//! Every device is separately revocable. Losing a phone costs you that phone's
//! pairing, not everyone's.
//!
//! ## Whose device
//!
//! Each device belongs to one person (`people.rs`), and its token opens that
//! person's journal and nothing else. The person comes **only** from here: no
//! request ever names one. Devices paired before people existed belong to the
//! owner, person 1.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// How stale `last_seen` may get in memory before we bother writing it to disk.
/// It's a "when did I last hear from this?" hint, not an audit log.
const TOUCH_WRITE_EVERY_SECS: u64 = 60;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Device {
    pub id: u64,
    pub name: String,
    /// Hex SHA-256 of the token. Never sent to the UI.
    #[serde(rename = "token_sha256")]
    hash: String,
    /// Unix seconds.
    pub created_at: u64,
    /// Unix seconds of the last accepted request, if any.
    pub last_seen: Option<u64>,
    /// The person whose journal this device opens. Absent in older files: the owner.
    #[serde(default = "owner")]
    pub person: u32,
}

/// The person who installed Field Notes. Their journal is `journal.db`.
pub const OWNER: u32 = 1;
fn owner() -> u32 {
    OWNER
}

/// What the UI is allowed to see about a device: everything but the hash.
#[derive(Clone, Debug, Serialize)]
pub struct DeviceInfo {
    pub id: u64,
    pub name: String,
    pub created_at: u64,
    pub last_seen: Option<u64>,
    pub person: u32,
}

impl From<&Device> for DeviceInfo {
    fn from(d: &Device) -> Self {
        DeviceInfo { id: d.id, name: d.name.clone(), created_at: d.created_at, last_seen: d.last_seen, person: d.person }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Registry {
    next_id: u64,
    devices: Vec<Device>,
}

pub struct Devices {
    path: PathBuf,
    inner: Mutex<Registry>,
    /// When `last_seen` was last flushed to disk, per the throttle above.
    last_write: Mutex<u64>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn hash(token: &str) -> String {
    Sha256::digest(token.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Compare without leaking where the mismatch was. `==` on a `String` short-circuits
/// on the first differing byte, which over enough requests tells an attacker the
/// value one byte at a time.
pub fn constant_time_eq(expected: &str, given: &str) -> bool {
    let (a, b) = (expected.as_bytes(), given.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// 256 bits of OS randomness, hex. Not a password — never typed, only scanned or pasted.
pub fn new_token() -> Result<String, String> {
    let mut buf = [0u8; 32];
    getrandom::getrandom(&mut buf).map_err(|e| format!("no secure randomness available: {e}"))?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

impl Devices {
    /// Load the registry at `path`. A missing file is an empty registry; an
    /// unreadable one is also treated as empty — the cost is re-pairing, which is
    /// the safe direction to fail in.
    pub fn load(path: PathBuf) -> Self {
        let reg = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Registry>(&s).ok())
            .unwrap_or_default();
        Devices { path, inner: Mutex::new(reg), last_write: Mutex::new(0) }
    }

    fn save(&self, reg: &Registry) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(reg).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }

    /// Pair a new device. Returns it with its token — the **only** time the token
    /// exists outside the device itself.
    pub fn pair(&self, name: &str) -> Result<(DeviceInfo, String), String> {
        self.pair_for(name, OWNER)
    }

    /// Pair a device that opens `person`'s journal.
    pub fn pair_for(&self, name: &str, person: u32) -> Result<(DeviceInfo, String), String> {
        let name = name.trim();
        let name = if name.is_empty() { "Unnamed device" } else { name };
        let token = new_token()?;
        let mut reg = self.inner.lock().unwrap();
        reg.next_id += 1;
        let device = Device {
            id: reg.next_id,
            name: name.chars().take(60).collect(),
            hash: hash(&token),
            created_at: now(),
            last_seen: None,
            person,
        };
        let info = DeviceInfo::from(&device);
        reg.devices.push(device);
        self.save(&reg)?;
        Ok((info, token))
    }

    pub fn list(&self) -> Vec<DeviceInfo> {
        self.inner.lock().unwrap().devices.iter().map(DeviceInfo::from).collect()
    }

    /// Forget a device. Its token stops working on the very next request.
    pub fn revoke(&self, id: u64) -> Result<(), String> {
        let mut reg = self.inner.lock().unwrap();
        reg.devices.retain(|d| d.id != id);
        self.save(&reg)
    }

    /// Forget `id`, but only if it belongs to `person`: a person managing their own
    /// devices from a phone can never reach anyone else's.
    pub fn revoke_own(&self, person: u32, id: u64) -> Result<(), String> {
        let mut reg = self.inner.lock().unwrap();
        let before = reg.devices.len();
        reg.devices.retain(|d| !(d.id == id && d.person == person));
        if reg.devices.len() == before {
            return Err("No such device.".into());
        }
        self.save(&reg)
    }

    /// Forget every device of one person, when that person is removed.
    pub fn revoke_person(&self, person: u32) -> Result<(), String> {
        let mut reg = self.inner.lock().unwrap();
        reg.devices.retain(|d| d.person != person);
        self.save(&reg)
    }

    /// Forget every device — used when the journal itself is wiped.
    pub fn revoke_all(&self) -> Result<(), String> {
        let mut reg = self.inner.lock().unwrap();
        reg.devices.clear();
        self.save(&reg)
    }

    /// Which device, if any, holds this token. Checks every entry rather than
    /// stopping at the first match, so timing says nothing about *which* device a
    /// near-miss resembled. Records `last_seen` on a match.
    pub fn verify(&self, token: &str) -> Option<DeviceInfo> {
        if token.is_empty() {
            return None;
        }
        let given = hash(token);
        let mut reg = self.inner.lock().unwrap();
        let mut found = None;
        for (i, d) in reg.devices.iter().enumerate() {
            if constant_time_eq(&d.hash, &given) {
                found = Some(i);
            }
        }
        let i = found?;
        let t = now();
        reg.devices[i].last_seen = Some(t);
        let info = DeviceInfo::from(&reg.devices[i]);

        let mut last = self.last_write.lock().unwrap();
        if t.saturating_sub(*last) >= TOUCH_WRITE_EVERY_SECS {
            *last = t;
            let _ = self.save(&reg);
        }
        Some(info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_registry(tag: &str) -> Devices {
        let dir = std::env::temp_dir().join(format!("fn-devices-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("devices.json");
        let _ = std::fs::remove_file(&path);
        Devices::load(path)
    }

    #[test]
    fn a_paired_device_is_recognised_and_a_stranger_is_not() {
        let reg = temp_registry("recognise");
        let (laptop, token) = reg.pair("Laptop").unwrap();
        assert_eq!(reg.verify(&token).map(|d| d.id), Some(laptop.id));
        assert!(reg.verify(&"0".repeat(64)).is_none());
        assert!(reg.verify("").is_none());
    }

    #[test]
    fn pairings_survive_a_restart() {
        let reg = temp_registry("restart");
        let (_, token) = reg.pair("Phone").unwrap();
        let reloaded = Devices::load(reg.path.clone());
        assert!(reloaded.verify(&token).is_some(), "a reboot must not un-pair the laptop");
    }

    #[test]
    fn the_file_never_holds_a_token() {
        let reg = temp_registry("nohash");
        let (_, token) = reg.pair("Phone").unwrap();
        let on_disk = std::fs::read_to_string(&reg.path).unwrap();
        assert!(!on_disk.contains(&token), "only the hash may be written down");
    }

    #[test]
    fn revoking_one_device_leaves_the_others_paired() {
        let reg = temp_registry("revoke");
        let (phone, phone_token) = reg.pair("Phone").unwrap();
        let (_, laptop_token) = reg.pair("Laptop").unwrap();
        reg.revoke(phone.id).unwrap();
        assert!(reg.verify(&phone_token).is_none());
        assert!(reg.verify(&laptop_token).is_some());
        // And it stays revoked across a reload.
        assert!(Devices::load(reg.path.clone()).verify(&phone_token).is_none());
    }

    #[test]
    fn tokens_are_long_and_not_repeated() {
        let a = new_token().unwrap();
        let b = new_token().unwrap();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }

    #[test]
    fn constant_time_eq_is_still_equality() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "ab"));
    }
}
