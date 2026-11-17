//! Error reports, wishes and log answers followed to an inbox on local GreenMail (phase 2b exit
//! criteria). Piet's test mailbox stands in for Martijn's address; Jan's is the user's. Its own
//! test binary, so the stand-in address never reaches the other tests.
//!     cargo test -p vem-core --features test-support --test reports_greenmail
#![cfg(feature = "test-support")]

use std::sync::Arc;
use std::time::Duration;

use vem_core::engine::{Engine, Sink};
use vem_core::error::Code;
use vem_core::imap::{Auth, Imap, ImapConfig};
use vem_core::model::{MessageSummary, OutboxItem, ServerSettings, SignIn};
use vem_core::net::{Security, Trust};
use vem_core::report::{self, AppFacts, LogRequests, SUBJECT_PREFIX};
use vem_core::secrets::{Secret, Secrets};
use vem_core::store::Store;
use vem_core::VemError;

struct NoSink;

impl Sink for NoSink {
    fn mail_changed(&self, _: &str, _: Option<String>) {}
    fn sync_state(&self, _: &str, _: bool, _: Option<VemError>) {}
    fn outbox_changed(&self, _: Vec<OutboxItem>) {}
    fn new_mail(&self, _: &str, _: Vec<MessageSummary>) {}
}

const MARTIJN: &str = "piet@example.test";
const PASSWORD: &str = "geheim";

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

/// Every message in Martijn's stand-in inbox whose text contains `marker`, as (subject, text).
async fn martijn_got(marker: &str) -> Vec<(String, String)> {
    let mut imap = Imap::connect(&ImapConfig {
        host: "127.0.0.1".into(),
        port: 3993,
        security: Security::Tls,
        username: MARTIJN.into(),
        auth: Auth::Password(PASSWORD.into()),
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
        .map(|m| vem_core::parse::parse(&m.raw))
        .map(|p| (p.subject, p.text.unwrap_or_default()))
        .filter(|(_, text)| text.contains(marker))
        .collect();
    imap.logout().await;
    found
}

async fn wait_for(marker: &str, count: usize) -> Vec<(String, String)> {
    for _ in 0..60 {
        let got = martijn_got(marker).await;
        if got.len() >= count {
            return got;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    martijn_got(marker).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reports_wishes_and_log_answers_reach_martijn_once_and_clean() {
    if !server_up() {
        return;
    }
    report::set_test_support_address(MARTIJN);
    let run = uuid::Uuid::new_v4().simple().to_string();
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(dir.path()).unwrap());
    let mut s = store.settings().unwrap();
    s.undo_send_seconds = 1;
    store.save_settings(&s).unwrap();
    let engine = Engine::new(
        store,
        Arc::new(Secrets::new(dir.path())),
        Arc::new(NoSink),
        Trust::AcceptAnyLocalTestServer,
        AppFacts { version: format!("test-{run}"), os: "test".into() },
    );
    engine.start().unwrap();
    let jan = engine
        .add_account("Jan", "jan@example.test", Secret::Password { password: PASSWORD.into() }, settings("jan@example.test"))
        .await
        .unwrap();

    // A forced error with a stranger's address in the server's reply: reported once a day per code.
    let reply = format!("a1 NO [{run}] mailbox of buurman@elders.nl is full; jan@example.test\r\na1 second line");
    let error = VemError::new(Code::ImapCmd01).with_reply(reply);
    assert!(engine.report(Some(&jan.id), &error).await, "the first one is queued");
    assert!(!engine.report(Some(&jan.id), &error).await, "the same code again today is not");

    let got = wait_for(&run, 1).await;
    let reports: Vec<_> = got.iter().filter(|(subject, _)| subject.contains("Fout VEM-IMAP-CMD-01")).collect();
    assert_eq!(reports.len(), 1, "exactly one report");
    let (subject, text) = reports[0];
    assert!(subject.starts_with(SUBJECT_PREFIX));
    let masked = format!("a1 NO [{run}] mailbox of <adres> is full; jan@example.test");
    assert!(text.contains(&masked) && text.contains("a1 second line"), "the reply exact except the stranger's address: {text}");
    assert!(!text.contains("buurman"), "no other address");
    assert!(!text.contains(PASSWORD), "no password");
    assert!(text.contains(&format!("Versie: test-{run}")));

    // A wish, with the version.
    engine.send_wish(&format!("Graag grotere letters {run}")).unwrap();
    let wishes = wait_for(&format!("Graag grotere letters {run}"), 1).await;
    assert_eq!(wishes.len(), 1);
    assert!(wishes[0].0.ends_with("Wens of idee"));
    assert!(wishes[0].1.contains(&format!("Versie: test-{run}")));

    // A log request for this install is answered once; one for another install sends nothing.
    let me = engine.store.install_id().unwrap();
    let list: LogRequests = serde_json::from_str(&format!(
        r#"{{"requests":[{{"install":"{me}","request":7}},{{"install":"iemand-anders","request":8}}]}}"#
    ))
    .unwrap();
    engine.answer_log_request(&list).unwrap();
    engine.answer_log_request(&list).unwrap();
    let is_log = |subject: &String| subject.contains("Logboek");
    let mut logs = Vec::new();
    for _ in 0..60 {
        logs = martijn_got(&me).await.into_iter().filter(|(s, _)| is_log(s)).collect();
        if !logs.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    // Give a second answer time to arrive, if one was (wrongly) sent.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let logs: Vec<_> = martijn_got(&me).await.into_iter().filter(|(s, _)| is_log(s)).collect();
    assert_eq!(logs.len(), 1, "answered once: {logs:?}");
    assert!(logs[0].0.contains("verzoek 7"));
    assert!(!logs[0].1.contains(PASSWORD));
}
