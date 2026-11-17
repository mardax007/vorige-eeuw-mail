//! What is copied (docs/pc-to-phone.md): the PC's list of its local mail, and how the phone brings
//! its copy in line with that list. The PC's values are remembered per message, so a change made on
//! the phone is only overwritten when the PC itself changed that message.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::Result;
use crate::log;
use crate::model::AccountKind;
use crate::store::lan::{CopyRow, OfferedAccount, OfferedMessage};
use crate::store::{NewMessage, Store};

/// Everything the PC offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub v: u32,
    pub accounts: Vec<OfferedAccount>,
    pub messages: Vec<OfferedMessage>,
}

pub fn manifest(store: &Store) -> Result<Manifest> {
    Ok(Manifest { v: 1, accounts: store.offered_accounts()?, messages: store.offered_messages()? })
}

/// A short fingerprint of the list: the phone keeps the last one it applied in full, so the PC
/// does not send the whole list every time when nothing changed.
pub fn digest(m: &Manifest) -> String {
    let bytes = serde_json::to_vec(m).unwrap_or_default();
    Sha256::digest(&bytes).iter().take(16).map(|b| format!("{b:02x}")).collect()
}

/// What applying the list did, and which PC messages the phone still needs.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Applied {
    pub wants: Vec<i64>,
    pub removed: u32,
    pub changed: u32,
}

fn folder_path(name: &str) -> Vec<String> {
    name.split('/').filter(|p| !p.is_empty()).map(str::to_string).collect()
}

/// The phone's local account for a PC account, made on first use. None when the user removed
/// that copy on the phone: what is gone on the phone stays gone.
fn phone_account(store: &Store, acc: &OfferedAccount) -> Result<Option<String>> {
    match store.copy_account(&acc.id)? {
        Some((id, true)) => Ok(Some(id)),
        Some((_, false)) => Ok(None),
        None => {
            let a = store.add_account(&acc.name, "", AccountKind::Local, None)?;
            store.set_copy_account(&acc.id, &a.id)?;
            Ok(Some(a.id))
        }
    }
}

fn follow_pc(store: &Store, account: &str, row: &CopyRow, m: &OfferedMessage) -> Result<bool> {
    let Some(id) = row.message_id else { return Ok(false) };
    let mut changed = false;
    if row.folder != m.folder {
        let folder = store.ensure_local_folder(account, &folder_path(&m.folder), m.role)?;
        store.move_local(id, folder)?;
        changed = true;
    }
    let unread = (row.unread != m.unread).then_some(m.unread);
    let important = (row.important != m.important).then_some(m.important);
    if unread.is_some() || important.is_some() {
        store.set_flags(id, unread, important)?;
        changed = true;
    }
    if changed {
        store.put_copy_row(&CopyRow { folder: m.folder.clone(), unread: m.unread, important: m.important, ..row.clone() })?;
    }
    Ok(changed)
}

/// The phone side, before any message is sent: follow the PC's moves, reads, stars and deletions,
/// and list what is new.
pub fn apply(store: &Store, m: &Manifest) -> Result<Applied> {
    let mut accounts: HashMap<&str, String> = HashMap::new();
    for acc in &m.accounts {
        if let Some(id) = phone_account(store, acc)? {
            accounts.insert(acc.id.as_str(), id);
        }
    }
    let rows: HashMap<(String, i64), CopyRow> =
        store.copy_rows()?.into_iter().map(|r| ((r.pc_account.clone(), r.pc_id), r)).collect();
    let mut out = Applied::default();
    let mut offered: HashSet<(&str, i64)> = HashSet::new();
    for msg in &m.messages {
        offered.insert((msg.account.as_str(), msg.id));
        let Some(account) = accounts.get(msg.account.as_str()) else { continue };
        match rows.get(&(msg.account.clone(), msg.id)) {
            None => out.wants.push(msg.id),
            Some(row) => out.changed += follow_pc(store, account, row, msg)? as u32,
        }
    }
    // Gone on the PC: gone on the phone.
    for row in rows.values().filter(|r| !offered.contains(&(r.pc_account.as_str(), r.pc_id))) {
        if let Some(id) = row.message_id {
            store.delete_message(id)?;
            out.removed += 1;
        }
        store.forget_copy_row(&row.pc_account, row.pc_id)?;
    }
    // A whole local account removed on the PC: its copy on the phone goes too.
    let still: HashSet<&str> = m.accounts.iter().map(|a| a.id.as_str()).collect();
    for (pc_account, account_id) in store.copy_accounts()? {
        if !still.contains(pc_account.as_str()) {
            if store.copy_account(&pc_account)?.is_some_and(|(_, exists)| exists) {
                store.remove_account(&account_id)?;
            }
            store.forget_copy_account(&pc_account)?;
        }
    }
    let (wants, removed, changed) = (out.wants.len().to_string(), out.removed.to_string(), out.changed.to_string());
    log::event("lan.applied", &[("wants", wants.as_str()), ("removed", removed.as_str()), ("changed", changed.as_str())]);
    Ok(out)
}

/// The phone side: one message from the PC, put in the same folder of the copy account.
pub fn receive(store: &Store, m: &OfferedMessage, raw: &[u8]) -> Result<()> {
    let Some((account, true)) = store.copy_account(&m.account)? else { return Ok(()) };
    let folder = store.ensure_local_folder(&account, &folder_path(&m.folder), m.role)?;
    let id = store.insert_message(&NewMessage {
        account_id: account,
        folder_id: folder,
        uid: None,
        raw: raw.to_vec(),
        has_body: true,
        unread: m.unread,
        important: m.important,
        size: raw.len() as u64,
        fallback_date: None,
    })?;
    store.put_copy_row(&CopyRow {
        pc_account: m.account.clone(),
        pc_id: m.id,
        message_id: Some(id),
        folder: m.folder.clone(),
        unread: m.unread,
        important: m.important,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FolderRole;
    use crate::store::testutil::store;

    fn pc_with_mail() -> (tempfile::TempDir, Store, String) {
        let (d, s) = store();
        let old = s.local_account("Oude mail uit Outlook").unwrap();
        let inbox = s.ensure_local_folder(&old.id, &["Postvak IN".into()], Some(FolderRole::Inbox)).unwrap();
        let work = s.ensure_local_folder(&old.id, &["Werk".into(), "Facturen".into()], None).unwrap();
        for (folder, n) in [(inbox, 1), (inbox, 2), (work, 3)] {
            s.insert_message(&NewMessage {
                account_id: old.id.clone(),
                folder_id: folder,
                raw: format!("From: a@b.nl\r\nSubject: m{n}\r\n\r\nx\r\n").into_bytes(),
                has_body: true,
                unread: n == 1,
                size: 30,
                ..Default::default()
            })
            .unwrap();
        }
        (d, s, old.id)
    }

    fn copy_all(pc: &Store, phone: &Store) -> Applied {
        let m = manifest(pc).unwrap();
        let applied = apply(phone, &m).unwrap();
        for msg in m.messages.iter().filter(|x| applied.wants.contains(&x.id)) {
            let raw = pc.read_eml(msg.id).unwrap().unwrap();
            receive(phone, msg, &raw).unwrap();
        }
        applied
    }

    #[test]
    fn copies_folders_then_follows_the_pc() {
        let (_a, pc, old) = pc_with_mail();
        let (_b, phone) = store();
        let first = copy_all(&pc, &phone);
        assert_eq!(first.wants.len(), 3);
        let phone_acc = phone.copy_account(&old).unwrap().unwrap().0;
        let folders = phone.folders(Some(&phone_acc)).unwrap();
        assert_eq!(folders.iter().map(|f| f.path.join("/")).collect::<Vec<_>>(), vec!["Postvak IN", "Werk/Facturen"]);
        assert_eq!(folders[0].role, Some(FolderRole::Inbox));
        assert!(manifest(&phone).unwrap().accounts.is_empty(), "a copy is never offered on");

        // Nothing new: nothing wanted.
        assert_eq!(copy_all(&pc, &phone), Applied::default());

        // Read and deleted on the PC.
        let m = manifest(&pc).unwrap();
        pc.set_flags(m.messages[0].id, Some(false), Some(true)).unwrap();
        pc.delete_message(m.messages[2].id).unwrap();
        let again = copy_all(&pc, &phone);
        assert_eq!((again.wants.len(), again.removed, again.changed), (0, 1, 1));
        let rows = phone.copy_rows().unwrap();
        assert_eq!(rows.len(), 2);
        let first_copy = rows.iter().find(|r| r.pc_id == m.messages[0].id).unwrap();
        let s = phone.message(first_copy.message_id.unwrap()).unwrap();
        assert!(!s.summary.unread && s.summary.important);
    }

    #[test]
    fn a_change_on_the_phone_stays_until_the_pc_changes() {
        let (_a, pc, _) = pc_with_mail();
        let (_b, phone) = store();
        copy_all(&pc, &phone);
        let row = phone.copy_rows().unwrap().into_iter().find(|r| r.unread).unwrap();
        let id = row.message_id.unwrap();
        phone.set_flags(id, Some(false), None).unwrap();
        copy_all(&pc, &phone);
        assert!(!phone.message(id).unwrap().summary.unread, "read on the phone stays read");
    }

    #[test]
    fn deleted_on_the_phone_is_not_sent_again() {
        let (_a, pc, _) = pc_with_mail();
        let (_b, phone) = store();
        copy_all(&pc, &phone);
        let row = phone.copy_rows().unwrap().remove(0);
        phone.delete_message(row.message_id.unwrap()).unwrap();
        assert!(copy_all(&pc, &phone).wants.is_empty());
    }

    #[test]
    fn a_local_account_removed_on_the_pc_goes_from_the_phone() {
        let (_a, pc, old) = pc_with_mail();
        let (_b, phone) = store();
        copy_all(&pc, &phone);
        let phone_acc = phone.copy_account(&old).unwrap().unwrap().0;
        pc.remove_account(&old).unwrap();
        let gone = copy_all(&pc, &phone);
        assert_eq!(gone.removed, 3);
        assert!(phone.account(&phone_acc).is_err());
        assert!(phone.copy_accounts().unwrap().is_empty());
    }

    #[test]
    fn own_imports_on_the_phone_never_land_in_a_copy() {
        let (_a, pc, old) = pc_with_mail();
        let (_b, phone) = store();
        copy_all(&pc, &phone);
        let copy = phone.copy_account(&old).unwrap().unwrap().0;
        let own = phone.local_account("Oude mail uit Outlook").unwrap();
        assert_ne!(own.id, copy);
    }

    #[test]
    fn digest_changes_with_the_list() {
        let (_a, pc, _) = pc_with_mail();
        let one = digest(&manifest(&pc).unwrap());
        assert_eq!(one, digest(&manifest(&pc).unwrap()));
        let first = manifest(&pc).unwrap().messages[0].id;
        pc.set_flags(first, Some(false), None).unwrap();
        assert_ne!(one, digest(&manifest(&pc).unwrap()));
    }
}
