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

    // The message file of that copy is removed behind the app's back (a disk cleaner): opening
    // it downloads it again from the server instead of failing every time.
    let row = jan.store.messages_in_folder(sent, 0, 200).unwrap().into_iter().find(|m| m.subject == subject).unwrap();
    let id: i64 = row.id.parse().unwrap();
    let rel = jan.store.message(id).unwrap().eml_path.expect("the copy has a file");
    std::fs::remove_file(jan.store.mail_dir().join(rel)).unwrap();
    jan.ensure_body(id).await.expect("fetched again");
    let raw = jan.store.read_eml(id).unwrap().expect("a body again");
    assert!(String::from_utf8_lossy(&raw).contains("Komt u zondag eten?"));
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

/// A new password is tried before it is saved: a wrong one changes nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn new_password_is_checked_before_it_is_saved() {
    if !server_up() {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let (e, _) = engine(d.path());
    let pw = |p: &str| Secret::Password { password: p.into() };
    let a = e.add_account("Jan", "jan@example.test", pw("geheim"), settings("jan@example.test")).await.unwrap();
    let err = e.update_secret(&a.id, pw("fout")).await.unwrap_err();
    assert_eq!(err.code, "VEM-IMAP-LOGIN-01");
    assert!(matches!(e.secrets.get(&a.id).unwrap(), Some(Secret::Password { password }) if password == "geheim"));
    e.update_secret(&a.id, pw("geheim")).await.unwrap();
}

/// A new password that works clears the mailbox's old error at once: the screens reload right
/// after saving, before the restarted mailbox has finished its first round.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_working_new_password_clears_the_old_error() {
    if !server_up() {
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let (e, _) = engine(d.path());
    let pw = |p: &str| Secret::Password { password: p.into() };
    let a = e.add_account("Jan", "jan@example.test", pw("geheim"), settings("jan@example.test")).await.unwrap();
    let old = vem_core::VemError::new(vem_core::Code::StoreSecret01).with_detail("unprotect");
    e.store.set_last_error(&a.id, Some(&old)).unwrap();
    e.update_secret(&a.id, pw("geheim")).await.unwrap();
    assert!(e.store.account(&a.id).unwrap().last_error.is_none());
}

/// Decision 24: on Android the app pauses its engine when the background service takes over, and
/// starts it again when it comes back. A paused engine does no work; a restarted one picks up
/// the mail that arrived meanwhile.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_paused_engine_does_nothing_and_catches_up_after_start() {
    if !server_up() {
        return;
    }
    let dj = tempfile::tempdir().unwrap();
    let dp = tempfile::tempdir().unwrap();
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
    assert!(until(20, || piet.store.folder_by_role(&pa.id, FolderRole::Inbox).unwrap().is_some()).await);

    piet.pause().await;
    let subject = format!("Pauze {}", uuid::Uuid::new_v4().simple());
    let draft = Draft {
        account_id: ja.id.clone(),
        to: vec![Addr::new(Some("Piet"), "piet@example.test")],
        subject: subject.clone(),
        body_text: "Terwijl de telefoon sliep.".into(),
        ..Default::default()
    };
    jan.send(draft).unwrap();
    assert!(until(20, || jan.store.outbox().unwrap().is_empty()).await, "Jan's mail went out");

    // Piet is paused: no IDLE, no fetch, nothing new in the index.
    let inbox = piet.store.folder_by_role(&pa.id, FolderRole::Inbox).unwrap().unwrap();
    let seen = |e: &Engine| e.store.messages_in_folder(inbox, 0, 200).unwrap().iter().any(|m| m.subject == subject);
    tokio::time::sleep(Duration::from_secs(4)).await;
    assert!(!seen(&piet), "a paused engine fetches nothing");

    // A paused engine does not send either: the send queue waits for the next start.
    let reply = Draft {
        account_id: pa.id.clone(),
        to: vec![Addr::new(Some("Jan"), "jan@example.test")],
        subject: format!("Re: {subject}"),
        body_text: "Goed.".into(),
        ..Default::default()
    };
    piet.send(reply).unwrap();
    tokio::time::sleep(Duration::from_secs(4)).await;
    assert_eq!(piet.store.outbox().unwrap().len(), 1, "nothing sent while paused");

    piet.start().unwrap();
    assert!(until(30, || seen(&piet)).await, "the restarted engine fetches what came in meanwhile");
    assert!(until(20, || piet.store.outbox().unwrap().is_empty()).await, "and sends what waited");
    assert!(piet_sink.errors.lock().unwrap().is_empty(), "no errors: {:?}", piet_sink.errors.lock().unwrap());
}

/// Every file below `dir`, recursively.
fn files_below(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(files_below(&path));
        } else {
            out.push(path);
        }
    }
    out
}

/// After a restart with the network off the app still opens, lists, reads and searches the mail it
/// had; the password survives the restart; and no file of the app (index, log, message files,
/// settings) holds the password in plain text, in UTF-8 or UTF-16.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn after_a_restart_mail_reads_offline_and_no_file_holds_the_password() {
    if !server_up() {
        return;
    }
    let run = uuid::Uuid::new_v4().simple().to_string();
    let subject = format!("Offline {run}");
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
    let msg = format!("From: Oma <oma@example.test>\r\nSubject: {subject}\r\nMessage-ID: <{run}@x>\r\nDate: Tue, 17 Nov 2026 10:00:00 +0100\r\n\r\nLees mij zonder net {run}\r\n");
    raw.append("INBOX", false, false, msg.as_bytes()).await.unwrap();
    raw.logout().await;

    let d = tempfile::tempdir().unwrap();
    vem_core::log::init(d.path());
    let (e, sink) = engine(d.path());
    e.start().unwrap();
    let jan = e
        .add_account("Jan", "jan@example.test", Secret::Password { password: "geheim".into() }, settings("jan@example.test"))
        .await
        .unwrap();
    let find = |store: &Store| -> Option<i64> {
        let inbox = store.folder_by_role(&jan.id, FolderRole::Inbox).ok()??;
        store.messages_in_folder(inbox, 0, 1000).ok()?.into_iter().find(|m| m.subject == subject).map(|m| m.id.parse().unwrap())
    };
    assert!(until(30, || find(&e.store).is_some()).await, "synced; errors: {:?}", sink.errors.lock().unwrap());
    let id = find(&e.store).unwrap();
    e.ensure_body(id).await.unwrap();
    e.pause().await;
    drop(e);

    // The restart, with the mail server out of reach.
    let store = Store::open(d.path()).unwrap();
    let mut off = store.account(&jan.id).unwrap().settings.unwrap();
    off.imap_port = 1;
    off.smtp_port = 1;
    store.set_account_settings(&jan.id, &off).unwrap();
    drop(store);
    let (e, _) = engine(d.path());
    e.start().unwrap();
    assert_eq!(find(&e.store), Some(id), "the list is there");
    e.ensure_body(id).await.expect("opening a message needs no server");
    let body = e.store.read_eml(id).unwrap().expect("its text is on disk");
    assert!(String::from_utf8_lossy(&body).contains(&format!("Lees mij zonder net {run}")));
    let hits = e.store.search(&run, &vem_core::model::SearchScope::All, 10).unwrap();
    assert_eq!(hits.len(), 1, "search works offline");
    match Secrets::new(d.path()).get(&jan.id).unwrap() {
        Some(Secret::Password { password }) => assert_eq!(password, "geheim", "the password survives a restart"),
        other => panic!("no password after a restart: {}", other.is_some()),
    }
    e.pause().await;

    let utf16: Vec<u8> = "geheim".encode_utf16().flat_map(u16::to_le_bytes).collect();
    let files = files_below(d.path());
    assert!(files.iter().any(|f| f.extension().is_some_and(|x| x == "log")), "a log was written");
    for f in files {
        let bytes = std::fs::read(&f).unwrap_or_default();
        let holds = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
        assert!(!holds(b"geheim") && !holds(&utf16), "the password in plain text in {}", f.display());
    }
}

/// Piet's messages whose subject contains `marker`, raw, read straight from GreenMail.
async fn piet_inbox(marker: &str) -> Vec<Vec<u8>> {
    let mut imap = vem_core::imap::Imap::connect(&vem_core::imap::ImapConfig {
        host: "127.0.0.1".into(),
        port: 3993,
        security: Security::Tls,
        username: "piet@example.test".into(),
        auth: vem_core::imap::Auth::Password("geheim".into()),
        trust: Trust::AcceptAnyLocalTestServer,
    })
    .await
    .unwrap();
    imap.open("INBOX", true).await.unwrap();
    let found = imap
        .fetch("1:*", true)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|m| m.raw)
        .filter(|raw| vem_core::parse::parse(raw).subject.contains(marker))
        .collect();
    imap.logout().await;
    found
}

/// A message with an attachment whose name has accents arrives with that name and the same bytes;
/// a reply to it carries In-Reply-To and References, so other programs thread it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn attachments_with_accents_and_replies_arrive_intact() {
    if !server_up() {
        return;
    }
    let run = uuid::Uuid::new_v4().simple().to_string();
    let d = tempfile::tempdir().unwrap();
    let (e, sink) = engine(d.path());
    e.start().unwrap();
    let jan = e
        .add_account("Jan", "jan@example.test", Secret::Password { password: "geheim".into() }, settings("jan@example.test"))
        .await
        .unwrap();
    let files = tempfile::tempdir().unwrap();
    let pdf = files.path().join("Recept crème brûlée — één.pdf");
    let bytes: Vec<u8> = (0..70_000u32).map(|i| (i * 7 % 251) as u8).collect();
    std::fs::write(&pdf, &bytes).unwrap();

    let first = Draft {
        account_id: jan.id.clone(),
        to: vec![Addr::new(Some("Piet"), "piet@example.test")],
        subject: format!("Recept {run}"),
        body_text: "Zie bijlage.".into(),
        attachments: vec![pdf.to_string_lossy().into_owned()],
        ..Default::default()
    };
    e.send(first).unwrap();
    let mut got = Vec::new();
    for _ in 0..60 {
        got = piet_inbox(&format!("Recept {run}")).await;
        if !got.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert_eq!(got.len(), 1, "arrived once; errors: {:?}", sink.errors.lock().unwrap());
    let parsed = vem_core::parse::parse(&got[0]);
    assert_eq!(parsed.attachments.len(), 1);
    assert_eq!(parsed.attachments[0].name, "Recept crème brûlée — één.pdf");
    assert_eq!(vem_core::parse::attachment_bytes(&got[0], 0).unwrap(), bytes, "the same bytes");
    let first_id = parsed.message_id.clone().expect("a Message-ID");

    let reply = Draft {
        account_id: jan.id.clone(),
        to: vec![Addr::new(Some("Piet"), "piet@example.test")],
        subject: format!("Re: Recept {run}"),
        body_text: "Nog een vraag.".into(),
        in_reply_to: Some(first_id.clone()),
        references: vec![first_id.clone()],
        ..Default::default()
    };
    e.send(reply).unwrap();
    let mut replies = Vec::new();
    for _ in 0..60 {
        replies = piet_inbox(&format!("Re: Recept {run}")).await;
        if !replies.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    assert_eq!(replies.len(), 1);
    let parsed = vem_core::parse::parse(&replies[0]);
    assert_eq!(parsed.in_reply_to.as_deref(), Some(first_id.as_str()));
    assert_eq!(parsed.references, vec![first_id]);
    e.pause().await;
}

/// A message written while the connection is down waits in Postvak UIT with the error shown, and
/// goes once, and only once, as soon as the connection is back (not at its later retry time).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_message_written_offline_is_sent_once_when_the_connection_returns() {
    if !server_up() {
        return;
    }
    let run = uuid::Uuid::new_v4().simple().to_string();
    let d = tempfile::tempdir().unwrap();
    let (e, _) = engine(d.path());
    e.start().unwrap();
    let jan = e
        .add_account("Jan", "jan@example.test", Secret::Password { password: "geheim".into() }, settings("jan@example.test"))
        .await
        .unwrap();
    assert!(until(20, || e.store.folder_by_role(&jan.id, FolderRole::Inbox).unwrap().is_some()).await);

    // The network goes away.
    e.stop_account(&jan.id);
    let mut off = settings("jan@example.test");
    off.imap_port = 1;
    off.smtp_port = 1;
    e.store.set_account_settings(&jan.id, &off).unwrap();
    e.start_account(&jan.id);
    assert!(until(10, || e.store.account(&jan.id).unwrap().last_error.is_some()).await, "the mailbox notices");

    let subject = format!("Offline geschreven {run}");
    e.send(Draft {
        account_id: jan.id.clone(),
        to: vec![Addr::new(Some("Piet"), "piet@example.test")],
        subject: subject.clone(),
        body_text: "Geschreven in de trein.".into(),
        ..Default::default()
    })
    .unwrap();
    let waiting = until(15, || {
        e.store.outbox().unwrap().iter().any(|i| {
            i.subject == subject
                && i.state == vem_core::model::OutboxState::Waiting
                && i.error.as_ref().is_some_and(|x| x.code == "VEM-SMTP-CONNECT-01")
        })
    })
    .await;
    assert!(waiting, "waits in Postvak UIT with the error: {:?}", e.store.outbox().unwrap());

    // The connection returns; the mailbox loop notices and Postvak UIT empties at once.
    e.store.set_account_settings(&jan.id, &settings("jan@example.test")).unwrap();
    e.wake(Some(&jan.id));
    assert!(until(30, || e.store.outbox().unwrap().is_empty()).await, "sent well before the five-minute retry");
    let mut got = Vec::new();
    for _ in 0..40 {
        got = piet_inbox(&subject).await;
        if !got.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert_eq!(piet_inbox(&subject).await.len(), 1, "arrived once, and only once");
    assert_eq!(got.len(), 1);
    e.pause().await;
}
