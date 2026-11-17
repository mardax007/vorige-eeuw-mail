//! Mail kept on the PC reaches the paired phone (decision 26, docs/pc-to-phone.md): a PC and a
//! phone in one process, over 127.0.0.1 only (so no firewall is involved), each with its own index.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use vem_core::contacts_sync;
use vem_core::lan::{self, Copied, Listen, PhoneEvent};
use vem_core::model::{AccountKind, FolderRole};
use vem_core::secrets::Secrets;
use vem_core::store::{NewMessage, Store};

struct Device {
    _dir: tempfile::TempDir,
    store: Arc<Store>,
    secrets: Arc<Secrets>,
    account: String,
}

fn device() -> Device {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let secrets = Secrets::new(dir.path());
    let account = store.add_account("Jan", "jan@example.test", AccountKind::Imap, None).unwrap().id;
    Device { _dir: dir, store: Arc::new(store), secrets: Arc::new(secrets), account }
}

fn pair(pc: &Device, phone: &Device) {
    let code = contacts_sync::pair_code(&pc.store, &pc.secrets, &pc.account).unwrap();
    contacts_sync::pair_from_code(&phone.store, &phone.secrets, &serde_json::to_string(&code).unwrap()).unwrap();
}

fn old_mail(pc: &Device, n: u32) -> String {
    let old = pc.store.local_account("Oude mail uit Outlook").unwrap();
    let inbox = pc.store.ensure_local_folder(&old.id, &["Postvak IN".into()], Some(FolderRole::Inbox)).unwrap();
    for i in 0..n {
        pc.store
            .insert_message(&NewMessage {
                account_id: old.id.clone(),
                folder_id: inbox,
                raw: format!("From: Oma <oma@example.test>\r\nSubject: Brief {i}\r\nDate: Mon, 1 Jan 2007 10:00:00 +0100\r\n\r\nHoi {i}\r\n").into_bytes(),
                has_body: true,
                size: 80,
                ..Default::default()
            })
            .unwrap();
    }
    old.id
}

fn start_phone(phone: &Device, find_port: u16, copy_port: u16) -> Arc<Mutex<Vec<PhoneEvent>>> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = events.clone();
    let listen = Listen { ip: Ipv4Addr::LOCALHOST, find_port, copy_port };
    tokio::spawn(lan::serve(
        phone.store.clone(),
        phone.secrets.clone(),
        listen,
        Arc::new(move |e| seen.lock().unwrap().push(e)),
    ));
    events
}

async fn offer(pc: &Device, find_port: u16) -> Option<Copied> {
    let target = [SocketAddr::from((Ipv4Addr::LOCALHOST, find_port))];
    lan::offer(&pc.store, &pc.secrets, &target, None).await.unwrap()
}

#[tokio::test]
async fn old_mail_on_the_pc_reaches_the_phone_and_follows_it() {
    let (pc, phone) = (device(), device());
    pair(&pc, &phone);
    let old = old_mail(&pc, 60);
    let events = start_phone(&phone, 47911, 47912);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let first = offer(&pc, 47911).await.expect("the phone answered");
    assert_eq!(first, Copied { received: 60, removed: 0, skipped: 0 });
    let (copy, _) = phone.store.copy_account(&old).unwrap().unwrap();
    assert_eq!(phone.store.account(&copy).unwrap().display_name, "Oude mail uit Outlook");
    let inbox = phone.store.folder_by_role(&copy, FolderRole::Inbox).unwrap().unwrap();
    let list = phone.store.messages_in_folder(inbox, 0, 100).unwrap();
    assert_eq!(list.len(), 60);
    assert!(list.iter().any(|m| m.subject == "Brief 7"));

    // Up to date: the find answer says so and no connection is made.
    assert_eq!(offer(&pc, 47911).await, Some(Copied::default()));

    // Deleted on the PC: deleted on the phone at the next copy.
    let gone = pc.store.offered_messages().unwrap()[0].id;
    pc.store.delete_message(gone).unwrap();
    assert_eq!(offer(&pc, 47911).await, Some(Copied { received: 0, removed: 1, skipped: 0 }));
    assert_eq!(phone.store.messages_in_folder(inbox, 0, 100).unwrap().len(), 59);

    let events = events.lock().unwrap();
    assert!(events.contains(&PhoneEvent::Progress { done: 0, total: 60 }));
    assert!(events.contains(&PhoneEvent::Done { received: 60, removed: 0 }));
    assert!(pc.store.kv_get::<lan::LastCopy>(lan::LAST_KEY).unwrap().is_some());
}

#[tokio::test]
async fn a_phone_with_another_key_neither_answers_nor_receives() {
    let (pc, phone, stranger) = (device(), device(), device());
    pair(&pc, &phone);
    // The stranger pairs with a PC of its own: a different key.
    pair(&device(), &stranger);
    old_mail(&pc, 3);
    start_phone(&stranger, 47921, 47922);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(offer(&pc, 47921).await, None);
    assert!(stranger.store.copy_accounts().unwrap().is_empty());
}

/// By hand, against the app in the Android emulator: this test plays the PC. It writes the pairing
/// code to `VEM_PAIR_OUT` (give it to the phone's `pair_with_code`), then offers its local mail to
/// 127.0.0.1:47801 for two minutes. Before: `adb forward tcp:47802 tcp:47802` and, in the emulator
/// console, `redir add udp:47801:47801`.
#[tokio::test]
#[ignore]
async fn emulator_pc() {
    let pc = device();
    old_mail(&pc, 40);
    let other = pc.store.local_account("Oude mail uit Windows Live Mail").unwrap();
    let folder = pc.store.ensure_local_folder(&other.id, &["Vakantie".into(), "2009".into()], None).unwrap();
    pc.store
        .insert_message(&NewMessage {
            account_id: other.id.clone(),
            folder_id: folder,
            raw: b"From: Piet <piet@example.test>\r\nSubject: Foto's Texel\r\nDate: Sat, 1 Aug 2009 12:00:00 +0200\r\n\r\nMooi weer.\r\n".to_vec(),
            has_body: true,
            unread: false,
            important: true,
            size: 90,
            ..Default::default()
        })
        .unwrap();
    let code = contacts_sync::pair_code(&pc.store, &pc.secrets, &pc.account).unwrap();
    std::fs::write(std::env::var("VEM_PAIR_OUT").unwrap(), serde_json::to_string(&code).unwrap()).unwrap();
    let target = [SocketAddr::from((Ipv4Addr::LOCALHOST, lan::FIND_PORT))];
    for _ in 0..60 {
        match lan::offer(&pc.store, &pc.secrets, &target, Some(Ipv4Addr::LOCALHOST.into())).await {
            Ok(Some(c)) => {
                println!("copied {c:?}");
                if c.received > 0 {
                    // Once more: now the phone is up to date and says so.
                    println!("again {:?}", lan::offer(&pc.store, &pc.secrets, &target, Some(Ipv4Addr::LOCALHOST.into())).await);
                    return;
                }
            }
            Ok(None) => println!("no answer"),
            Err(e) => println!("error {e}"),
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    panic!("the phone never took the mail");
}

#[tokio::test]
async fn nothing_is_offered_without_pairing_or_local_mail() {
    let (pc, phone) = (device(), device());
    old_mail(&pc, 2);
    start_phone(&phone, 47931, 47932);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(offer(&pc, 47931).await, None, "not paired");
    let (pc2, phone2) = (device(), device());
    pair(&pc2, &phone2);
    start_phone(&phone2, 47941, 47942);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(offer(&pc2, 47941).await, None, "no local mail");
}
