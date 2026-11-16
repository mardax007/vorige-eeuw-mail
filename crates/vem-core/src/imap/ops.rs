//! Actions the user took locally, waiting to be done on the server (docs/sync.md, Actions).
//!
//! The screen changes at once; the server follows when the next connection runs. An action
//! with `after` in the future waits, so "Ongedaan maken" can cancel it before the server
//! ever sees it.

use serde::{Deserialize, Serialize};

use super::client::Imap;
use crate::error::Result;
use crate::model::now;
use crate::store::Store;

/// Give up on an action after this many failed tries; the next full sync corrects the screen.
pub const MAX_ATTEMPTS: u32 = 5;

/// Gmail accepts about 500 MB of IMAP uploads a day (Google documents it for Workspace; consumer
/// accounts are treated the same). An import into Gmail stops below it and goes on the next day.
pub const GMAIL_UPLOAD_PER_DAY: u64 = 500 * 1024 * 1024;

const SECONDS_PER_DAY: i64 = 86_400;
/// Google's quota day follows Pacific time, not UTC: count days from 08:00 UTC so a new day
/// here never starts before Google's.
const QUOTA_DAY_OFFSET: i64 = 8 * 3600;

/// Bytes uploaded to one mailbox on one (UTC) day.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadDay {
    pub day: i64,
    pub bytes: u64,
}

fn upload_key(account_id: &str) -> String {
    format!("upload_day:{account_id}")
}

/// What was uploaded to this mailbox today; a new day starts at 0.
pub fn uploaded_today(store: &Store, account_id: &str, now: i64) -> Result<UploadDay> {
    let today = (now - QUOTA_DAY_OFFSET).div_euclid(SECONDS_PER_DAY);
    Ok(match store.kv_get::<UploadDay>(&upload_key(account_id))? {
        Some(d) if d.day == today => d,
        _ => UploadDay { day: today, bytes: 0 },
    })
}

/// Whether an upload of `size` bytes still fits in today's Gmail allowance. The first upload of a
/// day always goes, so one huge message cannot hold back the rest: the server then refuses it.
pub fn fits_today(day: UploadDay, size: u64) -> bool {
    day.bytes == 0 || day.bytes + size <= GMAIL_UPLOAD_PER_DAY
}

/// Gmail says the daily allowance is used up: wait for tomorrow, it is not a failed attempt.
fn over_quota(e: &crate::VemError) -> bool {
    e.server_reply.as_deref().is_some_and(|r| {
        let r = r.to_ascii_uppercase();
        r.contains("OVERQUOTA") || r.contains("BANDWIDTH LIMIT")
    })
}

/// A read or star change on a message that still waits for its upload has no server copy to
/// change yet: put it into the upload itself, so it is not lost.
fn fold_flags_into_uploads(store: &Store, account_id: &str) -> Result<()> {
    let mut ops = store.ops::<Op>(account_id)?;
    let flag_ops: Vec<(i64, Op)> = ops.iter().filter(|p| matches!(p.op, Op::Flags { .. })).map(|p| (p.id, p.op.clone())).collect();
    for (flag_id, op) in flag_ops {
        let Op::Flags { message_id, seen, flagged } = op else { continue };
        if store.message(message_id).map(|m| m.uid.is_some()).unwrap_or(true) {
            continue;
        }
        let Some(upload) = ops.iter_mut().find(|a| matches!(a.op, Op::Append { message_id: m, .. } if m == message_id)) else {
            continue;
        };
        let Op::Append { folder, seen: s0, flagged: f0, .. } = upload.op.clone() else { continue };
        let merged = Op::Append { message_id, folder, seen: seen.unwrap_or(s0), flagged: flagged.unwrap_or(f0) };
        store.replace_op(upload.id, &merged)?;
        upload.op = merged;
        store.op_done(flag_id)?;
    }
    Ok(())
}

/// The server refused the message as too big (`[TOOBIG]`, RFC 4469): trying again cannot help.
fn too_big(e: &crate::VemError) -> bool {
    e.server_reply.as_deref().is_some_and(|r| r.to_ascii_uppercase().contains("TOOBIG"))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Op {
    #[serde(rename_all = "camelCase")]
    Flags { message_id: i64, seen: Option<bool>, flagged: Option<bool> },
    #[serde(rename_all = "camelCase")]
    Keyword { message_id: i64, keyword: String, add: bool },
    /// Move on the server; `to_folder` None means delete for good (from Verwijderde items).
    #[serde(rename_all = "camelCase")]
    Move { message_id: i64, from_folder: i64, uid: u32, to_folder: Option<i64>, after: i64 },
    /// Put a local message (a sent copy, a draft, an import) into a server folder.
    #[serde(rename_all = "camelCase")]
    Append {
        message_id: i64,
        folder: i64,
        seen: bool,
        /// The star of an imported message; ops queued before this field have none.
        #[serde(default)]
        flagged: bool,
    },
    /// Make a folder on the server (an import into the mailbox). One that exists already is fine.
    #[serde(rename_all = "camelCase")]
    Create { folder: i64 },
}

impl Op {
    fn due(&self, now: i64) -> bool {
        match self {
            Op::Move { after, .. } => *after <= now,
            _ => true,
        }
    }
}

/// UIDs in a folder that wait for a delayed move or delete: sync must not show them again.
pub fn pending_move_uids(store: &Store, account_id: &str, folder_id: i64) -> Result<std::collections::HashSet<u32>> {
    Ok(store
        .ops::<Op>(account_id)?
        .into_iter()
        .filter_map(|p| match p.op {
            Op::Move { from_folder, uid, .. } if from_folder == folder_id => Some(uid),
            _ => None,
        })
        .collect())
}

pub async fn apply_pending(store: &Store, imap: &mut Imap, account_id: &str) -> Result<()> {
    let t = now();
    let gmail = imap.is_gmail();
    let mut today = uploaded_today(store, account_id, t)?;
    let mut paused = false;
    fold_flags_into_uploads(store, account_id)?;
    for p in store.ops::<Op>(account_id)? {
        if !p.op.due(t) {
            continue;
        }
        let upload = match &p.op {
            Op::Append { message_id, .. } => store.message(*message_id).map(|m| m.size).unwrap_or(0),
            _ => 0,
        };
        if gmail && upload > 0 && (paused || !fits_today(today, upload)) {
            if !paused {
                crate::log::event("upload.paused", &[("bytes_today", &today.bytes.to_string())]);
            }
            paused = true; // The rest of the uploads wait for tomorrow; other actions go on.
            continue;
        }
        match apply_one(store, imap, &p.op).await {
            Ok(()) => {
                store.op_done(p.id)?;
                if upload > 0 {
                    today.bytes += upload;
                    store.kv_set(&upload_key(account_id), &today)?;
                }
            }
            Err(e) if e.code().is_some_and(|c| c == crate::Code::ImapCmd02) => return Err(e),
            Err(e) if gmail && upload > 0 && over_quota(&e) => {
                crate::log::event("upload.over_quota", &[("bytes_today", &today.bytes.to_string())]);
                paused = true;
            }
            Err(e) if too_big(&e) => {
                crate::log::event("upload.too_big", &[("bytes", &upload.to_string())]);
                store.op_done(p.id)?;
            }
            Err(_) if p.attempts + 1 >= MAX_ATTEMPTS => store.op_done(p.id)?,
            Err(_) => store.op_failed(p.id)?,
        }
    }
    Ok(())
}

async fn apply_one(store: &Store, imap: &mut Imap, op: &Op) -> Result<()> {
    match op {
        Op::Flags { message_id, seen, flagged } => {
            let Ok(m) = store.message(*message_id) else { return Ok(()) };
            let Some(uid) = m.uid else { return Ok(()) };
            imap.open(&store.folder(m.folder_id)?.name, false).await?;
            if let Some(s) = seen {
                imap.store(&[uid], *s, "\\Seen").await?;
            }
            if let Some(f) = flagged {
                imap.store(&[uid], *f, "\\Flagged").await?;
            }
            Ok(())
        }
        Op::Keyword { message_id, keyword, add } => {
            let Ok(m) = store.message(*message_id) else { return Ok(()) };
            let Some(uid) = m.uid else { return Ok(()) };
            let folder = store.folder(m.folder_id)?;
            imap.open(&folder.name, false).await?;
            if let Some(label) = keyword.strip_prefix("gmail:") {
                imap.gmail_label(&[uid], *add, label).await
            } else if folder.keywords_ok || imap.is_gmail() {
                imap.store(&[uid], *add, keyword).await
            } else {
                Ok(()) // The server keeps no keywords here: the tag stays local only.
            }
        }
        Op::Move { message_id, from_folder, uid, to_folder, .. } => {
            let from = store.folder(*from_folder)?;
            imap.open(&from.name, false).await?;
            match to_folder {
                Some(to) => {
                    let target = store.folder(*to)?;
                    imap.move_to(&[*uid], &target.name).await?;
                    if store.message(*message_id).is_ok() {
                        store.move_local(*message_id, *to)?;
                    }
                }
                None => {
                    imap.expunge_uids(&[*uid]).await?;
                    if store.message(*message_id).is_ok() {
                        store.delete_message(*message_id)?;
                    }
                }
            }
            Ok(())
        }
        Op::Create { folder } => {
            let f = store.folder(*folder)?;
            match imap.create_folder(&f.name).await {
                Err(e) if e.code().is_some_and(|c| c == crate::Code::ImapCmd02) => Err(e),
                _ => Ok(()),
            }
        }
        Op::Append { message_id, folder, seen, flagged } => {
            let Some(raw) = store.read_eml(*message_id)? else { return Ok(()) };
            let target = store.folder(*folder)?;
            imap.append(&target.name, *seen, *flagged, &raw).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gmail_allowance_starts_again_each_day() {
        let (_d, s) = crate::store::testutil::store();
        let day1 = 20_000 * SECONDS_PER_DAY + 5;
        let d = uploaded_today(&s, "a", day1).unwrap();
        assert_eq!(d.bytes, 0);
        s.kv_set(&upload_key("a"), &UploadDay { day: d.day, bytes: GMAIL_UPLOAD_PER_DAY - 10 }).unwrap();
        let d = uploaded_today(&s, "a", day1 + 60).unwrap();
        assert!(fits_today(d, 10));
        assert!(!fits_today(d, 11));
        assert_eq!(uploaded_today(&s, "a", day1 + SECONDS_PER_DAY).unwrap().bytes, 0);
        assert_eq!(uploaded_today(&s, "b", day1).unwrap().bytes, 0, "per mailbox");
    }

    #[test]
    fn first_upload_of_a_day_always_goes() {
        assert!(fits_today(UploadDay { day: 1, bytes: 0 }, GMAIL_UPLOAD_PER_DAY + 1));
        assert!(!fits_today(UploadDay { day: 1, bytes: 1 }, GMAIL_UPLOAD_PER_DAY));
    }

    #[test]
    fn quota_day_starts_at_eight_utc() {
        let (_d, s) = crate::store::testutil::store();
        let midnight = 20_000 * SECONDS_PER_DAY;
        let before = uploaded_today(&s, "a", midnight + 7 * 3600).unwrap().day;
        let after = uploaded_today(&s, "a", midnight + 9 * 3600).unwrap().day;
        assert_eq!(after, before + 1);
        let e = crate::VemError::new(crate::Code::ImapCmd01).with_reply("a7 NO [OVERQUOTA] Account exceeded command or bandwidth limits.");
        assert!(over_quota(&e));
    }

    #[test]
    fn reading_a_message_that_waits_for_upload_changes_the_upload() {
        let (_d, s) = crate::store::testutil::store();
        let a = s.add_account("J", "j@x.nl", crate::model::AccountKind::Imap, None).unwrap();
        let f = s.upsert_folder(&a.id, "INBOX", Some("/"), Some(crate::model::FolderRole::Inbox), true).unwrap();
        let raw = b"Subject: x\r\n\r\ny\r\n".to_vec();
        let m = s
            .insert_message(&crate::store::NewMessage { account_id: a.id.clone(), folder_id: f, raw, has_body: true, unread: true, ..Default::default() })
            .unwrap();
        s.push_op(&a.id, &Op::Append { message_id: m, folder: f, seen: false, flagged: false }).unwrap();
        s.push_op(&a.id, &Op::Flags { message_id: m, seen: Some(true), flagged: None }).unwrap();
        s.push_op(&a.id, &Op::Flags { message_id: m, seen: None, flagged: Some(true) }).unwrap();
        fold_flags_into_uploads(&s, &a.id).unwrap();
        let ops = s.ops::<Op>(&a.id).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].op, Op::Append { message_id: m, folder: f, seen: true, flagged: true });
    }

    #[test]
    fn toobig_reply_is_final() {
        let e = crate::VemError::new(crate::Code::ImapCmd01).with_reply("a4 NO [TOOBIG] Message too large");
        assert!(too_big(&e));
        assert!(!too_big(&crate::VemError::new(crate::Code::ImapCmd01).with_reply("a4 NO [TRYCREATE]")));
    }

    #[test]
    fn op_json_round_trip_and_due() {
        let op = Op::Move { message_id: 1, from_folder: 2, uid: 3, to_folder: None, after: 100 };
        let s = serde_json::to_string(&op).unwrap();
        assert!(s.contains("\"op\":\"move\""));
        assert_eq!(serde_json::from_str::<Op>(&s).unwrap(), op);
        assert!(!op.due(99));
        assert!(op.due(100));
    }
}
