//! The local store: one SQLite index (with FTS5 search) plus every message as an `.eml` file.
//!
//! Passwords and tokens never go in here (CLAUDE.md, Secrets); they live in `secrets`.

mod accounts;
mod contacts;
mod folders;
mod kv;
mod messages;
mod outbox;
mod search;
mod tags;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;

use crate::error::{Code, Result, VemError};

pub use messages::{NewMessage, StoredMessage};
pub use outbox::OutboxRow;

const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS accounts (
    id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    email TEXT NOT NULL,
    kind TEXT NOT NULL,
    settings_json TEXT,
    signature TEXT,
    quota_used_kb INTEGER,
    quota_limit_kb INTEGER,
    last_error_json TEXT,
    server_saves_sent INTEGER,
    capabilities TEXT,
    position INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS folders (
    id INTEGER PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    delimiter TEXT,
    role TEXT,
    uidvalidity INTEGER,
    uidnext INTEGER,
    highest_modseq INTEGER,
    selectable INTEGER NOT NULL DEFAULT 1,
    keywords_ok INTEGER NOT NULL DEFAULT 0,
    server_total INTEGER,
    UNIQUE(account_id, name)
);
CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    folder_id INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    uid INTEGER,
    message_id_hdr TEXT,
    in_reply_to TEXT,
    references_json TEXT,
    subject TEXT NOT NULL DEFAULT '',
    from_name TEXT,
    from_email TEXT NOT NULL DEFAULT '',
    to_json TEXT NOT NULL DEFAULT '[]',
    cc_json TEXT NOT NULL DEFAULT '[]',
    date INTEGER NOT NULL DEFAULT 0,
    preview TEXT NOT NULL DEFAULT '',
    unread INTEGER NOT NULL DEFAULT 1,
    important INTEGER NOT NULL DEFAULT 0,
    has_attachments INTEGER NOT NULL DEFAULT 0,
    size INTEGER NOT NULL DEFAULT 0,
    eml_path TEXT,
    has_body INTEGER NOT NULL DEFAULT 0,
    is_draft INTEGER NOT NULL DEFAULT 0,
    draft_json TEXT,
    hidden INTEGER NOT NULL DEFAULT 0,
    remote_images_ok INTEGER NOT NULL DEFAULT 0
);
CREATE UNIQUE INDEX IF NOT EXISTS messages_folder_uid ON messages(folder_id, uid) WHERE uid IS NOT NULL;
CREATE INDEX IF NOT EXISTS messages_folder_date ON messages(folder_id, hidden, date DESC);
CREATE INDEX IF NOT EXISTS messages_account_date ON messages(account_id, hidden, date DESC);
CREATE INDEX IF NOT EXISTS messages_date ON messages(hidden, date DESC);
CREATE INDEX IF NOT EXISTS messages_from ON messages(from_email);
CREATE INDEX IF NOT EXISTS messages_nobody ON messages(has_body, date DESC) WHERE has_body = 0;
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    subject, addrs, body, attach,
    content='', contentless_delete=1,
    tokenize='unicode61 remove_diacritics 2',
    prefix='2 3'
);
CREATE TABLE IF NOT EXISTS tags (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    color TEXT NOT NULL,
    keyword TEXT NOT NULL UNIQUE
);
CREATE TABLE IF NOT EXISTS message_tags (
    message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (message_id, tag_id)
);
CREATE INDEX IF NOT EXISTS message_tags_tag ON message_tags(tag_id);
CREATE TABLE IF NOT EXISTS contacts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    emails_json TEXT NOT NULL,
    phone TEXT,
    note TEXT,
    changed_at INTEGER NOT NULL,
    deleted INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS contact_emails (
    email TEXT NOT NULL COLLATE NOCASE,
    contact_id TEXT NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
    PRIMARY KEY (email, contact_id)
);
CREATE TABLE IF NOT EXISTS outbox (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    draft_json TEXT NOT NULL,
    subject TEXT NOT NULL,
    send_at INTEGER NOT NULL,
    state TEXT NOT NULL,
    error_json TEXT,
    attempts INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS pending_ops (
    id INTEGER PRIMARY KEY,
    account_id TEXT NOT NULL,
    op_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS undo (
    token TEXT PRIMARY KEY,
    json TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS originals (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    path TEXT NOT NULL,
    size INTEGER NOT NULL,
    imported_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS kv (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

pub struct Store {
    conn: Mutex<Connection>,
    root: PathBuf,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").field("root", &self.root).finish()
    }
}

impl Store {
    /// Open (or create) the store in `root`, the app's data folder.
    pub fn open(root: &Path) -> Result<Store> {
        std::fs::create_dir_all(root.join("mail"))?;
        let conn = Connection::open(root.join("index.sqlite"))
            .map_err(|e| VemError::new(Code::StoreRead01).with_detail(e.to_string()))?;
        Self::init(conn, root.to_path_buf())
    }

    /// An in-memory index with files under `root`; for tests.
    pub fn open_in_memory(root: &Path) -> Result<Store> {
        std::fs::create_dir_all(root.join("mail"))?;
        let conn = Connection::open_in_memory()?;
        Self::init(conn, root.to_path_buf())
    }

    fn init(conn: Connection, root: PathBuf) -> Result<Store> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;
             PRAGMA temp_store=MEMORY; PRAGMA cache_size=-32000;",
        )?;
        conn.execute_batch(SCHEMA)?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < SCHEMA_VERSION {
            conn.execute_batch(&format!("PRAGMA user_version={SCHEMA_VERSION}"))?;
        }
        Ok(Store { conn: Mutex::new(conn), root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn db(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Where message files live.
    pub fn mail_dir(&self) -> PathBuf {
        self.root.join("mail")
    }

    pub fn originals_dir(&self) -> PathBuf {
        self.root.join("originals")
    }
}

pub(crate) fn json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".into())
}

pub(crate) fn from_json<T: serde::de::DeserializeOwned + Default>(s: Option<String>) -> T {
    s.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub(crate) fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[cfg(test)]
pub(crate) mod testutil {
    use super::*;

    pub fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open_in_memory(dir.path()).unwrap();
        (dir, store)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn opens_on_disk_twice() {
        let dir = tempfile::tempdir().unwrap();
        drop(super::Store::open(dir.path()).unwrap());
        let s = super::Store::open(dir.path()).unwrap();
        let v: i64 = s.db().query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, super::SCHEMA_VERSION);
    }
}
