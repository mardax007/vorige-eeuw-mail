//! A backup on a USB stick, and putting one back (C14, decision 29). Offered on Windows only.
//!
//! A backup is a folder "Vorige Eeuw Mail reservekopie <date>" holding a copy of the index, every
//! message file, the kept original import files and a description file. Never the `secrets`
//! folder (passwords, tokens, the pairing key) or the log. It is written under a `.bezig` name and
//! renamed only when complete, so a half backup never looks like a good one.
//!
//! A restore runs in two steps, because the running app holds the index open: first the backup is
//! copied into a staging folder in the data folder, then the app restarts and [`apply_staged`]
//! swaps it in before the index opens. What was there before stays in `voor-terugzetten` until the
//! next restore.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::error::{Code, Result, VemError};
use crate::model::{iso, now};
use crate::store::Store;

pub const MANIFEST: &str = "reservekopie.json";
pub const NAME_PREFIX: &str = "Vorige Eeuw Mail reservekopie";
/// Where the data before the last restore is kept, inside the data folder.
pub const PREVIOUS: &str = "voor-terugzetten";
const FORMAT: u32 = 1;
const INDEX: &str = "index.sqlite";
/// The folders of the data folder a backup holds, next to the index.
const FOLDERS: [&str; 2] = ["mail", "originals"];
const PARTIAL: &str = ".bezig";
const STAGING: &str = "terugzetten";
const READY: &str = "klaar";
/// A progress event at least every this many bytes.
const STEP_BYTES: u64 = 8 * 1024 * 1024;

/// The description file of a backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: u32,
    pub app_version: String,
    pub created_at: i64,
    /// The data folder it came from, to move the kept original files' paths along on restore.
    pub data_root: String,
    pub accounts: Vec<BackupAccount>,
    pub messages: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupAccount {
    pub name: String,
    pub email: String,
}

/// What the screen shows before it asks "Terugzetten?".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub path: String,
    pub created_at: String,
    pub accounts: Vec<BackupAccount>,
    pub messages: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupProgress {
    /// True while a backup is put back, false while one is made.
    pub restoring: bool,
    pub done_bytes: u64,
    pub total_bytes: u64,
    pub finished: bool,
    /// "Stoppen" was pressed; nothing half-done is left behind.
    pub stopped: bool,
    pub error: Option<VemError>,
    /// Only in the last event of a backup: the folder it wrote.
    pub folder: Option<String>,
}

fn write_error(e: std::io::Error) -> VemError {
    VemError::new(Code::BackupWrite01).with_detail(format!("{:?}: {e}", e.kind()))
}

fn read_error(detail: impl Into<String>) -> VemError {
    VemError::new(Code::BackupRead01).with_detail(detail.into())
}

/// Every file under `root`, as (path relative to `root`, size). A missing folder has none.
fn files_under(root: &Path) -> std::io::Result<Vec<(PathBuf, u64)>> {
    let mut out = Vec::new();
    let mut todo = vec![PathBuf::new()];
    while let Some(rel) = todo.pop() {
        let dir = root.join(&rel);
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        for entry in entries {
            let entry = entry?;
            let kind = entry.file_type()?;
            let name = rel.join(entry.file_name());
            if kind.is_dir() {
                todo.push(name);
            } else if kind.is_file() && name.extension().is_none_or(|x| x != "tmp") {
                out.push((name, entry.metadata()?.len()));
            }
        }
    }
    Ok(out)
}

/// Copies files and reports progress. Returns false when "Stoppen" was pressed.
struct Copier<'a> {
    done: u64,
    total: u64,
    last_emit: u64,
    restoring: bool,
    emit: &'a dyn Fn(BackupProgress),
    cancel: &'a AtomicBool,
}

impl Copier<'_> {
    fn progress(&mut self, force: bool) {
        if force || self.done - self.last_emit >= STEP_BYTES {
            self.last_emit = self.done;
            (self.emit)(BackupProgress { restoring: self.restoring, done_bytes: self.done, total_bytes: self.total, ..Default::default() });
        }
    }

    /// Copy `files` from `from` to `to`. A file that disappeared meanwhile (mail deleted while the
    /// backup runs) is skipped; the index copy no longer needs it either.
    fn copy(&mut self, from: &Path, to: &Path, files: &[(PathBuf, u64)], fail: fn(std::io::Error) -> VemError) -> Result<bool> {
        for (rel, size) in files {
            if self.cancel.load(Ordering::Relaxed) {
                return Ok(false);
            }
            let target = to.join(rel);
            if let Some(dir) = target.parent() {
                std::fs::create_dir_all(dir).map_err(fail)?;
            }
            match std::fs::copy(from.join(rel), &target) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(fail(e)),
            }
            self.done += size;
            self.progress(false);
        }
        Ok(true)
    }
}

/// A folder name under `parent` that is not taken yet: "… 2026-11-18", then "… 2026-11-18 (2)".
fn free_name(parent: &Path, day: &str) -> PathBuf {
    let base = format!("{NAME_PREFIX} {day}");
    let mut n = 1;
    loop {
        let name = if n == 1 { base.clone() } else { format!("{base} ({n})") };
        let candidate = parent.join(&name);
        if !candidate.exists() && !parent.join(format!("{name}{PARTIAL}")).exists() {
            return candidate;
        }
        n += 1;
    }
}

/// Make a backup in a new folder under `parent` (the USB stick). Returns the folder, or None when
/// "Stoppen" was pressed; a stopped or failed backup removes what it wrote.
pub fn write(store: &Store, parent: &Path, day: &str, emit: &dyn Fn(BackupProgress), cancel: &AtomicBool) -> Result<Option<PathBuf>> {
    let target = free_name(parent, day);
    let work = PathBuf::from(format!("{}{PARTIAL}", target.to_string_lossy()));
    let result = write_into(store, &work, emit, cancel);
    match result {
        Ok(true) => {
            std::fs::rename(&work, &target).map_err(write_error)?;
            crate::log::event("backup.written", &[]);
            Ok(Some(target))
        }
        Ok(false) => {
            let _ = std::fs::remove_dir_all(&work);
            crate::log::event("backup.stopped", &[]);
            Ok(None)
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&work);
            Err(e)
        }
    }
}

fn write_into(store: &Store, work: &Path, emit: &dyn Fn(BackupProgress), cancel: &AtomicBool) -> Result<bool> {
    std::fs::create_dir_all(work).map_err(write_error)?;
    store.backup_index(&work.join(INDEX))?;
    let root = store.root();
    let mut plan = Vec::new();
    for folder in FOLDERS {
        plan.push((folder, files_under(&root.join(folder)).map_err(write_error)?));
    }
    let index_size = std::fs::metadata(work.join(INDEX)).map(|m| m.len()).unwrap_or(0);
    let total = index_size + plan.iter().flat_map(|(_, f)| f.iter()).map(|(_, s)| s).sum::<u64>();
    let mut copier = Copier { done: index_size, total, last_emit: 0, restoring: false, emit, cancel };
    copier.progress(true);
    for (folder, files) in &plan {
        if !copier.copy(&root.join(folder), &work.join(folder), files, write_error)? {
            return Ok(false);
        }
    }
    let manifest = Manifest {
        format: FORMAT,
        app_version: crate::version().to_string(),
        created_at: now(),
        data_root: root.to_string_lossy().into_owned(),
        accounts: store.accounts()?.into_iter().map(|a| BackupAccount { name: a.display_name, email: a.email }).collect(),
        messages: store.message_count()?,
        bytes: total,
    };
    let text = serde_json::to_vec_pretty(&manifest).map_err(|e| VemError::new(Code::AppCmd01).with_detail(e.to_string()))?;
    std::fs::write(work.join(MANIFEST), text).map_err(write_error)?;
    copier.progress(true);
    Ok(true)
}

fn read_manifest(dir: &Path) -> Result<Manifest> {
    let text = std::fs::read(dir.join(MANIFEST)).map_err(|e| read_error(format!("no {MANIFEST}: {:?}", e.kind())))?;
    let manifest: Manifest = serde_json::from_slice(&text).map_err(|e| read_error(format!("{MANIFEST}: {e}")))?;
    if manifest.format > FORMAT {
        return Err(read_error(format!("made by a newer version ({})", manifest.app_version)));
    }
    Ok(manifest)
}

/// Check a chosen folder before anything is replaced: its description, and an index that opens
/// and passes SQLite's own check.
pub fn inspect(dir: &Path) -> Result<BackupInfo> {
    let manifest = read_manifest(dir)?;
    let index = dir.join(INDEX);
    let conn = rusqlite::Connection::open_with_flags(&index, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| read_error(format!("index: {e}")))?;
    let check: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0)).map_err(|e| read_error(format!("index: {e}")))?;
    if check != "ok" {
        return Err(read_error(format!("index: {check}")));
    }
    Ok(BackupInfo {
        path: dir.to_string_lossy().into_owned(),
        created_at: iso(manifest.created_at),
        accounts: manifest.accounts,
        messages: manifest.messages,
        bytes: manifest.bytes,
    })
}

/// Step one of a restore: copy the backup into the data folder's staging folder. The swap happens
/// at the next start ([`apply_staged`]). Returns false when "Stoppen" was pressed.
pub fn stage_restore(data_root: &Path, backup: &Path, emit: &dyn Fn(BackupProgress), cancel: &AtomicBool) -> Result<bool> {
    inspect(backup)?;
    let staging = data_root.join(STAGING);
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(|e| VemError::new(Code::StoreWrite01).with_detail(e.kind().to_string()))?;
    }
    let read_fail = |e: std::io::Error| read_error(format!("{:?}: {e}", e.kind()));
    let mut files = vec![(PathBuf::from(INDEX), std::fs::metadata(backup.join(INDEX)).map_err(read_fail)?.len())];
    files.push((PathBuf::from(MANIFEST), 0));
    for folder in FOLDERS {
        let under = files_under(&backup.join(folder)).map_err(read_fail)?;
        files.extend(under.into_iter().map(|(rel, size)| (Path::new(folder).join(rel), size)));
    }
    let total = files.iter().map(|(_, s)| s).sum();
    let mut copier = Copier { done: 0, total, last_emit: 0, restoring: true, emit, cancel };
    copier.progress(true);
    let done = copier.copy(backup, &staging, &files, |e| read_error(format!("{:?}: {e}", e.kind())));
    match done {
        Ok(true) => {
            std::fs::write(staging.join(READY), b"").map_err(|e| VemError::new(Code::StoreWrite01).with_detail(e.kind().to_string()))?;
            crate::log::event("restore.staged", &[]);
            Ok(true)
        }
        Ok(false) => {
            let _ = std::fs::remove_dir_all(&staging);
            Ok(false)
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staging);
            Err(e)
        }
    }
}

/// Step two, at start before the index opens: swap a complete staged backup in. Returns its
/// description, after which the caller runs `Store::after_restore`. A half-copied staging folder
/// (the app stopped meanwhile) is removed and nothing changes.
pub fn apply_staged(data_root: &Path) -> Result<Option<Manifest>> {
    let staging = data_root.join(STAGING);
    if !staging.exists() {
        return Ok(None);
    }
    let fail = |e: std::io::Error| VemError::new(Code::StoreWrite01).with_detail(format!("restore: {:?}", e.kind()));
    if !staging.join(READY).exists() {
        std::fs::remove_dir_all(&staging).map_err(fail)?;
        return Ok(None);
    }
    let manifest = read_manifest(&staging)?;
    let previous = data_root.join(PREVIOUS);
    if previous.exists() {
        std::fs::remove_dir_all(&previous).map_err(fail)?;
    }
    std::fs::create_dir_all(&previous).map_err(fail)?;
    for name in [INDEX, "index.sqlite-wal", "index.sqlite-shm", FOLDERS[0], FOLDERS[1]] {
        let here = data_root.join(name);
        if here.exists() {
            std::fs::rename(&here, previous.join(name)).map_err(fail)?;
        }
    }
    for name in [INDEX, FOLDERS[0], FOLDERS[1]] {
        let staged = staging.join(name);
        if staged.exists() {
            std::fs::rename(&staged, data_root.join(name)).map_err(fail)?;
        }
    }
    std::fs::remove_dir_all(&staging).map_err(fail)?;
    crate::log::event("restore.applied", &[]);
    Ok(Some(manifest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AccountKind;
    use crate::store::NewMessage;

    fn filled_store(root: &Path) -> Store {
        let s = Store::open(root).unwrap();
        let a = s.add_account("Oma", "oma@solcon.test", AccountKind::Imap, None).unwrap();
        let f = s.upsert_folder(&a.id, "INBOX", Some("/"), Some(crate::model::FolderRole::Inbox), true).unwrap();
        let raw = b"From: kees@voorbeeld.nl\r\nSubject: Koffie\r\n\r\nKom je?\r\n".to_vec();
        s.insert_message(&NewMessage { account_id: a.id.clone(), folder_id: f, raw, has_body: true, unread: true, ..Default::default() }).unwrap();
        std::fs::create_dir_all(root.join("secrets")).unwrap();
        std::fs::write(root.join("secrets").join("geheim.bin"), b"wachtwoord").unwrap();
        s
    }

    #[test]
    fn a_backup_holds_index_mail_and_description_but_no_secrets() {
        let data = tempfile::tempdir().unwrap();
        let stick = tempfile::tempdir().unwrap();
        let s = filled_store(data.path());
        let events = std::sync::Mutex::new(Vec::new());
        let folder = write(&s, stick.path(), "2026-11-18", &|p| events.lock().unwrap().push(p), &AtomicBool::new(false)).unwrap().unwrap();
        assert_eq!(folder.file_name().unwrap(), "Vorige Eeuw Mail reservekopie 2026-11-18");
        assert!(folder.join(INDEX).exists());
        assert_eq!(files_under(&folder.join("mail")).unwrap().len(), 1);
        assert!(!folder.join("secrets").exists(), "never the secrets");
        let info = inspect(&folder).unwrap();
        assert_eq!(info.messages, 1);
        assert_eq!(info.accounts[0].email, "oma@solcon.test");
        let last = events.lock().unwrap().last().cloned().unwrap();
        assert_eq!(last.done_bytes, last.total_bytes);

        // A second backup the same day gets its own folder.
        let second = write(&s, stick.path(), "2026-11-18", &|_| {}, &AtomicBool::new(false)).unwrap().unwrap();
        assert_eq!(second.file_name().unwrap(), "Vorige Eeuw Mail reservekopie 2026-11-18 (2)");
    }

    #[test]
    fn stopping_leaves_nothing_behind() {
        let data = tempfile::tempdir().unwrap();
        let stick = tempfile::tempdir().unwrap();
        let s = filled_store(data.path());
        let stopped = write(&s, stick.path(), "2026-11-18", &|_| {}, &AtomicBool::new(true)).unwrap();
        assert!(stopped.is_none());
        assert_eq!(std::fs::read_dir(stick.path()).unwrap().count(), 0);
    }

    #[test]
    fn a_folder_that_is_no_backup_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(inspect(dir.path()).unwrap_err().code(), Some(Code::BackupRead01));
        std::fs::write(dir.path().join(MANIFEST), b"{\"format\": 99}").unwrap();
        assert_eq!(inspect(dir.path()).unwrap_err().code(), Some(Code::BackupRead01));
    }

    #[test]
    fn a_restore_swaps_the_backup_in_at_the_next_start_and_keeps_the_old_data() {
        let old = tempfile::tempdir().unwrap();
        let stick = tempfile::tempdir().unwrap();
        let folder = {
            let s = filled_store(old.path());
            write(&s, stick.path(), "2026-11-18", &|_| {}, &AtomicBool::new(false)).unwrap().unwrap()
        };
        // Another PC, with its own (empty) store and its own secrets.
        let new = tempfile::tempdir().unwrap();
        drop(Store::open(new.path()).unwrap());
        std::fs::create_dir_all(new.path().join("secrets")).unwrap();
        std::fs::write(new.path().join("secrets").join("eigen.bin"), b"x").unwrap();
        assert!(stage_restore(new.path(), &folder, &|_| {}, &AtomicBool::new(false)).unwrap());

        let manifest = apply_staged(new.path()).unwrap().unwrap();
        assert_eq!(manifest.data_root, old.path().to_string_lossy());
        let s = Store::open(new.path()).unwrap();
        s.after_restore(&manifest.data_root).unwrap();
        assert_eq!(s.message_count().unwrap(), 1);
        assert_eq!(s.accounts().unwrap()[0].email, "oma@solcon.test");
        assert!(new.path().join(PREVIOUS).join(INDEX).exists(), "the data before stays");
        assert!(new.path().join("secrets").join("eigen.bin").exists(), "secrets are not touched");
        assert!(!new.path().join(STAGING).exists());
        assert!(apply_staged(new.path()).unwrap().is_none(), "only once");
    }

    #[test]
    fn a_half_staged_restore_changes_nothing() {
        let data = tempfile::tempdir().unwrap();
        drop(filled_store(data.path()));
        std::fs::create_dir_all(data.path().join(STAGING).join("mail")).unwrap();
        assert!(apply_staged(data.path()).unwrap().is_none());
        assert!(!data.path().join(STAGING).exists());
        assert_eq!(Store::open(data.path()).unwrap().message_count().unwrap(), 1);
    }
}
