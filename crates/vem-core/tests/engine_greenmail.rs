//! The engine end to end against local GreenMail (never a real mailbox): account setup, the send
//! queue with its undo delay, IDLE noticing new mail, and the sent copy.
//!     cargo test -p vem-core --features test-support --test engine_greenmail
#![cfg(feature = "test-support")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use vem_core::engine::{Engine, Sink};
use vem_core::model::{Addr, Draft, FolderRole, MessageSummary, OutboxItem, ServerSettings, SignIn};
use vem_core::net::{Security, Trust};
use vem_core::report::AppFacts;
use vem_core::secrets::{Secret, Secrets};
use vem_core::store::Store;
use vem_core::VemError;

#[derive(Default)]
struct TestSink {
    new_mail: Mutex<Vec<MessageSummary>>,
    errors: Mutex<Vec<VemError>>,
}

impl Sink for TestSink {
    fn mail_changed(&self, _: &str, _: Option<String>) {}
    fn sync_state(&self, _: &str, _: bool, error: Option<VemError>) {
        if let Some(e) = error {
            self.errors.lock().unwrap().push(e);
        }
    }
    fn outbox_changed(&self, _: Vec<OutboxItem>) {}
    fn new_mail(&self, _: &str, messages: Vec<MessageSummary>) {
        self.new_mail.lock().unwrap().extend(messages);
    }
}

fn server_up() -> bool {
    let up = std::net::TcpStream::connect_timeout(&"127.0.0.1:3993".parse().unwrap(), Duration::from_secs(1)).is_ok();
    if !up {
        eprintln!("GreenMail is not running; skipping");
    }
    up
}

fn settings(user: &str) -> ServerSettings {
    ServerSettings {
        imap_host: "127.0.0.1".into(),
        imap_port: 3993,
        imap_tls: Security::Tls,
        smtp_host: "127.0.0.1".into(),
        smtp_port: 3465,
        smtp_tls: Security::Tls,
        username: user.into(),
        source: "test".into(),
        needs_confirmation: false,
        sign_in: SignIn::Password,
    }
}

fn engine(dir: &std::path::Path) -> (Arc<Engine>, Arc<TestSink>) {
    let store = Arc::new(Store::open(dir).unwrap());
    let mut s = store.settings().unwrap();
    s.undo_send_seconds = 2;
    store.save_settings(&s).unwrap();
    let sink = Arc::new(TestSink::default());
    let e = Engine::new(
        store,
        Arc::new(Secrets::new(dir)),
        sink.clone(),
        Trust::AcceptAnyLocalTestServer,
        AppFacts { version: "test".into(), os: "test".into() },
    );
    (e, sink)
}

async fn until<F: Fn() -> bool>(secs: u64, f: F) -> bool {
    for _ in 0..secs * 4 {
        if f() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    f()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wrong_password_saves_nothing() {
    if !server_up() {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let (e, _) = engine(d.path());
    let err = e
        .add_account("Jan", "jan@example.test", Secret::Password { password: "fout".into() }, settings("jan@example.test"))
        .await
        .unwrap_err();
    assert_eq!(err.code, "VEM-IMAP-LOGIN-01");
    assert!(e.store.accounts().unwrap().is_empty());
    assert!(!d.path().join("secrets").exists() || std::fs::read_dir(d.path().join("secrets")).unwrap().count() == 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn send_with_undo_delay_arrives_by_idle_and_keeps_a_sent_copy() {
    if !server_up() {
        return;
    }
    let dj = tempfile::tempdir().unwrap();
    let dp = tempfile::tempdir().unwrap();
    // GreenMail starts with Postvak IN only; real providers have Verzonden items already.
    let mut raw = vem_core::imap::Imap::connect(&vem_core::imap::ImapConfig {
        host: "127.0.0.1".into(),
        port: 3993,
        security: Security::Tls,
        username: "jan@example.test".into(),
        auth: vem_core::imap::Auth::Password("geheim".into()),
        trust: Trust::AcceptAnyLocalTestServer,
    })
    .await
    .unwrap();
    let _ = raw.create_folder("Sent").await;
    raw.logout().await;
    let (jan, _) = engine(dj.path());
    let (piet, piet_sink) = engine(dp.path());
    jan.start().unwrap();
    piet.start().unwrap();
    let ja = jan
        .add_account("Jan", "jan@example.test", Secret::Password { password: "geheim".into() }, settings("jan@example.test"))
        .await
        .unwrap();
    let pa = piet
        .add_account("Piet", "piet@example.test", Secret::Password { password: "geheim".into() }, settings("piet@example.test"))
        .await
        .unwrap();
    assert!(dj.path().join("secrets").join(format!("{}.bin", ja.id)).exists());

    // Wait until both mailboxes are synced once (Postvak IN known) and Piet idles.
    assert!(until(20, || piet.store.folder_by_role(&pa.id, FolderRole::Inbox).unwrap().is_some()).await);
    assert!(until(20, || jan.store.folder_by_role(&ja.id, FolderRole::Sent).unwrap().is_some()).await, "Sent folder");
    tokio::time::sleep(Duration::from_secs(1)).await;

    let subject = format!("Engine {}", uuid::Uuid::new_v4().simple());
    let draft = Draft {
        account_id: ja.id.clone(),
        to: vec![Addr::new(Some("Piet"), "piet@example.test")],
        subject: subject.clone(),
        body_text: "Komt u zondag eten?".into(),
        ..Default::default()
    };

    // Undo within the delay: nothing goes out.
    let item = jan.send(draft.clone()).unwrap();
    let back = jan.cancel_send(&item.id).unwrap();
    assert_eq!(back.subject, subject);
    assert!(jan.store.outbox().unwrap().is_empty());

    // Now for real.
    jan.send(draft).unwrap();
    assert_eq!(jan.store.outbox().unwrap().len(), 1);
    let arrived = until(30, || piet_sink.new_mail.lock().unwrap().iter().any(|m| m.subject == subject)).await;
    assert!(arrived, "Piet's IDLE loop reports the new message; errors: {:?}", piet_sink.errors.lock().unwrap());
    assert!(until(10, || jan.store.outbox().unwrap().is_empty()).await, "outbox empties after sending");

    // Jan's copy in Verzonden items, appended to the server and adopted (one row, with a uid).
    let sent = jan.store.folder_by_role(&ja.id, FolderRole::Sent).unwrap().unwrap();
    let ok = until(30, || {
        let rows: Vec<_> =
            jan.store.messages_in_folder(sent, 0, 200).unwrap().into_iter().filter(|m| m.subject == subject).collect();
        rows.len() == 1 && jan.store.message(rows[0].id.parse().unwrap()).unwrap().uid.is_some()
    })
    .await;
    assert!(ok, "one sent copy, adopted from the server");
    assert!(jan.store.contact_by_email("piet@example.test").unwrap().is_some(), "recipient learned");
}

/// "Uit een ander postvak": Jan's old mailbox is copied into Piet's, with read state and star.
/// Only GreenMail's test accounts are written to.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mailbox_import_copies_mail_with_state_into_another_mailbox() {
    if !server_up() {
        return;
    }
    let run = uuid::Uuid::new_v4().simple().to_string();
    let config = |user: &str| vem_core::imap::ImapConfig {
        host: "127.0.0.1".into(),
        port: 3993,
        security: Security::Tls,
        username: user.into(),
        auth: vem_core::imap::Auth::Password("geheim".into()),
        trust: Trust::AcceptAnyLocalTestServer,
    };
    let mut raw = vem_core::imap::Imap::connect(&config("jan@example.test")).await.unwrap();
    for (n, seen, flagged) in [(1, true, true), (2, false, false)] {
        let msg = format!("From: Oma <oma@example.test>\r\nSubject: Oud {run} {n}\r\nMessage-ID: <oud{n}.{run}@x>\r\nDate: Tue, 3 Feb 2004 10:00:00 +0100\r\n\r\nOude mail {n}\r\n");
        raw.append("INBOX", seen, flagged, msg.as_bytes()).await.unwrap();
    }
    raw.logout().await;

    let d = tempfile::tempdir().unwrap();
    let (e, sink) = engine(d.path());
    e.start().unwrap();
    let pw = || Secret::Password { password: "geheim".into() };
    let jan = e.add_account("Jan", "jan@example.test", pw(), settings("jan@example.test")).await.unwrap();
    let piet = e.add_account("Piet", "piet@example.test", pw(), settings("piet@example.test")).await.unwrap();
    let synced = until(30, || {
        let Some(inbox) = e.store.folder_by_role(&jan.id, FolderRole::Inbox).unwrap() else { return false };
        e.store.messages_in_folder(inbox, 0, 500).unwrap().iter().filter(|m| m.subject.contains(&run)).count() == 2
    })
    .await;
    assert!(synced, "Jan's two old messages are synced; errors: {:?}", sink.errors.lock().unwrap());

    let cancel = std::sync::atomic::AtomicBool::new(false);
    e.download_bodies(&jan.id, &cancel).await.unwrap();
    let target = vem_core::model::ImportTarget::Mailbox { account_id: piet.id.clone() };
    let report = vem_core::import::account::run(&e.store, "job", &jan.id, &target, &|_| {}, &cancel).unwrap();
    assert!(report.added >= 2, "at least this run's two messages: {report:?}");
    e.wake(Some(&piet.id));

    // Piet's mailbox now has them under "Oude mail", with Jan's read state and star.
    let mut found = Vec::new();
    for _ in 0..60 {
        let mut p = vem_core::imap::Imap::connect(&config("piet@example.test")).await.unwrap();
        let folders = p.list_folders().await.unwrap();
        if let Some(f) = folders.iter().find(|f| f.name.starts_with("Oude mail") && f.name.ends_with("INBOX")) {
            p.open(&f.name, true).await.unwrap();
            found = p.fetch("1:*", false).await.unwrap_or_default().into_iter().filter(|m| String::from_utf8_lossy(&m.raw).contains(&run)).collect();
        }
        p.logout().await;
        if found.len() == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert_eq!(found.len(), 2, "both copied into Piet's mailbox; errors: {:?}", sink.errors.lock().unwrap());
    let first = found.iter().find(|m| String::from_utf8_lossy(&m.raw).contains(&format!("Oud {run} 1"))).unwrap();
    let second = found.iter().find(|m| String::from_utf8_lossy(&m.raw).contains(&format!("Oud {run} 2"))).unwrap();
    assert!(first.seen && first.flagged, "read and starred stays so");
    assert!(!second.seen && !second.flagged, "unread stays unread");
    assert!(!String::from_utf8_lossy(&first.raw).contains("X-VEM-Import-Status"));
}
