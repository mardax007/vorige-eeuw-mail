//! What a backup and a restore need from the index (C14, decision 29; `crate::backup`).

use std::path::Path;

use rusqlite::params;

use super::Store;
use crate::error::{Code, Result, VemError};

impl Store {
    /// A consistent copy of the index at `to`, made while the app runs (`VACUUM INTO`).
    pub fn backup_index(&self, to: &Path) -> Result<()> {
        self.db()
            .execute("VACUUM INTO ?1", [to.to_string_lossy()])
            .map_err(|e| VemError::new(Code::BackupWrite01).with_detail(e.to_string()))?;
        Ok(())
    }

    /// How many messages the index holds, for the description file of a backup.
    pub fn message_count(&self) -> Result<u64> {
        let n: i64 = self.db().query_row("SELECT COUNT(*) FROM messages WHERE hidden = 0", [], |r| r.get(0))?;
        Ok(n as u64)
    }

    /// Right after a restore. Nothing that was waiting at backup time may reach a server now:
    /// queued actions, unsent mail and undo tokens go. The kept original files moved with the
    /// data folder, so their paths follow from `old_root` to this store's root.
    pub fn after_restore(&self, old_root: &str) -> Result<()> {
        let db = self.db();
        db.execute_batch("DELETE FROM pending_ops; DELETE FROM outbox; DELETE FROM undo;")?;
        let old = Path::new(old_root).join("originals");
        let new = self.originals_dir();
        let rows: Vec<(String, String)> = {
            let mut st = db.prepare("SELECT id, path FROM originals")?;
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?
        };
        for (id, path) in rows {
            if let Ok(rest) = Path::new(&path).strip_prefix(&old) {
                db.execute("UPDATE originals SET path = ?2 WHERE id = ?1", params![id, new.join(rest).to_string_lossy()])?;
            }
        }
        Ok(())
    }

    pub fn kv_delete(&self, key: &str) -> Result<()> {
        self.db().execute("DELETE FROM kv WHERE key = ?1", [key])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::imap::ops::Op;
    use crate::model::AccountKind;

    #[test]
    fn after_restore_drops_waiting_work_and_moves_original_paths() {
        let (dir, s) = crate::store::testutil::store();
        let a = s.add_account("J", "j@x.nl", AccountKind::Imap, None).unwrap();
        s.push_op(&a.id, &Op::Create { folder: 1 }).unwrap();
        let old_root = Path::new("C:/oud/data");
        s.add_original("Outlook.pst", &old_root.join("originals").join("x").join("Outlook.pst"), 10).unwrap();
        s.after_restore(&old_root.to_string_lossy()).unwrap();
        assert!(s.ops::<Op>(&a.id).unwrap().is_empty());
        let path: String = s.db().query_row("SELECT path FROM originals", [], |r| r.get(0)).unwrap();
        assert_eq!(Path::new(&path), dir.path().join("originals").join("x").join("Outlook.pst"));
    }
}
