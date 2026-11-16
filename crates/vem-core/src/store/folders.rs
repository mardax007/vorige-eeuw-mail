use rusqlite::{OptionalExtension, params};

use super::Store;
use crate::error::{Code, Result, VemError};
use crate::model::{Folder, FolderRole};

/// What sync remembers about a folder on the server (docs/sync.md, Identity).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FolderSync {
    pub uidvalidity: Option<u32>,
    pub uidnext: Option<u32>,
    pub highest_modseq: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct FolderInfo {
    pub id: i64,
    pub account_id: String,
    pub name: String,
    pub delimiter: Option<String>,
    pub role: Option<FolderRole>,
    pub sync: FolderSync,
    pub keywords_ok: bool,
}

pub fn folder_key(id: i64) -> String {
    id.to_string()
}

pub fn parse_folder_key(key: &str) -> Result<i64> {
    key.parse().map_err(|_| VemError::new(Code::StoreRead01).with_detail("folder"))
}

impl Store {
    /// Add a folder or update its name details; returns its id. The role is set only when given,
    /// so a role found once is not lost when a later LIST lacks the flag.
    pub fn upsert_folder(
        &self,
        account_id: &str,
        name: &str,
        delimiter: Option<&str>,
        role: Option<FolderRole>,
        selectable: bool,
    ) -> Result<i64> {
        let db = self.db();
        db.execute(
            "INSERT INTO folders (account_id, name, delimiter, role, selectable)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(account_id, name) DO UPDATE SET
                delimiter = excluded.delimiter,
                role = COALESCE(excluded.role, folders.role),
                selectable = excluded.selectable",
            params![account_id, name, delimiter, role.map(FolderRole::as_str), selectable as i64],
        )?;
        let id = db.query_row(
            "SELECT id FROM folders WHERE account_id = ?1 AND name = ?2",
            params![account_id, name],
            |r| r.get(0),
        )?;
        Ok(id)
    }

    pub fn folders(&self, account_id: Option<&str>) -> Result<Vec<Folder>> {
        let db = self.db();
        let mut st = db.prepare(
            "SELECT f.id, f.account_id, f.name, f.delimiter, f.role,
                    COALESCE(SUM(CASE WHEN m.unread = 1 AND m.hidden = 0 THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN m.hidden = 0 THEN 1 ELSE 0 END), 0)
             FROM folders f
             JOIN accounts a ON a.id = f.account_id
             LEFT JOIN messages m ON m.folder_id = f.id
             WHERE f.selectable = 1 AND (?1 IS NULL OR f.account_id = ?1)
             GROUP BY f.id
             ORDER BY a.position,
                CASE f.role WHEN 'inbox' THEN 0 WHEN 'drafts' THEN 1 WHEN 'sent' THEN 2
                    WHEN 'outbox' THEN 3 WHEN 'archive' THEN 4 WHEN 'junk' THEN 5 WHEN 'trash' THEN 6
                    ELSE 7 END,
                f.name COLLATE NOCASE",
        )?;
        let rows = st
            .query_map(params![account_id], |r| {
                let name: String = r.get(2)?;
                let delim: Option<String> = r.get(3)?;
                let role: Option<String> = r.get(4)?;
                let shown = crate::imap::utf7::decode(&name);
                let path = match delim.as_deref().filter(|d| !d.is_empty()) {
                    Some(d) => shown.split(d).map(str::to_string).collect(),
                    None => vec![shown],
                };
                Ok(Folder {
                    id: folder_key(r.get(0)?),
                    account_id: r.get(1)?,
                    name,
                    role: role.as_deref().and_then(FolderRole::parse),
                    unread: r.get::<_, i64>(5)? as u32,
                    total: r.get::<_, i64>(6)? as u32,
                    path,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn folder(&self, id: i64) -> Result<FolderInfo> {
        self.db()
            .query_row(
                "SELECT id, account_id, name, delimiter, role, uidvalidity, uidnext, highest_modseq, keywords_ok
                 FROM folders WHERE id = ?1",
                [id],
                |r| {
                    let role: Option<String> = r.get(4)?;
                    Ok(FolderInfo {
                        id: r.get(0)?,
                        account_id: r.get(1)?,
                        name: r.get(2)?,
                        delimiter: r.get(3)?,
                        role: role.as_deref().and_then(FolderRole::parse),
                        sync: FolderSync {
                            uidvalidity: r.get::<_, Option<i64>>(5)?.map(|v| v as u32),
                            uidnext: r.get::<_, Option<i64>>(6)?.map(|v| v as u32),
                            highest_modseq: r.get::<_, Option<i64>>(7)?.map(|v| v as u64),
                        },
                        keywords_ok: r.get::<_, i64>(8)? != 0,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| VemError::new(Code::StoreRead01).with_detail("folder"))
    }

    pub fn folder_infos(&self, account_id: &str) -> Result<Vec<FolderInfo>> {
        let ids: Vec<i64> = {
            let db = self.db();
            let mut st = db.prepare("SELECT id FROM folders WHERE account_id = ?1 AND selectable = 1")?;
            st.query_map([account_id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?
        };
        ids.into_iter().map(|id| self.folder(id)).collect()
    }

    pub fn folder_by_role(&self, account_id: &str, role: FolderRole) -> Result<Option<i64>> {
        Ok(self
            .db()
            .query_row(
                "SELECT id FROM folders WHERE account_id = ?1 AND role = ?2 ORDER BY id LIMIT 1",
                params![account_id, role.as_str()],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn set_folder_sync(&self, id: i64, sync: &FolderSync) -> Result<()> {
        self.db().execute(
            "UPDATE folders SET uidvalidity = ?2, uidnext = ?3, highest_modseq = ?4 WHERE id = ?1",
            params![
                id,
                sync.uidvalidity.map(i64::from),
                sync.uidnext.map(i64::from),
                sync.highest_modseq.map(|v| v as i64)
            ],
        )?;
        Ok(())
    }

    pub fn set_folder_keywords_ok(&self, id: i64, ok: bool) -> Result<()> {
        self.db().execute("UPDATE folders SET keywords_ok = ?2 WHERE id = ?1", params![id, ok as i64])?;
        Ok(())
    }

    /// Drop every cached message of a folder (UIDVALIDITY changed: docs/sync.md).
    pub fn reset_folder(&self, id: i64) -> Result<()> {
        let paths = self.eml_paths_in_folder(id)?;
        {
            let db = self.db();
            db.execute("DELETE FROM messages_fts WHERE rowid IN (SELECT id FROM messages WHERE folder_id = ?1)", [id])?;
            db.execute("DELETE FROM messages WHERE folder_id = ?1 AND uid IS NOT NULL", [id])?;
        }
        for p in paths {
            let _ = std::fs::remove_file(self.mail_dir().join(p));
        }
        Ok(())
    }

    /// Remove folders that no longer exist on the server.
    pub fn remove_folders_except(&self, account_id: &str, keep: &[String]) -> Result<()> {
        let infos = self.folder_infos(account_id)?;
        for f in infos.into_iter().filter(|f| !keep.contains(&f.name)) {
            self.reset_folder(f.id)?;
            self.db().execute("DELETE FROM folders WHERE id = ?1", [f.id])?;
        }
        Ok(())
    }

    /// A folder in a local account, made on first use. `path` parts are joined with "/".
    pub fn ensure_local_folder(&self, account_id: &str, path: &[String], role: Option<FolderRole>) -> Result<i64> {
        let name = if path.is_empty() { "Postvak IN".to_string() } else { path.join("/") };
        self.upsert_folder(account_id, &name, Some("/"), role, true)
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{AccountKind, FolderRole};
    use crate::store::testutil::store;

    #[test]
    fn upsert_keeps_role_and_orders_inbox_first() {
        let (_d, s) = store();
        let a = s.add_account("J", "j@x.nl", AccountKind::Imap, None).unwrap();
        let zz = s.upsert_folder(&a.id, "Zaken", Some("/"), None, true).unwrap();
        let inbox = s.upsert_folder(&a.id, "INBOX", Some("/"), Some(FolderRole::Inbox), true).unwrap();
        assert_eq!(s.upsert_folder(&a.id, "INBOX", Some("/"), None, true).unwrap(), inbox);
        s.upsert_folder(&a.id, "Werk/Facturen", Some("/"), None, true).unwrap();
        let f = s.folders(Some(&a.id)).unwrap();
        assert_eq!(f[0].role, Some(FolderRole::Inbox));
        assert_eq!(f[2].path, vec!["Zaken"]);
        assert_eq!(f[1].path, vec!["Werk", "Facturen"]);
        assert_eq!(s.folder_by_role(&a.id, FolderRole::Inbox).unwrap(), Some(inbox));
        s.remove_folders_except(&a.id, &["INBOX".into()]).unwrap();
        assert!(s.folder(zz).is_err());
    }
}
