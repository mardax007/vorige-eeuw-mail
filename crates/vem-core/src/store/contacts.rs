use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{Store, json, new_id};
use crate::error::{Code, Result, VemError};
use crate::model::{Contact, now};

/// A contact with its change time and deleted mark, for merging between devices (docs/sync.md).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncContact {
    pub contact: Contact,
    pub changed_at: i64,
    pub deleted: bool,
}

fn clean_emails(emails: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for e in emails {
        let e = e.trim().to_lowercase();
        if e.contains('@') && !out.contains(&e) {
            out.push(e);
        }
    }
    out
}

impl Store {
    pub fn contacts(&self, query: &str) -> Result<Vec<Contact>> {
        let q = format!("%{}%", query.trim().to_lowercase());
        let db = self.db();
        let mut st = db.prepare(
            "SELECT id, name, emails_json, phone, note FROM contacts
             WHERE deleted = 0 AND (lower(name) LIKE ?1 OR lower(emails_json) LIKE ?1)
             ORDER BY name COLLATE NOCASE",
        )?;
        let rows = st
            .query_map([q], |r| {
                Ok(Contact {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    emails: serde_json::from_str(&r.get::<_, String>(2)?).unwrap_or_default(),
                    phone: r.get(3)?,
                    note: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    pub fn contact(&self, id: &str) -> Result<Contact> {
        self.contacts("")?
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| VemError::new(Code::StoreRead01).with_detail("contact"))
    }

    pub fn save_contact(&self, c: &Contact) -> Result<Contact> {
        let emails = clean_emails(&c.emails);
        let id = if c.id.is_empty() { new_id() } else { c.id.clone() };
        let saved = Contact {
            id: id.clone(),
            name: c.name.trim().to_string(),
            emails: emails.clone(),
            phone: c.phone.clone().filter(|p| !p.trim().is_empty()),
            note: c.note.clone().filter(|n| !n.trim().is_empty()),
        };
        self.write_contact(&saved, now(), false)?;
        Ok(saved)
    }

    fn write_contact(&self, c: &Contact, changed_at: i64, deleted: bool) -> Result<()> {
        let db = self.db();
        db.execute(
            "INSERT INTO contacts (id, name, emails_json, phone, note, changed_at, deleted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, emails_json = excluded.emails_json,
                phone = excluded.phone, note = excluded.note, changed_at = excluded.changed_at,
                deleted = excluded.deleted",
            params![c.id, c.name, json(&c.emails), c.phone, c.note, changed_at, deleted as i64],
        )?;
        db.execute("DELETE FROM contact_emails WHERE contact_id = ?1", [&c.id])?;
        if !deleted {
            for e in &c.emails {
                db.execute(
                    "INSERT OR IGNORE INTO contact_emails (email, contact_id) VALUES (?1, ?2)",
                    params![e, c.id],
                )?;
            }
        }
        Ok(())
    }

    /// Deleting keeps a mark, so a paired phone learns about it.
    pub fn delete_contact(&self, id: &str) -> Result<()> {
        let c = self.contact(id)?;
        self.write_contact(&c, now(), true)
    }

    pub fn contact_by_email(&self, email: &str) -> Result<Option<Contact>> {
        let id: Option<String> = self
            .db()
            .query_row(
                "SELECT contact_id FROM contact_emails WHERE email = ?1 LIMIT 1",
                [email.trim()],
                |r| r.get(0),
            )
            .optional()?;
        match id {
            Some(id) => Ok(Some(self.contact(&id)?)),
            None => Ok(None),
        }
    }

    /// A contact whose name matches but whose addresses do not include `email`
    /// (the look-alike warning in docs/screens.md).
    pub fn contact_with_name_other_address(&self, name: &str, email: &str) -> Result<Option<Contact>> {
        let name = name.trim();
        if name.len() < 3 {
            return Ok(None);
        }
        let email = email.trim().to_lowercase();
        Ok(self
            .contacts("")?
            .into_iter()
            .find(|c| c.name.eq_ignore_ascii_case(name) && !c.emails.contains(&email)))
    }

    /// Fill the address book from sent mail: add an address only if nobody has it yet.
    pub fn learn_contact(&self, name: Option<&str>, email: &str) -> Result<()> {
        let email = email.trim().to_lowercase();
        if !email.contains('@') || self.contact_by_email(&email)?.is_some() {
            return Ok(());
        }
        let name = name.map(str::trim).filter(|n| !n.is_empty()).unwrap_or(&email).to_string();
        self.save_contact(&Contact { id: String::new(), name, emails: vec![email], phone: None, note: None })?;
        Ok(())
    }

    pub fn contacts_for_sync(&self) -> Result<Vec<SyncContact>> {
        let db = self.db();
        let mut st = db.prepare("SELECT id, name, emails_json, phone, note, changed_at, deleted FROM contacts")?;
        let rows = st
            .query_map([], |r| {
                Ok(SyncContact {
                    contact: Contact {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        emails: serde_json::from_str(&r.get::<_, String>(2)?).unwrap_or_default(),
                        phone: r.get(3)?,
                        note: r.get(4)?,
                    },
                    changed_at: r.get(5)?,
                    deleted: r.get::<_, i64>(6)? != 0,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    /// Take the other device's list: per contact, the newest change wins. Returns whether
    /// anything here changed.
    pub fn merge_contacts(&self, remote: &[SyncContact]) -> Result<bool> {
        let local = self.contacts_for_sync()?;
        let mut changed = false;
        for r in remote {
            let newer = local
                .iter()
                .find(|l| l.contact.id == r.contact.id)
                .is_none_or(|l| r.changed_at > l.changed_at);
            if newer {
                self.write_contact(&r.contact, r.changed_at, r.deleted)?;
                changed = true;
            }
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::testutil::store;

    fn c(name: &str, email: &str) -> Contact {
        Contact { id: String::new(), name: name.into(), emails: vec![email.into()], phone: None, note: None }
    }

    #[test]
    fn save_find_delete() {
        let (_d, s) = store();
        let a = s.save_contact(&c("Tante Riet", " Riet@Example.NL ")).unwrap();
        s.save_contact(&c("Bert", "bert@x.nl")).unwrap();
        assert_eq!(a.emails, vec!["riet@example.nl"]);
        assert_eq!(s.contacts("").unwrap().iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["Bert", "Tante Riet"]);
        assert_eq!(s.contacts("riet").unwrap().len(), 1);
        assert_eq!(s.contact_by_email("RIET@example.nl").unwrap().unwrap().id, a.id);
        assert!(s.contact_with_name_other_address("tante riet", "riet@evil.com").unwrap().is_some());
        assert!(s.contact_with_name_other_address("Tante Riet", "riet@example.nl").unwrap().is_none());
        s.delete_contact(&a.id).unwrap();
        assert_eq!(s.contacts("").unwrap().len(), 1);
        assert!(s.contacts_for_sync().unwrap().iter().any(|x| x.deleted));
    }

    #[test]
    fn learn_does_not_duplicate() {
        let (_d, s) = store();
        s.learn_contact(Some("Piet"), "piet@x.nl").unwrap();
        s.learn_contact(Some("P."), "PIET@x.nl").unwrap();
        s.learn_contact(None, "geen-adres").unwrap();
        assert_eq!(s.contacts("").unwrap().len(), 1);
    }

    #[test]
    fn merge_newest_wins() {
        let (_d, s) = store();
        let a = s.save_contact(&c("Oud", "a@x.nl")).unwrap();
        let mut remote = s.contacts_for_sync().unwrap();
        remote[0].contact.name = "Nieuw".into();
        remote[0].changed_at += 10;
        remote.push(SyncContact { contact: Contact { id: "r1".into(), ..c("Van telefoon", "t@x.nl") }, changed_at: 5, deleted: false });
        assert!(s.merge_contacts(&remote).unwrap());
        assert_eq!(s.contact(&a.id).unwrap().name, "Nieuw");
        assert_eq!(s.contacts("").unwrap().len(), 2);
        let mut stale = s.contacts_for_sync().unwrap();
        stale.iter_mut().for_each(|x| { x.changed_at -= 100; x.contact.name = "Stale".into() });
        assert!(!s.merge_contacts(&stale).unwrap());
    }
}
