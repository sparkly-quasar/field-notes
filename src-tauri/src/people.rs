// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! More than one person on one server, each with a journal only they can open.
//!
//! The owner (person 1, `devices::OWNER`) is whoever installed Field Notes: their
//! journal is `journal.db` and nothing about it changes here. Everyone else is
//! listed in `people.json` and has their own folder, `people/<id>/`, holding their
//! own `journal.db`. Separate files rather than a person column in shared tables,
//! so a missed `WHERE` can't leak one person's entries into another's.
//!
//! ## The privacy rules
//!
//! 1. **The person comes from the device, never the request.** `devices.json` says
//!    whose device a token is; no portal command takes a person. A phone has no way
//!    to ask for someone else's journal.
//! 2. **Another person's journal is always encrypted, with a password they choose
//!    on their own device.** The server keeps the opened connection in memory and
//!    never writes the password anywhere, unless that person turns on "keep my
//!    journal unlocked on this server" (`keychain::set_person`). A restart forgets
//!    it and the journal locks again.
//! 3. **The owner can't read it.** The desktop app shows names, device counts and
//!    locked or unlocked, never entries, and there's no command that opens another
//!    person's journal without their password.
//! 4. **Wrong passwords are slowed down,** per device, so a phone can't be used to
//!    guess.
//! 5. **A PIN is a shortcut on one device, never a key on the server.** A person can
//!    set a short PIN on a device that already knows their password. The server
//!    seals the password under a fresh random key and hands the sealed copy to that
//!    device, keeping only the key and a stretched hash of the PIN (`pins.json` in
//!    their folder). Neither half opens anything alone: the owner, holding the
//!    server's files, has no sealed copy; someone holding the phone has no key, and
//!    the server forgets the key after [`PIN_TRIES`] wrong PINs. So the password
//!    stays the only thing that can be guessed offline, and it stays long.
//!
//! The limit, said plainly in the app too: while a journal is unlocked it is open in
//! this program's memory, so someone with full control of the computer and the
//! skill to inspect a running process could reach it.

use crate::{db, devices::OWNER, keychain, Db};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The shortest password a person may choose. Their journal can't be recovered
/// without it, and nobody else can reset it, so it's worth a real one.
pub const MIN_PASSWORD: usize = 8;
/// Wrong passwords allowed from one device before it has to wait.
const FREE_TRIES: u32 = 5;
/// A PIN is digits, this many or more (and at most [`MAX_PIN`]).
pub const MIN_PIN: usize = 4;
pub const MAX_PIN: usize = 12;
/// Wrong PINs allowed on one device before the server forgets that device's PIN
/// and only the password will do. Counted on disk, so a restart doesn't reset it.
pub const PIN_TRIES: u32 = 5;

/// One device's PIN (rule 5). `key` unseals the copy of the password that only that
/// device holds; on its own it opens nothing.
#[derive(Clone, Serialize, Deserialize)]
struct Pin {
    salt: String,
    hash: String,
    key: String,
    #[serde(default)]
    fails: u32,
}

#[derive(Clone, Serialize, Deserialize)]
struct Person {
    id: u32,
    name: String,
    created_at: u64,
    /// The password is kept in the keychain so the journal reopens after a restart.
    #[serde(default)]
    remembered: bool,
}

#[derive(Default, Serialize, Deserialize)]
struct Registry {
    next_id: u32,
    people: Vec<Person>,
}

/// What the owner's Settings may see about a person: never entries.
#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct PersonInfo {
    pub id: u32,
    pub name: String,
    pub created_at: u64,
    /// They've chosen a password and their journal exists.
    pub has_journal: bool,
    pub unlocked: bool,
    pub remembered: bool,
}

/// `db_status` for another person's device.
#[derive(Serialize)]
pub struct PersonStatus {
    pub encrypted: bool,
    pub unlocked: bool,
    pub opening: bool,
    pub error: Option<String>,
    /// No journal yet: the phone asks them to choose a password.
    pub new_journal: bool,
    pub person_name: String,
    pub remembered: bool,
    /// The device asking has a PIN it can unlock with.
    pub pin: bool,
}

struct Tries {
    fails: u32,
    until: Option<Instant>,
}

pub struct People {
    file: PathBuf,
    dir: PathBuf,
    reg: Mutex<Registry>,
    dbs: Mutex<HashMap<u32, Arc<Db>>>,
    tries: Mutex<HashMap<u64, Tries>>,
    /// Held while `pins.json` is read and rewritten.
    pins: Mutex<()>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl People {
    /// `dir` is the app's data folder, beside the owner's `journal.db`.
    pub fn load(dir: &Path) -> Self {
        let file = dir.join("people.json");
        let reg = std::fs::read_to_string(&file)
            .ok()
            .and_then(|s| serde_json::from_str::<Registry>(&s).ok())
            .unwrap_or_default();
        People {
            file,
            dir: dir.to_path_buf(),
            reg: Mutex::new(reg),
            dbs: Mutex::new(HashMap::new()),
            tries: Mutex::new(HashMap::new()),
            pins: Mutex::new(()),
        }
    }

    fn save(&self, reg: &Registry) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let tmp = self.file.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(reg).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.file).map_err(|e| e.to_string())
    }

    fn folder(&self, id: u32) -> PathBuf {
        self.dir.join("people").join(id.to_string())
    }

    pub fn journal_path(&self, id: u32) -> PathBuf {
        self.folder(id).join("journal.db")
    }

    /// A person other than the owner, who is still on the list.
    pub fn exists(&self, id: u32) -> bool {
        id != OWNER && self.reg.lock().unwrap().people.iter().any(|p| p.id == id)
    }

    pub fn name(&self, id: u32) -> Option<String> {
        self.reg.lock().unwrap().people.iter().find(|p| p.id == id).map(|p| p.name.clone())
    }

    fn info(&self, p: &Person) -> PersonInfo {
        PersonInfo {
            id: p.id,
            name: p.name.clone(),
            created_at: p.created_at,
            has_journal: self.journal_path(p.id).exists(),
            unlocked: self.dbs.lock().unwrap().get(&p.id).is_some_and(|d| d.is_unlocked()),
            remembered: p.remembered,
        }
    }

    pub fn list(&self) -> Vec<PersonInfo> {
        let people = self.reg.lock().unwrap().people.clone();
        people.iter().map(|p| self.info(p)).collect()
    }

    /// Add a person. Their journal doesn't exist until they choose a password.
    pub fn add(&self, name: &str) -> Result<PersonInfo, String> {
        let name: String = name.trim().chars().take(40).collect();
        if name.is_empty() {
            return Err("Give them a name you'll recognise.".into());
        }
        let mut reg = self.reg.lock().unwrap();
        if reg.people.iter().any(|p| p.name.eq_ignore_ascii_case(&name)) {
            return Err(format!("There's already someone called {name}."));
        }
        reg.next_id = reg.next_id.max(OWNER) + 1;
        let p = Person { id: reg.next_id, name, created_at: now(), remembered: false };
        reg.people.push(p.clone());
        self.save(&reg)?;
        drop(reg);
        Ok(self.info(&p))
    }

    /// Remove a person and delete their journal. The caller un-pairs their devices.
    pub fn remove(&self, id: u32) -> Result<(), String> {
        if id == OWNER {
            return Err("The owner can't be removed.".into());
        }
        let mut reg = self.reg.lock().unwrap();
        if !reg.people.iter().any(|p| p.id == id) {
            return Err("No such person.".into());
        }
        // Close their journal before deleting the file under it.
        if let Some(db) = self.dbs.lock().unwrap().remove(&id) {
            *db.conn.lock().unwrap() = None;
        }
        let folder = self.folder(id);
        if folder.exists() {
            std::fs::remove_dir_all(&folder).map_err(|e| format!("Couldn't delete their journal: {e}"))?;
        }
        let _ = keychain::forget_person(id);
        reg.people.retain(|p| p.id != id);
        self.save(&reg)
    }

    /// Remove everyone: used when the whole server's data is erased.
    pub fn remove_all(&self) {
        let ids: Vec<u32> = self.reg.lock().unwrap().people.iter().map(|p| p.id).collect();
        for id in ids {
            let _ = self.remove(id);
        }
        let _ = std::fs::remove_dir_all(self.dir.join("people"));
    }

    /// That person's journal, open or locked. `None` for the owner (use the app's
    /// `Db`) and for anyone not on the list.
    pub fn db(&self, id: u32) -> Option<Arc<Db>> {
        if !self.exists(id) {
            return None;
        }
        let mut dbs = self.dbs.lock().unwrap();
        Some(dbs.entry(id).or_insert_with(|| Arc::new(Db::new(None, self.journal_path(id)))).clone())
    }

    /// `device` is the one asking, for whether it has a PIN.
    pub fn status(&self, id: u32, device: u64) -> Option<PersonStatus> {
        let db = self.db(id)?;
        let reg = self.reg.lock().unwrap();
        let p = reg.people.iter().find(|p| p.id == id)?;
        Some(PersonStatus {
            encrypted: true,
            unlocked: db.is_unlocked(),
            opening: false,
            error: None,
            new_journal: !db.path.exists(),
            person_name: p.name.clone(),
            remembered: p.remembered,
            pin: self.read_pins(id).contains_key(&device),
        })
    }

    /// How long this device must wait before another try, if at all.
    fn wait(&self, device: u64) -> Option<Duration> {
        let tries = self.tries.lock().unwrap();
        let t = tries.get(&device)?;
        t.until.and_then(|u| u.checked_duration_since(Instant::now()))
    }

    fn failed(&self, device: u64) {
        let mut tries = self.tries.lock().unwrap();
        let t = tries.entry(device).or_insert(Tries { fails: 0, until: None });
        t.fails += 1;
        if t.fails >= FREE_TRIES {
            // 30 s, then doubling, capped at an hour.
            let secs = 30u64.saturating_mul(1 << (t.fails - FREE_TRIES).min(7)).min(3600);
            t.until = Some(Instant::now() + Duration::from_secs(secs));
        }
    }

    /// Open (or, the first time, create) this person's journal with their password.
    /// Returns the journal so the caller can load the dose reference into it.
    pub fn unlock(&self, id: u32, device: u64, password: &str) -> Result<Arc<Db>, String> {
        let db = self.db(id).ok_or("No such person.")?;
        if db.is_unlocked() {
            return Ok(db);
        }
        if let Some(w) = self.wait(device) {
            return Err(format!("Too many wrong passwords. Try again in {} seconds.", w.as_secs().max(1)));
        }
        let conn = if db.path.exists() {
            match db::open(&db.path, Some(password)) {
                Ok(c) => c,
                Err(_) => {
                    self.failed(device);
                    return Err("That password didn't open your journal.".into());
                }
            }
        } else {
            if password.chars().count() < MIN_PASSWORD {
                return Err(format!("Choose a password of at least {MIN_PASSWORD} characters."));
            }
            if let Some(dir) = db.path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            db::open(&db.path, Some(password)).map_err(|e| format!("Couldn't create your journal: {e}"))?
        };
        self.tries.lock().unwrap().remove(&device);
        let mut guard = db.conn.lock().unwrap();
        if guard.is_none() {
            *guard = Some(conn);
        }
        drop(guard);
        Ok(db)
    }

    /// Keep (or stop keeping) this person's password in the keychain, so their
    /// journal reopens by itself after a restart. Only from their own device.
    pub fn set_remember(&self, id: u32, device: u64, remember: bool, password: Option<&str>) -> Result<(), String> {
        let db = self.db(id).ok_or("No such person.")?;
        if remember {
            let pw = password.filter(|p| !p.is_empty()).ok_or("Type your password to keep it on the server.")?;
            if let Some(w) = self.wait(device) {
                return Err(format!("Too many wrong passwords. Try again in {} seconds.", w.as_secs().max(1)));
            }
            // Prove it's the right one before storing it.
            if db::open(&db.path, Some(pw)).is_err() {
                self.failed(device);
                return Err("That password didn't open your journal.".into());
            }
            keychain::set_person(id, pw)?;
        } else {
            keychain::forget_person(id)?;
        }
        let mut reg = self.reg.lock().unwrap();
        if let Some(p) = reg.people.iter_mut().find(|p| p.id == id) {
            p.remembered = remember;
        }
        self.save(&reg)
    }

    /// Change this person's password, from their own device. Their journal is
    /// re-encrypted under the new one. Needs the current password even though the
    /// journal is open: an unlocked phone left on a table shouldn't be enough to
    /// lock its owner out.
    pub fn change_password(&self, id: u32, device: u64, current: &str, new: &str) -> Result<(), String> {
        let db = self.db(id).ok_or("No such person.")?;
        if let Some(w) = self.wait(device) {
            return Err(format!("Too many wrong passwords. Try again in {} seconds.", w.as_secs().max(1)));
        }
        if new.chars().count() < MIN_PASSWORD {
            return Err(format!("Choose a new password of at least {MIN_PASSWORD} characters."));
        }
        if new == current {
            return Err("That's the password you already have.".into());
        }
        // Hold the connection's lock throughout, so another device of theirs waits
        // for a moment rather than finding the journal closed and showing "locked".
        let mut guard = db.conn.lock().unwrap();
        if guard.is_none() {
            return Err(Db::locked_err());
        }
        if db::open(&db.path, Some(current)).is_err() {
            self.failed(device);
            return Err("That isn't your current password.".into());
        }
        self.tries.lock().unwrap().remove(&device);
        *guard = None; // release the file so it can be rewritten
        if let Err(e) = db::convert(&db.path, Some(current), Some(new)) {
            // Not changed: reopen with the old one so they aren't left locked out.
            *guard = db::open(&db.path, Some(current)).ok();
            return Err(format!("Couldn't change your password: {e}"));
        }
        *guard = Some(db::open(&db.path, Some(new)).map_err(|e| e.to_string())?);
        drop(guard);
        // Every device's sealed copy is of the old password now. Forget their PINs
        // so they ask for the password once and can set a PIN again.
        let _ = self.write_pins(id, &HashMap::new());
        // Keep a remembered password in step, or the next restart can't open it.
        // If the keychain won't take it, stop remembering rather than keep a stale one.
        let mut reg = self.reg.lock().unwrap();
        if let Some(p) = reg.people.iter_mut().find(|p| p.id == id && p.remembered) {
            if keychain::set_person(id, new).is_err() {
                let _ = keychain::forget_person(id);
                p.remembered = false;
                self.save(&reg)?;
            }
        }
        Ok(())
    }

    // ---- a PIN on one device (rule 5) ----

    fn pins_path(&self, id: u32) -> PathBuf {
        self.folder(id).join("pins.json")
    }

    fn read_pins(&self, id: u32) -> HashMap<u64, Pin> {
        std::fs::read_to_string(self.pins_path(id))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn write_pins(&self, id: u32, pins: &HashMap<u64, Pin>) -> Result<(), String> {
        let path = self.pins_path(id);
        if pins.is_empty() {
            return match std::fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            };
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(pins).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }

    /// Set a PIN for this device, proving the password first. Returns the sealed
    /// copy of the password for the device to keep; the server never stores it.
    pub fn set_pin(&self, id: u32, device: u64, password: &str, pin: &str) -> Result<String, String> {
        let db = self.db(id).ok_or("No such person.")?;
        let pin = pin.trim();
        if pin.len() < MIN_PIN || pin.len() > MAX_PIN || !pin.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("Choose a PIN of {MIN_PIN} to {MAX_PIN} digits."));
        }
        if let Some(w) = self.wait(device) {
            return Err(format!("Too many wrong passwords. Try again in {} seconds.", w.as_secs().max(1)));
        }
        if !db.path.exists() || db::open(&db.path, Some(password)).is_err() {
            self.failed(device);
            return Err("That password didn't open your journal.".into());
        }
        self.tries.lock().unwrap().remove(&device);
        let (key, sealed) = seal(password)?;
        let salt = crate::devices::new_token()?;
        let rec = Pin { hash: crate::owner_auth::stretch(&salt, pin), salt, key, fails: 0 };
        let _g = self.pins.lock().unwrap();
        let mut pins = self.read_pins(id);
        pins.insert(device, rec);
        self.write_pins(id, &pins)?;
        Ok(sealed)
    }

    /// Turn this device's PIN off. Turning off one that isn't on is fine.
    pub fn forget_pin(&self, id: u32, device: u64) -> Result<(), String> {
        let _g = self.pins.lock().unwrap();
        let mut pins = self.read_pins(id);
        if pins.remove(&device).is_some() {
            self.write_pins(id, &pins)?;
        }
        Ok(())
    }

    /// Open this person's journal with this device's PIN and the sealed password
    /// the device kept. [`PIN_TRIES`] wrong PINs and the PIN is gone.
    pub fn unlock_with_pin(&self, id: u32, device: u64, pin: &str, sealed: &str) -> Result<Arc<Db>, String> {
        let db = self.db(id).ok_or("No such person.")?;
        if db.is_unlocked() {
            return Ok(db);
        }
        let _g = self.pins.lock().unwrap();
        let mut pins = self.read_pins(id);
        let gone = || "This device's PIN is off. Unlock with your password, then you can set a PIN again.".to_string();
        let rec = pins.get_mut(&device).ok_or_else(gone)?;
        let right = crate::devices::constant_time_eq(&rec.hash, &crate::owner_auth::stretch(&rec.salt, pin.trim()));
        if !right {
            rec.fails += 1;
            if rec.fails >= PIN_TRIES {
                pins.remove(&device);
                self.write_pins(id, &pins)?;
                return Err("That's not your PIN, and that was the last try. Unlock with your password.".into());
            }
            let left = PIN_TRIES - rec.fails;
            self.write_pins(id, &pins)?;
            return Err(format!("That's not your PIN. {left} tr{} left before it turns off.", if left == 1 { "y" } else { "ies" }));
        }
        // The right PIN, but the sealed copy must still open the journal: it won't
        // if it was tampered with or the password changed some other way.
        let conn = unseal(&rec.key, sealed).and_then(|pw| db::open(&db.path, Some(&pw)).ok());
        let Some(conn) = conn else {
            pins.remove(&device);
            self.write_pins(id, &pins)?;
            return Err(gone());
        };
        if rec.fails > 0 {
            rec.fails = 0;
            self.write_pins(id, &pins)?;
        }
        drop(_g);
        let mut guard = db.conn.lock().unwrap();
        if guard.is_none() {
            *guard = Some(conn);
        }
        drop(guard);
        Ok(db)
    }

    /// A copy of this person's journal, for them to keep: still encrypted with
    /// their password, so it's as private on their phone as it is here. It opens
    /// in Field Notes on any computer (Settings, Restore a backup) with that password.
    pub fn backup(&self, id: u32) -> Result<Vec<u8>, String> {
        let db = self.db(id).ok_or("No such person.")?;
        let mut name = [0u8; 8];
        getrandom::getrandom(&mut name).map_err(|e| e.to_string())?;
        let hex: String = name.iter().map(|b| format!("{b:02x}")).collect();
        // Beside their journal, so it never sits anywhere less private than that.
        let tmp = self.folder(id).join(format!("backup-{hex}.tmp"));
        let made = db.with(|c| db::backup_to(c, &tmp));
        let bytes = made.and_then(|_| std::fs::read(&tmp).map_err(|e| e.to_string()));
        let _ = std::fs::remove_file(&tmp);
        bytes
    }

    /// At launch: reopen every journal whose person chose to keep it unlocked here.
    /// Returns the ones that opened, for the dose reference to be loaded into.
    pub fn open_remembered(&self) -> Vec<Arc<Db>> {
        let ids: Vec<u32> = self.reg.lock().unwrap().people.iter().filter(|p| p.remembered).map(|p| p.id).collect();
        let mut opened = Vec::new();
        for id in ids {
            let Some(db) = self.db(id) else { continue };
            let Some(pw) = keychain::get_person(id) else { continue };
            if let Ok(c) = db::open(&db.path, Some(&pw)) {
                *db.conn.lock().unwrap() = Some(c);
                opened.push(db);
            }
        }
        opened
    }

    /// Whose journals are open only in memory, so a restart would lock them until
    /// they unlock them again from their own device. For the owner's update prompt,
    /// which shows only names and locked-or-unlocked: what Settings already shows (rule 3).
    pub fn would_lock(&self) -> Vec<String> {
        self.list().into_iter().filter(|p| p.unlocked && !p.remembered).map(|p| p.name).collect()
    }

    /// Who has a session that hasn't been ended, among the journals open right now.
    /// Only whether one is open: never its title or anything else in it. A locked
    /// journal can't be read, and nobody can be mid-session in one.
    pub fn in_session(&self) -> Vec<u32> {
        let open: Vec<(u32, Arc<Db>)> = self.dbs.lock().unwrap().iter().map(|(id, d)| (*id, d.clone())).collect();
        open.into_iter()
            .filter(|(_, d)| {
                d.with(|c| {
                    c.query_row(
                        "SELECT EXISTS(SELECT 1 FROM experiences WHERE kind = 'session' AND ended_at IS NULL)",
                        [],
                        |r| r.get::<_, bool>(0),
                    )
                })
                .unwrap_or(false)
            })
            .map(|(id, _)| id)
            .collect()
    }
}

/// Sealing a password for one device's PIN (rule 5). A fresh random key per seal,
/// used once, as a SHA-256 keystream; padded to a multiple of 256 bytes so the
/// sealed copy doesn't give away the password's length. A wrong or altered copy
/// unseals to something that doesn't open the journal, which is all that's checked.
fn keystream(key: &[u8], data: &mut [u8]) {
    use sha2::{Digest, Sha256};
    for (i, chunk) in data.chunks_mut(32).enumerate() {
        let block = Sha256::new().chain_update(key).chain_update((i as u64).to_le_bytes()).finalize();
        chunk.iter_mut().zip(block.iter()).for_each(|(b, k)| *b ^= k);
    }
}

/// Returns (the key, hex, for the server; the sealed password, base64, for the device).
fn seal(password: &str) -> Result<(String, String), String> {
    use base64::Engine;
    let pw = password.as_bytes();
    let len = u16::try_from(pw.len()).map_err(|_| "That password is too long to use with a PIN.")?;
    let mut buf = vec![0u8; (pw.len() + 2).div_ceil(256) * 256];
    getrandom::getrandom(&mut buf).map_err(|e| e.to_string())?;
    buf[..2].copy_from_slice(&len.to_le_bytes());
    buf[2..2 + pw.len()].copy_from_slice(pw);
    let mut key = [0u8; 32];
    getrandom::getrandom(&mut key).map_err(|e| e.to_string())?;
    keystream(&key, &mut buf);
    let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
    Ok((hex, base64::engine::general_purpose::STANDARD.encode(buf)))
}

fn unseal(key_hex: &str, sealed: &str) -> Option<String> {
    use base64::Engine;
    let key: Vec<u8> = (0..key_hex.len())
        .step_by(2)
        .map(|i| key_hex.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok()))
        .collect::<Option<_>>()?;
    let mut buf = base64::engine::general_purpose::STANDARD.decode(sealed.trim()).ok()?;
    if buf.len() < 2 {
        return None;
    }
    keystream(&key, &mut buf);
    let len = u16::from_le_bytes([buf[0], buf[1]]) as usize;
    String::from_utf8(buf.get(2..2 + len)?.to_vec()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn people(tag: &str) -> People {
        let dir = std::env::temp_dir().join(format!("fn-people-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        People::load(&dir)
    }

    #[test]
    fn a_new_person_chooses_a_password_and_only_that_password_opens_it() {
        let p = people("pw");
        let sam = p.add("Sam").unwrap();
        assert_eq!(sam.id, 2);
        assert!(!sam.has_journal && !sam.unlocked);
        assert!(p.status(2, 7).unwrap().new_journal);

        // Too short to be the only key to a journal nobody can recover.
        assert!(p.unlock(2, 7, "short").err().unwrap().contains("at least"));
        p.unlock(2, 7, "correct horse battery").unwrap();
        assert!(p.list()[0].has_journal && p.list()[0].unlocked);
        // Encrypted on disk.
        assert!(db::is_encrypted(&p.journal_path(2)));

        // Locked again (a restart), the wrong password doesn't open it.
        *p.db(2).unwrap().conn.lock().unwrap() = None;
        assert!(p.unlock(2, 7, "wrong password").is_err());
        assert!(!p.db(2).unwrap().is_unlocked());
        p.unlock(2, 7, "correct horse battery").unwrap();
        assert!(p.db(2).unwrap().is_unlocked());
    }

    #[test]
    fn wrong_passwords_are_slowed_down_per_device() {
        let p = people("tries");
        p.add("Sam").unwrap();
        p.unlock(2, 1, "correct horse battery").unwrap();
        *p.db(2).unwrap().conn.lock().unwrap() = None;
        for _ in 0..FREE_TRIES {
            assert!(p.unlock(2, 1, "nope nope nope").err().unwrap().contains("didn't open"));
        }
        // Now even the right password waits, from that device.
        assert!(p.unlock(2, 1, "correct horse battery").err().unwrap().contains("Too many"));
        // Another device isn't held up by the first one's mistakes.
        p.unlock(2, 2, "correct horse battery").unwrap();
    }

    #[test]
    fn changing_the_password_needs_the_old_one_and_only_the_new_one_opens_it_after() {
        let p = people("change");
        p.add("Sam").unwrap();
        p.unlock(2, 1, "correct horse battery").unwrap();
        assert!(p.change_password(2, 1, "correct horse battery", "short").unwrap_err().contains("at least"));
        assert!(p.change_password(2, 1, "not my password", "a brand new password").unwrap_err().contains("current"));
        p.change_password(2, 1, "correct horse battery", "a brand new password").unwrap();
        // Still open for them, with no gap.
        assert!(p.db(2).unwrap().is_unlocked());
        // After a restart, only the new one opens it.
        *p.db(2).unwrap().conn.lock().unwrap() = None;
        assert!(p.unlock(2, 1, "correct horse battery").is_err());
        p.unlock(2, 1, "a brand new password").unwrap();
    }

    #[test]
    fn a_wrong_current_password_counts_towards_the_wait() {
        let p = people("change-tries");
        p.add("Sam").unwrap();
        p.unlock(2, 1, "correct horse battery").unwrap();
        for _ in 0..FREE_TRIES {
            assert!(p.change_password(2, 1, "guess guess guess", "a brand new password").is_err());
        }
        assert!(p.change_password(2, 1, "correct horse battery", "a brand new password").unwrap_err().contains("Too many"));
    }

    #[test]
    fn a_backup_is_their_journal_still_encrypted_with_their_password() {
        let p = people("backup");
        p.add("Sam").unwrap();
        assert!(p.backup(2).is_err(), "nothing to copy before they've unlocked it");
        let db = p.unlock(2, 1, "correct horse battery").unwrap();
        db.with(|c| c.execute_batch("CREATE TABLE marker (x TEXT); INSERT INTO marker VALUES ('SAMSECRET')")).unwrap();
        let bytes = p.backup(2).unwrap();
        // No temporary copy left behind.
        let left: Vec<_> = std::fs::read_dir(p.folder(2)).unwrap().filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp")).collect();
        assert!(left.is_empty());
        // The secret isn't readable in the bytes, and only their password opens them.
        assert!(!bytes.windows(9).any(|w| w == b"SAMSECRET"));
        let out = p.folder(2).join("restored.db");
        std::fs::write(&out, &bytes).unwrap();
        assert!(db::is_encrypted(&out));
        assert!(db::open(&out, Some("wrong password!")).is_err());
        let c = db::open(&out, Some("correct horse battery")).unwrap();
        let x: String = c.query_row("SELECT x FROM marker", [], |r| r.get(0)).unwrap();
        assert_eq!(x, "SAMSECRET");
    }

    #[test]
    fn a_restart_names_who_it_would_lock_and_sees_only_whether_a_session_is_open() {
        let p = people("restart");
        p.add("Sam").unwrap();
        p.add("Alex").unwrap();
        assert!(p.would_lock().is_empty() && p.in_session().is_empty());
        let sam = p.unlock(2, 1, "correct horse battery").unwrap();
        p.unlock(3, 2, "another good password").unwrap();
        assert_eq!(p.would_lock(), vec!["Sam".to_string(), "Alex".to_string()]);
        assert!(p.in_session().is_empty());
        sam.with(|c| c.execute("INSERT INTO experiences (kind, title, started_at) VALUES ('session', 'x', '2026-10-02T10:00:00Z')", []))
            .unwrap();
        assert_eq!(p.in_session(), vec![2]);
        sam.with(|c| c.execute("UPDATE experiences SET ended_at = '2026-10-02T14:00:00Z'", [])).unwrap();
        assert!(p.in_session().is_empty());
    }

    #[test]
    fn the_owner_is_never_a_listed_person_and_removing_someone_deletes_their_journal() {
        let p = people("remove");
        assert!(p.db(OWNER).is_none());
        assert!(!p.exists(OWNER));
        assert!(p.remove(OWNER).is_err());
        p.add("Sam").unwrap();
        assert!(p.add("sam").unwrap_err().contains("already"));
        p.unlock(2, 1, "correct horse battery").unwrap();
        assert!(p.journal_path(2).exists());
        p.remove(2).unwrap();
        assert!(!p.journal_path(2).exists());
        assert!(p.db(2).is_none());
        assert!(p.list().is_empty());
        // Ids aren't reused, so an old device record can't land on a new person.
        assert_eq!(p.add("Alex").unwrap().id, 3);
    }

    #[test]
    fn a_pin_reopens_the_journal_on_its_own_device_only() {
        let p = people("pin");
        p.add("Sam").unwrap();
        p.unlock(2, 1, "correct horse battery").unwrap();
        assert!(p.set_pin(2, 1, "correct horse battery", "12").unwrap_err().contains("digits"));
        assert!(p.set_pin(2, 1, "correct horse battery", "12ab").unwrap_err().contains("digits"));
        assert!(p.set_pin(2, 1, "not my password", "4821").unwrap_err().contains("didn't open"));
        let sealed = p.set_pin(2, 1, "correct horse battery", "4821").unwrap();
        assert!(p.status(2, 1).unwrap().pin && !p.status(2, 9).unwrap().pin);

        // Neither the server's file nor the device's copy holds the password or PIN.
        let stored = std::fs::read_to_string(p.pins_path(2)).unwrap();
        assert!(!stored.contains("correct horse") && !stored.contains("4821"));
        assert!(!sealed.contains("correct horse"));
        use base64::Engine;
        let raw = base64::engine::general_purpose::STANDARD.decode(&sealed).unwrap();
        assert!(!raw.windows(7).any(|w| w == b"correct"));
        assert_eq!(raw.len() % 256, 0, "padded, so its length says nothing");

        // After a restart the PIN opens it, from that device.
        *p.db(2).unwrap().conn.lock().unwrap() = None;
        assert!(p.unlock_with_pin(2, 9, "4821", &sealed).err().unwrap().contains("off"), "another device has no PIN");
        assert!(!p.db(2).unwrap().is_unlocked());
        p.unlock_with_pin(2, 1, "4821", &sealed).unwrap();
        assert!(p.db(2).unwrap().is_unlocked());

        // A tampered copy doesn't open it, and the PIN turns off.
        *p.db(2).unwrap().conn.lock().unwrap() = None;
        let mut bad = raw.clone();
        bad[5] ^= 1;
        let bad = base64::engine::general_purpose::STANDARD.encode(bad);
        assert!(p.unlock_with_pin(2, 1, "4821", &bad).err().unwrap().contains("off"));
        assert!(!p.status(2, 1).unwrap().pin);
    }

    #[test]
    fn wrong_pins_turn_the_pin_off_even_across_a_restart() {
        let p = people("pin-tries");
        p.add("Sam").unwrap();
        p.unlock(2, 1, "correct horse battery").unwrap();
        let sealed = p.set_pin(2, 1, "correct horse battery", "4821").unwrap();
        *p.db(2).unwrap().conn.lock().unwrap() = None;
        for _ in 0..PIN_TRIES - 1 {
            assert!(p.unlock_with_pin(2, 1, "0000", &sealed).err().unwrap().contains("not your PIN"));
        }
        // The count is on disk: a fresh `People` (a restart) still has it.
        let p = People::load(&p.dir);
        assert!(p.unlock_with_pin(2, 1, "0000", &sealed).err().unwrap().contains("last try"));
        assert!(p.unlock_with_pin(2, 1, "4821", &sealed).err().unwrap().contains("off"));
        assert!(!p.db(2).unwrap().is_unlocked());
        // The password still works, and a new PIN can be set.
        p.unlock(2, 1, "correct horse battery").unwrap();
        p.set_pin(2, 1, "correct horse battery", "9999").unwrap();
    }

    #[test]
    fn changing_the_password_turns_every_pin_off() {
        let p = people("pin-change");
        p.add("Sam").unwrap();
        p.unlock(2, 1, "correct horse battery").unwrap();
        p.set_pin(2, 1, "correct horse battery", "4821").unwrap();
        p.set_pin(2, 2, "correct horse battery", "1357").unwrap();
        p.change_password(2, 1, "correct horse battery", "a brand new password").unwrap();
        assert!(!p.status(2, 1).unwrap().pin && !p.status(2, 2).unwrap().pin);
        assert!(!p.pins_path(2).exists());
    }

    #[test]
    fn turning_a_pin_off_leaves_other_devices_alone() {
        let p = people("pin-forget");
        p.add("Sam").unwrap();
        p.unlock(2, 1, "correct horse battery").unwrap();
        p.set_pin(2, 1, "correct horse battery", "4821").unwrap();
        p.set_pin(2, 2, "correct horse battery", "1357").unwrap();
        p.forget_pin(2, 1).unwrap();
        p.forget_pin(2, 1).unwrap();
        assert!(!p.status(2, 1).unwrap().pin && p.status(2, 2).unwrap().pin);
    }
}
