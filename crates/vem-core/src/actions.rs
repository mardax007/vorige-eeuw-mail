//! What the user does to messages: read, important, delete, move, tags, undo, and opening one.
//!
//! The screen changes at once in the local index; the server follows through the pending
//! operations (imap::ops). Moves and deletes wait until the undo window has passed, so
//! "Ongedaan maken" never has to reach the server.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::error::{Code, Result, VemError};
use crate::imap::ops::Op;
use crate::model::{Attachment, FolderRole, MailWarning, MessageDetail, MessageSummary, UndoToken, WarningKind, now};
use crate::parse;
use crate::sanitize;
use crate::store::kv::UNDO_KEEP_SECS;
use crate::store::messages::parse_message_key;
use crate::store::{Store, folders::parse_folder_key};

/// Moves and deletes reach the server only after this many seconds (a little past the undo window).
pub const DELAY_SECS: i64 = UNDO_KEEP_SECS + 5;
const PURGE_KEY: &str = "purge_queue";

/// Accounts whose screens changed, so the caller can send `mail:changed` and wake their sync.
pub type Touched = BTreeSet<String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UndoEntry {
    message_id: i64,
    account_id: String,
    folder_id: i64,
    uid: Option<u32>,
    /// The pending operation this action replaced, put back on undo.
    replaced: Option<Op>,
    /// The pending operation this action added, removed on undo.
    added: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PurgeItem {
    message_id: i64,
    after: i64,
}

fn ids(keys: &[String]) -> Result<Vec<i64>> {
    keys.iter().map(|k| parse_message_key(k)).collect()
}

pub fn set_read(store: &Store, keys: &[String], read: bool) -> Result<Touched> {
    let mut touched = Touched::new();
    for id in ids(keys)? {
        let m = store.message(id)?;
        if m.summary.unread == !read {
            continue;
        }
        store.set_flags(id, Some(!read), None)?;
        if m.uid.is_some() {
            store.push_op(&m.summary.account_id, &Op::Flags { message_id: id, seen: Some(read), flagged: None })?;
        }
        touched.insert(m.summary.account_id);
    }
    Ok(touched)
}

pub fn set_important(store: &Store, keys: &[String], important: bool) -> Result<Touched> {
    let mut touched = Touched::new();
    for id in ids(keys)? {
        let m = store.message(id)?;
        store.set_flags(id, None, Some(important))?;
        if m.uid.is_some() {
            store.push_op(&m.summary.account_id, &Op::Flags { message_id: id, seen: None, flagged: Some(important) })?;
        }
        touched.insert(m.summary.account_id);
    }
    Ok(touched)
}

/// The pending move of a message, if one waits (a second move within the undo window).
fn pending_move(store: &Store, account_id: &str, message_id: i64) -> Result<Option<(i64, Op)>> {
    Ok(store
        .ops::<Op>(account_id)?
        .into_iter()
        .find(|p| matches!(p.op, Op::Move { message_id: m, .. } if m == message_id))
        .map(|p| (p.id, p.op)))
}

/// Move one message locally and plan the server move. `to` None means delete for good.
fn relocate(store: &Store, id: i64, to: Option<i64>) -> Result<UndoEntry> {
    let m = store.message(id)?;
    let account = m.summary.account_id.clone();
    let mut entry = UndoEntry {
        message_id: id,
        account_id: account.clone(),
        folder_id: m.folder_id,
        uid: m.uid,
        replaced: None,
        added: None,
    };
    // Where the server copy really is: its current place, or the start of a move still waiting.
    let mut source = m.uid.map(|uid| (m.folder_id, uid));
    if let Some((op_id, op)) = pending_move(store, &account, id)? {
        if let Op::Move { from_folder, uid, .. } = op {
            source = Some((from_folder, uid));
        }
        store.op_done(op_id)?;
        entry.replaced = Some(op);
    }
    match to {
        Some(folder) => store.move_local(id, folder)?,
        None => store.set_hidden(id, true)?,
    }
    match source {
        Some((from_folder, uid)) if Some(from_folder) != to => {
            let op = Op::Move { message_id: id, from_folder, uid, to_folder: to, after: now() + DELAY_SECS };
            entry.added = Some(store.push_op(&account, &op)?);
        }
        Some((from_folder, uid)) => {
            // Moved back where the server has it: nothing to do there, keep the UID.
            store.set_uid(id, Some(uid))?;
            let _ = from_folder;
        }
        None if to.is_none() => queue_purge(store, id)?,
        None => {}
    }
    Ok(entry)
}

fn queue_purge(store: &Store, id: i64) -> Result<()> {
    let mut q: Vec<PurgeItem> = store.kv_get(PURGE_KEY)?.unwrap_or_default();
    q.push(PurgeItem { message_id: id, after: now() + DELAY_SECS });
    store.kv_set(PURGE_KEY, &q)
}

/// Delete for good the local-only messages whose undo window passed. Called now and then.
pub fn purge_due(store: &Store) -> Result<usize> {
    let q: Vec<PurgeItem> = store.kv_get(PURGE_KEY)?.unwrap_or_default();
    let t = now();
    let (due, keep): (Vec<_>, Vec<_>) = q.into_iter().partition(|p| p.after <= t);
    for p in &due {
        if store.message(p.message_id).is_ok() {
            store.delete_message(p.message_id)?;
        }
    }
    store.kv_set(PURGE_KEY, &keep)?;
    Ok(due.len())
}

fn finish(store: &Store, entries: Vec<UndoEntry>) -> Result<(UndoToken, Touched)> {
    let touched: Touched = entries.iter().map(|e| e.account_id.clone()).collect();
    let count = entries.len();
    let token = store.put_undo(&entries)?;
    Ok((UndoToken { token, count }, touched))
}

/// "Verwijderen": to Verwijderde items; from Verwijderde items (or without one) for good.
pub fn delete_messages(store: &Store, keys: &[String]) -> Result<(UndoToken, Touched)> {
    let mut entries = Vec::new();
    for id in ids(keys)? {
        let m = store.message(id)?;
        let trash = store.folder_by_role(&m.summary.account_id, FolderRole::Trash)?;
        let to = match trash {
            Some(t) if t != m.folder_id => Some(t),
            _ => None,
        };
        entries.push(relocate(store, id, to)?);
    }
    finish(store, entries)
}

pub fn move_messages(store: &Store, keys: &[String], folder_key: &str) -> Result<(UndoToken, Touched)> {
    let target = parse_folder_key(folder_key)?;
    let target_account = store.folder(target)?.account_id;
    let mut entries = Vec::new();
    for id in ids(keys)? {
        let m = store.message(id)?;
        if m.summary.account_id != target_account {
            // Moving between mailboxes is not offered on screen; refuse rather than lose mail.
            return Err(VemError::new(Code::AppCmd01).with_detail("move between accounts"));
        }
        if m.folder_id != target {
            entries.push(relocate(store, id, Some(target))?);
        }
    }
    finish(store, entries)
}

/// "Ongedaan maken". An expired token does nothing.
pub fn undo(store: &Store, token: &str) -> Result<Touched> {
    let Some(entries) = store.take_undo::<Vec<UndoEntry>>(token)? else { return Ok(Touched::new()) };
    let mut q: Vec<PurgeItem> = store.kv_get(PURGE_KEY)?.unwrap_or_default();
    let mut touched = Touched::new();
    for e in entries.into_iter().rev() {
        if let Some(op_id) = e.added {
            store.op_done(op_id)?;
        }
        if let Some(op) = &e.replaced {
            store.push_op(&e.account_id, op)?;
        }
        q.retain(|p| p.message_id != e.message_id);
        if store.message(e.message_id).is_ok() {
            store.move_local(e.message_id, e.folder_id)?;
            store.set_uid(e.message_id, e.uid)?;
        }
        touched.insert(e.account_id);
    }
    store.kv_set(PURGE_KEY, &q)?;
    Ok(touched)
}

/// "Verwijderde items leegmaken": everything in it goes for good, now (the screen asked first).
pub fn empty_trash(store: &Store, account_id: &str) -> Result<usize> {
    let Some(trash) = store.folder_by_role(account_id, FolderRole::Trash)? else { return Ok(0) };
    let mut n = 0;
    loop {
        let batch = store.messages_in_folder(trash, 0, 500)?;
        if batch.is_empty() {
            break;
        }
        for s in batch {
            let id = parse_message_key(&s.id)?;
            let m = store.message(id)?;
            if let Some((op_id, _)) = pending_move(store, account_id, id)? {
                store.op_done(op_id)?;
            }
            match m.uid {
                Some(uid) => {
                    store.set_hidden(id, true)?;
                    store.push_op(account_id, &Op::Move { message_id: id, from_folder: trash, uid, to_folder: None, after: 0 })?;
                }
                None => store.delete_message(id)?,
            }
            n += 1;
        }
    }
    Ok(n)
}

pub fn set_message_tag(store: &Store, keys: &[String], tag_id: &str, on: bool) -> Result<Touched> {
    let keyword = store.tag_keyword(tag_id)?;
    let mut touched = Touched::new();
    for id in ids(keys)? {
        let m = store.message(id)?;
        let has = m.summary.tag_ids.iter().any(|t| t == tag_id);
        if has == on {
            continue;
        }
        store.set_message_tag(id, tag_id, on)?;
        if m.uid.is_some() {
            store.push_op(&m.summary.account_id, &Op::Keyword { message_id: id, keyword: keyword.clone(), add: on })?;
        }
        touched.insert(m.summary.account_id);
    }
    Ok(touched)
}

pub fn contact_messages(store: &Store, contact_id: &str) -> Result<Vec<MessageSummary>> {
    let c = store.contact(contact_id)?;
    store.messages_from_addresses(&c.emails, 200)
}

/// Everything the reading pane shows. The body must be local already (the engine fetches it).
pub fn message_detail(store: &Store, key: &str, show_remote: bool) -> Result<MessageDetail> {
    let id = parse_message_key(key)?;
    let m = store.message(id)?;
    let raw = store.read_eml(id)?.unwrap_or_default();
    let p = parse::parse(&raw);
    if show_remote && !m.remote_images_ok {
        store.set_remote_images_ok(id)?;
    }
    let allow = show_remote || m.remote_images_ok;
    let inline = parse::inline_images(&raw);
    let clean = p.html.as_deref().map(|h| sanitize::clean_html(h, &inline, allow));

    let mut warnings: Vec<MailWarning> = clean.as_ref().map(|c| sanitize::link_warnings(&c.html)).unwrap_or_default();
    let from = &m.summary.from;
    let own = store.accounts()?.iter().any(|a| a.email.eq_ignore_ascii_case(&from.email));
    if !own {
        if let Some(name) = from.name.as_deref()
            && let Some(c) = store.contact_with_name_other_address(name, &from.email)? {
                warnings.push(MailWarning {
                    kind: WarningKind::LookalikeContact,
                    shown: Some(c.name),
                    actual: Some(from.email.clone()),
                });
            }
        let text = p.text.clone().or_else(|| p.html.as_deref().map(mail_parser::html_to_text)).unwrap_or_default();
        let known = store.contact_by_email(&from.email)?.is_some();
        if !known && (sanitize::mentions_money(&text) || sanitize::mentions_money(&p.subject)) {
            warnings.push(MailWarning { kind: WarningKind::FirstContactMoney, shown: None, actual: Some(from.email.clone()) });
        }
    }

    let inline_ids: BTreeSet<String> = inline.iter().map(|(cid, _, _)| cid.to_ascii_lowercase()).collect();
    let html_text = p.html.as_deref().unwrap_or("").to_ascii_lowercase();
    let attachments = p
        .attachments
        .iter()
        .filter(|a| {
            // Pictures shown inside the message are not listed again as attachments.
            !a.content_id.as_deref().is_some_and(|c| {
                let c = c.trim_matches(['<', '>']).to_ascii_lowercase();
                inline_ids.contains(&c) && html_text.contains(&format!("cid:{c}"))
            })
        })
        .map(|a| Attachment { id: a.index.to_string(), name: a.name.clone(), size: a.size as u64, mime: a.mime.clone() })
        .collect();

    Ok(MessageDetail {
        cc: p.cc.clone(),
        reply_to: p.reply_to.clone(),
        has_remote_images: clean.as_ref().is_some_and(|c| c.has_remote_images),
        remote_images_shown: allow,
        html: clean.map(|c| c.html),
        text: p.text,
        attachments,
        warnings,
        message_id_header: m.message_id_hdr.clone(),
        references: m.references.clone(),
        summary: m.summary,
    })
}

/// "Doorsturen": the attachments the reading pane lists for this message, written as files in
/// `dir`, so the Write screen sends them along as Outlook does. The body must be local already.
pub fn forward_attachments(store: &Store, key: &str, dir: &std::path::Path) -> Result<Vec<std::path::PathBuf>> {
    let shown = message_detail(store, key, false)?.attachments;
    if shown.is_empty() {
        return Ok(Vec::new());
    }
    let raw = store
        .read_eml(parse_message_key(key)?)?
        .ok_or_else(|| VemError::new(Code::StoreRead01).with_detail("body"))?;
    std::fs::create_dir_all(dir)?;
    let mut paths = Vec::new();
    for a in shown {
        let index: usize = a.id.parse().map_err(|_| VemError::new(Code::StoreRead01).with_detail("attachment"))?;
        let bytes =
            parse::attachment_bytes(&raw, index).ok_or_else(|| VemError::new(Code::StoreRead01).with_detail("attachment"))?;
        let name = parse::safe_file_name(&a.name);
        // Two attachments with the same name stay two files.
        let mut path = dir.join(&name);
        let mut n = 2;
        while path.exists() {
            path = dir.join(format!("{n} {name}"));
            n += 1;
        }
        std::fs::write(&path, bytes)?;
        paths.push(path);
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AccountKind;
    use crate::store::NewMessage;
    use crate::store::testutil::store;

    fn setup() -> (tempfile::TempDir, Store, String, i64, i64, i64) {
        let (d, s) = store();
        let a = s.add_account("Jan", "jan@x.nl", AccountKind::Imap, None).unwrap();
        let inbox = s.upsert_folder(&a.id, "INBOX", Some("/"), Some(FolderRole::Inbox), true).unwrap();
        let trash = s.upsert_folder(&a.id, "Trash", Some("/"), Some(FolderRole::Trash), true).unwrap();
        let raw = b"From: Piet <piet@y.nl>\r\nSubject: Hoi\r\nMessage-ID: <1@y>\r\n\r\nTot morgen\r\n".to_vec();
        let id = s
            .insert_message(&NewMessage {
                account_id: a.id.clone(),
                folder_id: inbox,
                uid: Some(7),
                raw: raw.clone(),
                has_body: true,
                unread: true,
                size: raw.len() as u64,
                ..Default::default()
            })
            .unwrap();
        (d, s, a.id, inbox, trash, id)
    }

    #[test]
    fn delete_then_undo_restores_and_drops_the_server_op() {
        let (_d, s, acc, inbox, trash, id) = setup();
        let (tok, touched) = delete_messages(&s, &[id.to_string()]).unwrap();
        assert!(touched.contains(&acc));
        assert_eq!(s.message(id).unwrap().folder_id, trash);
        let ops = s.ops::<Op>(&acc).unwrap();
        assert!(matches!(ops[0].op, Op::Move { from_folder, uid: 7, to_folder: Some(t), after, .. }
            if from_folder == inbox && t == trash && after > now()));
        undo(&s, &tok.token).unwrap();
        let m = s.message(id).unwrap();
        assert_eq!((m.folder_id, m.uid), (inbox, Some(7)));
        assert!(s.ops::<Op>(&acc).unwrap().is_empty());
        assert!(undo(&s, &tok.token).unwrap().is_empty(), "a token works once");
    }

    #[test]
    fn delete_twice_goes_for_good_from_the_original_place() {
        let (_d, s, acc, inbox, _trash, id) = setup();
        delete_messages(&s, &[id.to_string()]).unwrap();
        let (tok, _) = delete_messages(&s, &[id.to_string()]).unwrap();
        let ops = s.ops::<Op>(&acc).unwrap();
        assert_eq!(ops.len(), 1, "the first move is replaced, not kept");
        assert!(matches!(ops[0].op, Op::Move { from_folder, uid: 7, to_folder: None, .. } if from_folder == inbox));
        assert!(s.messages_in_folder(inbox, 0, 10).unwrap().is_empty());
        undo(&s, &tok.token).unwrap();
        let ops = s.ops::<Op>(&acc).unwrap();
        assert!(matches!(ops[0].op, Op::Move { to_folder: Some(_), .. }), "undo puts the first move back");
    }

    #[test]
    fn move_back_cancels_the_server_move() {
        let (_d, s, acc, inbox, trash, id) = setup();
        move_messages(&s, &[id.to_string()], &trash.to_string()).unwrap();
        move_messages(&s, &[id.to_string()], &inbox.to_string()).unwrap();
        assert!(s.ops::<Op>(&acc).unwrap().is_empty());
        assert_eq!(s.message(id).unwrap().uid, Some(7));
    }

    #[test]
    fn read_and_tags_queue_server_changes() {
        let (_d, s, acc, _inbox, _trash, id) = setup();
        set_read(&s, &[id.to_string()], true).unwrap();
        assert!(!s.message(id).unwrap().summary.unread);
        let tag = s.create_tag("Familie", "#0a7").unwrap();
        set_message_tag(&s, &[id.to_string()], &tag.id, true).unwrap();
        let ops = s.ops::<Op>(&acc).unwrap();
        assert_eq!(ops.len(), 2);
        assert!(matches!(&ops[1].op, Op::Keyword { keyword, add: true, .. } if keyword.starts_with("$vem_")));
        assert!(set_read(&s, &[id.to_string()], true).unwrap().is_empty(), "no change, no op");
    }

    #[test]
    fn local_delete_for_good_waits_for_the_undo_window() {
        let (_d, s) = store();
        let a = s.local_account("Oude mail").unwrap();
        let f = s.ensure_local_folder(&a.id, &[], Some(FolderRole::Inbox)).unwrap();
        let raw = b"Subject: oud\r\n\r\nx\r\n".to_vec();
        let id = s
            .insert_message(&NewMessage { account_id: a.id.clone(), folder_id: f, raw, has_body: true, ..Default::default() })
            .unwrap();
        delete_messages(&s, &[id.to_string()]).unwrap();
        assert!(s.messages_in_folder(f, 0, 10).unwrap().is_empty());
        assert_eq!(purge_due(&s).unwrap(), 0);
        assert!(s.message(id).is_ok(), "still there until the window passes");
    }

    #[test]
    fn detail_warns_about_lookalike_and_money() {
        let (_d, s, acc, inbox, _trash, _id) = setup();
        s.save_contact(&crate::model::Contact {
            id: String::new(),
            name: "Piet de Vries".into(),
            emails: vec!["piet@ziggo.nl".into()],
            phone: None,
            note: None,
        })
        .unwrap();
        let raw = b"From: Piet de Vries <piet.devries@mail.example>\r\nSubject: Dringend\r\nContent-Type: text/html\r\n\r\n<p>Kun je 450 euro overmaken? <a href=\"https://evil.example\">www.ing.nl</a> <img src=\"https://t.example/p.gif\"></p>\r\n".to_vec();
        let id = s
            .insert_message(&NewMessage { account_id: acc, folder_id: inbox, uid: Some(9), raw, has_body: true, ..Default::default() })
            .unwrap();
        let d = message_detail(&s, &id.to_string(), false).unwrap();
        let kinds: Vec<_> = d.warnings.iter().map(|w| w.kind).collect();
        assert!(kinds.contains(&WarningKind::LinkMismatch));
        assert!(kinds.contains(&WarningKind::LookalikeContact));
        assert!(kinds.contains(&WarningKind::FirstContactMoney));
        assert!(d.has_remote_images && !d.remote_images_shown);
        assert!(!d.html.unwrap().contains("t.example"));
        let d = message_detail(&s, &id.to_string(), true).unwrap();
        assert!(d.remote_images_shown && d.html.unwrap().contains("t.example"));
        assert!(message_detail(&s, &id.to_string(), false).unwrap().remote_images_shown, "the choice is remembered");
    }

    #[test]
    fn forwarding_takes_the_attachments_along_as_files() {
        let (d, s, acc, inbox, _, plain) = setup();
        let files = tempfile::tempdir().unwrap();
        let a = files.path().join("Recept crème.pdf");
        let b = files.path().join("sub").join("Recept crème.pdf");
        std::fs::create_dir_all(b.parent().unwrap()).unwrap();
        std::fs::write(&a, b"eerste").unwrap();
        std::fs::write(&b, b"tweede").unwrap();
        let draft = crate::model::Draft {
            account_id: acc.clone(),
            to: vec![crate::model::Addr::new(None, "jan@x.nl")],
            subject: "Recepten".into(),
            body_text: "Zie bijlagen.".into(),
            attachments: vec![a.to_string_lossy().into(), b.to_string_lossy().into()],
            shrink_photos: false,
            ..Default::default()
        };
        let raw = crate::compose::build(&draft, &crate::model::Addr::new(Some("Piet"), "piet@y.nl"), now()).unwrap().raw;
        let id = s
            .insert_message(&NewMessage { account_id: acc, folder_id: inbox, uid: Some(8), raw: raw.clone(), has_body: true, size: raw.len() as u64, ..Default::default() })
            .unwrap();

        let out = d.path().join("doorsturen");
        let paths = forward_attachments(&s, &id.to_string(), &out).unwrap();
        let names: Vec<String> = paths.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["Recept crème.pdf", "2 Recept crème.pdf"]);
        assert_eq!(std::fs::read(&paths[0]).unwrap(), b"eerste");
        assert_eq!(std::fs::read(&paths[1]).unwrap(), b"tweede");
        assert!(forward_attachments(&s, &plain.to_string(), &out).unwrap().is_empty(), "nothing to take along");
    }
}
