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

    pub fn status(&self, id: u32) -> Option<PersonStatus> {
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

    /// Someone's journal is open only in memory, so a restart would lock it until
    /// they unlock it again from their own device.
    pub fn unlocked_in_memory_only(&self) -> bool {
        self.list().iter().any(|p| p.unlocked && !p.remembered)
    }
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
        assert!(p.status(2).unwrap().new_journal);

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
}
