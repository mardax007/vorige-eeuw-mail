//! Reading an Outlook data file (.pst), read-only, as RFC 5322 messages.
//!
//! A PST holds MAPI properties, not MIME, so every mail item is rebuilt: the original internet
//! headers when Outlook kept them (PR_TRANSPORT_MESSAGE_HEADERS), otherwise headers made from the
//! properties; a new MIME body from the plain text, the HTML or the compressed RTF; and the
//! attachments. The file is opened with `File::open` only (the crate's own `open_store` also
//! opens it for writing, so it is not used) and is never changed.

use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;
use std::rc::Rc;

use encoding_rs::{Encoding, WINDOWS_1252};
use mail_builder::MessageBuilder;
use mail_builder::headers::address::Address;
use mail_builder::headers::date::Date;
use mail_builder::headers::message_id::MessageId;
use outlook_pst::ltp::prop_context::PropertyValue;
use outlook_pst::ltp::table_context::TableContext;
use outlook_pst::messaging::attachment::{
    AnsiAttachment, Attachment, AttachmentData, AttachmentProperties, UnicodeAttachment,
};
use outlook_pst::messaging::folder::{Folder, FolderProperties};
use outlook_pst::messaging::message::{AnsiMessage, Message, MessageProperties, UnicodeMessage};
use outlook_pst::messaging::store::{AnsiStore, EntryId, Store, UnicodeStore};
use outlook_pst::ndb::node_id::NodeId;
use outlook_pst::{AnsiPstFile, UnicodePstFile};

use outlook_pst::messaging::named_prop::{NamedPropertyGuid, NamedPropertyId};

use super::meta::{keywords_line, status_line};
use super::pst_contact::{self, OldContact};
use super::rtf::{RtfBody, decompress_rtf, rtf_to_body};

/// What `read` calls for every message: folder path and RFC 5322 bytes.
pub type Emit<'a> = dyn FnMut(&[String], Vec<u8>) -> Result<(), String> + 'a;

pub struct ReadStats {
    pub messages: u64,
    /// (folder path joined with "/", short reason without mail content)
    pub skipped: Vec<(String, String)>,
    /// Outlook contacts with an e-mail address, for the address book.
    pub contacts: Vec<OldContact>,
}

const MAX_FOLDER_DEPTH: usize = 64;
const FILETIME_TICKS_PER_SECOND: i64 = 10_000_000;
/// Seconds from 1 January 1601 (the FILETIME epoch) to 1 January 1970.
const FILETIME_TO_UNIX_SECONDS: i64 = 11_644_473_600;
const DEFAULT_CODE_PAGE: u16 = 1252;
/// PR_MESSAGE_FLAGS: the message was read.
const MSGFLAG_READ: i32 = 0x0001;
/// PR_FLAG_STATUS: flagged for follow-up (1 is "completed", not a star).
const FOLLOWUP_FLAGGED: i32 = 2;
/// Outlook keeps categories in the named property "Keywords" of PS_PUBLIC_STRINGS.
const CATEGORIES_NAME: &str = "Keywords";
const MAPI_TO: i32 = 1;
const MAPI_CC: i32 = 2;
const MAPI_BCC: i32 = 3;
/// Header fields of the original message that describe its old body, not the rebuilt one.
const DROPPED_HEADERS: [&str; 4] =
    ["content-type", "content-transfer-encoding", "mime-version", "content-disposition"];

// MAPI property ids ([MS-OXPROPS]).
const PR_MESSAGE_CLASS: u16 = 0x001A;
const PR_SUBJECT: u16 = 0x0037;
const PR_CLIENT_SUBMIT_TIME: u16 = 0x0039;
const PR_SENT_REPRESENTING_NAME: u16 = 0x0042;
const PR_SENT_REPRESENTING_EMAIL: u16 = 0x0065;
const PR_TRANSPORT_MESSAGE_HEADERS: u16 = 0x007D;
const PR_RECIPIENT_TYPE: u16 = 0x0C15;
const PR_SENDER_NAME: u16 = 0x0C1A;
const PR_SENDER_EMAIL: u16 = 0x0C1F;
const PR_DELIVERY_TIME: u16 = 0x0E06;
const PR_MESSAGE_FLAGS: u16 = 0x0E07;
const PR_FLAG_STATUS: u16 = 0x1090;
const PR_BODY: u16 = 0x1000;
const PR_RTF_COMPRESSED: u16 = 0x1009;
const PR_HTML: u16 = 0x1013;
const PR_MESSAGE_ID: u16 = 0x1035;
const PR_REFERENCES: u16 = 0x1039;
const PR_IN_REPLY_TO: u16 = 0x1042;
const PR_DISPLAY_NAME: u16 = 0x3001;
const PR_ADDRTYPE: u16 = 0x3002;
const PR_EMAIL_ADDRESS: u16 = 0x3003;
const PR_CREATION_TIME: u16 = 0x3007;
const PR_SMTP_ADDRESS: u16 = 0x39FE;
const PR_ATTACH_FILENAME: u16 = 0x3704;
const PR_ATTACH_LONG_FILENAME: u16 = 0x3707;
const PR_ATTACH_MIME_TAG: u16 = 0x370E;
const PR_ATTACH_CONTENT_ID: u16 = 0x3712;
const PR_INTERNET_CPID: u16 = 0x3FDE;
const PR_MESSAGE_CODEPAGE: u16 = 0x3FFD;
const PR_SENDER_SMTP_ADDRESS: u16 = 0x5D01;

/// Read every mail item and hand it to `emit` as RFC 5322 bytes, with its folder path.
pub fn read(
    path: &Path,
    emit: &mut Emit<'_>,
) -> Result<ReadStats, String> {
    let store = PstStore::open(path)?;
    let mut stats = ReadStats { messages: 0, skipped: Vec::new(), contacts: Vec::new() };
    let dyn_store = store.as_dyn();
    let root_id = dyn_store
        .properties()
        .ipm_sub_tree_entry_id()
        .map_err(|e| format!("PST has no mail folder tree: {e}"))?;
    let root = dyn_store
        .open_folder(&root_id)
        .map_err(|e| format!("PST mail folder tree cannot be opened: {e}"))?;
    let categories = categories_prop_id(dyn_store.as_ref());
    let contact_emails = pst_contact::email_prop_ids(dyn_store.as_ref());
    let mut walker = Walker { store: &store, categories, contact_emails, stats: &mut stats, emit };
    walker.sub_folders(root.as_ref(), &mut Vec::new())?;
    Ok(stats)
}

/// PidTagPstPassword: the CRC-32 of the password Outlook protects the file with; 0 when none.
const PR_PST_PASSWORD: u16 = 0x67FF;

/// The stored password check of a protected PST, or None when it has no password. Only the
/// store's properties are read.
pub fn password_crc(path: &Path) -> Result<Option<u32>, String> {
    let store = PstStore::open(path)?;
    Ok(match store.as_dyn().properties().get(PR_PST_PASSWORD) {
        Some(PropertyValue::Integer32(v)) if *v != 0 => Some(*v as u32),
        _ => None,
    })
}

/// Whether a typed password matches the stored check. The file's content is not encrypted by it;
/// this only respects that its owner put a password on it. Outlook keeps the CRC-32 of the
/// password in the Windows code page; the variant without inversion ([MS-PST] 5.3) and the usual
/// one are both accepted, as tools differ on which Outlook wrote.
pub fn password_matches(crc: u32, password: &str) -> bool {
    let (bytes, _, _) = WINDOWS_1252.encode(password);
    let plain = crc32(0, &bytes);
    let usual = !crc32(!0, &bytes);
    crc == plain || crc == usual
}

fn crc32(mut crc: u32, bytes: &[u8]) -> u32 {
    for b in bytes {
        crc ^= u32::from(*b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    crc
}

/// The two PST flavours have different concrete types; attachments need the concrete message.
enum PstStore {
    Unicode(Rc<UnicodeStore>),
    Ansi(Rc<AnsiStore>),
}

struct Opened {
    message: Rc<dyn Message>,
    attachments: Vec<io::Result<Rc<dyn Attachment>>>,
}

impl PstStore {
    fn open(path: &Path) -> Result<Self, String> {
        let reader = || -> Result<Box<BufReader<File>>, String> {
            let file = File::open(path).map_err(|e| format!("PST cannot be opened: {e}"))?;
            Ok(Box::new(BufReader::new(file)))
        };
        if let Ok(pst) = UnicodePstFile::read_from(reader()?) {
            let store = UnicodeStore::read(Rc::new(pst))
                .map_err(|e| format!("PST cannot be read: {e}"))?;
            return Ok(Self::Unicode(store));
        }
        let pst = AnsiPstFile::read_from(reader()?)
            .map_err(|e| format!("not a PST file or damaged header: {e}"))?;
        let store = AnsiStore::read(Rc::new(pst)).map_err(|e| format!("PST cannot be read: {e}"))?;
        Ok(Self::Ansi(store))
    }

    fn as_dyn(&self) -> Rc<dyn Store> {
        match self {
            Self::Unicode(s) => s.clone(),
            Self::Ansi(s) => s.clone(),
        }
    }

    fn open_message(&self, id: &EntryId) -> io::Result<Opened> {
        match self {
            Self::Unicode(store) => {
                let message = UnicodeMessage::read(store.clone(), id, None)?;
                let attachments = attachment_nodes(message.as_ref())
                    .map(|node| {
                        UnicodeAttachment::read(message.clone(), node, None)
                            .map(|a| a as Rc<dyn Attachment>)
                    })
                    .collect();
                Ok(Opened { message, attachments })
            }
            Self::Ansi(store) => {
                let message = AnsiMessage::read(store.clone(), id, None)?;
                let attachments = attachment_nodes(message.as_ref())
                    .map(|node| {
                        AnsiAttachment::read(message.clone(), node, None)
                            .map(|a| a as Rc<dyn Attachment>)
                    })
                    .collect();
                Ok(Opened { message, attachments })
            }
        }
    }
}

fn attachment_nodes(message: &dyn Message) -> impl Iterator<Item = NodeId> {
    let ids: Vec<NodeId> = message
        .attachment_table()
        .map(|t| t.rows_matrix().map(|row| NodeId::from(u32::from(row.id()))).collect())
        .unwrap_or_default();
    ids.into_iter()
}

struct Walker<'a> {
    store: &'a PstStore,
    /// The property id that holds Outlook categories in this file, if it has any.
    categories: Option<u16>,
    /// The property ids of the contact e-mail fields in this file.
    contact_emails: Vec<u16>,
    stats: &'a mut ReadStats,
    emit: &'a mut Emit<'a>,
}

impl Walker<'_> {
    fn skip(&mut self, path: &[String], reason: impl Into<String>) {
        self.stats.skipped.push((path.join("/"), reason.into()));
    }

    fn sub_folders(&mut self, folder: &dyn Folder, path: &mut Vec<String>) -> Result<(), String> {
        if path.len() >= MAX_FOLDER_DEPTH {
            self.skip(path, "folders nested too deep");
            return Ok(());
        }
        let Some(table) = folder.hierarchy_table() else { return Ok(()) };
        let store = self.store.as_dyn();
        for row in table.rows_matrix() {
            let node = NodeId::from(u32::from(row.id()));
            let opened = store.properties().make_entry_id(node).and_then(|id| store.open_folder(&id));
            let child = match opened {
                Ok(child) => child,
                Err(_) => {
                    self.skip(path, "a folder could not be opened");
                    continue;
                }
            };
            path.push(folder_name(child.properties()));
            self.messages(child.as_ref(), path)?;
            self.sub_folders(child.as_ref(), path)?;
            path.pop();
        }
        Ok(())
    }

    fn messages(&mut self, folder: &dyn Folder, path: &[String]) -> Result<(), String> {
        let Some(table) = folder.contents_table() else { return Ok(()) };
        let store = self.store.as_dyn();
        let ids: Vec<NodeId> = table.rows_matrix().map(|r| NodeId::from(u32::from(r.id()))).collect();
        for node in ids {
            let opened = store.properties().make_entry_id(node).and_then(|id| self.store.open_message(&id));
            let opened = match opened {
                Ok(opened) => opened,
                Err(_) => {
                    self.skip(path, "a message could not be read");
                    continue;
                }
            };
            let props = opened.message.properties();
            let class = string(props.prop(PR_MESSAGE_CLASS), DEFAULT_CODE_PAGE);
            if class.as_deref().is_some_and(pst_contact::is_contact_class) {
                let cp = code_page(props);
                let decode8 = |b: &[u8]| encoding_for(cp).decode_without_bom_handling(b).0.into_owned();
                let text = |id: u16| pst_contact::text_of(props.prop(id), &decode8);
                if let Some(c) = pst_contact::contact(&text, &self.contact_emails) {
                    self.stats.contacts.push(c);
                }
                continue;
            }
            if !is_mail_class(props) {
                continue;
            }
            let mut notes = Vec::new();
            let item = read_item(opened.message.as_ref(), opened.attachments, self.categories, &mut notes);
            for note in notes {
                self.skip(path, note);
            }
            (self.emit)(path, build(&item))?;
            self.stats.messages += 1;
        }
        Ok(())
    }
}

/// Mail is "IPM.Note" and its variants (S/MIME, secure); contacts, appointments, tasks and
/// reports are not imported. An item without a class is treated as mail.
fn is_mail_class(props: &MessageProperties) -> bool {
    match string(props.prop(PR_MESSAGE_CLASS), DEFAULT_CODE_PAGE) {
        None => true,
        Some(class) => class_is_mail(&class),
    }
}

fn class_is_mail(class: &str) -> bool {
    let class = class.trim().to_ascii_lowercase();
    class == "ipm.note" || class.starts_with("ipm.note.") || class == "ipm"
}

fn folder_name(props: &FolderProperties) -> String {
    string(props.get(PR_DISPLAY_NAME), DEFAULT_CODE_PAGE)
        .filter(|n| !n.trim().is_empty())
        // A folder without a name keeps a neutral mark rather than screen text.
        .unwrap_or_else(|| "_".to_string())
}

// ---------------------------------------------------------------------------------------------
// From MAPI properties to a neutral mail item.

trait Props {
    fn prop(&self, id: u16) -> Option<&PropertyValue>;
}

impl Props for MessageProperties {
    fn prop(&self, id: u16) -> Option<&PropertyValue> {
        self.get(id)
    }
}

impl Props for AttachmentProperties {
    fn prop(&self, id: u16) -> Option<&PropertyValue> {
        self.get(id)
    }
}

impl Props for Vec<(u16, PropertyValue)> {
    fn prop(&self, id: u16) -> Option<&PropertyValue> {
        self.iter().find(|(p, _)| *p == id).map(|(_, v)| v)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Person {
    name: Option<String>,
    email: String,
}

#[derive(Debug, Clone)]
enum Attached {
    File { name: String, mime: String, data: Vec<u8>, content_id: Option<String> },
    Message { name: String, raw: Vec<u8> },
}

#[derive(Debug, Clone, Default)]
struct MailItem {
    /// The original internet header block, when Outlook kept it.
    header_block: Option<String>,
    subject: Option<String>,
    from: Option<Person>,
    to: Vec<Person>,
    cc: Vec<Person>,
    bcc: Vec<Person>,
    /// Unix seconds.
    date: Option<i64>,
    message_id: Option<String>,
    in_reply_to: Option<String>,
    references: Option<String>,
    text: Option<String>,
    html: Option<String>,
    attachments: Vec<Attached>,
    unread: bool,
    flagged: bool,
    categories: Vec<String>,
}

fn read_item(
    message: &dyn Message,
    attachments: Vec<io::Result<Rc<dyn Attachment>>>,
    categories: Option<u16>,
    notes: &mut Vec<String>,
) -> MailItem {
    let p = message.properties();
    let cp = code_page(p);
    let text_of = |id| string(p.prop(id), cp).filter(|s| !s.trim().is_empty());
    let (to, cc, bcc) = recipients(message, cp);
    let date = [PR_CLIENT_SUBMIT_TIME, PR_DELIVERY_TIME, PR_CREATION_TIME]
        .into_iter()
        .find_map(|id| match p.prop(id) {
            Some(PropertyValue::Time(t)) => Some(filetime_to_unix(*t)),
            _ => None,
        });
    let rtf = match p.prop(PR_RTF_COMPRESSED) {
        Some(PropertyValue::Binary(b)) => decompress_rtf(b.buffer()).map(|r| rtf_to_body(&r)),
        _ => None,
    };
    let mut text = text_of(PR_BODY);
    let mut html = html_body(p, cp);
    match rtf {
        Some(RtfBody::Html(h)) if html.is_none() => html = Some(h),
        Some(RtfBody::Text(t)) if text.is_none() && html.is_none() => text = Some(t),
        _ => {}
    }
    let mut item = MailItem {
        header_block: text_of(PR_TRANSPORT_MESSAGE_HEADERS),
        subject: string(p.prop(PR_SUBJECT), cp).map(|s| strip_subject_prefix(&s)),
        from: sender(p, cp),
        to,
        cc,
        bcc,
        date,
        message_id: text_of(PR_MESSAGE_ID),
        in_reply_to: text_of(PR_IN_REPLY_TO),
        references: text_of(PR_REFERENCES),
        text,
        html,
        attachments: Vec::new(),
        unread: matches!(p.prop(PR_MESSAGE_FLAGS), Some(PropertyValue::Integer32(f)) if f & MSGFLAG_READ == 0),
        flagged: matches!(p.prop(PR_FLAG_STATUS), Some(PropertyValue::Integer32(FOLLOWUP_FLAGGED))),
        categories: categories.map(|id| string_list(p.prop(id), cp)).unwrap_or_default(),
    };
    for (n, attachment) in attachments.into_iter().enumerate() {
        match attachment {
            Ok(a) => {
                if let Some(att) = read_attachment(a.as_ref(), n + 1, notes) {
                    item.attachments.push(att);
                }
            }
            Err(_) => notes.push("an attachment could not be read".into()),
        }
    }
    item
}

fn read_attachment(a: &dyn Attachment, n: usize, notes: &mut Vec<String>) -> Option<Attached> {
    let p = a.properties();
    let name = [PR_ATTACH_LONG_FILENAME, PR_ATTACH_FILENAME, PR_DISPLAY_NAME]
        .into_iter()
        .find_map(|id| string(p.prop(id), DEFAULT_CODE_PAGE).filter(|s| !s.trim().is_empty()));
    match a.data() {
        Some(AttachmentData::Binary(b)) => Some(Attached::File {
            name: name.unwrap_or_else(|| format!("attachment-{n}")),
            mime: string(p.prop(PR_ATTACH_MIME_TAG), DEFAULT_CODE_PAGE)
                .map(|m| m.trim().to_ascii_lowercase())
                .filter(|m| valid_mime_type(m))
                .unwrap_or_else(|| "application/octet-stream".into()),
            data: b.buffer().to_vec(),
            content_id: string(p.prop(PR_ATTACH_CONTENT_ID), DEFAULT_CODE_PAGE)
                .map(|c| c.trim().trim_matches(['<', '>']).to_string())
                .filter(|c| !c.is_empty()),
        }),
        Some(AttachmentData::Message(m)) => {
            if m.attachment_table().is_some_and(|t| t.rows_matrix().next().is_some()) {
                notes.push("attachments inside an attached message were left out".into());
            }
            // The attached message keeps no state or categories of its own: it is a file here.
            let inner = read_item(m.as_ref(), Vec::new(), None, notes);
            let name = name
                .or_else(|| inner.subject.clone())
                .unwrap_or_else(|| format!("attachment-{n}"));
            let name = if name.to_ascii_lowercase().ends_with(".eml") { name } else { format!("{name}.eml") };
            Some(Attached::Message { name, raw: build_message(&inner) })
        }
        None => {
            notes.push("an attachment without data (a link) was left out".into());
            None
        }
    }
}

fn valid_mime_type(m: &str) -> bool {
    m.split_once('/').is_some_and(|(a, b)| !a.is_empty() && !b.is_empty())
        && m.bytes().all(|c| c.is_ascii_graphic() && !b"\"()<>@,;:\\[]?=".contains(&c) || c == b'/')
}

fn recipients(message: &dyn Message, cp: u16) -> (Vec<Person>, Vec<Person>, Vec<Person>) {
    let (mut to, mut cc, mut bcc) = (Vec::new(), Vec::new(), Vec::new());
    let Some(table) = message.recipient_table() else { return (to, cc, bcc) };
    for row in table.rows_matrix() {
        let props = row_props(table.as_ref(), row);
        let Some(person) = recipient(&props, cp) else { continue };
        match props.prop(PR_RECIPIENT_TYPE) {
            Some(PropertyValue::Integer32(MAPI_CC)) => cc.push(person),
            Some(PropertyValue::Integer32(MAPI_BCC)) => bcc.push(person),
            Some(PropertyValue::Integer32(MAPI_TO)) | None => to.push(person),
            _ => {}
        }
    }
    (to, cc, bcc)
}

fn row_props(
    table: &dyn TableContext,
    row: &outlook_pst::ltp::table_context::TableRowData,
) -> Vec<(u16, PropertyValue)> {
    let context = table.context();
    let Ok(values) = row.columns(context) else { return Vec::new() };
    context
        .columns()
        .iter()
        .zip(values)
        .filter_map(|(col, value)| {
            let value = table.read_column(value.as_ref()?, col.prop_type()).ok()?;
            Some((col.prop_id(), value))
        })
        .collect()
}

fn recipient(props: &Vec<(u16, PropertyValue)>, cp: u16) -> Option<Person> {
    let name = string(props.prop(PR_DISPLAY_NAME), cp).filter(|s| !s.trim().is_empty());
    let is_exchange = string(props.prop(PR_ADDRTYPE), cp).is_some_and(|t| t.eq_ignore_ascii_case("EX"));
    let email = string(props.prop(PR_SMTP_ADDRESS), cp).filter(|e| e.contains('@')).or_else(|| {
        string(props.prop(PR_EMAIL_ADDRESS), cp).filter(|e| !is_exchange && e.contains('@'))
    });
    person(name, email)
}

fn sender(p: &MessageProperties, cp: u16) -> Option<Person> {
    let name = [PR_SENDER_NAME, PR_SENT_REPRESENTING_NAME]
        .into_iter()
        .find_map(|id| string(p.prop(id), cp).filter(|s| !s.trim().is_empty()));
    let email = [PR_SENDER_SMTP_ADDRESS, PR_SENDER_EMAIL, PR_SENT_REPRESENTING_EMAIL]
        .into_iter()
        .find_map(|id| string(p.prop(id), cp).filter(|e| e.contains('@')));
    person(name, email)
}

fn person(name: Option<String>, email: Option<String>) -> Option<Person> {
    let email = email.map(|e| e.trim().to_string()).unwrap_or_default();
    let name = name.map(|n| n.trim().to_string()).filter(|n| *n != email);
    if name.is_none() && email.is_empty() {
        return None;
    }
    Some(Person { name, email })
}

fn code_page(p: &MessageProperties) -> u16 {
    [PR_MESSAGE_CODEPAGE, PR_INTERNET_CPID]
        .into_iter()
        .find_map(|id| match p.prop(id) {
            Some(PropertyValue::Integer32(cp)) => u16::try_from(*cp).ok().filter(|cp| *cp != 0),
            _ => None,
        })
        .unwrap_or(DEFAULT_CODE_PAGE)
}

pub(super) fn encoding_for(code_page: u16) -> &'static Encoding {
    codepage::to_encoding(code_page).unwrap_or(WINDOWS_1252)
}

/// A string property as text: String8 in the given code page, Unicode as UTF-16.
fn string(value: Option<&PropertyValue>, code_page: u16) -> Option<String> {
    let s = match value? {
        PropertyValue::String8(v) => encoding_for(code_page).decode_without_bom_handling(v.buffer()).0.into_owned(),
        PropertyValue::Unicode(v) => String::from_utf16_lossy(v.buffer()),
        _ => return None,
    };
    Some(s.trim_end_matches('\0').to_string())
}

/// PR_HTML is binary in the internet code page (PR_INTERNET_CPID), sometimes a string.
fn html_body(p: &MessageProperties, cp: u16) -> Option<String> {
    let html = match p.prop(PR_HTML)? {
        PropertyValue::Binary(b) => {
            let bytes = b.buffer();
            let internet_cp = match p.prop(PR_INTERNET_CPID) {
                Some(PropertyValue::Integer32(c)) => u16::try_from(*c).ok(),
                _ => None,
            };
            match internet_cp {
                Some(c) => encoding_for(c).decode_without_bom_handling(bytes).0.into_owned(),
                None => match std::str::from_utf8(bytes) {
                    Ok(s) => s.to_string(),
                    Err(_) => encoding_for(cp).decode_without_bom_handling(bytes).0.into_owned(),
                },
            }
        }
        other => string(Some(other), cp)?,
    };
    let html = html.trim_end_matches('\0').to_string();
    (!html.trim().is_empty()).then_some(html)
}

/// A subject may start with SOH and a length byte marking the "RE: " prefix ([MS-OXCMSG]).
fn strip_subject_prefix(subject: &str) -> String {
    if subject.starts_with('\u{1}') { subject.chars().skip(2).collect() } else { subject.to_string() }
}

/// FILETIME (100-nanosecond ticks since 1601-01-01 UTC) to Unix seconds.
fn filetime_to_unix(filetime: i64) -> i64 {
    filetime.div_euclid(FILETIME_TICKS_PER_SECOND) - FILETIME_TO_UNIX_SECONDS
}

// ---------------------------------------------------------------------------------------------
// From the mail item to RFC 5322 bytes.

fn build(item: &MailItem) -> Vec<u8> {
    let mut out = meta_lines(item).into_bytes();
    out.extend(build_message(item));
    out
}

/// Read state, star and categories as header lines in front of the message (see import/meta.rs).
fn meta_lines(item: &MailItem) -> String {
    let mut lines = status_line(item.unread, item.flagged).unwrap_or_default();
    lines.push_str(&keywords_line(item.categories.iter().map(String::as_str)).unwrap_or_default());
    lines
}

/// A multi-valued string property (PtypMultipleString or PtypMultipleString8).
fn string_list(value: Option<&PropertyValue>, code_page: u16) -> Vec<String> {
    match value {
        Some(PropertyValue::MultipleUnicode(list)) => {
            list.iter().map(|v| String::from_utf16_lossy(v.buffer()).trim_end_matches('\0').to_string()).collect()
        }
        Some(PropertyValue::MultipleString8(list)) => list
            .iter()
            .map(|v| encoding_for(code_page).decode_without_bom_handling(v.buffer()).0.trim_end_matches('\0').to_string())
            .collect(),
        Some(single @ (PropertyValue::Unicode(_) | PropertyValue::String8(_))) => string(Some(single), code_page).into_iter().collect(),
        _ => Vec::new(),
    }
}

/// Which property id this PST uses for the named property "Keywords" (Outlook categories).
fn categories_prop_id(store: &dyn Store) -> Option<u16> {
    let map = store.named_property_map().ok()?;
    let props = map.properties();
    props.stream_entry().ok()?.into_iter().find_map(|entry| {
        let NamedPropertyId::StringOffset(offset) = entry.id() else { return None };
        if entry.guid() != NamedPropertyGuid::PublicStrings {
            return None;
        }
        let name = props.lookup_string(offset).ok()?;
        let units: Vec<u16> = name.buffer().as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        (String::from_utf16_lossy(&units) == CATEGORIES_NAME).then(|| entry.prop_id())
    })
}

fn build_message(item: &MailItem) -> Vec<u8> {
    let mut b = MessageBuilder::new();
    if let Some(text) = &item.text {
        b = b.text_body(text.as_str());
    }
    if let Some(html) = &item.html {
        b = b.html_body(html.as_str());
    }
    for att in &item.attachments {
        b = match att {
            Attached::File { mime, data, content_id: Some(cid), .. } if item.html.is_some() => {
                b.inline(mime.as_str(), cid.as_str(), data.as_slice())
            }
            Attached::File { name, mime, data, .. } => b.attachment(mime.as_str(), name.as_str(), data.as_slice()),
            Attached::Message { name, raw } => b.attachment("message/rfc822", name.as_str(), raw.as_slice()),
        };
    }
    if let Some(block) = &item.header_block {
        let mut head = filter_transport_headers(block);
        if !has_field(&head, "date")
            && let Some(date) = item.date
        {
            head.push_str(&format!("Date: {}\r\n", Date::new(date).to_rfc822()));
        }
        head.push_str("MIME-Version: 1.0\r\n");
        let mut out = head.into_bytes();
        b.serialize_body(&mut out);
        return out;
    }
    if let Some(from) = &item.from {
        b = b.from(address(from));
    }
    for (list, header) in [(&item.to, "To"), (&item.cc, "Cc"), (&item.bcc, "Bcc")] {
        if !list.is_empty() {
            b = b.header(header, Address::new_list(list.iter().map(address).collect()));
        }
    }
    if let Some(subject) = &item.subject {
        b = b.subject(subject.as_str());
    }
    if let Some(date) = item.date {
        b = b.date(Date::new(date));
    }
    if let Some(id) = &item.message_id {
        b = b.message_id(bare_id(id));
    }
    if let Some(ids) = &item.in_reply_to {
        b = b.in_reply_to(MessageId::new_list(ids.split_whitespace().map(bare_id)));
    }
    if let Some(ids) = &item.references {
        b = b.references(MessageId::new_list(ids.split_whitespace().map(bare_id)));
    }
    let mut out = Vec::new();
    b.serialize(&mut out);
    out
}

fn address(p: &Person) -> Address<'_> {
    Address::new_address(p.name.as_deref(), p.email.as_str())
}

fn bare_id(id: &str) -> &str {
    id.trim().trim_start_matches('<').trim_end_matches('>')
}

/// The original header block without the fields that describe the old body. Lines are joined
/// with CRLF; folded lines stay with their field; lines that are not a field are dropped.
fn filter_transport_headers(block: &str) -> String {
    let mut out = String::with_capacity(block.len());
    let mut keep = false;
    for line in block.split('\n').map(|l| l.trim_end_matches('\r')) {
        if line.is_empty() {
            if out.is_empty() && !keep {
                continue;
            }
            break;
        }
        if line.starts_with([' ', '\t']) {
            if keep {
                out.push_str(line);
                out.push_str("\r\n");
            }
            continue;
        }
        keep = match line.split_once(':') {
            Some((name, _)) if !name.is_empty() && name.bytes().all(|c| c.is_ascii_graphic()) => {
                !DROPPED_HEADERS.contains(&name.to_ascii_lowercase().as_str())
            }
            _ => false,
        };
        if keep {
            out.push_str(line);
            out.push_str("\r\n");
        }
    }
    out
}

fn has_field(head: &str, name: &str) -> bool {
    head.split("\r\n").any(|line| {
        line.split_once(':').is_some_and(|(n, _)| n.eq_ignore_ascii_case(name))
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn pst_password_check() {
        assert_eq!(!crc32(!0, b"123456789"), 0xCBF4_3926, "the usual CRC-32 check value");
        assert!(password_matches(0xCBF4_3926, "123456789"));
        assert!(password_matches(crc32(0, &[b'g', b'e', b'h', b'e', b'i', b'm', 0x80]), "geheim€"), "the Windows code page: € is 0x80");
        assert!(!password_matches(0xCBF4_3926, "12345678"));
        assert_eq!(password_crc(&empty_pst()).unwrap(), None, "Outlook's empty example file has no password");
    }

    pub(crate) fn empty_pst() -> std::path::PathBuf {
        let home = std::env::var_os("CARGO_HOME").map(std::path::PathBuf::from).unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var_os("USERPROFILE").or(std::env::var_os("HOME")).unwrap()).join(".cargo")
        });
        let registry = home.join("registry").join("src");
        std::fs::read_dir(&registry)
            .expect("cargo registry")
            .filter_map(|e| e.ok())
            .map(|e| e.path().join("outlook-pst-1.2.0").join("examples").join("Empty.pst"))
            .find(|p| p.is_file())
            .expect("Empty.pst from the outlook-pst crate")
    }

    #[test]
    fn empty_pst_has_no_messages_and_no_error() {
        let mut count = 0;
        let stats = read(&empty_pst(), &mut |_, _| {
            count += 1;
            Ok(())
        })
        .expect("Empty.pst reads");
        assert_eq!(stats.messages, 0);
        assert_eq!(count, 0);
    }

    #[test]
    fn a_file_that_is_not_a_pst_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("niet.pst");
        std::fs::write(&path, b"dit is geen pst").unwrap();
        assert!(read(&path, &mut |_, _| Ok(())).is_err());
    }

    #[test]
    fn filetime_converts_to_unix_seconds() {
        assert_eq!(filetime_to_unix(116_444_736_000_000_000), 0);
        assert_eq!(filetime_to_unix(132_223_104_000_000_000), 1_577_836_800); // 2020-01-01
        assert_eq!(filetime_to_unix(132_223_104_009_999_999), 1_577_836_800);
        assert_eq!(filetime_to_unix(0), -11_644_473_600);
    }

    #[test]
    fn transport_headers_lose_the_old_body_fields() {
        let block = "Microsoft Mail Internet Headers Version 2.0\r\nReceived: from a\r\n\tby b\r\n\
                     Content-Type: multipart/alternative;\r\n\tboundary=\"x\"\r\nMIME-Version: 1.0\r\n\
                     Subject: Hallo\r\ncontent-transfer-encoding: 7bit\nFrom: Oma <oma@example.nl>\r\n\r\nrest";
        let out = filter_transport_headers(block);
        assert_eq!(out, "Received: from a\r\n\tby b\r\nSubject: Hallo\r\nFrom: Oma <oma@example.nl>\r\n");
        assert!(!has_field(&out, "date"));
        assert!(has_field(&out, "subject"));
    }

    #[test]
    fn mail_classes() {
        assert!(class_is_mail("IPM.Note"));
        assert!(class_is_mail("IPM.Note.SMIME.MultipartSigned"));
        assert!(!class_is_mail("IPM.Contact"));
        assert!(!class_is_mail("IPM.Appointment"));
        assert!(!class_is_mail("IPM.Notebook"));
        assert!(!class_is_mail("REPORT.IPM.Note.NDR"));
    }

    #[test]
    fn subject_prefix_marker_is_removed() {
        assert_eq!(strip_subject_prefix("\u{1}\u{4}RE: Hoi"), "RE: Hoi");
        assert_eq!(strip_subject_prefix("Hoi"), "Hoi");
    }

    fn item() -> MailItem {
        MailItem {
            subject: Some("Verjaardag van José".into()),
            from: Some(Person { name: Some("Opa Jansen".into()), email: "opa@example.nl".into() }),
            to: vec![Person { name: None, email: "kind@example.nl".into() }],
            cc: vec![Person { name: Some("Tante Één".into()), email: "tante@example.nl".into() }],
            date: Some(1_577_836_800),
            message_id: Some("<abc@example.nl>".into()),
            text: Some("Gefeliciteerd!".into()),
            html: Some("<p>Gefeliciteerd!</p>".into()),
            attachments: vec![Attached::File {
                name: "foto.jpg".into(),
                mime: "image/jpeg".into(),
                data: vec![0xFF, 0xD8, 0xFF, 0x00, 0x01],
                content_id: None,
            }],
            ..MailItem::default()
        }
    }

    #[test]
    fn message_from_properties_parses_back() {
        let raw = build(&item());
        let m = mail_parser::MessageParser::default().parse(&raw).expect("parses");
        assert_eq!(m.subject(), Some("Verjaardag van José"));
        assert_eq!(m.from().and_then(|a| a.first()).and_then(|a| a.address()), Some("opa@example.nl"));
        assert_eq!(m.to().and_then(|a| a.first()).and_then(|a| a.address()), Some("kind@example.nl"));
        assert_eq!(m.cc().and_then(|a| a.first()).and_then(|a| a.name()), Some("Tante Één"));
        assert_eq!(m.date().map(|d| d.to_timestamp()), Some(1_577_836_800));
        assert_eq!(m.message_id(), Some("abc@example.nl"));
        assert_eq!(m.body_text(0).as_deref().map(str::trim), Some("Gefeliciteerd!"));
        assert!(m.body_html(0).is_some_and(|h| h.contains("<p>Gefeliciteerd!</p>")));
        let att = m.attachments().next().expect("attachment");
        assert_eq!(att.attachment_name(), Some("foto.jpg"));
        assert_eq!(&att.decoded()[..], &[0xFF, 0xD8, 0xFF, 0x00, 0x01]);
    }

    #[test]
    fn message_with_transport_headers_keeps_them_and_gets_a_new_body() {
        let mut it = item();
        it.header_block = Some(
            "Received: from x\r\nFrom: Oma <oma@example.nl>\r\nSubject: Origineel\r\n\
             Message-ID: <orig@example.nl>\r\nContent-Type: text/plain; charset=us-ascii\r\n\r\n"
                .into(),
        );
        let raw = build(&it);
        let text = String::from_utf8_lossy(&raw);
        assert!(text.starts_with("Received: from x\r\n"));
        assert_eq!(text.matches("MIME-Version").count(), 1);
        assert!(!text.contains("us-ascii"));
        let m = mail_parser::MessageParser::default().parse(&raw).expect("parses");
        assert_eq!(m.subject(), Some("Origineel"));
        assert_eq!(m.message_id(), Some("orig@example.nl"));
        assert_eq!(m.date().map(|d| d.to_timestamp()), Some(1_577_836_800));
        assert!(m.body_html(0).is_some_and(|h| h.contains("Gefeliciteerd!")));
        assert_eq!(m.attachments().next().and_then(|a| a.attachment_name()), Some("foto.jpg"));
    }

    #[test]
    fn attached_message_is_message_rfc822() {
        let inner = build(&item());
        let mut it = item();
        it.attachments = vec![Attached::Message { name: "Doorgestuurd.eml".into(), raw: inner }];
        let raw = build(&it);
        let m = mail_parser::MessageParser::default().parse(&raw).expect("parses");
        let att = m.attachments().next().expect("attachment");
        let nested = att.nested().expect("a nested message");
        assert_eq!(nested.subject(), Some("Verjaardag van José"));
    }

    #[test]
    fn string8_uses_the_code_page() {
        assert_eq!(encoding_for(1252).decode_without_bom_handling(&[0x80]).0, "€");
        assert_eq!(encoding_for(65001).decode_without_bom_handling("é".as_bytes()).0, "é");
        assert_eq!(encoding_for(9999).name(), "windows-1252");
    }

}
