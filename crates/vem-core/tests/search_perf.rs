//! How fast search is on a big mailbox (docs/PLAN.md: results while typing). Slow to set up,
//! so it only runs when asked:
//!     cargo test -p vem-core --release --test search_perf -- --ignored --nocapture

use std::time::Instant;

use vem_core::model::{AccountKind, FolderRole, SearchScope};
use vem_core::store::{NewMessage, Store};

const MESSAGES: usize = 100_000;
const WORDS: &[&str] = &[
    "verjaardag", "oma", "zaterdag", "factuur", "vakantie", "foto", "kerst", "tuin", "dokter", "afspraak",
    "kleinkinderen", "fiets", "boodschappen", "bank", "verzekering", "huis", "auto", "koffie", "taart", "bloemen",
];

#[test]
#[ignore]
fn search_100k_messages() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let a = store.add_account("Jan", "jan@x.nl", AccountKind::Imap, None).unwrap();
    let inbox = store.upsert_folder(&a.id, "INBOX", Some("/"), Some(FolderRole::Inbox), true).unwrap();
    let t = Instant::now();
    let mut seed: u64 = 42;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as usize
    };
    for i in 0..MESSAGES {
        let w: Vec<&str> = (0..30).map(|_| WORDS[next() % WORDS.len()]).collect();
        let raw = format!(
            "From: Persoon {} <p{}@voorbeeld.nl>\r\nSubject: {} {} {i}\r\nMessage-ID: <{i}@x>\r\nDate: Tue, 3 Feb 2015 10:00:00 +0100\r\n\r\n{}\r\n",
            i % 300,
            i % 300,
            w[0],
            w[1],
            w.join(" ")
        );
        store
            .insert_message(&NewMessage {
                account_id: a.id.clone(),
                folder_id: inbox,
                uid: Some(i as u32 + 1),
                raw: raw.into_bytes(),
                has_body: true,
                size: 500,
                ..Default::default()
            })
            .unwrap();
    }
    eprintln!("inserted {MESSAGES} in {:?}", t.elapsed());
    for q in ["oma", "verj", "factuur bank", "persoon 17", "kerst taart bloemen", "zz"] {
        let t = Instant::now();
        let hits = store.search(q, &SearchScope::All, 50).unwrap();
        let took = t.elapsed();
        eprintln!("search {q:?}: {} hits in {took:?}", hits.len());
        assert!(took.as_millis() < 300, "search {q:?} took {took:?}");
    }
    let t = Instant::now();
    let page = store.messages_in_folder(inbox, 0, 100).unwrap();
    eprintln!("first page of {} in {:?}", page.len(), t.elapsed());
    assert!(t.elapsed().as_millis() < 100);
}
