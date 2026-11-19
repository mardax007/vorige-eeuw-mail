//! Data shapes shared with the screens (src/api/types.ts). camelCase on the wire.

use serde::{Deserialize, Serialize};

pub use crate::parse::Addr;
use crate::error::VemError;
use crate::net::Security;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FolderRole {
    Inbox,
    Sent,
    Trash,
    Drafts,
    Junk,
    Archive,
    Outbox,
}

impl FolderRole {
    pub fn as_str(self) -> &'static str {
        match self {
            FolderRole::Inbox => "inbox",
            FolderRole::Sent => "sent",
            FolderRole::Trash => "trash",
            FolderRole::Drafts => "drafts",
            FolderRole::Junk => "junk",
            FolderRole::Archive => "archive",
            FolderRole::Outbox => "outbox",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "inbox" => FolderRole::Inbox,
            "sent" => FolderRole::Sent,
            "trash" => FolderRole::Trash,
            "drafts" => FolderRole::Drafts,
            "junk" => FolderRole::Junk,
            "archive" => FolderRole::Archive,
            "outbox" => FolderRole::Outbox,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    Imap,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignIn {
    Password,
    Google,
    Microsoft,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerSettings {
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_tls: Security,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_tls: Security,
    pub username: String,
    pub source: String,
    pub needs_confirmation: bool,
    pub sign_in: SignIn,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quota {
    pub used_kb: u64,
    pub limit_kb: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub display_name: String,
    pub email: String,
    pub kind: AccountKind,
    pub quota: Option<Quota>,
    pub signature: Option<String>,
    pub last_error: Option<VemError>,
    /// How this mailbox signs in, so the screens offer a password field or "Opnieuw inloggen".
    /// None for the local mailboxes, which never sign in.
    pub sign_in: Option<SignIn>,
    /// Other addresses that arrive in this mailbox, such as custom-domain addresses on iCloud
    /// (decision 32). Lower case, without the main address.
    pub addresses: Vec<String>,
    #[serde(skip)]
    pub settings: Option<ServerSettings>,
}

impl Account {
    /// Every address this mailbox may send from: the main one first.
    pub fn sender_addresses(&self) -> Vec<String> {
        std::iter::once(self.email.to_lowercase()).chain(self.addresses.iter().cloned()).collect()
    }

    /// The address to send from: the one the draft names when it belongs to this mailbox,
    /// otherwise the main address.
    pub fn send_from(&self, wanted: Option<&str>) -> String {
        match wanted.map(|w| w.trim().to_lowercase()) {
            Some(w) if self.addresses.contains(&w) => w,
            _ => self.email.clone(),
        }
    }
}

/// A Postvak IN for one domain of a mailbox's other addresses (decision 32): the mailbox's
/// Postvak IN, only the mail addressed to that domain.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DomainInbox {
    pub account_id: String,
    pub domain: String,
    pub unread: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub account_id: String,
    pub name: String,
    pub role: Option<FolderRole>,
    pub unread: u32,
    pub total: u32,
    pub path: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageSummary {
    pub id: String,
    pub account_id: String,
    pub folder_id: String,
    pub from: Addr,
    pub to: Vec<Addr>,
    pub subject: String,
    pub date: String,
    pub preview: String,
    pub unread: bool,
    pub important: bool,
    pub has_attachments: bool,
    pub tag_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub mime: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WarningKind {
    LinkMismatch,
    LookalikeContact,
    FirstContactMoney,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MailWarning {
    pub kind: WarningKind,
    pub shown: Option<String>,
    pub actual: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageDetail {
    #[serde(flatten)]
    pub summary: MessageSummary,
    pub cc: Vec<Addr>,
    pub reply_to: Vec<Addr>,
    pub html: Option<String>,
    pub text: Option<String>,
    pub has_remote_images: bool,
    pub remote_images_shown: bool,
    pub attachments: Vec<Attachment>,
    pub warnings: Vec<MailWarning>,
    pub message_id_header: Option<String>,
    pub references: Vec<String>,
    /// An invitation in the message, shown readably above the text (C11).
    pub invite: Option<crate::invite::Invite>,
    /// Addresses the mail server delivered this message to (Delivered-To, X-Original-To),
    /// lower case. Used to answer from the address the message came in on (decision 32).
    pub delivered_to: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SearchScope {
    All,
    #[serde(rename_all = "camelCase")]
    Account { account_id: String },
    #[serde(rename_all = "camelCase")]
    Folder { folder_id: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub message: MessageSummary,
    pub account_name: String,
    pub folder_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Contact {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub emails: Vec<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    #[serde(default)]
    pub id: Option<String>,
    pub account_id: String,
    #[serde(default)]
    pub to: Vec<Addr>,
    #[serde(default)]
    pub cc: Vec<Addr>,
    #[serde(default)]
    pub bcc: Vec<Addr>,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub body_text: String,
    #[serde(default)]
    pub attachments: Vec<String>,
    #[serde(default)]
    pub in_reply_to: Option<String>,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub shrink_photos: bool,
    /// The address to send from, when the mailbox has more than one (decision 32). None or an
    /// address that is not the mailbox's: the main address.
    #[serde(default)]
    pub from_address: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutboxState {
    Waiting,
    Sending,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboxItem {
    pub id: String,
    pub subject: String,
    pub send_at: String,
    pub state: OutboxState,
    pub error: Option<VemError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub text_size: u32,
    pub notifications: bool,
    pub start_with_windows: bool,
    pub undo_send_seconds: u32,
    pub shrink_photos: bool,
    pub report_notice_shown: bool,
    pub tray_notice_shown: bool,
    /// Android: a background service keeps watching for new mail (decision 24). Off by default;
    /// settings saved before it existed lack the field.
    #[serde(default)]
    pub background_mail: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            text_size: 18,
            notifications: true,
            start_with_windows: true,
            undo_send_seconds: 10,
            shrink_photos: true,
            report_notice_shown: false,
            tray_notice_shown: false,
            background_mail: false,
        }
    }
}

impl Settings {
    /// Keep values inside what the screens can show.
    pub fn clamped(self) -> Self {
        Settings {
            text_size: self.text_size.clamp(18, 32),
            undo_send_seconds: self.undo_send_seconds.min(60),
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportKind {
    Pst,
    Wlm,
    Dbx,
    Eml,
    Mbox,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportSource {
    pub kind: ImportKind,
    pub path: String,
    pub size: Option<u64>,
    pub label: String,
    /// An Outlook file with a password: the screen asks for it before the import starts.
    #[serde(default)]
    pub protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ImportTarget {
    Local,
    #[serde(rename_all = "camelCase")]
    Mailbox { account_id: String },
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub job_id: String,
    pub done: u64,
    pub total: Option<u64>,
    pub current_folder: Option<String>,
    pub finished: bool,
    pub error: Option<VemError>,
    /// Only in the last event: messages that were there already (same Message-ID).
    pub already_there: u64,
    /// Only in the last event: contacts from the file that went into the address book.
    pub contacts_added: u64,
    /// Only in the last event: the source keeps no read state we can read, so all came in as read.
    pub all_marked_read: bool,
    /// Only in the last event: read with the rescue tool, so read state may be lost.
    pub rescued: bool,
    /// Only in the last event: what was left out, by folder, with a short technical reason
    /// (shown under "Details" and in the saved list, like a server reply).
    pub skipped: Vec<ImportSkip>,
    /// What this import reads from, as the screen named it: the result stays with its file.
    pub source: Option<String>,
    /// Only in the last event: the folder the first new message went into, for "bekijken".
    pub folder_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSkip {
    pub folder: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OriginalCopy {
    pub id: String,
    pub label: String,
    pub size: u64,
    pub imported_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoToken {
    pub token: String,
    pub count: usize,
}

/// Unix seconds to the ISO form the screens read.
pub fn iso(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .unwrap_or_default()
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_reads_screen_json() {
        let s: SearchScope = serde_json::from_str(r#"{"kind":"folder","folderId":"f1"}"#).unwrap();
        assert_eq!(s, SearchScope::Folder { folder_id: "f1".into() });
        let s: SearchScope = serde_json::from_str(r#"{"kind":"all"}"#).unwrap();
        assert_eq!(s, SearchScope::All);
    }

    #[test]
    fn target_reads_screen_json() {
        let t: ImportTarget = serde_json::from_str(r#"{"kind":"mailbox","accountId":"a"}"#).unwrap();
        assert_eq!(t, ImportTarget::Mailbox { account_id: "a".into() });
    }

    #[test]
    fn settings_are_clamped() {
        let s = Settings { text_size: 4, undo_send_seconds: 600, ..Default::default() }.clamped();
        assert_eq!((s.text_size, s.undo_send_seconds), (18, 60));
    }

    #[test]
    fn detail_flattens_summary() {
        let d = MessageDetail {
            summary: MessageSummary {
                id: "1".into(),
                account_id: "a".into(),
                folder_id: "f".into(),
                from: Addr::new(None, "x@y.nl"),
                to: vec![],
                subject: "s".into(),
                date: iso(0),
                preview: String::new(),
                unread: true,
                important: false,
                has_attachments: false,
                tag_ids: vec![],
            },
            cc: vec![],
            reply_to: vec![],
            html: None,
            text: None,
            has_remote_images: false,
            remote_images_shown: false,
            attachments: vec![],
            warnings: vec![],
            message_id_header: None,
            references: vec![],
            invite: None,
            delivered_to: vec![],
        };
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["subject"], "s");
        assert_eq!(v["accountId"], "a");
        assert_eq!(v["date"], "1970-01-01T00:00:00Z");
    }
}
