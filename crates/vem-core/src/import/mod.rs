//! Bringing in old mail (docs/PLAN.md section 4): PST, Windows Live Mail, Outlook Express, EML
//! folders and mbox. Sources are opened read-only and never changed; a copy of the original
//! goes into the app's data folder first (decision 15), and the messages are read from that copy.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{Code, Result, VemError};
use crate::imap::ops::Op;
use crate::log;
use crate::model::{FolderRole, ImportKind, ImportProgress, ImportSource, ImportTarget};
use crate::parse;
use crate::store::{NewMessage, Store};

pub mod account;
pub mod dbx;
pub mod export;
pub mod meta;
pub mod mbox;
pub mod pst;
mod pst_contact;
pub mod rescue;
pub mod vcard;
mod rtf;

/// Leave this much free on the disk after the copy and the converted mail.
const SPARE_BYTES: u64 = 500 * 1024 * 1024;
/// Converted mail takes about this much more than the source (index, one file per message).
const GROWTH_PERCENT: u64 = 120;
const PROGRESS_EVERY: u64 = 50;
/// Where imported mail goes inside a mailbox.
pub const MAILBOX_FOLDER: &str = "Oude mail";

#[derive(Debug, Default, Clone)]
pub struct ImportReport {
    pub added: u64,
    pub already_there: u64,
    /// Contacts from the file that went into the address book (PST only).
    pub contacts: u64,
    /// The source keeps no read state we can read (Windows Live Mail): everything came in as read.
    pub all_marked_read: bool,
    /// Read with the rescue tool: what was read and what not may be lost.
    pub rescued: bool,
    pub skipped: Vec<(String, String)>,
}

/// Size of a file, or of every file below a folder.
pub fn size_of(path: &Path) -> u64 {
    match std::fs::metadata(path) {
        Ok(m) if m.is_file() => m.len(),
        Ok(m) if m.is_dir() => std::fs::read_dir(path)
            .map(|rd| rd.flatten().map(|e| size_of(&e.path())).sum())
            .unwrap_or(0),
        _ => 0,
    }
}

#[cfg(windows)]
pub fn free_space(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let mut dir = path.to_path_buf();
    while !dir.exists() {
        dir = dir.parent()?.to_path_buf();
    }
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut free: u64 = 0;
    // SAFETY: `wide` is a NUL-terminated UTF-16 path; the out pointer is a valid u64.
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, std::ptr::null_mut(), std::ptr::null_mut()) };
    (ok != 0).then_some(free)
}

#[cfg(not(windows))]
pub fn free_space(_path: &Path) -> Option<u64> {
    None
}

/// The kind of a file or folder chosen by hand, from its name.
pub fn kind_of(path: &Path) -> Option<ImportKind> {
    if path.is_dir() {
        return Some(ImportKind::Eml);
    }
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "pst" => Some(ImportKind::Pst),
        "dbx" => Some(ImportKind::Dbx),
        "eml" => Some(ImportKind::Eml),
        "mbox" | "mbx" | "" => Some(ImportKind::Mbox),
        _ => None,
    }
}

pub fn source_for(path: &Path) -> Result<ImportSource> {
    // An .ost is Outlook's offline copy of a mailbox: the mail is still in that mailbox.
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ost")) {
        return Err(VemError::new(Code::ImportRead04));
    }
    let kind = kind_of(path).ok_or_else(|| VemError::new(Code::ImportRead03).with_detail(ext_detail(path)))?;
    Ok(ImportSource {
        kind,
        path: path.to_string_lossy().into_owned(),
        size: Some(size_of(path)),
        label: path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Oude mail".into()),
        protected: kind == ImportKind::Pst && pst_password(path).is_some(),
    })
}

/// The password check of a PST, read without changing the file. A file that cannot be read
/// counts as unprotected here; the import itself then reports what is wrong.
fn pst_password(path: &Path) -> Option<u32> {
    std::panic::catch_unwind(|| pst::password_crc(path)).ok()?.ok()?
}

/// Before an import: a PST with a password is only read with the right one (phase 3 task 3).
/// The password is not kept, logged or put in the error.
pub fn check_password(source: &ImportSource, password: Option<&str>) -> Result<()> {
    if source.kind != ImportKind::Pst {
        return Ok(());
    }
    let Some(crc) = pst_password(Path::new(&source.path)) else { return Ok(()) };
    match password {
        Some(p) if pst::password_matches(crc, p) => Ok(()),
        Some(_) => Err(VemError::new(Code::ImportRead05).with_detail("wrong")),
        None => Err(VemError::new(Code::ImportRead05).with_detail("missing")),
    }
}

fn ext_detail(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Old mail programs' usual places on this PC (docs/family-inventory.md).
pub fn find_sources() -> Vec<ImportSource> {
    let mut out = Vec::new();
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let profile = std::env::var_os("USERPROFILE").map(PathBuf::from);
    let mut pst_dirs = Vec::new();
    if let Some(l) = &local {
        pst_dirs.push(l.join("Microsoft").join("Outlook"));
    }
    if let Some(p) = &profile {
        for docs in ["Documents", "Documenten"] {
            pst_dirs.push(p.join(docs).join("Outlook Files"));
            pst_dirs.push(p.join(docs).join("Outlook-bestanden"));
        }
    }
    for dir in pst_dirs {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = e.path();
            if path.extension().is_some_and(|x| x.eq_ignore_ascii_case("pst"))
                && let Ok(s) = source_for(&path) {
                    out.push(s);
                }
        }
    }
    if let Some(l) = &local {
        let wlm = l.join("Microsoft").join("Windows Live Mail");
        if wlm.is_dir() {
            out.push(ImportSource {
                kind: ImportKind::Wlm,
                path: wlm.to_string_lossy().into_owned(),
                size: Some(size_of(&wlm)),
                label: "Windows Live Mail".into(),
                protected: false,
            });
        }
        let ids = l.join("Identities");
        for id in std::fs::read_dir(&ids).into_iter().flatten().flatten() {
            let oe = id.path().join("Microsoft").join("Outlook Express");
            if oe.is_dir() {
                out.push(ImportSource {
                    kind: ImportKind::Dbx,
                    path: oe.to_string_lossy().into_owned(),
                    size: Some(size_of(&oe)),
                    label: "Outlook Express".into(),
                    protected: false,
                });
            }
        }
    }
    out
}

/// Copy the original into `<data>/originals/<id>/`, read-only from the source.
fn keep_original(store: &Store, src: &Path) -> Result<PathBuf> {
    let dest_root = store.originals_dir().join(uuid::Uuid::new_v4().simple().to_string());
    std::fs::create_dir_all(&dest_root)?;
    let name = src.file_name().map(|n| n.to_os_string()).unwrap_or_else(|| "bron".into());
    let dest = dest_root.join(name);
    copy_tree(src, &dest)?;
    Ok(dest)
}

fn copy_tree(src: &Path, dest: &Path) -> Result<()> {
    let meta = std::fs::metadata(src).map_err(read_error)?;
    if meta.is_dir() {
        std::fs::create_dir_all(dest)?;
        for e in std::fs::read_dir(src).map_err(read_error)?.flatten() {
            copy_tree(&e.path(), &dest.join(e.file_name()))?;
        }
    } else {
        std::fs::copy(src, dest).map_err(read_error)?;
    }
    Ok(())
}

fn read_error(e: std::io::Error) -> VemError {
    // Outlook keeps its PST locked while it runs (ERROR_SHARING_VIOLATION = 32, LOCK_VIOLATION = 33).
    match e.raw_os_error() {
        Some(32 | 33) => VemError::new(Code::ImportRead02),
        _ => VemError::new(Code::ImportRead01).with_detail(e.kind().to_string()),
    }
}

/// Where messages of one source folder go.
struct Sink<'a> {
    store: &'a Store,
    account_id: String,
    mailbox: bool,
    delimiter: String,
    folders: std::collections::HashMap<Vec<String>, i64>,
    report: ImportReport,
    /// The largest message the target mailbox accepts (`APPENDLIMIT`, RFC 7889), if it says.
    append_limit: Option<u64>,
}

impl Sink<'_> {
    fn folder(&mut self, path: &[String]) -> Result<i64> {
        if let Some(id) = self.folders.get(path) {
            return Ok(*id);
        }
        let id = if self.mailbox {
            let mut parts = vec![MAILBOX_FOLDER.to_string()];
            parts.extend(path.iter().map(|p| p.replace(self.delimiter.as_str(), " ")));
            let name = crate::imap::utf7::encode(&parts.join(&self.delimiter));
            let id = self.store.upsert_folder(&self.account_id, &name, Some(&self.delimiter), None, true)?;
            self.store.push_op(&self.account_id, &Op::Create { folder: id })?;
            id
        } else {
            let role = (path.len() == 1 && looks_like_inbox(&path[0])).then_some(FolderRole::Inbox);
            self.store.ensure_local_folder(&self.account_id, path, role)?
        };
        self.folders.insert(path.to_vec(), id);
        Ok(id)
    }

    fn add(&mut self, path: &[String], raw: Vec<u8>) -> Result<()> {
        let folder = self.folder(path)?;
        let (raw, meta) = meta::take(raw);
        let p = parse::parse(&raw);
        if let Some(mid) = p.message_id.as_deref()
            && self.store.has_message_id(folder, mid)? {
                self.report.already_there += 1;
                return Ok(());
            }
        let size = raw.len() as u64;
        // The server counts the message with CRLF line ends: a bare LF becomes two bytes.
        let wire_size = size + bare_line_feeds(&raw);
        if self.mailbox && self.append_limit.is_some_and(|max| wire_size > max) {
            self.report.skipped.push((path.join("/"), format!("the message is larger than the mailbox accepts ({size} bytes)")));
            return Ok(());
        }
        let id = self.store.insert_message(&NewMessage {
            account_id: self.account_id.clone(),
            folder_id: folder,
            uid: None,
            raw,
            has_body: true,
            unread: meta.unread,
            important: meta.important,
            size,
            fallback_date: None,
        })?;
        for name in &meta.tags {
            let tag = self.store.create_tag(name, "")?;
            self.store.set_message_tag(id, &tag.id, true)?;
        }
        if self.mailbox {
            // Tags stay in the index: a server that keeps no keywords would refuse the whole APPEND.
            let op = Op::Append { message_id: id, folder, seen: !meta.unread, flagged: meta.important };
            self.store.push_op(&self.account_id, &op)?;
        }
        self.report.added += 1;
        Ok(())
    }
}

fn looks_like_inbox(name: &str) -> bool {
    let n = name.to_lowercase();
    ["postvak in", "inbox", "postvak-in", "inkomend"].contains(&n.as_str())
}

pub type Emit<'a> = dyn FnMut(&[String], Vec<u8>) -> std::result::Result<(), String> + 'a;
pub type Skipped = Vec<(String, String)>;

/// Run one import. `progress` is called now and then; `cancel` stops it between messages
/// (what came in stays).
pub fn run(
    store: &Store,
    job_id: &str,
    source: &ImportSource,
    target: &ImportTarget,
    progress: &dyn Fn(ImportProgress),
    cancel: &AtomicBool,
) -> Result<ImportReport> {
    run_inner(store, job_id, source, target, progress, cancel, None)
}

/// The same import of a PST, read with the rescue tool instead (phase 3 task 5). Messages that
/// came in before are recognised by their Message-ID and not added twice.
pub fn run_rescue(
    store: &Store,
    job_id: &str,
    source: &ImportSource,
    target: &ImportTarget,
    progress: &dyn Fn(ImportProgress),
    cancel: &AtomicBool,
    tool: &Path,
) -> Result<ImportReport> {
    if source.kind != ImportKind::Pst {
        return Err(VemError::new(Code::ImportRead03).with_detail("rescue is for PST files"));
    }
    run_inner(store, job_id, source, target, progress, cancel, Some(tool))
}

fn run_inner(
    store: &Store,
    job_id: &str,
    source: &ImportSource,
    target: &ImportTarget,
    progress: &dyn Fn(ImportProgress),
    cancel: &AtomicBool,
    rescue_tool: Option<&Path>,
) -> Result<ImportReport> {
    let src = PathBuf::from(&source.path);
    if !src.exists() {
        return Err(VemError::new(Code::ImportRead01).with_detail("missing"));
    }
    let size = size_of(&src);
    check_disk(store, size)?;
    let sink = open_sink(store, &source.label, size, target)?;
    log::event("import.start", &[("job", job_id), ("kind", &format!("{:?}", source.kind)), ("bytes", &size.to_string())]);
    let copy = keep_original(store, &src)?;
    store.add_original(&source.label, &copy, size)?;
    let contacts = std::cell::Cell::new(0u64);
    let report = pump(sink, job_id, progress, cancel, |emit| {
        // A damaged old file can make a reader crate panic; that ends this import, not the app.
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match source.kind {
            ImportKind::Eml => eml_tree(&copy, emit),
            ImportKind::Wlm => wlm_tree(&copy, emit),
            ImportKind::Mbox => mbox::read_path(&copy, emit),
            ImportKind::Pst => match rescue_tool {
                Some(tool) => read_rescued(store, tool, &copy, job_id, emit),
                None => read_pst(store, &copy, emit, &contacts),
            },
            ImportKind::Dbx => read_dbx_tree(&copy, emit),
        }))
        .unwrap_or_else(|_| Err("the file is damaged: the reader stopped".into()))
    })?;
    let all_marked_read = source.kind == ImportKind::Wlm && report.added > 0;
    Ok(ImportReport { contacts: contacts.get(), all_marked_read, rescued: rescue_tool.is_some(), ..report })
}

/// Rescue mode: the tool writes `.eml` files into our own scratch folder, which is read and
/// then removed. The scratch folder is ours alone; the original and its copy stay as they are.
fn read_rescued(store: &Store, tool: &Path, pst: &Path, job_id: &str, emit: &mut Emit<'_>) -> std::result::Result<Skipped, String> {
    let work = store.root().join("rescue").join(job_id);
    let result = rescue::convert(tool, pst, &work).and_then(|()| eml_tree(&work.join(rescue::OUTPUT), emit));
    let _ = std::fs::remove_dir_all(&work);
    result
}

/// Enough room on this disk for the copy of the original and the converted mail.
fn check_disk(store: &Store, size: u64) -> Result<()> {
    if let Some(free) = free_space(&store.originals_dir()) {
        let need = size + size * GROWTH_PERCENT / 100 + SPARE_BYTES;
        if free < need {
            return Err(VemError::new(Code::ImportSpace01).with_detail(format!("{} MB", need / 1024 / 1024)));
        }
    }
    Ok(())
}

/// Where the mail goes: a local account named after the source, or a mailbox with room for it.
fn open_sink<'a>(store: &'a Store, label: &str, size: u64, target: &ImportTarget) -> Result<Sink<'a>> {
    let (account_id, mailbox) = match target {
        ImportTarget::Local => (store.local_account(label)?.id, false),
        ImportTarget::Mailbox { account_id } => {
            let a = store.account(account_id)?;
            if let Some(q) = a.quota.as_ref()
                && let Some(limit) = q.limit_kb
                    && q.used_kb + size / 1024 > limit {
                        return Err(VemError::new(Code::ImportSpace02));
                    }
            (a.id, true)
        }
    };
    let delimiter = store
        .folder_infos(&account_id)?
        .into_iter()
        .find_map(|f| f.delimiter.filter(|d| !d.is_empty()))
        .unwrap_or_else(|| "/".into());
    let append_limit = if mailbox { append_limit(&store.capabilities(&account_id)?) } else { None };
    Ok(Sink { store, account_id, mailbox, delimiter, folders: Default::default(), report: ImportReport::default(), append_limit })
}

fn bare_line_feeds(raw: &[u8]) -> u64 {
    raw.iter().enumerate().filter(|(i, c)| **c == b'\n' && (*i == 0 || raw[i - 1] != b'\r')).count() as u64
}

/// `APPENDLIMIT=<bytes>` from the capabilities the mailbox announced at its last login.
fn append_limit(caps: &[String]) -> Option<u64> {
    caps.iter().find_map(|c| c.to_ascii_uppercase().strip_prefix("APPENDLIMIT=").and_then(|n| n.parse().ok()))
}

/// Feed every message `read` hands over into the sink, with progress and the stop switch.
fn pump(
    mut sink: Sink<'_>,
    job_id: &str,
    progress: &dyn Fn(ImportProgress),
    cancel: &AtomicBool,
    read: impl FnOnce(&mut Emit<'_>) -> std::result::Result<Skipped, String>,
) -> Result<ImportReport> {
    let mut done: u64 = 0;
    let mut failure: Option<VemError> = None;
    let read = {
        let mut emit = |path: &[String], raw: Vec<u8>| -> std::result::Result<(), String> {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled".into());
            }
            if let Err(e) = sink.add(path, raw) {
                failure = Some(e);
                return Err("store".into());
            }
            done += 1;
            if done.is_multiple_of(PROGRESS_EVERY) {
                progress(ImportProgress {
                    job_id: job_id.to_string(),
                    done,
                    total: None,
                    current_folder: path.last().cloned(),
                    finished: false,
                    error: None,
                    ..Default::default()
                });
            }
            Ok(())
        };
        read(&mut emit)
    };
    if let Some(e) = failure {
        return Err(e);
    }
    let skipped = match read {
        Ok(s) => s,
        Err(e) if e == "cancelled" => Vec::new(),
        Err(e) => return Err(VemError::new(Code::ImportRead01).with_detail(e)),
    };
    let mut report = sink.report;
    report.skipped.extend(skipped);
    log::event(
        "import.done",
        &[("job", job_id), ("added", &report.added.to_string()), ("skipped", &report.skipped.len().to_string())],
    );
    Ok(report)
}

/// A folder of .eml files with subfolders (Windows Live Mail, or saved messages).
pub fn eml_tree(root: &Path, emit: &mut Emit<'_>) -> std::result::Result<Skipped, String> {
    let mut skipped = Vec::new();
    if root.is_file() {
        let raw = std::fs::read(root).map_err(|e| e.kind().to_string())?;
        emit(&[], raw)?;
        return Ok(skipped);
    }
    walk(root, &mut Vec::new(), emit, &mut skipped, false)?;
    Ok(skipped)
}

/// Windows Live Mail (and Vista's Windows Mail): .eml files in folders. Read state and stars live
/// in its database, which is not read yet, so every message comes in as read
/// ([`ImportReport::all_marked_read`]). Messages of which only the headers were downloaded are
/// listed, not imported as complete.
pub fn wlm_tree(root: &Path, emit: &mut Emit<'_>) -> std::result::Result<Skipped, String> {
    let mut skipped = Vec::new();
    walk(root, &mut Vec::new(), emit, &mut skipped, true)?;
    Ok(skipped)
}

/// A Windows Live Mail file of an IMAP folder that holds only the headers: nothing after them
/// and no Content-Type (a real message without text still names its content type).
fn headers_only(raw: &[u8]) -> bool {
    let head_end = raw.windows(4).position(|w| w == b"\r\n\r\n").or_else(|| raw.windows(2).position(|w| w == b"\n\n"));
    let head = String::from_utf8_lossy(&raw[..head_end.unwrap_or(raw.len())]).to_ascii_lowercase();
    if head.contains("\ncontent-type:") || head.starts_with("content-type:") {
        return false;
    }
    empty_body(raw)
}

fn empty_body(raw: &[u8]) -> bool {
    let body = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| &raw[i + 4..])
        .or_else(|| raw.windows(2).position(|w| w == b"\n\n").map(|i| &raw[i + 2..]));
    match body {
        Some(b) => b.iter().all(|c| c.is_ascii_whitespace()),
        None => true,
    }
}

fn walk(dir: &Path, path: &mut Vec<String>, emit: &mut Emit<'_>, skipped: &mut Skipped, live_mail: bool) -> std::result::Result<(), String> {
    let mut entries: Vec<_> = std::fs::read_dir(dir).map_err(|e| e.kind().to_string())?.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        if p.is_dir() {
            path.push(e.file_name().to_string_lossy().into_owned());
            walk(&p, path, emit, skipped, live_mail)?;
            path.pop();
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("eml")) {
            match std::fs::read(&p) {
                Ok(raw) if live_mail && headers_only(&raw) => {
                    skipped.push((path.join("/"), "only the headers of this message were downloaded".into()));
                }
                Ok(raw) => emit(path, raw)?,
                Err(err) => skipped.push((path.join("/"), err.kind().to_string())),
            }
        }
    }
    Ok(())
}

/// Outlook Express: one .dbx file per folder, or a folder holding them.
fn read_dbx_tree(path: &Path, emit: &mut Emit<'_>) -> std::result::Result<Skipped, String> {
    if path.is_file() {
        return read_dbx(path, emit);
    }
    let mut skipped = Vec::new();
    let mut files: Vec<_> = std::fs::read_dir(path).map_err(|e| e.kind().to_string())?.flatten().map(|e| e.path()).collect();
    files.sort();
    for f in files.into_iter().filter(|f| f.extension().is_some_and(|x| x.eq_ignore_ascii_case("dbx"))) {
        skipped.extend(read_dbx(&f, emit)?);
    }
    Ok(skipped)
}

// The PST and DBX readers live in their own modules.
fn read_pst(store: &Store, path: &Path, emit: &mut Emit<'_>, contacts: &std::cell::Cell<u64>) -> std::result::Result<Skipped, String> {
    let stats = pst::read(path, emit)?;
    contacts.set(add_contacts(store, &stats.contacts).map_err(|e| e.code)?);
    Ok(stats.skipped)
}

/// Contacts from vCard files: one `.vcf` file, or a folder of them (Windows Live Mail's export).
/// The files are only read. Returns (cards with an address found, contacts added).
pub fn import_vcards(store: &Store, path: &Path) -> Result<(u64, u64)> {
    let files: Vec<PathBuf> = if path.is_dir() {
        let mut f: Vec<PathBuf> = std::fs::read_dir(path)
            .map_err(|e| VemError::new(Code::ImportRead01).with_detail(e.kind().to_string()))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("vcf")))
            .collect();
        f.sort();
        f
    } else {
        vec![path.to_path_buf()]
    };
    let mut found = Vec::new();
    for f in &files {
        let bytes = std::fs::read(f).map_err(|e| VemError::new(Code::ImportRead01).with_detail(e.kind().to_string()))?;
        found.extend(vcard::parse(&bytes));
    }
    let added = add_contacts(store, &found)?;
    log::event("import.vcards", &[("files", &files.len().to_string()), ("found", &found.len().to_string()), ("added", &added.to_string())]);
    Ok((found.len() as u64, added))
}

/// Put contacts from an old file into the address book. Someone already there under one of the
/// addresses is left as it is, so an import never overwrites what the user typed.
fn add_contacts(store: &Store, contacts: &[pst_contact::OldContact]) -> Result<u64> {
    let mut added = 0;
    for c in contacts {
        let mut known = None;
        for e in &c.emails {
            if known.is_none() {
                known = store.contact_by_email(e)?;
            }
        }
        if let Some(existing) = known {
            // Keep what was typed; only addresses nobody has yet are added to that person.
            let mut missing = Vec::new();
            for e in &c.emails {
                if !existing.emails.contains(e) && store.contact_by_email(e)?.is_none() {
                    missing.push(e.clone());
                }
            }
            if !missing.is_empty() {
                let emails = existing.emails.iter().cloned().chain(missing).collect();
                store.save_contact(&crate::model::Contact { emails, ..existing })?;
            }
            continue;
        }
        store.save_contact(&crate::model::Contact {
            id: String::new(),
            name: c.name.clone(),
            emails: c.emails.clone(),
            phone: c.phone.clone(),
            note: None,
        })?;
        added += 1;
    }
    Ok(added)
}

fn read_dbx(path: &Path, emit: &mut Emit<'_>) -> std::result::Result<Skipped, String> {
    Ok(dbx::read(path, emit)?.skipped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::testutil::store;
    use std::sync::Mutex;

    fn eml(subject: &str, id: &str) -> String {
        format!("From: Oma <oma@x.nl>\r\nSubject: {subject}\r\nMessage-ID: <{id}@x>\r\nDate: Tue, 3 Feb 2004 10:00:00 +0100\r\n\r\nTekst van {subject}\r\n")
    }

    /// Every `.pst` below `VEM_REAL_PST_DIR` (real files kept outside the repository, see
    /// `docs/PROGRESS.md`), read by the normal reader and, when `VEM_READPST` names the rescue
    /// tool, by that too. Both must open every file without error and leave it unchanged. When
    /// both readers run, each message the rescue tool wrote must also come in through the normal
    /// reader (so a real PST the patched `outlook-pst` silently drops messages from would fail
    /// here). A protected file is refused with no password and, when `VEM_REAL_PST_PASSWORD`
    /// gives a string whose check matches, accepted with it.
    #[test]
    fn real_pst_files_import_with_both_readers() {
        let Some(dir) = std::env::var_os("VEM_REAL_PST_DIR").map(PathBuf::from) else {
            eprintln!("note: VEM_REAL_PST_DIR not set, no real PST files tried");
            return;
        };
        let rescue_tool = std::env::var_os("VEM_READPST").map(PathBuf::from);
        let mut files: Vec<PathBuf> = walk_psts(&dir);
        files.sort();
        assert!(!files.is_empty(), "no .pst below {}", dir.display());
        for file in files {
            let before = std::fs::read(&file).unwrap();
            let source = source_for(&file).unwrap();
            if source.protected {
                assert!(check_password(&source, None).is_err(), "{}: protected, so refused with no password", source.label);
                if let Ok(p) = std::env::var("VEM_REAL_PST_PASSWORD") {
                    assert!(check_password(&source, Some(&p)).is_ok(), "{}: a matching check is accepted", source.label);
                }
            }

            let (_d1, s1) = store();
            let normal = run(&s1, "j", &source, &ImportTarget::Local, &|_| {}, &AtomicBool::new(false))
                .unwrap_or_else(|e| panic!("{}: the normal reader opens it ({})", source.label, e.code));
            if let Some(tool) = &rescue_tool {
                let (_d2, s2) = store();
                let rescued = run_rescue(&s2, "j", &source, &ImportTarget::Local, &|_| {}, &AtomicBool::new(false), tool)
                    .unwrap_or_else(|e| panic!("{}: the rescue tool opens it ({})", source.label, e.code));
                assert!(
                    normal.added >= rescued.added,
                    "{}: the normal reader added {}, fewer than the rescue tool's {}",
                    source.label,
                    normal.added,
                    rescued.added
                );
            }
            assert_eq!(std::fs::read(&file).unwrap(), before, "{} was changed", file.display());
        }
    }

    fn walk_psts(dir: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .flat_map(|e| {
                let p = e.path();
                if p.is_dir() {
                    walk_psts(&p)
                } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("pst")) {
                    vec![p]
                } else {
                    vec![]
                }
            })
            .collect()
    }

    #[test]
    fn ost_is_refused_with_its_own_code() {
        let e = source_for(Path::new("C:/Users/oma/outlook.OST")).unwrap_err();
        assert_eq!(e.code, "VEM-IMPORT-READ-04");
        assert!(source_for(Path::new("C:/oud/archief.pst")).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn rescue_mode_imports_what_the_tool_wrote_and_cleans_up() {
        let (_d, s) = store();
        let src = tempfile::tempdir().unwrap();
        let pst = src.path().join("Kapot.pst");
        std::fs::write(&pst, b"not really a pst").unwrap();
        // A stand-in for readpst: the output folder is its 8th argument, the input its 9th.
        let tool = src.path().join("readpst.cmd");
        std::fs::write(
            &tool,
            concat!(
                "@echo off\r\n",
                "if not exist \"%~9\" exit /b 3\r\n",
                "mkdir \"%~8\\Postvak IN\"\r\n",
                "(echo Subject: gered& echo Message-ID: ^<r1@x^>& echo.& echo hallo) > \"%~8\\Postvak IN\\1.eml\"\r\n",
            ),
        )
        .unwrap();
        let source = source_for(&pst).unwrap();
        let r = run_rescue(&s, "j", &source, &ImportTarget::Local, &|_| {}, &AtomicBool::new(false), &tool).unwrap();
        assert_eq!(r.added, 1);
        assert!(r.rescued);
        assert!(!s.root().join("rescue").join("j").exists(), "the scratch folder is removed");
        assert_eq!(std::fs::read(&pst).unwrap(), b"not really a pst", "the original is not changed");
    }

    #[test]
    fn only_a_protected_pst_needs_its_password() {
        let mbox = ImportSource { kind: ImportKind::Mbox, path: "x.mbox".into(), size: None, label: "x".into(), protected: false };
        assert!(check_password(&mbox, None).is_ok());
        let missing = ImportSource { kind: ImportKind::Pst, path: "C:/bestaat/niet.pst".into(), ..mbox };
        assert!(!source_for(Path::new(&missing.path)).unwrap().protected);
        assert!(check_password(&missing, None).is_ok(), "an unreadable file is reported by the import itself");
    }

    #[test]
    fn import_keeps_read_state_star_and_tags() {
        let (_d, s) = store();
        let src = tempfile::tempdir().unwrap();
        let mbox = src.path().join("Thunderbird.mbox");
        std::fs::write(
            &mbox,
            "From - Tue Feb  3 10:00:00 2004\r\nX-Mozilla-Status: 0004\r\nX-Mozilla-Keys: $label2\r\nSubject: Nieuw\r\nMessage-ID: <t1@x>\r\n\r\nTekst\r\n",
        )
        .unwrap();
        let source = source_for(&mbox).unwrap();
        let r = run(&s, "job", &source, &ImportTarget::Local, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!(r.added, 1);
        let a = s.local_account("Thunderbird").unwrap();
        let folder = s.folder_infos(&a.id).unwrap().into_iter().next().unwrap().id;
        let m = &s.messages_in_folder(folder, 0, 10).unwrap()[0];
        assert!(m.unread && m.important);
        let names: Vec<String> = m.tag_ids.iter().filter_map(|id| s.tags().unwrap().into_iter().find(|t| &t.id == id)).map(|t| t.name).collect();
        assert_eq!(names, vec!["Work"]);
    }

    #[test]
    fn eml_folder_import_keeps_original_and_skips_repeats() {
        let (_d, s) = store();
        let src = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(src.path().join("Postvak IN")).unwrap();
        std::fs::create_dir_all(src.path().join("Familie/Kerst")).unwrap();
        std::fs::write(src.path().join("Postvak IN/a.eml"), eml("Eerste", "1")).unwrap();
        std::fs::write(src.path().join("Familie/Kerst/b.eml"), eml("Kerstdiner", "2")).unwrap();
        let before = std::fs::read(src.path().join("Postvak IN/a.eml")).unwrap();

        let source = ImportSource { kind: ImportKind::Wlm, path: src.path().to_string_lossy().into(), size: None, label: "Live Mail".into(), protected: false };
        let seen = Mutex::new(0);
        let r = run(&s, "j1", &source, &ImportTarget::Local, &|_| *seen.lock().unwrap() += 1, &AtomicBool::new(false)).unwrap();
        assert_eq!(r.added, 2);
        assert_eq!(std::fs::read(src.path().join("Postvak IN/a.eml")).unwrap(), before, "source unchanged");
        assert_eq!(s.originals().unwrap().len(), 1);

        let acc = s.local_account("Live Mail").unwrap();
        let folders = s.folders(Some(&acc.id)).unwrap();
        assert!(folders.iter().any(|f| f.role == Some(FolderRole::Inbox)));
        assert!(folders.iter().any(|f| f.path == vec!["Familie", "Kerst"]));
        let hits = s.search("kerstdiner", &crate::model::SearchScope::All, 10).unwrap();
        assert_eq!(hits.len(), 1);

        let r = run(&s, "j2", &source, &ImportTarget::Local, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!((r.added, r.already_there), (0, 2), "a second import adds nothing twice");
    }

    #[test]
    fn live_mail_lists_headers_only_messages_and_says_all_are_read() {
        let (_d, s) = store();
        let src = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(src.path().join("Gmail/Inbox")).unwrap();
        std::fs::write(src.path().join("Gmail/Inbox/a.eml"), eml("Compleet", "tekst")).unwrap();
        std::fs::write(src.path().join("Gmail/Inbox/b.eml"), "Subject: Alleen kop\r\nMessage-ID: <kop@x>\r\n\r\n").unwrap();
        let source = ImportSource { kind: ImportKind::Wlm, path: src.path().to_string_lossy().into(), size: None, label: "Live Mail".into(), protected: false };
        let r = run(&s, "j", &source, &ImportTarget::Local, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!(r.added, 1);
        assert!(r.skipped.iter().any(|(f, why)| f == "Gmail/Inbox" && why.contains("only the headers")));
        assert!(r.all_marked_read);
        assert_eq!(r.skipped.len(), 1);
        let local = s.local_account("Live Mail").unwrap();
        let folder = s.folders(Some(&local.id)).unwrap().into_iter().find(|f| f.total > 0).unwrap();
        assert_eq!(folder.unread, 0, "everything comes in as read");
        assert!(headers_only(b"Subject: x\n\n  \n"));
        assert!(!headers_only(b"Subject: x\r\n\r\nHallo"));
        assert!(!headers_only(b"Subject: leeg\r\nContent-Type: text/plain\r\n\r\n"), "an empty mail is still a mail");
        assert_eq!(bare_line_feeds(b"a\nb\r\nc\n"), 2);
    }

    #[test]
    fn mailbox_target_plans_create_and_append() {
        let (_d, s) = store();
        let a = s.add_account("Jan", "jan@x.nl", crate::model::AccountKind::Imap, None).unwrap();
        s.upsert_folder(&a.id, "INBOX", Some("."), Some(FolderRole::Inbox), true).unwrap();
        let src = tempfile::tempdir().unwrap();
        let file = src.path().join("oud.mbox");
        std::fs::write(&file, format!("From x Tue Feb  3 10:00:00 2004\n{}\nFrom y Tue Feb  3 11:00:00 2004\n{}", eml("A", "a"), eml("B", "b"))).unwrap();
        let source = source_for(&file).unwrap();
        assert_eq!(source.kind, ImportKind::Mbox);
        let r = run(&s, "j", &source, &ImportTarget::Mailbox { account_id: a.id.clone() }, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!(r.added, 2);
        let ops = s.ops::<Op>(&a.id).unwrap();
        assert!(matches!(ops[0].op, Op::Create { .. }));
        assert_eq!(ops.iter().filter(|o| matches!(o.op, Op::Append { .. })).count(), 2);
        assert!(s.folder_infos(&a.id).unwrap().iter().any(|f| f.name == "Oude mail.oud"));
    }

    #[test]
    fn outlook_contacts_join_the_address_book_without_overwriting() {
        let (_d, s) = store();
        let typed = s
            .save_contact(&crate::model::Contact { id: String::new(), name: "Kees (zelf getypt)".into(), emails: vec!["kees@voorbeeld.nl".into()], phone: None, note: None })
            .unwrap();
        let from_pst = vec![
            pst_contact::OldContact { name: "Kees Jansen".into(), emails: vec!["kees@voorbeeld.nl".into()], phone: None },
            pst_contact::OldContact { name: "Tante Truus".into(), emails: vec!["truus@voorbeeld.nl".into()], phone: Some("06-12345678".into()) },
        ];
        assert_eq!(add_contacts(&s, &from_pst).unwrap(), 1);
        assert_eq!(s.contact(&typed.id).unwrap().name, "Kees (zelf getypt)");
        let truus = s.contact_by_email("truus@voorbeeld.nl").unwrap().unwrap();
        assert_eq!(truus.phone.as_deref(), Some("06-12345678"));
        assert_eq!(add_contacts(&s, &from_pst).unwrap(), 0, "a second import adds nobody twice");
        let more = vec![pst_contact::OldContact {
            name: "Kees Jansen".into(),
            emails: vec!["kees@voorbeeld.nl".into(), "kees.werk@voorbeeld.nl".into(), "truus@voorbeeld.nl".into()],
            phone: None,
        }];
        assert_eq!(add_contacts(&s, &more).unwrap(), 0);
        let kees = s.contact(&typed.id).unwrap();
        assert_eq!(kees.name, "Kees (zelf getypt)");
        assert_eq!(kees.emails, vec!["kees@voorbeeld.nl", "kees.werk@voorbeeld.nl"], "a new address joins; Truus's stays hers");
    }

    #[test]
    fn live_mail_vcard_folder_fills_the_address_book_and_stays_unchanged() {
        let (_d, s) = store();
        let src = tempfile::tempdir().unwrap();
        let a = "BEGIN:VCARD\r\nVERSION:2.1\r\nFN:Kees Jansen\r\nEMAIL;INTERNET:kees@voorbeeld.nl\r\nEND:VCARD\r\n";
        let b = "BEGIN:VCARD\r\nVERSION:2.1\r\nFN:Bep Mulder\r\nEMAIL;INTERNET:bep@voorbeeld.nl\r\nTEL;CELL:06-1\r\nEND:VCARD\r\n";
        std::fs::write(src.path().join("Kees Jansen.vcf"), a).unwrap();
        std::fs::write(src.path().join("Bep Mulder.vcf"), b).unwrap();
        std::fs::write(src.path().join("leesmij.txt"), "geen kaart").unwrap();
        assert_eq!(import_vcards(&s, src.path()).unwrap(), (2, 2));
        assert_eq!(s.contact_by_email("bep@voorbeeld.nl").unwrap().unwrap().phone.as_deref(), Some("06-1"));
        assert_eq!(import_vcards(&s, &src.path().join("Kees Jansen.vcf")).unwrap(), (1, 0), "already there");
        assert_eq!(std::fs::read_to_string(src.path().join("Kees Jansen.vcf")).unwrap(), a, "source unchanged");
    }

    #[test]
    fn message_over_appendlimit_is_skipped_with_a_reason() {
        let (_d, s) = store();
        let a = s.add_account("Jan", "jan@x.nl", crate::model::AccountKind::Imap, None).unwrap();
        s.upsert_folder(&a.id, "INBOX", Some("."), Some(FolderRole::Inbox), true).unwrap();
        s.set_capabilities(&a.id, &["IMAP4REV1".into(), "APPENDLIMIT=200".into()]).unwrap();
        let src = tempfile::tempdir().unwrap();
        let file = src.path().join("oud.mbox");
        let big = eml("Groot", &"x".repeat(400));
        std::fs::write(&file, format!("From x Tue Feb  3 10:00:00 2004\n{}\nFrom y Tue Feb  3 11:00:00 2004\n{}", eml("A", "a"), big)).unwrap();
        let source = source_for(&file).unwrap();
        let r = run(&s, "j", &source, &ImportTarget::Mailbox { account_id: a.id.clone() }, &|_| {}, &AtomicBool::new(false)).unwrap();
        assert_eq!(r.added, 1);
        assert_eq!(r.skipped.len(), 1);
        assert!(r.skipped[0].1.contains("larger than the mailbox accepts"));
        assert_eq!(append_limit(&["APPENDLIMIT=1000".into()]), Some(1000));
        assert_eq!(append_limit(&["APPENDLIMIT".into()]), None, "no number: no limit known");
    }

    #[test]
    fn unknown_file_kind_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("foto.jpg");
        std::fs::write(&f, b"x").unwrap();
        assert_eq!(source_for(&f).unwrap_err().code, "VEM-IMPORT-READ-03");
    }
}
