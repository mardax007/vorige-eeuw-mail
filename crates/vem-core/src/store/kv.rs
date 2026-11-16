//! Small values: settings, the install number, undo tokens, pending server actions, original
//! import files.

use rusqlite::{OptionalExtension, params};
use serde::{Serialize, de::DeserializeOwned};

use super::{Store, json, new_id};
use crate::error::{Code, Result, VemError};
use crate::model::{OriginalCopy, Settings, iso, now};

/// Undo tokens live this long (the bar shows about ten seconds; docs/screens.md).
pub const UNDO_KEEP_SECS: i64 = 120;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingOp<T> {
    pub id: i64,
    pub account_id: String,
    pub op: T,
    pub attempts: u32,
}

impl Store {
    pub fn kv_get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let v: Option<String> = self
            .db()
            .query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get(0))
            .optional()?;
        Ok(v.and_then(|v| serde_json::from_str(&v).ok()))
    }

    pub fn kv_set<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.db().execute(
            "INSERT INTO kv (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, json(value)],
        )?;
        Ok(())
    }

    pub fn settings(&self) -> Result<Settings> {
        Ok(self.kv_get::<Settings>("settings")?.unwrap_or_default())
    }

    pub fn save_settings(&self, s: &Settings) -> Result<Settings> {
        let s = s.clone().clamped();
        self.kv_set("settings", &s)?;
        Ok(s)
    }

    /// A random number made at first start; in every report (PLAN.md §5).
    pub fn install_id(&self) -> Result<String> {
        if let Some(id) = self.kv_get::<String>("install_id")? {
            return Ok(id);
        }
        let id = new_id()[..12].to_string();
        self.kv_set("install_id", &id)?;
        Ok(id)
    }

    pub fn put_undo<T: Serialize>(&self, value: &T) -> Result<String> {
        let token = new_id();
        let db = self.db();
        db.execute("DELETE FROM undo WHERE created_at < ?1", [now() - UNDO_KEEP_SECS])?;
        db.execute(
            "INSERT INTO undo (token, json, created_at) VALUES (?1, ?2, ?3)",
            params![token, json(value), now()],
        )?;
        Ok(token)
    }

    pub fn take_undo<T: DeserializeOwned>(&self, token: &str) -> Result<Option<T>> {
        let db = self.db();
        let v: Option<String> = db
            .query_row(
                "SELECT json FROM undo WHERE token = ?1 AND created_at >= ?2",
                params![token, now() - UNDO_KEEP_SECS],
                |r| r.get(0),
            )
            .optional()?;
        db.execute("DELETE FROM undo WHERE token = ?1", [token])?;
        Ok(v.and_then(|v| serde_json::from_str(&v).ok()))
    }

    pub fn push_op<T: Serialize>(&self, account_id: &str, op: &T) -> Result<i64> {
        let db = self.db();
        db.execute(
            "INSERT INTO pending_ops (account_id, op_json, created_at) VALUES (?1, ?2, ?3)",
            params![account_id, json(op), now()],
        )?;
        Ok(db.last_insert_rowid())
    }

    pub fn ops<T: DeserializeOwned>(&self, account_id: &str) -> Result<Vec<PendingOp<T>>> {
        let db = self.db();
        let mut st = db.prepare("SELECT id, op_json, attempts FROM pending_ops WHERE account_id = ?1 ORDER BY id")?;
        let rows = st
            .query_map([account_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .filter_map(|(id, j, attempts)| {
                serde_json::from_str(&j).ok().map(|op| PendingOp {
                    id,
                    account_id: account_id.to_string(),
                    op,
                    attempts: attempts as u32,
                })
            })
            .collect())
    }

    pub fn op_done(&self, id: i64) -> Result<()> {
        self.db().execute("DELETE FROM pending_ops WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Change a queued action in place (it keeps its place in the queue).
    pub fn replace_op<T: Serialize>(&self, id: i64, op: &T) -> Result<()> {
        self.db().execute("UPDATE pending_ops SET op_json = ?2 WHERE id = ?1", params![id, json(op)])?;
        Ok(())
    }

    pub fn op_failed(&self, id: i64) -> Result<()> {
        self.db().execute("UPDATE pending_ops SET attempts = attempts + 1 WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn add_original(&self, label: &str, path: &std::path::Path, size: u64) -> Result<String> {
        let id = new_id();
        self.db().execute(
            "INSERT INTO originals (id, label, path, size, imported_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, label, path.to_string_lossy(), size as i64, now()],
        )?;
        Ok(id)
    }

    pub fn originals(&self) -> Result<Vec<OriginalCopy>> {
        let db = self.db();
        let mut st = db.prepare("SELECT id, label, size, imported_at FROM originals ORDER BY imported_at DESC")?;
        let rows = st
            .query_map([], |r| {
                Ok(OriginalCopy {
                    id: r.get(0)?,
                    label: r.get(1)?,
                    size: r.get::<_, i64>(2)? as u64,
                    imported_at: iso(r.get(3)?),
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    /// Delete a kept copy of an original import file. Only paths inside our own originals
    /// folder are ever removed; the user's source file is never touched (CLAUDE.md, Mail safety).
    pub fn delete_original(&self, id: &str) -> Result<()> {
        let path: Option<String> = self
            .db()
            .query_row("SELECT path FROM originals WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        let Some(path) = path else { return Ok(()) };
        let path = std::path::PathBuf::from(path);
        let ours = self.originals_dir();
        let inside = match (path.canonicalize(), ours.canonicalize()) {
            (Ok(p), Ok(o)) => p.starts_with(&o),
            _ => !path.exists(),
        };
        if !inside {
            return Err(VemError::new(Code::StoreWrite01).with_detail("not our copy"));
        }
        if path.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else if path.exists() {
            std::fs::remove_file(&path)?;
        }
        self.db().execute("DELETE FROM originals WHERE id = ?1", [id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::store::testutil::store;

    #[test]
    fn settings_and_install_id_persist() {
        let (_d, s) = store();
        assert_eq!(s.settings().unwrap().text_size, 18);
        let mut st = s.settings().unwrap();
        st.text_size = 24;
        s.save_settings(&st).unwrap();
        assert_eq!(s.settings().unwrap().text_size, 24);
        let id = s.install_id().unwrap();
        assert_eq!(id.len(), 12);
        assert_eq!(s.install_id().unwrap(), id);
    }

    #[test]
    fn undo_token_is_single_use() {
        let (_d, s) = store();
        let t = s.put_undo(&vec![1, 2, 3]).unwrap();
        assert_eq!(s.take_undo::<Vec<i32>>(&t).unwrap(), Some(vec![1, 2, 3]));
        assert_eq!(s.take_undo::<Vec<i32>>(&t).unwrap(), None);
    }

    #[test]
    fn ops_queue() {
        let (_d, s) = store();
        let id = s.push_op("a", &"x".to_string()).unwrap();
        s.op_failed(id).unwrap();
        let ops = s.ops::<String>("a").unwrap();
        assert_eq!((ops[0].op.as_str(), ops[0].attempts), ("x", 1));
        s.op_done(id).unwrap();
        assert!(s.ops::<String>("a").unwrap().is_empty());
    }

    #[test]
    fn originals_only_inside_our_folder() {
        let (d, s) = store();
        std::fs::create_dir_all(s.originals_dir()).unwrap();
        let ours = s.originals_dir().join("a.pst");
        std::fs::write(&ours, b"x").unwrap();
        let id = s.add_original("a.pst", &ours, 1).unwrap();
        let outside = d.path().join("user.pst");
        std::fs::write(&outside, b"y").unwrap();
        let bad = s.add_original("user.pst", &outside, 1).unwrap();
        assert_eq!(s.originals().unwrap().len(), 2);
        s.delete_original(&id).unwrap();
        assert!(!ours.exists());
        assert!(s.delete_original(&bad).is_err());
        assert!(outside.exists());
    }
}
