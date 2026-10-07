// SPDX-License-Identifier: LicenseRef-PolyForm-Noncommercial-1.0.0
//! Tagging someone else on this server in a dose (owner's request, 2026-10-07).
//!
//! Two people who have each said "they can tag me" may tag each other: when one
//! logs a dose, they can send a copy of it to the other. The other sees it on
//! their own device, can change anything about it, and adds it to their own
//! journal, or declines.
//!
//! ## What this never does
//!
//! - **It never reads or writes anyone's journal but the caller's.** Sending reads
//!   one dose from the sender's own journal, by its id, and copies only the dose
//!   itself (substance, amount, unit, route, time, form). Accepting writes nothing
//!   here: the recipient's phone logs the dose into their own journal through the
//!   usual `log_dose`, and then says so.
//! - **A pending tag is only ever in memory.** Nothing about a dose is written to
//!   this computer's disk outside the journals, so the computer's owner can't read
//!   who tagged whom with what. A restart drops pending tags, and the sender's list
//!   of what they sent with them.
//! - **The sender learns accepted or declined, nothing more.** Not what the other
//!   person changed, and not anything else in their journal (owner's choice).
//!
//! Who allows whom *is* saved (`tagging.json`), as pairs of person ids and nothing
//! else, so the setting survives a restart.

use crate::db::DoseDetail;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Waiting tags one person can have, so nobody can fill another's memory.
const MAX_PENDING: usize = 50;
/// Sent tags kept per sender, newest first.
const MAX_SENT: usize = 20;

/// The dose, as it travels: only what the dose itself was.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaggedDose {
    pub substance_name: String,
    pub amount: Option<f64>,
    pub unit: String,
    pub route: String,
    pub taken_at: String,
    #[serde(flatten)]
    pub detail: DoseDetail,
}

impl From<&crate::db::Dose> for TaggedDose {
    fn from(d: &crate::db::Dose) -> Self {
        TaggedDose {
            substance_name: d.substance_name.clone(),
            amount: d.amount,
            unit: d.unit.clone(),
            route: d.route.clone(),
            taken_at: d.taken_at.clone(),
            detail: d.detail.clone(),
        }
    }
}

struct Pending {
    id: u64,
    from: u32,
    to: u32,
    dose: TaggedDose,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TagStatus {
    Waiting,
    Accepted,
    Declined,
}

struct Sent {
    id: u64,
    from: u32,
    to: u32,
    substance_name: String,
    taken_at: String,
    at: u64,
    status: TagStatus,
}

#[derive(Default, Serialize, Deserialize)]
struct Saved {
    /// `(a, b)`: a lets b tag them.
    allows: Vec<(u32, u32)>,
    /// What everyone else sees the owner as. The owner has no name otherwise.
    #[serde(default)]
    owner_name: String,
}

/// Someone else on this server, as the tagging settings show them.
#[derive(Serialize, Debug, PartialEq)]
pub struct TagPerson {
    pub id: u32,
    pub name: String,
    /// I let them tag me.
    pub you_allow: bool,
    /// They let me tag them.
    pub they_allow: bool,
}

/// A tag waiting for me.
#[derive(Serialize, Debug)]
pub struct IncomingTag {
    pub id: u64,
    pub from_name: String,
    pub dose: TaggedDose,
}

/// A tag I sent, and what became of it.
#[derive(Serialize, Debug)]
pub struct SentTag {
    pub id: u64,
    pub to_name: String,
    pub substance_name: String,
    pub taken_at: String,
    pub at: u64,
    pub status: TagStatus,
}

pub struct Tagging {
    file: PathBuf,
    saved: Mutex<Saved>,
    pending: Mutex<Vec<Pending>>,
    sent: Mutex<Vec<Sent>>,
    next: Mutex<u64>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The owner, as others see them, when they haven't given a name.
pub const OWNER_FALLBACK: &str = "Server owner";

impl Tagging {
    pub fn load(file: PathBuf) -> Self {
        let saved = std::fs::read_to_string(&file)
            .ok()
            .and_then(|s| serde_json::from_str::<Saved>(&s).ok())
            .unwrap_or_default();
        // Ids only need to be unique while the server runs; start somewhere a
        // guess from an earlier run won't land on.
        let seed = now().wrapping_mul(1000);
        Tagging { file, saved: Mutex::new(saved), pending: Mutex::new(vec![]), sent: Mutex::new(vec![]), next: Mutex::new(seed) }
    }

    fn save(&self, s: &Saved) -> Result<(), String> {
        let tmp = self.file.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.file).map_err(|e| e.to_string())
    }

    pub fn owner_name(&self) -> String {
        let n = self.saved.lock().unwrap().owner_name.clone();
        if n.is_empty() { OWNER_FALLBACK.to_string() } else { n }
    }

    pub fn set_owner_name(&self, name: &str) -> Result<String, String> {
        let name: String = name.trim().chars().take(40).collect();
        let mut s = self.saved.lock().unwrap();
        s.owner_name = name;
        self.save(&s)?;
        drop(s);
        Ok(self.owner_name())
    }

    fn allows(s: &Saved, a: u32, b: u32) -> bool {
        s.allows.contains(&(a, b))
    }

    /// Both have said yes.
    pub fn linked(&self, a: u32, b: u32) -> bool {
        let s = self.saved.lock().unwrap();
        a != b && Self::allows(&s, a, b) && Self::allows(&s, b, a)
    }

    /// Everyone else on the server, with who allows whom. `everyone` is every
    /// person on the server with their name, the owner included.
    pub fn people(&self, me: u32, everyone: &[(u32, String)]) -> Vec<TagPerson> {
        let s = self.saved.lock().unwrap();
        everyone
            .iter()
            .filter(|(id, _)| *id != me)
            .map(|(id, name)| TagPerson {
                id: *id,
                name: name.clone(),
                you_allow: Self::allows(&s, me, *id),
                they_allow: Self::allows(&s, *id, me),
            })
            .collect()
    }

    /// Let `other` tag me, or stop. Stopping also drops anything they'd sent me
    /// that I hadn't answered, as declined.
    pub fn allow(&self, me: u32, other: u32, on: bool, everyone: &[(u32, String)]) -> Result<(), String> {
        if other == me || !everyone.iter().any(|(id, _)| *id == other) {
            return Err("That person isn't on this server.".into());
        }
        let mut s = self.saved.lock().unwrap();
        s.allows.retain(|p| *p != (me, other));
        if on {
            s.allows.push((me, other));
        }
        self.save(&s)?;
        drop(s);
        if !on {
            let dropped: Vec<u64> = {
                let mut p = self.pending.lock().unwrap();
                let ids = p.iter().filter(|t| t.from == other && t.to == me).map(|t| t.id).collect();
                p.retain(|t| !(t.from == other && t.to == me));
                ids
            };
            self.mark(&dropped, TagStatus::Declined);
        }
        Ok(())
    }

    /// Send a copy of one of my doses to each of `to`. Everyone must be linked.
    pub fn send(&self, from: u32, to: &[u32], dose: TaggedDose) -> Result<usize, String> {
        if to.is_empty() {
            return Err("Pick someone to tag.".into());
        }
        if to.iter().any(|t| !self.linked(from, *t)) {
            return Err("You can only tag someone who has also let you tag them.".into());
        }
        let mut pending = self.pending.lock().unwrap();
        let mut sent = self.sent.lock().unwrap();
        let mut next = self.next.lock().unwrap();
        let mut n = 0;
        let mut seen = vec![];
        for &t in to {
            if seen.contains(&t) {
                continue;
            }
            seen.push(t);
            if pending.iter().filter(|p| p.to == t).count() >= MAX_PENDING {
                continue;
            }
            *next += 1;
            pending.push(Pending { id: *next, from, to: t, dose: dose.clone() });
            sent.insert(0, Sent {
                id: *next,
                from,
                to: t,
                substance_name: dose.substance_name.clone(),
                taken_at: dose.taken_at.clone(),
                at: now(),
                status: TagStatus::Waiting,
            });
            n += 1;
        }
        // Keep only the newest few per sender.
        let mut per: std::collections::HashMap<u32, usize> = Default::default();
        sent.retain(|s| {
            let c = per.entry(s.from).or_default();
            *c += 1;
            *c <= MAX_SENT
        });
        if n == 0 {
            return Err("They have too many tags waiting already.".into());
        }
        Ok(n)
    }

    /// What's waiting for me, from people I still allow.
    pub fn inbox(&self, me: u32, name: impl Fn(u32) -> Option<String>) -> Vec<IncomingTag> {
        let p = self.pending.lock().unwrap();
        p.iter()
            .filter(|t| t.to == me)
            .filter_map(|t| Some(IncomingTag { id: t.id, from_name: name(t.from)?, dose: t.dose.clone() }))
            .collect()
    }

    /// Accept or decline one of my tags. Accepting means my phone has already put
    /// the dose in my journal; nothing here touches it.
    pub fn answer(&self, me: u32, id: u64, accept: bool) -> Result<(), String> {
        let mut p = self.pending.lock().unwrap();
        let before = p.len();
        p.retain(|t| !(t.id == id && t.to == me));
        if p.len() == before {
            return Err("That tag is no longer waiting.".into());
        }
        drop(p);
        self.mark(&[id], if accept { TagStatus::Accepted } else { TagStatus::Declined });
        Ok(())
    }

    fn mark(&self, ids: &[u64], status: TagStatus) {
        for s in self.sent.lock().unwrap().iter_mut().filter(|s| ids.contains(&s.id)) {
            s.status = status;
        }
    }

    /// What I sent recently, and whether each was accepted.
    pub fn sent(&self, me: u32, name: impl Fn(u32) -> Option<String>) -> Vec<SentTag> {
        self.sent
            .lock()
            .unwrap()
            .iter()
            .filter(|s| s.from == me)
            .filter_map(|s| {
                Some(SentTag {
                    id: s.id,
                    to_name: name(s.to)?,
                    substance_name: s.substance_name.clone(),
                    taken_at: s.taken_at.clone(),
                    at: s.at,
                    status: s.status,
                })
            })
            .collect()
    }

    /// Someone left the server: forget everything about them.
    pub fn forget(&self, id: u32) {
        let mut s = self.saved.lock().unwrap();
        s.allows.retain(|(a, b)| *a != id && *b != id);
        let _ = self.save(&s);
        drop(s);
        self.pending.lock().unwrap().retain(|t| t.from != id && t.to != id);
        self.sent.lock().unwrap().retain(|t| t.from != id && t.to != id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(name: &str) -> Tagging {
        let dir = std::env::temp_dir().join(format!("fn-tagging-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Tagging::load(dir.join("tagging.json"))
    }

    fn everyone() -> Vec<(u32, String)> {
        vec![(1, "Owner".into()), (2, "Sam".into()), (3, "Kit".into())]
    }

    fn kratom() -> TaggedDose {
        TaggedDose {
            substance_name: "kratom".into(),
            amount: Some(3.0),
            unit: "g".into(),
            route: "oral".into(),
            taken_at: "2026-10-07T20:00:00Z".into(),
            detail: DoseDetail { form: "leaf".into(), ..Default::default() },
        }
    }

    fn name(id: u32) -> Option<String> {
        everyone().into_iter().find(|(i, _)| *i == id).map(|(_, n)| n)
    }

    #[test]
    fn tagging_needs_both_to_say_yes() {
        let t = fresh("mutual");
        let all = everyone();
        assert!(t.send(1, &[2], kratom()).is_err());
        t.allow(2, 1, true, &all).unwrap();
        assert!(t.send(1, &[2], kratom()).is_err(), "one side isn't enough");
        t.allow(1, 2, true, &all).unwrap();
        assert_eq!(t.send(1, &[2], kratom()).unwrap(), 1);
        // Kit never agreed.
        assert!(t.send(1, &[2, 3], kratom()).is_err());
        assert!(t.allow(1, 1, true, &all).is_err());
        assert!(t.allow(1, 9, true, &all).is_err());
    }

    #[test]
    fn a_tag_reaches_only_its_recipient_and_the_sender_learns_only_the_answer() {
        let t = fresh("flow");
        let all = everyone();
        for (a, b) in [(1, 2), (2, 1), (1, 3), (3, 1)] {
            t.allow(a, b, true, &all).unwrap();
        }
        t.send(1, &[2], kratom()).unwrap();
        assert!(t.inbox(3, name).is_empty(), "Kit sees nothing meant for Sam");
        assert!(t.inbox(1, name).is_empty());
        let inbox = t.inbox(2, name);
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].from_name, "Owner");
        assert_eq!(inbox[0].dose, kratom());

        // Only the recipient can answer it.
        assert!(t.answer(3, inbox[0].id, true).is_err());
        assert!(t.answer(1, inbox[0].id, true).is_err());
        assert_eq!(t.sent(1, name)[0].status, TagStatus::Waiting);
        t.answer(2, inbox[0].id, true).unwrap();
        assert!(t.inbox(2, name).is_empty());
        assert!(t.answer(2, inbox[0].id, false).is_err(), "answered once");
        let sent = t.sent(1, name);
        assert_eq!((sent[0].to_name.as_str(), sent[0].status), ("Sam", TagStatus::Accepted));
        assert!(t.sent(2, name).is_empty());
    }

    #[test]
    fn withdrawing_permission_drops_what_was_waiting() {
        let t = fresh("withdraw");
        let all = everyone();
        t.allow(1, 2, true, &all).unwrap();
        t.allow(2, 1, true, &all).unwrap();
        t.send(1, &[2], kratom()).unwrap();
        t.allow(2, 1, false, &all).unwrap();
        assert!(t.inbox(2, name).is_empty());
        assert_eq!(t.sent(1, name)[0].status, TagStatus::Declined);
        assert!(t.send(1, &[2], kratom()).is_err());
    }

    #[test]
    fn only_who_allows_whom_is_saved_never_a_dose() {
        let t = fresh("saved");
        let all = everyone();
        t.allow(1, 2, true, &all).unwrap();
        t.allow(2, 1, true, &all).unwrap();
        t.send(1, &[2], kratom()).unwrap();
        let on_disk = std::fs::read_to_string(&t.file).unwrap();
        assert!(!on_disk.contains("kratom") && !on_disk.contains("2026"), "{on_disk}");
        // After a restart the permission holds and the pending tag is gone.
        let again = Tagging::load(t.file.clone());
        assert!(again.linked(1, 2));
        assert!(again.inbox(2, name).is_empty());
    }

    #[test]
    fn someone_leaving_takes_their_tags_with_them() {
        let t = fresh("forget");
        let all = everyone();
        t.allow(1, 2, true, &all).unwrap();
        t.allow(2, 1, true, &all).unwrap();
        t.send(2, &[1], kratom()).unwrap();
        t.forget(2);
        assert!(t.inbox(1, name).is_empty());
        assert!(!t.linked(1, 2));
    }
}
