//! "Uit een ander postvak" (phase 3 task 7): copy the mail of an old mailbox into the new one, or
//! keep it on this computer. The old mailbox is added like any other account first; the engine
//! downloads its messages read-only (BODY.PEEK), and this module copies them from the local
//! store. Nothing is ever changed in the old mailbox.

use std::sync::atomic::AtomicBool;

use super::meta::{keywords_line, status_line};
use super::{ImportReport, Skipped, check_disk, open_sink, pump};
use crate::error::{Code, Result, VemError};
use crate::imap::utf7;
use crate::log;
use crate::model::{FolderRole, ImportProgress, ImportTarget};
use crate::store::Store;
use crate::store::folders::FolderInfo;

const PAGE: u32 = 500;

/// Copy every folder of `source_account` (except Verwijderde items, Ongewenste e-mail, Concepten
/// and Postvak UIT) to the target. Messages whose text is not downloaded yet are listed as skipped.
pub fn run(
    store: &Store,
    job_id: &str,
    source_account: &str,
    target: &ImportTarget,
    progress: &dyn Fn(ImportProgress),
    cancel: &AtomicBool,
) -> Result<ImportReport> {
    if matches!(target, ImportTarget::Mailbox { account_id } if account_id == source_account) {
        return Err(VemError::new(Code::AppCmd01).with_detail("same mailbox"));
    }
    let source = store.account(source_account)?;
    let size = store.account_bytes(&source.id)?;
    check_disk(store, size)?;
    let sink = open_sink(store, &source.email, size, target)?;
    let folders: Vec<FolderInfo> = store.folder_infos(&source.id)?.into_iter().filter(worth_copying).collect();
    let tags = store.tags()?;
    log::event("import.start", &[("job", job_id), ("kind", "mailbox"), ("bytes", &size.to_string())]);
    pump(sink, job_id, progress, cancel, |emit| {
        let mut skipped: Skipped = Vec::new();
        for f in &folders {
            let path = folder_path(f);
            let mut offset = 0;
            loop {
                let page = store.messages_in_folder(f.id, offset, PAGE).map_err(|e| e.code.clone())?;
                for m in &page {
                    let id: i64 = m.id.parse().map_err(|_| "message id".to_string())?;
                    let Some(raw) = store.read_eml(id).map_err(|e| e.code.clone())? else {
                        skipped.push((path.join("/"), "the text of a message was not downloaded yet".into()));
                        continue;
                    };
                    let names = m.tag_ids.iter().filter_map(|t| tags.iter().find(|tag| &tag.id == t)).map(|t| t.name.as_str());
                    let mut out = status_line(m.unread, m.important).unwrap_or_default().into_bytes();
                    out.extend(keywords_line(names).unwrap_or_default().into_bytes());
                    out.extend(raw);
                    emit(&path, out)?;
                }
                if (page.len() as u32) < PAGE {
                    break;
                }
                offset += PAGE;
            }
        }
        Ok(skipped)
    })
}

fn worth_copying(f: &FolderInfo) -> bool {
    !matches!(f.role, Some(FolderRole::Trash | FolderRole::Junk | FolderRole::Drafts | FolderRole::Outbox))
}

/// The folder's name as the user knows it, split into its levels.
fn folder_path(f: &FolderInfo) -> Vec<String> {
    let name = utf7::decode(&f.name);
    match f.delimiter.as_deref().filter(|d| !d.is_empty()) {
        Some(d) => name.split(d).map(str::to_string).collect(),
        None => vec![name],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AccountKind;
    use crate::store::NewMessage;
    use crate::store::testutil::store;

    #[test]
    fn mailbox_copy_keeps_folders_state_and_tags_and_skips_trash() {
        let (_d, s) = store();
        let old = s.add_account("Oma", "oma@oud.nl", AccountKind::Imap, None).unwrap();
        let inbox = s.upsert_folder(&old.id, "INBOX", Some("/"), Some(FolderRole::Inbox), true).unwrap();
        let sub = s.upsert_folder(&old.id, "INBOX/Familie", Some("/"), None, true).unwrap();
        let trash = s.upsert_folder(&old.id, "Trash", Some("/"), Some(FolderRole::Trash), true).unwrap();
        let add = |folder: i64, n: u32, body: bool, unread: bool| {
            s.insert_message(&NewMessage {
                account_id: old.id.clone(),
                folder_id: folder,
                uid: Some(n),
                raw: format!("Subject: m{n}\r\nMessage-ID: <m{n}@x>\r\n\r\nTekst\r\n").into_bytes(),
                has_body: body,
                unread,
                important: n == 2,
                size: 40,
                ..Default::default()
            })
            .unwrap()
        };
        add(inbox, 1, true, true);
        let starred = add(sub, 2, true, false);
        add(sub, 3, false, false);
        add(trash, 4, true, false);
        let tag = s.create_tag("Familie", "").unwrap();
        s.set_message_tag(starred, &tag.id, true).unwrap();

        let r = run(&s, "job", &old.id, &ImportTarget::Local, &|_| {}, &AtomicBool::new(false)).unwrap();

        assert_eq!(r.added, 2, "trash and the one without text are not copied");
        assert_eq!(r.skipped.len(), 1);
        let local = s.local_account("oma@oud.nl").unwrap();
        let folders = s.folder_infos(&local.id).unwrap();
        let fam = folders.iter().find(|f| f.name.ends_with("Familie")).expect("Familie folder");
        let copied = &s.messages_in_folder(fam.id, 0, 10).unwrap()[0];
        assert!(copied.important && !copied.unread);
        assert_eq!(copied.tag_ids, vec![tag.id.clone()]);
        let raw = s.read_eml(copied.id.parse().unwrap()).unwrap().unwrap();
        assert!(!String::from_utf8_lossy(&raw).contains("X-VEM-Import-Status"));
        let in_box = folders.iter().find(|f| f.role == Some(FolderRole::Inbox)).expect("inbox");
        assert!(s.messages_in_folder(in_box.id, 0, 10).unwrap()[0].unread);
    }

    #[test]
    fn copying_a_mailbox_into_itself_is_refused() {
        let (_d, s) = store();
        let a = s.add_account("Oma", "oma@x.nl", AccountKind::Imap, None).unwrap();
        let target = ImportTarget::Mailbox { account_id: a.id.clone() };
        assert!(run(&s, "job", &a.id, &target, &|_| {}, &AtomicBool::new(false)).is_err());
    }
}
