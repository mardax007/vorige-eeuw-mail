//! Keep the local copy in step with the server (docs/sync.md).
//!
//! Order per account: send our pending actions first, then folders, then each folder's new
//! messages, flags and deletions, then bodies in the background, newest first.

use std::collections::{HashMap, HashSet};

use super::client::{Imap, RemoteMessage};
use super::ops;
use crate::error::Result;
use crate::model::FolderRole;
use crate::store::folders::{FolderInfo, FolderSync};
use crate::store::{NewMessage, Store};

/// Fetch this many messages per command, so one huge folder does not hold one giant reply.
pub const FETCH_CHUNK: usize = 200;

#[derive(Debug, Default, Clone)]
pub struct SyncReport {
    /// New unread messages in a Postvak IN, for Windows notifications.
    pub new_in_inbox: Vec<i64>,
    pub changed_folders: HashSet<i64>,
    /// A folder came or went on the server: the folder list on screen must be read again.
    pub folder_list_changed: bool,
}

impl SyncReport {
    fn merge(&mut self, other: SyncReport) {
        self.new_in_inbox.extend(other.new_in_inbox);
        self.changed_folders.extend(other.changed_folders);
        self.folder_list_changed |= other.folder_list_changed;
    }
}

/// Sync the folder list, then every folder. `first_run` marks existing mail as not new.
pub async fn sync_account(store: &Store, imap: &mut Imap, account_id: &str, first_run: bool) -> Result<SyncReport> {
    ops::apply_pending(store, imap, account_id).await?;
    store.set_capabilities(account_id, &imap.capabilities())?;
    let folder_list_changed = sync_folders(store, imap, account_id).await?;
    let mut report = SyncReport { folder_list_changed, ..SyncReport::default() };
    let mut infos = store.folder_infos(account_id)?;
    // Postvak IN first: that is where people look.
    infos.sort_by_key(|f| if f.role == Some(FolderRole::Inbox) { 0 } else { 1 });
    for f in infos {
        let r = sync_folder(store, imap, &f, first_run).await?;
        report.merge(r);
    }
    update_quota(store, imap, account_id).await?;
    Ok(report)
}

/// Brings the stored folder list in line with the server. True when a folder the screens show
/// (a selectable one) came or went.
pub async fn sync_folders(store: &Store, imap: &mut Imap, account_id: &str) -> Result<bool> {
    let remote = imap.list_folders().await?;
    let shown_before: HashSet<String> = store.folder_infos(account_id)?.into_iter().map(|f| f.name).collect();
    let mut keep = Vec::new();
    let mut shown_now = HashSet::new();
    // The contacts folder of decision 19 holds one encrypted message for our own use; it is not
    // mail and never shows in the folder list (docs/sync.md).
    for f in remote.iter().filter(|f| !f.skip && f.name != crate::contacts_sync::FOLDER) {
        store.upsert_folder(account_id, &f.name, f.delimiter.as_deref(), f.role, f.selectable)?;
        keep.push(f.name.clone());
        if f.selectable {
            shown_now.insert(f.name.clone());
        }
    }
    store.remove_folders_except(account_id, &keep)?;
    Ok(shown_before != shown_now)
}

/// One folder: UIDVALIDITY check, new messages, then flags and deletions of known ones.
pub async fn sync_folder(store: &Store, imap: &mut Imap, f: &FolderInfo, first_run: bool) -> Result<SyncReport> {
    let state = imap.open(&f.name, false).await?;
    let mut report = SyncReport::default();
    if f.sync.uidvalidity.is_some() && state.uidvalidity != f.sync.uidvalidity {
        // The server renumbered the folder: everything cached for it is invalid.
        store.reset_folder(f.id)?;
        report.changed_folders.insert(f.id);
    }
    store.set_folder_keywords_ok(f.id, state.keywords_ok)?;
    let known = store.uids_in_folder(f.id)?;
    let max_known = known.iter().map(|(_, u)| *u).max().unwrap_or(0);

    if state.exists > 0 {
        let unchanged_next = f.sync.uidnext.is_some() && f.sync.uidnext == state.uidnext && !known.is_empty();
        if !unchanged_next {
            let new = imap.fetch(&format!("{}:*", max_known + 1), false).await?;
            // "n:*" always returns the last message, even when its UID is below n.
            let waiting = ops::pending_move_uids(store, &f.account_id, f.id)?;
            let new: Vec<RemoteMessage> =
                new.into_iter().filter(|m| m.uid > max_known && !waiting.contains(&m.uid)).collect();
            for chunk in new.chunks(FETCH_CHUNK) {
                for m in chunk {
                    let id = store_remote(store, f, m)?;
                    report.changed_folders.insert(f.id);
                    if !first_run && f.role == Some(FolderRole::Inbox) && !m.seen {
                        report.new_in_inbox.push(id);
                    }
                }
            }
        }
        if !known.is_empty() {
            let r = sync_known(store, imap, f, &known, max_known).await?;
            report.merge(r);
        }
    } else if !known.is_empty() {
        for (id, _) in &known {
            store.delete_message(*id)?;
        }
        report.changed_folders.insert(f.id);
    }
    store.set_folder_sync(
        f.id,
        &FolderSync { uidvalidity: state.uidvalidity, uidnext: state.uidnext, highest_modseq: state.highest_modseq },
    )?;
    Ok(report)
}

/// Flags changed elsewhere, and messages deleted elsewhere.
async fn sync_known(
    store: &Store,
    imap: &mut Imap,
    f: &FolderInfo,
    known: &[(i64, u32)],
    max_known: u32,
) -> Result<SyncReport> {
    let mut report = SyncReport::default();
    let server: HashSet<u32> = imap.uids().await?;
    for (id, uid) in known {
        if !server.contains(uid) {
            store.delete_message(*id)?;
            report.changed_folders.insert(f.id);
        }
    }
    let flags = imap.fetch_flags(&format!("1:{max_known}")).await?;
    let by_uid: HashMap<u32, i64> = known.iter().map(|(id, uid)| (*uid, *id)).collect();
    for m in flags {
        let Some(id) = by_uid.get(&m.uid) else { continue };
        let cur = store.message(*id)?;
        if cur.summary.unread == m.seen || cur.summary.important != m.flagged {
            store.set_flags(*id, Some(!m.seen), Some(m.flagged))?;
            report.changed_folders.insert(f.id);
        }
        apply_keywords(store, *id, &m)?;
    }
    Ok(report)
}

/// Add one server message to the index. A message we moved or appended ourselves is already
/// there without a UID; it is adopted by its Message-ID instead of shown twice.
fn store_remote(store: &Store, f: &FolderInfo, m: &RemoteMessage) -> Result<i64> {
    let parsed_id = crate::parse::parse(&m.raw).message_id;
    if let Some(mid) = parsed_id.as_deref()
        && let Some(id) = store.adopt_by_message_id(f.id, mid, m.uid)? {
            store.set_flags(id, Some(!m.seen), Some(m.flagged))?;
            apply_keywords(store, id, m)?;
            return Ok(id);
        }
    let id = store.insert_message(&NewMessage {
        account_id: f.account_id.clone(),
        folder_id: f.id,
        uid: Some(m.uid),
        raw: m.raw.clone(),
        has_body: false,
        unread: !m.seen,
        important: m.flagged,
        size: m.size,
        fallback_date: m.internal_date,
    })?;
    apply_keywords(store, id, m)?;
    Ok(id)
}

/// Server keywords and Gmail labels become tags (docs/sync.md, Tags and the star).
fn apply_keywords(store: &Store, id: i64, m: &RemoteMessage) -> Result<()> {
    let mut tag_ids = Vec::new();
    for k in &m.keywords {
        if let Some(name) = k.strip_prefix("$vem_") {
            let shown = store.tag_for_keyword(k)?.map(|t| t.name).unwrap_or_else(|| name.replace('_', " "));
            tag_ids.push(store.tag_from_server(k, &shown)?.id);
        }
    }
    for l in &m.gmail_labels {
        if l.starts_with('\\') {
            continue;
        }
        let shown = super::utf7::decode(l);
        tag_ids.push(store.tag_from_server(&format!("gmail:{l}"), &shown)?.id);
    }
    if !tag_ids.is_empty() || !m.keywords.is_empty() || !m.gmail_labels.is_empty() {
        store.set_message_tags(id, &tag_ids)?;
    }
    Ok(())
}

/// Download up to `max` bodies, newest first. Returns how many arrived.
pub async fn fetch_bodies(store: &Store, imap: &mut Imap, account_id: &str, max: u32) -> Result<usize> {
    let todo = store.without_body(account_id, max)?;
    let mut by_folder: HashMap<i64, Vec<(i64, u32)>> = HashMap::new();
    for (id, folder, uid) in todo {
        by_folder.entry(folder).or_default().push((id, uid));
    }
    let mut done = 0;
    for (folder, items) in by_folder {
        let info = store.folder(folder)?;
        imap.open(&info.name, false).await?;
        for chunk in items.chunks(25) {
            let uids: Vec<u32> = chunk.iter().map(|(_, u)| *u).collect();
            let got = imap.fetch(&super::client::uid_set(&uids), true).await?;
            for m in got {
                if let Some((id, _)) = chunk.iter().find(|(_, u)| *u == m.uid) {
                    store.set_body(*id, &m.raw)?;
                    done += 1;
                }
            }
        }
    }
    Ok(done)
}

/// Mailbox space: QUOTA where offered, else the sum of message sizes (PLAN.md §2).
pub async fn update_quota(store: &Store, imap: &mut Imap, account_id: &str) -> Result<()> {
    let quota = match imap.quota().await? {
        Some(q) => Some(q),
        None => {
            let used = store.account_size_kb(account_id)?;
            let email = store.account(account_id)?.email;
            Some(crate::model::Quota { used_kb: used, limit_kb: crate::lookup::known_limit_kb(&email) })
        }
    };
    store.set_quota(account_id, quota.as_ref())
}
