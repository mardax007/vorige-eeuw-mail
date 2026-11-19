use std::collections::HashMap;

use rusqlite::{OptionalExtension, Row, params, params_from_iter};

use super::{Store, from_json, json};
use crate::error::{Code, Result, VemError};
use crate::model::{Addr, Draft, MessageSummary, iso};
use crate::parse::{self, ParsedMail};

/// A message as it arrives from the server or an import.
#[derive(Debug, Clone, Default)]
pub struct NewMessage {
    pub account_id: String,
    pub folder_id: i64,
    pub uid: Option<u32>,
    /// Either the whole message (`has_body`) or only its header block.
    pub raw: Vec<u8>,
    pub has_body: bool,
    pub unread: bool,
    pub important: bool,
    pub size: u64,
    /// Used when the message has no Date header (the server's INTERNALDATE).
    pub fallback_date: Option<i64>,
}

/// One row of the index, with what the reading pane needs beyond the summary.
#[derive(Debug, Clone)]
pub struct StoredMessage {
    pub id: i64,
    pub summary: MessageSummary,
    pub uid: Option<u32>,
    pub folder_id: i64,
    pub cc: Vec<Addr>,
    pub message_id_hdr: Option<String>,
    pub references: Vec<String>,
    pub eml_path: Option<String>,
    pub has_body: bool,
    pub is_draft: bool,
    pub draft: Option<Draft>,
    pub remote_images_ok: bool,
    pub size: u64,
}

pub fn message_key(id: i64) -> String {
    id.to_string()
}

pub fn parse_message_key(key: &str) -> Result<i64> {
    key.parse().map_err(|_| VemError::new(Code::StoreRead01).with_detail("message"))
}

const SUMMARY_COLS: &str = "m.id, m.account_id, m.folder_id, m.from_name, m.from_email, m.to_json, m.subject,
    m.date, m.preview, m.unread, m.important, m.has_attachments";

fn row_to_summary(r: &Row<'_>) -> rusqlite::Result<MessageSummary> {
    let to: String = r.get(5)?;
    Ok(MessageSummary {
        id: message_key(r.get(0)?),
        account_id: r.get(1)?,
        folder_id: r.get::<_, i64>(2)?.to_string(),
        from: Addr { name: r.get(3)?, email: r.get(4)? },
        to: serde_json::from_str(&to).unwrap_or_default(),
        subject: r.get(6)?,
        date: iso(r.get(7)?),
        preview: r.get(8)?,
        unread: r.get::<_, i64>(9)? != 0,
        important: r.get::<_, i64>(10)? != 0,
        has_attachments: r.get::<_, i64>(11)? != 0,
        tag_ids: Vec::new(),
    })
}

fn addr_text(p: &ParsedMail) -> String {
    let mut out = String::new();
    for a in std::iter::once(&p.from).chain(&p.to).chain(&p.cc) {
        if let Some(n) = &a.name {
            out.push_str(n);
            out.push(' ');
        }
        out.push_str(&a.email);
        out.push(' ');
    }
    out
}

fn body_text(p: &ParsedMail) -> String {
    match (&p.text, &p.html) {
        (Some(t), _) => t.clone(),
        (None, Some(h)) => mail_parser::html_to_text(h),
        _ => String::new(),
    }
}

/// Postvak IN of mailbox ?1, not hidden, with an address at a domain: ?2 matches the JSON of To
/// and Cc (`"email":"x@domain"`), ?3 the space-separated delivery addresses.
const DOMAIN_FILTER: &str = "m.account_id = ?1 AND m.hidden = 0
    AND m.folder_id IN (SELECT id FROM folders WHERE account_id = ?1 AND role = 'inbox')
    AND (lower(m.to_json) LIKE ?2 OR lower(m.cc_json) LIKE ?2 OR m.delivered_to LIKE ?3)";

/// The two LIKE patterns for a domain. Only letters, digits, dots and hyphens are accepted, so
/// nothing in it acts as a wildcard.
fn domain_patterns(domain: &str) -> Result<(String, String)> {
    let d = domain.trim().trim_start_matches('@').to_lowercase();
    let ok = d.contains('.') && d.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    if !ok {
        return Err(crate::error::VemError::new(crate::error::Code::AppCmd01).with_detail(format!("not a domain: {domain}")));
    }
    Ok((format!("%@{d}\"%"), format!("%@{d} %")))
}

/// Delivery addresses as stored: space-separated with a space at both ends, or NULL.
fn delivered_text(list: &[String]) -> Option<String> {
    if list.is_empty() { None } else { Some(format!(" {} ", list.join(" "))) }
}

impl Store {
    fn write_eml(&self, account_id: &str, id: i64, raw: &[u8]) -> Result<String> {
        let rel = format!("{account_id}/{id}.eml");
        let path = self.mail_dir().join(&rel);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, raw)?;
        std::fs::rename(&tmp, &path)?;
        Ok(rel)
    }

    /// Add a message to the index (and its file, when the body is there). Returns its id.
    /// A message whose (folder, uid) is already known is updated instead.
    pub fn insert_message(&self, m: &NewMessage) -> Result<i64> {
        let p = parse::parse(&m.raw);
        let date = if p.date != 0 { p.date } else { m.fallback_date.unwrap_or(0) };
        let id: i64 = {
            let db = self.db();
            if let Some(uid) = m.uid {
                let existing: Option<i64> = db
                    .query_row(
                        "SELECT id FROM messages WHERE folder_id = ?1 AND uid = ?2",
                        params![m.folder_id, uid],
                        |r| r.get(0),
                    )
                    .optional()?;
                if let Some(id) = existing {
                    drop(db);
                    self.set_flags(id, Some(m.unread), Some(m.important))?;
                    if m.has_body {
                        self.set_body(id, &m.raw)?;
                    }
                    return Ok(id);
                }
            }
            db.execute(
                "INSERT INTO messages (account_id, folder_id, uid, message_id_hdr, in_reply_to, references_json,
                    subject, from_name, from_email, to_json, cc_json, date, preview, unread, important,
                    has_attachments, size, has_body, delivered_to)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                params![
                    m.account_id,
                    m.folder_id,
                    m.uid,
                    p.message_id,
                    p.in_reply_to,
                    json(&p.references),
                    p.subject,
                    p.from.name,
                    p.from.email,
                    json(&p.to),
                    json(&p.cc),
                    date,
                    if m.has_body { p.preview.clone() } else { String::new() },
                    m.unread as i64,
                    m.important as i64,
                    (m.has_body && !p.attachments.is_empty()) as i64,
                    m.size.max(m.raw.len() as u64) as i64,
                    m.has_body as i64,
                    delivered_text(&p.delivered_to),
                ],
            )?;
            db.last_insert_rowid()
        };
        if m.has_body {
            let rel = self.write_eml(&m.account_id, id, &m.raw)?;
            self.db().execute("UPDATE messages SET eml_path = ?2 WHERE id = ?1", params![id, rel])?;
        }
        self.index_search(id, &p, m.has_body)?;
        Ok(id)
    }

    fn index_search(&self, id: i64, p: &ParsedMail, with_body: bool) -> Result<()> {
        let attach: Vec<&str> = p.attachments.iter().map(|a| a.name.as_str()).collect();
        let db = self.db();
        db.execute("DELETE FROM messages_fts WHERE rowid = ?1", [id])?;
        db.execute(
            "INSERT INTO messages_fts (rowid, subject, addrs, body, attach) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id,
                p.subject,
                addr_text(p),
                if with_body { body_text(p) } else { String::new() },
                attach.join(" ")
            ],
        )?;
        Ok(())
    }

    /// The full message arrived: write the file, fill preview and search text.
    pub fn set_body(&self, id: i64, raw: &[u8]) -> Result<()> {
        let account_id: String = self.db().query_row("SELECT account_id FROM messages WHERE id = ?1", [id], |r| r.get(0))?;
        let p = parse::parse(raw);
        let rel = self.write_eml(&account_id, id, raw)?;
        self.db().execute(
            "UPDATE messages SET eml_path = ?2, has_body = 1, preview = ?3, has_attachments = ?4,
                size = MAX(size, ?5), delivered_to = COALESCE(delivered_to, ?6) WHERE id = ?1",
            params![id, rel, p.preview, (!p.attachments.is_empty()) as i64, raw.len() as i64, delivered_text(&p.delivered_to)],
        )?;
        self.index_search(id, &p, true)
    }

    /// The file of a mailbox message is gone or empty (removed by a cleaner, a bad disk): forget
    /// it, so the next fetch downloads the message again from the server. A message that only
    /// lives here (imported, no server copy) is left as it is: its loss is reported, not hidden.
    /// Returns whether the body was forgotten.
    pub fn forget_lost_body(&self, id: i64) -> Result<bool> {
        let row: Option<(Option<String>, Option<i64>, bool)> = self
            .db()
            .query_row("SELECT eml_path, uid, has_body FROM messages WHERE id = ?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()?;
        let Some((Some(rel), Some(_uid), true)) = row else { return Ok(false) };
        let intact = std::fs::metadata(self.mail_dir().join(&rel)).is_ok_and(|m| m.len() > 0);
        if intact {
            return Ok(false);
        }
        self.db().execute("UPDATE messages SET has_body = 0, eml_path = NULL WHERE id = ?1", [id])?;
        Ok(true)
    }

    pub fn read_eml(&self, id: i64) -> Result<Option<Vec<u8>>> {
        let path: Option<String> = self
            .db()
            .query_row("SELECT eml_path FROM messages WHERE id = ?1", [id], |r| r.get(0))
            .optional()?
            .flatten();
        match path {
            None => Ok(None),
            Some(rel) => match std::fs::read(self.mail_dir().join(rel)) {
                // Every stored message has at least a header; an empty file is a lost one.
                Ok(raw) if raw.is_empty() => Err(VemError::new(Code::StoreRead01).with_detail("empty message file")),
                Ok(raw) => Ok(Some(raw)),
                Err(e) => Err(VemError::new(Code::StoreRead01).with_detail(e.kind().to_string())),
            },
        }
    }

    pub(super) fn eml_paths_in_folder(&self, folder_id: i64) -> Result<Vec<String>> {
        let db = self.db();
        let mut st = db.prepare("SELECT eml_path FROM messages WHERE folder_id = ?1 AND eml_path IS NOT NULL")?;
        let rows = st.query_map([folder_id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    fn with_tags(&self, mut list: Vec<MessageSummary>) -> Result<Vec<MessageSummary>> {
        if list.is_empty() {
            return Ok(list);
        }
        let ids: Vec<i64> = list.iter().filter_map(|m| m.id.parse().ok()).collect();
        let placeholders = vec!["?"; ids.len()].join(",");
        let db = self.db();
        let mut st = db.prepare(&format!(
            "SELECT message_id, tag_id FROM message_tags WHERE message_id IN ({placeholders}) ORDER BY tag_id"
        ))?;
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for row in st.query_map(params_from_iter(ids.iter()), |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (m, t) = row?;
            map.entry(message_key(m)).or_default().push(t);
        }
        for m in &mut list {
            if let Some(t) = map.remove(&m.id) {
                m.tag_ids = t;
            }
        }
        Ok(list)
    }

    pub(crate) fn summaries_where(&self, filter: &str, args: &[&dyn rusqlite::ToSql]) -> Result<Vec<MessageSummary>> {
        let list = {
            let db = self.db();
            let mut st = db.prepare(&format!("SELECT {SUMMARY_COLS} FROM messages m WHERE {filter}"))?;
            st.query_map(args, row_to_summary)?.collect::<rusqlite::Result<Vec<_>>>()?
        };
        self.with_tags(list)
    }

    pub fn messages_in_folder(&self, folder_id: i64, offset: u32, limit: u32) -> Result<Vec<MessageSummary>> {
        self.summaries_where(
            "m.folder_id = ?1 AND m.hidden = 0 ORDER BY m.date DESC, m.id DESC LIMIT ?2 OFFSET ?3",
            &[&folder_id, &limit, &offset],
        )
    }

    /// Every account's Postvak IN together (C7, "Alle postvakken").
    pub fn messages_in_all_inboxes(&self, offset: u32, limit: u32) -> Result<Vec<MessageSummary>> {
        self.summaries_where(
            "m.hidden = 0 AND m.folder_id IN (SELECT id FROM folders WHERE role = 'inbox')
             ORDER BY m.date DESC, m.id DESC LIMIT ?1 OFFSET ?2",
            &[&limit, &offset],
        )
    }

    /// Postvak IN of one mailbox, only the mail addressed to one domain (decision 32): in To,
    /// Cc or the server's delivery header. The main Postvak IN keeps showing everything.
    pub fn messages_for_domain(&self, account_id: &str, domain: &str, offset: u32, limit: u32) -> Result<Vec<MessageSummary>> {
        let (to, delivered) = domain_patterns(domain)?;
        self.summaries_where(
            &format!("{DOMAIN_FILTER} ORDER BY m.date DESC, m.id DESC LIMIT ?4 OFFSET ?5"),
            &[&account_id, &to, &delivered, &limit, &offset],
        )
    }

    /// Unread messages in Postvak IN addressed to one domain, for the count in the folder pane.
    pub fn domain_unread(&self, account_id: &str, domain: &str) -> Result<u32> {
        let (to, delivered) = domain_patterns(domain)?;
        let n: i64 = self.db().query_row(
            &format!("SELECT COUNT(*) FROM messages m WHERE {DOMAIN_FILTER} AND m.unread = 1"),
            params![account_id, to, delivered],
            |r| r.get(0),
        )?;
        Ok(n as u32)
    }

    pub fn messages_from_addresses(&self, emails: &[String], limit: u32) -> Result<Vec<MessageSummary>> {
        if emails.is_empty() {
            return Ok(Vec::new());
        }
        let lowered: Vec<String> = emails.iter().map(|e| e.to_lowercase()).collect();
        let placeholders = vec!["?"; lowered.len()].join(",");
        let mut args: Vec<&dyn rusqlite::ToSql> = lowered.iter().map(|e| e as &dyn rusqlite::ToSql).collect();
        args.push(&limit);
        self.summaries_where(
            &format!(
                "m.hidden = 0 AND lower(m.from_email) IN ({placeholders}) ORDER BY m.date DESC LIMIT ?{}",
                lowered.len() + 1
            ),
            &args,
        )
    }

    pub fn message(&self, id: i64) -> Result<StoredMessage> {
        let row = self
            .db()
            .query_row(
                &format!(
                    "SELECT {SUMMARY_COLS}, m.uid, m.cc_json, m.message_id_hdr, m.references_json, m.eml_path,
                        m.has_body, m.is_draft, m.draft_json, m.remote_images_ok, m.size
                     FROM messages m WHERE m.id = ?1"
                ),
                [id],
                |r| {
                    let summary = row_to_summary(r)?;
                    Ok(StoredMessage {
                        id,
                        folder_id: summary.folder_id.parse().unwrap_or_default(),
                        summary,
                        uid: r.get::<_, Option<i64>>(12)?.map(|u| u as u32),
                        cc: serde_json::from_str(&r.get::<_, String>(13)?).unwrap_or_default(),
                        message_id_hdr: r.get(14)?,
                        references: from_json(r.get(15)?),
                        eml_path: r.get(16)?,
                        has_body: r.get::<_, i64>(17)? != 0,
                        is_draft: r.get::<_, i64>(18)? != 0,
                        draft: r.get::<_, Option<String>>(19)?.and_then(|s| serde_json::from_str(&s).ok()),
                        remote_images_ok: r.get::<_, i64>(20)? != 0,
                        size: r.get::<_, i64>(21)? as u64,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| VemError::new(Code::StoreRead01).with_detail("message"))?;
        let mut tagged = self.with_tags(vec![row.summary.clone()])?;
        Ok(StoredMessage { summary: tagged.remove(0), ..row })
    }

    pub fn set_flags(&self, id: i64, unread: Option<bool>, important: Option<bool>) -> Result<()> {
        self.db().execute(
            "UPDATE messages SET unread = COALESCE(?2, unread), important = COALESCE(?3, important) WHERE id = ?1",
            params![id, unread.map(|v| v as i64), important.map(|v| v as i64)],
        )?;
        Ok(())
    }

    pub fn set_remote_images_ok(&self, id: i64) -> Result<()> {
        self.db().execute("UPDATE messages SET remote_images_ok = 1 WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn set_hidden(&self, id: i64, hidden: bool) -> Result<()> {
        self.db().execute("UPDATE messages SET hidden = ?2 WHERE id = ?1", params![id, hidden as i64])?;
        Ok(())
    }

    /// Move locally; the server copy is moved by a pending operation. The UID is forgotten
    /// because the message gets a new one in the target folder.
    pub fn move_local(&self, id: i64, folder_id: i64) -> Result<()> {
        self.db().execute(
            "UPDATE messages SET folder_id = ?2, uid = NULL, hidden = 0 WHERE id = ?1",
            params![id, folder_id],
        )?;
        Ok(())
    }

    pub fn delete_message(&self, id: i64) -> Result<()> {
        let path: Option<String> = self
            .db()
            .query_row("SELECT eml_path FROM messages WHERE id = ?1", [id], |r| r.get(0))
            .optional()?
            .flatten();
        {
            let db = self.db();
            db.execute("DELETE FROM messages_fts WHERE rowid = ?1", [id])?;
            db.execute("DELETE FROM messages WHERE id = ?1", [id])?;
        }
        if let Some(rel) = path {
            let _ = std::fs::remove_file(self.mail_dir().join(rel));
        }
        Ok(())
    }

    /// (id, uid) of the server messages known in a folder.
    pub fn uids_in_folder(&self, folder_id: i64) -> Result<Vec<(i64, u32)>> {
        let db = self.db();
        let mut st = db.prepare("SELECT id, uid FROM messages WHERE folder_id = ?1 AND uid IS NOT NULL")?;
        let rows = st
            .query_map([folder_id], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u32)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    pub fn id_for_uid(&self, folder_id: i64, uid: u32) -> Result<Option<i64>> {
        Ok(self
            .db()
            .query_row("SELECT id FROM messages WHERE folder_id = ?1 AND uid = ?2", params![folder_id, uid], |r| r.get(0))
            .optional()?)
    }

    /// A message we put in this folder ourselves (moved, sent copy, draft, import) has no UID
    /// yet. When the server reports it, give it the UID instead of adding it twice.
    pub fn adopt_by_message_id(&self, folder_id: i64, message_id_hdr: &str, uid: u32) -> Result<Option<i64>> {
        let id: Option<i64> = self
            .db()
            .query_row(
                "SELECT id FROM messages WHERE folder_id = ?1 AND uid IS NULL AND message_id_hdr = ?2
                 ORDER BY id LIMIT 1",
                params![folder_id, message_id_hdr],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = id {
            self.set_uid(id, Some(uid))?;
        }
        Ok(id)
    }

    /// Whether a folder already holds a message with this Message-ID (a second import of the same file).
    pub fn has_message_id(&self, folder_id: i64, message_id_hdr: &str) -> Result<bool> {
        Ok(self
            .db()
            .query_row(
                "SELECT 1 FROM messages WHERE folder_id = ?1 AND message_id_hdr = ?2 LIMIT 1",
                params![folder_id, message_id_hdr],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    pub fn set_uid(&self, id: i64, uid: Option<u32>) -> Result<()> {
        self.db().execute("UPDATE messages SET uid = ?2 WHERE id = ?1", params![id, uid])?;
        Ok(())
    }

    /// Messages whose body is still on the server, newest first (docs/sync.md, Bodies).
    pub fn without_body(&self, account_id: &str, limit: u32) -> Result<Vec<(i64, i64, u32)>> {
        let db = self.db();
        let mut st = db.prepare(
            "SELECT id, folder_id, uid FROM messages
             WHERE has_body = 0 AND uid IS NOT NULL AND account_id = ?1 ORDER BY date DESC LIMIT ?2",
        )?;
        let rows = st
            .query_map(params![account_id, limit], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? as u32)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    /// How many bytes of mail an account holds (visible messages), for the space check of an import.
    pub fn account_bytes(&self, account_id: &str) -> Result<u64> {
        let n: i64 = self.db().query_row(
            "SELECT COALESCE(SUM(size), 0) FROM messages WHERE hidden = 0 AND account_id = ?1",
            [account_id],
            |r| r.get(0),
        )?;
        Ok(n.max(0) as u64)
    }

    pub fn count_without_body(&self, account_id: &str) -> Result<u64> {
        let n: i64 = self.db().query_row(
            "SELECT COUNT(*) FROM messages WHERE has_body = 0 AND uid IS NOT NULL AND account_id = ?1",
            [account_id],
            |r| r.get(0),
        )?;
        Ok(n as u64)
    }

    /// Sum of message sizes, for mailbox space when the server has no QUOTA.
    pub fn account_size_kb(&self, account_id: &str) -> Result<u64> {
        let n: i64 = self.db().query_row(
            "SELECT COALESCE(SUM(size), 0) FROM messages WHERE account_id = ?1 AND uid IS NOT NULL",
            [account_id],
            |r| r.get(0),
        )?;
        Ok((n as u64).div_ceil(1024))
    }

    /// Save a draft in the account's Concepten folder (never lose text: docs/screens.md).
    pub fn save_draft(&self, draft: &Draft, folder_id: i64, raw: &[u8]) -> Result<i64> {
        let existing = draft.id.as_deref().and_then(|k| k.parse::<i64>().ok());
        let id = match existing {
            Some(id) if self.message(id).map(|m| m.is_draft).unwrap_or(false) => {
                let p = parse::parse(raw);
                self.db().execute(
                    "UPDATE messages SET subject = ?2, to_json = ?3, cc_json = ?4, date = ?5, preview = ?6
                     WHERE id = ?1",
                    params![id, p.subject, json(&p.to), json(&p.cc), crate::model::now(), p.preview],
                )?;
                self.set_body(id, raw)?;
                id
            }
            _ => {
                let id = self.insert_message(&NewMessage {
                    account_id: draft.account_id.clone(),
                    folder_id,
                    uid: None,
                    raw: raw.to_vec(),
                    has_body: true,
                    unread: false,
                    important: false,
                    size: raw.len() as u64,
                    fallback_date: Some(crate::model::now()),
                })?;
                self.db().execute(
                    "UPDATE messages SET is_draft = 1, date = ?2 WHERE id = ?1",
                    params![id, crate::model::now()],
                )?;
                id
            }
        };
        let stored = Draft { id: Some(message_key(id)), ..draft.clone() };
        self.db()
            .execute("UPDATE messages SET draft_json = ?2 WHERE id = ?1", params![id, json(&stored)])?;
        Ok(id)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::AccountKind;
    use crate::store::testutil::store;

    pub fn mail(subject: &str, from: &str, body: &str, date: &str) -> Vec<u8> {
        format!(
            "From: {from}\r\nTo: Ann <ann@example.com>\r\nSubject: {subject}\r\nDate: {date}\r\nMessage-ID: <{subject}@t>\r\n\r\n{body}\r\n"
        )
        .into_bytes()
    }

    pub fn seed(s: &Store) -> (String, i64) {
        let a = s.add_account("Jan", "jan@x.nl", AccountKind::Imap, None).unwrap();
        let f = s.upsert_folder(&a.id, "INBOX", Some("/"), Some(crate::model::FolderRole::Inbox), true).unwrap();
        (a.id, f)
    }

    /// Decision 32: a mailbox with custom-domain addresses shows, per domain, only the Postvak IN
    /// mail addressed there: in To, in Cc, or only in the server's Delivered-To (BCC).
    #[test]
    fn postvak_in_per_domain_finds_to_cc_and_delivered_to() {
        let (_d, s) = store();
        let (acc, inbox) = seed(&s);
        let put = |raw: String, unread: bool| {
            s.insert_message(&NewMessage { account_id: acc.clone(), folder_id: inbox, raw: raw.into_bytes(), has_body: true, unread, ..Default::default() })
                .unwrap()
        };
        let head = "From: a@b.nl
Date: Mon, 16 Nov 2026 10:00:00 +0100
";
        put(format!("{head}To: Info <INFO@Familie.nl>
Subject: aan

x
"), true);
        put(format!("{head}To: iemand@elders.nl
Cc: oma@familie.nl
Subject: cc

x
"), false);
        put(format!("{head}Delivered-To: <ria@familie.nl>
To: lijst@club.nl
Subject: bcc

x
"), true);
        put(format!("{head}To: jan@x.nl
Subject: gewoon

x
"), true);
        put(format!("{head}To: iemand@nietfamilie.nl
Subject: lijkt erop

x
"), true);
        let mut subjects: Vec<String> = s.messages_for_domain(&acc, "familie.nl", 0, 50).unwrap().into_iter().map(|m| m.subject).collect();
        subjects.sort();
        assert_eq!(subjects, ["aan", "bcc", "cc"]);
        assert_eq!(s.domain_unread(&acc, "familie.nl").unwrap(), 2);
        assert_eq!(s.messages_in_folder(inbox, 0, 50).unwrap().len(), 5, "the main Postvak IN keeps everything");
        assert!(s.messages_for_domain(&acc, "fam%lie.nl", 0, 50).is_err(), "no wildcards");

        assert!(s.domain_inboxes().unwrap().is_empty());
        s.set_account_addresses(&acc, &["Info@Familie.nl".into(), "jan@x.nl".into(), "oma@familie.nl".into(), "post@werk.nl".into()]).unwrap();
        let account = s.account(&acc).unwrap();
        assert_eq!(account.addresses, ["info@familie.nl", "oma@familie.nl", "post@werk.nl"], "lower case, no main address");
        let inboxes: Vec<(String, u32)> = s.domain_inboxes().unwrap().into_iter().map(|d| (d.domain, d.unread)).collect();
        assert_eq!(inboxes, [("familie.nl".to_string(), 2), ("werk.nl".to_string(), 0)]);
        assert!(s.set_account_addresses(&acc, &["geen-adres".into()]).is_err());
        assert_eq!(account.send_from(Some("OMA@familie.nl")), "oma@familie.nl");
        assert_eq!(account.send_from(Some("vreemd@elders.nl")), "jan@x.nl", "never an address that is not the mailbox's");
        assert_eq!(account.send_from(None), "jan@x.nl");
    }

    #[test]
    fn insert_list_and_body_later() {
        let (_d, s) = store();
        let (acc, inbox) = seed(&s);
        let raw = mail("Hoi", "Piet <piet@x.nl>", "Gezellig", "Mon, 16 Nov 2026 10:00:00 +0100");
        let header_end = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        let id = s
            .insert_message(&NewMessage {
                account_id: acc.clone(),
                folder_id: inbox,
                uid: Some(7),
                raw: raw[..header_end].to_vec(),
                unread: true,
                ..Default::default()
            })
            .unwrap();
        let list = s.messages_in_folder(inbox, 0, 50).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].from.email, "piet@x.nl");
        assert!(list[0].preview.is_empty());
        assert_eq!(s.without_body(&acc, 10).unwrap(), vec![(id, inbox, 7)]);
        s.set_body(id, &raw).unwrap();
        assert_eq!(s.messages_in_folder(inbox, 0, 50).unwrap()[0].preview, "Gezellig");
        assert_eq!(s.read_eml(id).unwrap().unwrap(), raw);
        assert!(s.without_body(&acc, 10).unwrap().is_empty());
        // Same uid again only updates flags.
        let again = s
            .insert_message(&NewMessage { account_id: acc, folder_id: inbox, uid: Some(7), raw, unread: false, has_body: true, ..Default::default() })
            .unwrap();
        assert_eq!(again, id);
        assert!(!s.message(id).unwrap().summary.unread);
    }

    /// A message file removed behind the app's back: a mailbox message is fetched again, a
    /// message that only lives here reports its loss (VEM-STORE-READ-01) instead of showing empty.
    #[test]
    fn a_lost_message_file_is_fetched_again_or_reported() {
        let (_d, s) = store();
        let (acc, inbox) = seed(&s);
        let raw = mail("Hoi", "Piet <piet@x.nl>", "Gezellig", "Mon, 16 Nov 2026 10:00:00 +0100");
        let on_server = s
            .insert_message(&NewMessage { account_id: acc.clone(), folder_id: inbox, uid: Some(3), raw: raw.clone(), has_body: true, ..Default::default() })
            .unwrap();
        let only_here = s
            .insert_message(&NewMessage { account_id: acc.clone(), folder_id: inbox, uid: None, raw: raw.clone(), has_body: true, ..Default::default() })
            .unwrap();
        assert!(!s.forget_lost_body(on_server).unwrap(), "an intact file is kept");

        let path_of = |id: i64| -> std::path::PathBuf {
            let rel: String = s.db().query_row("SELECT eml_path FROM messages WHERE id = ?1", [id], |r| r.get(0)).unwrap();
            s.mail_dir().join(rel)
        };
        std::fs::remove_file(path_of(on_server)).unwrap();
        std::fs::write(path_of(only_here), b"").unwrap();

        assert!(s.forget_lost_body(on_server).unwrap());
        assert_eq!(s.without_body(&acc, 10).unwrap(), vec![(on_server, inbox, 3)], "queued for download");
        assert_eq!(s.read_eml(on_server).unwrap(), None);
        s.set_body(on_server, &raw).unwrap();
        assert_eq!(s.read_eml(on_server).unwrap().unwrap(), raw, "fetched again, readable again");

        assert!(!s.forget_lost_body(only_here).unwrap(), "no server copy: nothing to fetch");
        assert_eq!(s.read_eml(only_here).unwrap_err().code(), Some(Code::StoreRead01), "an emptied file is not shown as an empty mail");
        std::fs::remove_file(path_of(only_here)).unwrap();
        assert_eq!(s.read_eml(only_here).unwrap_err().code(), Some(Code::StoreRead01));
    }

    #[test]
    fn hidden_messages_drop_out_and_unified_inbox() {
        let (_d, s) = store();
        let (acc, inbox) = seed(&s);
        let b = s.add_account("B", "b@x.nl", AccountKind::Imap, None).unwrap();
        let inbox_b = s.upsert_folder(&b.id, "INBOX", None, Some(crate::model::FolderRole::Inbox), true).unwrap();
        let m1 = s.insert_message(&NewMessage { account_id: acc, folder_id: inbox, uid: Some(1), raw: mail("a", "p@x.nl", "x", "Mon, 16 Nov 2026 10:00:00 +0100"), has_body: true, ..Default::default() }).unwrap();
        s.insert_message(&NewMessage { account_id: b.id, folder_id: inbox_b, uid: Some(1), raw: mail("b", "p@x.nl", "x", "Tue, 17 Nov 2026 10:00:00 +0100"), has_body: true, ..Default::default() }).unwrap();
        let all = s.messages_in_all_inboxes(0, 10).unwrap();
        assert_eq!(all.iter().map(|m| m.subject.as_str()).collect::<Vec<_>>(), vec!["b", "a"]);
        s.set_hidden(m1, true).unwrap();
        assert_eq!(s.messages_in_all_inboxes(0, 10).unwrap().len(), 1);
        s.delete_message(m1).unwrap();
        assert!(s.message(m1).is_err());
    }

    #[test]
    fn drafts_update_in_place() {
        let (_d, s) = store();
        let (acc, inbox) = seed(&s);
        let drafts = s.upsert_folder(&acc, "Drafts", None, Some(crate::model::FolderRole::Drafts), true).unwrap();
        let d = Draft { account_id: acc.clone(), subject: "Eerste".into(), ..Default::default() };
        let id = s.save_draft(&d, drafts, &mail("Eerste", "jan@x.nl", "a", "Mon, 16 Nov 2026 10:00:00 +0100")).unwrap();
        let d2 = Draft { id: Some(id.to_string()), subject: "Tweede".into(), ..d };
        let id2 = s.save_draft(&d2, drafts, &mail("Tweede", "jan@x.nl", "b", "Mon, 16 Nov 2026 10:00:00 +0100")).unwrap();
        assert_eq!(id, id2);
        let m = s.message(id).unwrap();
        assert!(m.is_draft);
        assert_eq!(m.summary.subject, "Tweede");
        assert_eq!(m.draft.unwrap().subject, "Tweede");
        assert_eq!(s.messages_in_folder(inbox, 0, 10).unwrap().len(), 0);
    }
}
