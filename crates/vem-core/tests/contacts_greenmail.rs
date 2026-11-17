//! Contacts between a PC and a phone through the mailbox (decision 19, phase 5 task 9), against
//! local GreenMail (never a real mailbox). Two stores stand in for the two devices; the phone
//! links by the text of the QR code the PC shows.
//!     cargo test -p vem-core --features test-support --test contacts_greenmail
#![cfg(feature = "test-support")]

use std::time::Duration;

use vem_core::contacts_sync;
use vem_core::imap::{Auth, Imap, ImapConfig};
use vem_core::model::{AccountKind, Contact};
use vem_core::net::{Security, Trust};
use vem_core::secrets::Secrets;
use vem_core::store::Store;

const USER: &str = "jan@example.test";

fn server_up() -> bool {
    let up = std::net::TcpStream::connect_timeout(&"127.0.0.1:3993".parse().unwrap(), Duration::from_secs(1)).is_ok();
    if !up {
        eprintln!("GreenMail is not running; skipping");
    }
    up
}

fn imap_cfg() -> ImapConfig {
    ImapConfig {
        host: "127.0.0.1".into(),
        port: 3993,
        security: Security::Tls,
        username: USER.into(),
        auth: Auth::Password("geheim".into()),
        trust: Trust::AcceptAnyLocalTestServer,
    }
}

struct Device {
    _dir: tempfile::TempDir,
    store: Store,
    secrets: Secrets,
    account: String,
}

fn device() -> Device {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let secrets = Secrets::new(dir.path());
    let account = store.add_account("Jan", USER, AccountKind::Imap, None).unwrap().id;
    Device { _dir: dir, store, secrets, account }
}

async fn sync(d: &Device) -> bool {
    let mut imap = Imap::connect(&imap_cfg()).await.unwrap();
    let changed = contacts_sync::sync(&d.store, &d.secrets, &mut imap, &d.account).await.unwrap();
    imap.logout().await;
    changed
}

fn names(d: &Device) -> Vec<String> {
    d.store.contacts("").unwrap().into_iter().map(|c| c.name).collect()
}

fn contact(name: &str, email: &str) -> Contact {
    Contact { id: String::new(), name: name.into(), emails: vec![email.into()], phone: None, note: None }
}

#[tokio::test]
async fn a_phone_linked_by_the_code_shares_contacts_both_ways() {
    if !server_up() {
        return;
    }
    let pc = device();
    let phone = device();
    let code = contacts_sync::pair_code(&pc.store, &pc.secrets, &pc.account).unwrap();
    let text = serde_json::to_string(&code).unwrap();
    let linked = contacts_sync::pair_from_code(&phone.store, &phone.secrets, &text).unwrap();
    assert_eq!(linked, phone.account, "the phone links its own mailbox with the same address");

    let oma = pc.store.save_contact(&contact("Oma", "oma@example.test")).unwrap();
    sync(&pc).await;
    assert!(sync(&phone).await, "the phone takes the PC's list");
    assert_eq!(names(&phone), vec!["Oma"]);

    // The contacts folder now exists on the server, but it is not mail: no folder list shows it.
    let mut imap = Imap::connect(&imap_cfg()).await.unwrap();
    vem_core::imap::sync::sync_folders(&phone.store, &mut imap, &phone.account).await.unwrap();
    imap.logout().await;
    let folders = phone.store.folders(Some(&phone.account)).unwrap();
    assert!(folders.iter().any(|f| f.name == "INBOX"), "the folder list is filled");
    assert!(!folders.iter().any(|f| f.name == contacts_sync::FOLDER), "the contacts folder stays hidden");

    // Changes on the phone reach the PC, in the same second too: a deletion stays a deletion.
    phone.store.delete_contact(&oma.id).unwrap();
    phone.store.save_contact(&contact("Bert", "bert@example.test")).unwrap();
    sync(&phone).await;
    assert!(sync(&pc).await);
    assert_eq!(names(&pc), vec!["Bert"]);

    // A new code on the PC makes a new key: the old pairing can no longer read the list.
    contacts_sync::pair_code(&pc.store, &pc.secrets, &pc.account).unwrap();
    pc.store.save_contact(&contact("Geheim", "geheim@example.test")).unwrap();
    sync(&pc).await;
    assert!(!sync(&phone).await, "the old key reads nothing");
    assert_eq!(names(&phone), vec!["Bert"]);
}
