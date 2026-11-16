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
    Append { message_id: i64, folder: i64, seen: bool },
}

impl Op {
    fn due(&self, now: i64) -> bool {
        match self {
            Op::Move { after, .. } => *after <= now,
            _ => true,
        }
    }
}

pub async fn apply_pending(store: &Store, imap: &mut Imap, account_id: &str) -> Result<()> {
    let t = now();
    for p in store.ops::<Op>(account_id)? {
        if !p.op.due(t) {
            continue;
        }
        match apply_one(store, imap, &p.op).await {
            Ok(()) => store.op_done(p.id)?,
            Err(e) if e.code().is_some_and(|c| c == crate::Code::ImapCmd02) => return Err(e),
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
        Op::Append { message_id, folder, seen } => {
            let Some(raw) = store.read_eml(*message_id)? else { return Ok(()) };
            let target = store.folder(*folder)?;
            imap.append(&target.name, *seen, &raw).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
