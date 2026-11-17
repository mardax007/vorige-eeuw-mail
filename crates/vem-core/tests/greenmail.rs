//! End-to-end tests against a local GreenMail server (never a real mailbox: CLAUDE.md, Mail safety).
//!
//! Start GreenMail first (see docs/PROGRESS.md, "Test server"), then run:
//!     cargo test -p vem-core --features test-support --test greenmail
//! Without a server on 127.0.0.1:3993 every test prints a note and passes.
#![cfg(feature = "test-support")]

use std::time::Duration;

use vem_core::imap::{Auth, Imap, ImapConfig, ops::Op, sync};
use vem_core::model::{AccountKind, FolderRole, SearchScope};
use vem_core::net::{Security, Trust};
use vem_core::smtp::{self, SmtpConfig};
use vem_core::store::{NewMessage, Store};

const HOST: &str = "127.0.0.1";
const IMAPS: u16 = 3993;
const SMTPS: u16 = 3465;
const PASSWORD: &str = "geheim";

fn server_up() -> bool {
    let addr: std::net::SocketAddr = format!("{HOST}:{IMAPS}").parse().unwrap();
    let up = std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(1)).is_ok();
    if !up {
        eprintln!("GreenMail is not running on {HOST}:{IMAPS}; skipping");
    }
    up
}

fn imap_cfg(user: &str, password: &str) -> ImapConfig {
    ImapConfig {
        host: HOST.into(),
        port: IMAPS,
        security: Security::Tls,
        username: user.into(),
        auth: Auth::Password(password.into()),
        trust: Trust::AcceptAnyLocalTestServer,
    }
}

fn smtp_cfg(user: &str) -> SmtpConfig {
    SmtpConfig {
        host: HOST.into(),
        port: SMTPS,
        security: Security::Tls,
        username: user.into(),
        auth: Auth::Password(PASSWORD.into()),
        trust: Trust::AcceptAnyLocalTestServer,
    }
}

fn unique(tag: &str) -> String {
    format!("{tag}{}", &uuid::Uuid::new_v4().simple().to_string()[..10])
}

fn message(from: &str, to: &str, subject: &str, body: &str) -> Vec<u8> {
    format!(
        "From: Jan <{from}>\r\nTo: <{to}>\r\nSubject: {subject}\r\nMessage-ID: <{}@test>\r\nDate: Mon, 16 Nov 2026 10:00:00 +0100\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}\r\n",
        uuid::Uuid::new_v4()
    )
    .into_bytes()
}

#[tokio::test]
async fn wrong_password_gives_login_code_and_exact_reply() {
    if !server_up() {
        return;
    }
    let e = match Imap::connect(&imap_cfg("jan@example.test", "fout")).await {
        Ok(_) => panic!("login with a wrong password must fail"),
        Err(e) => e,
    };
    assert_eq!(e.code, "VEM-IMAP-LOGIN-01", "{e:?}");
    let reply = e.server_reply.expect("server reply kept");
    assert!(reply.contains("NO"), "reply: {reply}");
    assert!(!reply.contains("fout"), "password must not appear in the error");
}

#[tokio::test]
async fn smtp_wrong_password_is_refused() {
    if !server_up() {
        return;
    }
    let mut cfg = smtp_cfg("jan@example.test");
    cfg.auth = Auth::Password("fout".into());
    let e = smtp::check_login(&cfg).await.unwrap_err();
    assert_eq!(e.code, "VEM-SMTP-LOGIN-01", "{e:?}");
    assert!(e.server_reply.is_some_and(|r| r.starts_with("535")));
}

#[tokio::test]
async fn send_sync_search_flag_move_delete() {
    if !server_up() {
        return;
    }
    let subject = unique("Verjaardag");
    // A word unique to this run: Piet's test mailbox keeps the mail of every earlier run.
    let word = unique("taart");
    let raw = message("jan@example.test", "piet@example.test", &subject, &format!("Tot zaterdag bij oma, met {word}. Groet, Jan"));
    smtp::send(&smtp_cfg("jan@example.test"), "jan@example.test", &["piet@example.test".into()], &raw)
        .await
        .expect("send");

    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let acc = store.add_account("Piet", "piet@example.test", AccountKind::Imap, None).unwrap();
    let mut imap = Imap::connect(&imap_cfg("piet@example.test", PASSWORD)).await.expect("connect");

    let mut found = None;
    for _ in 0..20 {
        sync::sync_account(&store, &mut imap, &acc.id, false).await.expect("sync");
        let inbox = store.folder_by_role(&acc.id, FolderRole::Inbox).unwrap().expect("inbox");
        found = store.messages_in_folder(inbox, 0, 500).unwrap().into_iter().find(|m| m.subject == subject);
        if found.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let summary = found.expect("sent message arrives in Postvak IN");
    assert!(summary.unread);
    assert_eq!(summary.from.email, "jan@example.test");

    // Bodies come in a second pass, newest first.
    sync::fetch_bodies(&store, &mut imap, &acc.id, 500).await.unwrap();

    let hits = store.search(&word, &SearchScope::All, 50).unwrap();
    assert!(hits.iter().any(|h| h.message.id == summary.id), "search finds body text");
    let hits = store.search(&subject[..6], &SearchScope::All, 50).unwrap();
    assert!(hits.iter().any(|h| h.message.id == summary.id), "search finds subject prefix");

    let id: i64 = summary.id.parse().unwrap();
    store.set_flags(id, Some(false), Some(true)).unwrap();
    store.push_op(&acc.id, &Op::Flags { message_id: id, seen: Some(true), flagged: Some(true) }).unwrap();
    sync::sync_account(&store, &mut imap, &acc.id, false).await.expect("sync after flag");
    let uid = store.message(id).unwrap().uid.expect("uid known");
    imap.open("INBOX", true).await.unwrap();
    let flags = imap.fetch_flags(&uid.to_string()).await.unwrap();
    assert!(flags[0].seen && flags[0].flagged, "server has \\Seen and \\Flagged");

    let familie_name = unique("Familie");
    imap.create_folder(&familie_name).await.unwrap();
    assert!(sync::sync_folders(&store, &mut imap, &acc.id).await.unwrap(), "a new folder changes the list");
    assert!(!sync::sync_folders(&store, &mut imap, &acc.id).await.unwrap(), "nothing new, no change");
    let familie =
        store.folder_infos(&acc.id).unwrap().into_iter().find(|f| f.name == familie_name).expect("new folder listed").id;
    let inbox = store.folder_by_role(&acc.id, FolderRole::Inbox).unwrap().unwrap();
    store.move_local(id, familie).unwrap();
    store
        .push_op(&acc.id, &Op::Move { message_id: id, from_folder: inbox, uid, to_folder: Some(familie), after: 0 })
        .unwrap();
    sync::sync_account(&store, &mut imap, &acc.id, false).await.expect("sync after move");
    let in_familie: Vec<_> =
        store.messages_in_folder(familie, 0, 50).unwrap().into_iter().filter(|m| m.subject == subject).collect();
    assert_eq!(in_familie.len(), 1, "moved message shown once");
    assert!(store.messages_in_folder(inbox, 0, 500).unwrap().iter().all(|m| m.subject != subject));

    // Delete for good, only ever in this test-server folder.
    let moved_id: i64 = in_familie[0].id.parse().unwrap();
    let moved_uid = store.message(moved_id).unwrap().uid.expect("uid after move");
    store.set_hidden(moved_id, true).unwrap();
    store
        .push_op(
            &acc.id,
            &Op::Move { message_id: moved_id, from_folder: familie, uid: moved_uid, to_folder: None, after: 0 },
        )
        .unwrap();
    sync::sync_account(&store, &mut imap, &acc.id, false).await.expect("sync after delete");
    imap.open(&familie_name, true).await.unwrap();
    assert!(!imap.uids().await.unwrap().contains(&moved_uid), "gone on the server");
    assert!(store.messages_in_folder(familie, 0, 50).unwrap().iter().all(|m| m.subject != subject));
    imap.logout().await;
}

/// The app is killed while it syncs, several times at different moments; the index, opened again
/// as after a restart, ends with every message exactly once.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn killing_the_app_during_sync_loses_nothing_and_doubles_nothing() {
    if !server_up() {
        return;
    }
    const COUNT: usize = 150;
    let folder = unique("Afgebroken");
    let mut raw = Imap::connect(&imap_cfg("jan@example.test", PASSWORD)).await.expect("connect");
    raw.create_folder(&folder).await.unwrap();
    for i in 0..COUNT {
        let m = message("oma@example.test", "jan@example.test", &format!("{folder} brief {i}"), "tekst");
        raw.append(&folder, i % 2 == 0, false, &m).await.unwrap();
    }
    raw.logout().await;

    let dir = tempfile::tempdir().unwrap();
    let acc = Store::open(dir.path()).unwrap().add_account("Jan", "jan@example.test", AccountKind::Imap, None).unwrap();
    let mut cut_short = 0;
    for cut_ms in [5u64, 20, 60, 150, 400] {
        // A fresh "start of the app": the index opened again, a new connection, killed after cut_ms.
        let store = Store::open(dir.path()).unwrap();
        let mut imap = Imap::connect(&imap_cfg("jan@example.test", PASSWORD)).await.expect("connect");
        let run = tokio::time::timeout(Duration::from_millis(cut_ms), sync::sync_account(&store, &mut imap, &acc.id, true)).await;
        if run.is_err() {
            cut_short += 1;
        }
        drop(imap);
        drop(store);
    }
    let store = Store::open(dir.path()).unwrap();
    let mut imap = Imap::connect(&imap_cfg("jan@example.test", PASSWORD)).await.expect("connect");
    assert!(cut_short >= 2, "the sync was really cut off part-way ({cut_short} times)");
    sync::sync_account(&store, &mut imap, &acc.id, true).await.expect("a full sync at last");
    imap.logout().await;

    let id = store.folders(Some(&acc.id)).unwrap().into_iter().find(|f| f.name == folder).expect("folder synced").id;
    let rows = store.messages_in_folder(id.parse().unwrap(), 0, 1000).unwrap();
    let mut subjects: Vec<&str> = rows.iter().map(|m| m.subject.as_str()).collect();
    subjects.sort_unstable();
    subjects.dedup();
    assert_eq!(rows.len(), COUNT, "every message, once");
    assert_eq!(subjects.len(), COUNT, "no doubles");
    assert_eq!(rows.iter().filter(|m| m.unread).count(), COUNT / 2, "read state as on the server");
}

/// One full sync of Jan's test mailbox into `store`.
async fn sync_once(store: &Store, account_id: &str) {
    let mut imap = Imap::connect(&imap_cfg("jan@example.test", PASSWORD)).await.expect("connect");
    sync::sync_account(store, &mut imap, account_id, true).await.expect("sync");
    imap.logout().await;
}

/// A tag set here is stored on the server as an ordinary IMAP keyword, so other programs see it;
/// a tag set elsewhere (here: a second copy of the program on another device) shows here.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tags_go_to_the_server_and_come_back() {
    if !server_up() {
        return;
    }
    let folder = unique("Labels");
    let subject = unique("Gelabeld");
    let mut raw = Imap::connect(&imap_cfg("jan@example.test", PASSWORD)).await.expect("connect");
    raw.create_folder(&folder).await.unwrap();
    raw.append(&folder, false, false, &message("oma@example.test", "jan@example.test", &subject, "tekst")).await.unwrap();

    // Two devices, each with its own index.
    let (da, db) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (a, b) = (Store::open(da.path()).unwrap(), Store::open(db.path()).unwrap());
    let acc_a = a.add_account("Jan", "jan@example.test", AccountKind::Imap, None).unwrap();
    let acc_b = b.add_account("Jan", "jan@example.test", AccountKind::Imap, None).unwrap();
    let row = |store: &Store, acc: &str| -> vem_core::model::MessageSummary {
        let f = store.folders(Some(acc)).unwrap().into_iter().find(|f| f.name == folder).expect("folder");
        store.messages_in_folder(f.id.parse().unwrap(), 0, 10).unwrap().into_iter().find(|m| m.subject == subject).expect("message")
    };
    sync_once(&a, &acc_a.id).await;
    sync_once(&b, &acc_b.id).await;

    // Names unique per run: earlier runs on the same GreenMail left their own tags in the mailbox.
    let (fam, xmas) = (unique("Familie"), unique("Kerst"));

    // Set here: "Familie" on device A, then A syncs.
    let familie = a.create_tag(&fam, "").unwrap();
    vem_core::actions::set_message_tag(&a, &[row(&a, &acc_a.id).id], &familie.id, true).unwrap();
    sync_once(&a, &acc_a.id).await;
    raw.open(&folder, true).await.unwrap();
    let on_server = raw.fetch("1:*", false).await.unwrap();
    let keyword = a.tag_keyword(&familie.id).unwrap();
    assert!(on_server[0].keywords.contains(&keyword), "an IMAP keyword other programs can read: {:?}", on_server[0].keywords);

    // Seen elsewhere: device B gets "Familie" by name.
    sync_once(&b, &acc_b.id).await;
    let tags_b: Vec<String> = row(&b, &acc_b.id).tag_ids.iter().map(|t| b.tags().unwrap().into_iter().find(|x| &x.id == t).unwrap().name).collect();
    assert_eq!(tags_b, vec![fam.clone()]);

    // Set there: B adds "Kerst"; A shows it after its next sync.
    let kerst = b.create_tag(&xmas, "").unwrap();
    vem_core::actions::set_message_tag(&b, &[row(&b, &acc_b.id).id], &kerst.id, true).unwrap();
    sync_once(&b, &acc_b.id).await;
    sync_once(&a, &acc_a.id).await;
    let mut tags_a: Vec<String> = row(&a, &acc_a.id).tag_ids.iter().map(|t| a.tags().unwrap().into_iter().find(|x| &x.id == t).unwrap().name).collect();
    tags_a.sort();
    assert_eq!(tags_a, vec![fam, xmas]);
    raw.logout().await;
}

#[tokio::test]
async fn append_is_adopted_not_doubled() {
    if !server_up() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let acc = store.add_account("Jan", "jan@example.test", AccountKind::Imap, None).unwrap();
    let mut imap = Imap::connect(&imap_cfg("jan@example.test", PASSWORD)).await.expect("connect");
    sync::sync_account(&store, &mut imap, &acc.id, true).await.unwrap();
    let inbox = store.folder_by_role(&acc.id, FolderRole::Inbox).unwrap().unwrap();

    let subject = unique("Kopie");
    let raw = message("jan@example.test", "piet@example.test", &subject, "lokaal eerst");
    let id = store
        .insert_message(&NewMessage {
            account_id: acc.id.clone(),
            folder_id: inbox,
            uid: None,
            raw: raw.clone(),
            has_body: true,
            unread: false,
            important: false,
            size: raw.len() as u64,
            fallback_date: None,
        })
        .unwrap();
    store.push_op(&acc.id, &Op::Append { message_id: id, folder: inbox, seen: true, flagged: false }).unwrap();
    sync::sync_account(&store, &mut imap, &acc.id, false).await.unwrap();
    let same: Vec<_> =
        store.messages_in_folder(inbox, 0, 500).unwrap().into_iter().filter(|m| m.subject == subject).collect();
    assert_eq!(same.len(), 1, "appended message adopted by Message-ID");
    assert!(store.message(id).unwrap().uid.is_some(), "local row got the server uid");
    imap.logout().await;
}
