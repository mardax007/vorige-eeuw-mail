//! What the copy of the PC's local folders to the paired phone needs from the index
//! (docs/pc-to-phone.md). On the PC: the list of local mail it offers. On the phone: which local
//! account holds the copy of which PC account, and what the PC's values were at the last copy.

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::Store;
use crate::error::Result;
use crate::model::FolderRole;

/// A local account on the PC whose mail is offered to the phone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfferedAccount {
    pub id: String,
    pub name: String,
}

/// One message in a local folder on the PC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfferedMessage {
    pub account: String,
    pub id: i64,
    /// The folder's name in the local account ("Werk/Facturen").
    pub folder: String,
    pub role: Option<FolderRole>,
    pub unread: bool,
    pub important: bool,
    pub size: u64,
}

/// A copied message as the phone remembers it. `message_id` is None once the user deleted the
/// copy on the phone: it is then not sent again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyRow {
    pub pc_account: String,
    pub pc_id: i64,
    pub message_id: Option<i64>,
    pub folder: String,
    pub unread: bool,
    pub important: bool,
}

/// Local accounts that are themselves a copy from a PC are never offered on.
const OWN_LOCAL: &str = "a.kind = 'local' AND a.id NOT IN (SELECT account_id FROM lan_accounts)";

impl Store {
    /// The PC's own local accounts, in their order.
    pub fn offered_accounts(&self) -> Result<Vec<OfferedAccount>> {
        let db = self.db();
        let mut st = db.prepare(&format!(
            "SELECT a.id, a.display_name FROM accounts a WHERE {OWN_LOCAL} ORDER BY a.position, a.created_at, a.id"
        ))?;
        let rows = st
            .query_map([], |r| Ok(OfferedAccount { id: r.get(0)?, name: r.get(1)? }))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Every whole message in the PC's own local accounts.
    pub fn offered_messages(&self) -> Result<Vec<OfferedMessage>> {
        let db = self.db();
        let mut st = db.prepare(&format!(
            "SELECT m.account_id, m.id, f.name, f.role, m.unread, m.important, m.size
             FROM messages m JOIN folders f ON f.id = m.folder_id JOIN accounts a ON a.id = m.account_id
             WHERE {OWN_LOCAL} AND m.has_body = 1 AND m.is_draft = 0 AND m.hidden = 0
             ORDER BY m.id"
        ))?;
        let rows = st
            .query_map([], |r| {
                let role: Option<String> = r.get(3)?;
                Ok(OfferedMessage {
                    account: r.get(0)?,
                    id: r.get(1)?,
                    folder: r.get(2)?,
                    role: role.as_deref().and_then(FolderRole::parse),
                    unread: r.get::<_, i64>(4)? != 0,
                    important: r.get::<_, i64>(5)? != 0,
                    size: r.get::<_, i64>(6)?.max(0) as u64,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// The phone's local account that holds the copy of a PC account, with whether it still exists.
    pub fn copy_account(&self, pc_account: &str) -> Result<Option<(String, bool)>> {
        let row: Option<(String, i64)> = self
            .db()
            .query_row(
                "SELECT l.account_id, (SELECT COUNT(*) FROM accounts a WHERE a.id = l.account_id)
                 FROM lan_accounts l WHERE l.pc_account = ?1",
                [pc_account],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(row.map(|(id, n)| (id, n > 0)))
    }

    /// Every PC account the phone holds a copy of: (PC account, phone account).
    pub fn copy_accounts(&self) -> Result<Vec<(String, String)>> {
        let db = self.db();
        let mut st = db.prepare("SELECT pc_account, account_id FROM lan_accounts")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn set_copy_account(&self, pc_account: &str, account_id: &str) -> Result<()> {
        self.db().execute(
            "INSERT INTO lan_accounts (pc_account, account_id) VALUES (?1, ?2)
             ON CONFLICT(pc_account) DO UPDATE SET account_id = excluded.account_id",
            params![pc_account, account_id],
        )?;
        Ok(())
    }

    pub fn forget_copy_account(&self, pc_account: &str) -> Result<()> {
        let db = self.db();
        db.execute("DELETE FROM lan_messages WHERE pc_account = ?1", [pc_account])?;
        db.execute("DELETE FROM lan_accounts WHERE pc_account = ?1", [pc_account])?;
        Ok(())
    }

    /// Every copied message. A copy the user deleted on the phone comes back with no message id.
    pub fn copy_rows(&self) -> Result<Vec<CopyRow>> {
        let db = self.db();
        let mut st = db.prepare(
            "SELECT l.pc_account, l.pc_id, m.id, l.folder, l.unread, l.important
             FROM lan_messages l LEFT JOIN messages m ON m.id = l.message_id",
        )?;
        let rows = st
            .query_map([], |r| {
                Ok(CopyRow {
                    pc_account: r.get(0)?,
                    pc_id: r.get(1)?,
                    message_id: r.get(2)?,
                    folder: r.get(3)?,
                    unread: r.get::<_, i64>(4)? != 0,
                    important: r.get::<_, i64>(5)? != 0,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn put_copy_row(&self, row: &CopyRow) -> Result<()> {
        self.db().execute(
            "INSERT INTO lan_messages (pc_account, pc_id, message_id, folder, unread, important)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(pc_account, pc_id) DO UPDATE SET message_id = excluded.message_id,
               folder = excluded.folder, unread = excluded.unread, important = excluded.important",
            params![row.pc_account, row.pc_id, row.message_id, row.folder, row.unread as i64, row.important as i64],
        )?;
        Ok(())
    }

    pub fn forget_copy_row(&self, pc_account: &str, pc_id: i64) -> Result<()> {
        self.db()
            .execute("DELETE FROM lan_messages WHERE pc_account = ?1 AND pc_id = ?2", params![pc_account, pc_id])?;
        Ok(())
    }
}
