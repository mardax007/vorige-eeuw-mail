//! How fast search is on a big mailbox (docs/PLAN.md: results while typing). Slow to set up,
//! so it only runs when asked:
//!     cargo test -p vem-core --release --test search_perf -- --ignored --nocapture
//! Building the index takes minutes. With VEM_PERF_DIR set to a folder, the index is kept there
//! and reused by the next run, which then only searches.

use std::time::Instant;

use vem_core::model::{AccountKind, FolderRole, SearchScope};
use vem_core::store::{NewMessage, Store};

const MESSAGES: usize = 100_000;
const WORDS: &[&str] = &[
    "verjaardag", "oma", "zaterdag", "factuur", "vakantie", "foto", "kerst", "tuin", "dokter", "afspraak",
    "kleinkinderen", "fiets", "boodschappen", "bank", "verzekering", "huis", "auto", "koffie", "taart", "bloemen",
];

/// The target (PLAN.md section 2, "Search"): first results within 0.2 s on 100,000 messages, at
/// all three levels. Phase 5 task 6 runs the same test on a phone (CLAUDE.md, Commands).
const TARGET_MS: u128 = 200;
const ACCOUNTS: usize = 3;
const FOLDERS_PER_ACCOUNT: usize = 10;

#[test]
#[ignore]
fn search_100k_messages() {
    let temp = tempfile::tempdir().unwrap();
    let dir = std::env::var_os("VEM_PERF_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| temp.path().to_path_buf());
    std::fs::create_dir_all(&dir).unwrap();
    let ready = dir.join("perf-ready");
    let store = Store::open(&dir).unwrap();
    // Three mailboxes with ten folders each; most mail in each Postvak IN, as in real mailboxes.
    let mut accounts = store.accounts().unwrap();
    if accounts.is_empty() {
        for n in 0..ACCOUNTS {
            accounts.push(store.add_account(&format!("Persoon {n}"), &format!("p{n}@x.nl"), AccountKind::Imap, None).unwrap());
        }
    }
    let mut folders = Vec::new();
    for a in &accounts {
        for f in 0..FOLDERS_PER_ACCOUNT {
            let (name, role) = if f == 0 { ("INBOX".to_string(), Some(FolderRole::Inbox)) } else { (format!("Map {f}"), None) };
            let id = store.upsert_folder(&a.id, &name, Some("/"), role, true).unwrap();
            folders.push((a.id.clone(), id));
        }
    }
    if ready.exists() {
        eprintln!("reusing the index in {}", dir.display());
    } else {
        insert_messages(&store, &folders);
        std::fs::write(&ready, b"").unwrap();
    }
    search_levels(&store, &folders);
}

fn insert_messages(store: &Store, folders: &[(String, i64)]) {
    let t = Instant::now();
    let mut seed: u64 = 42;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as usize
    };
    for i in 0..MESSAGES {
        let w: Vec<&str> = (0..30).map(|_| WORDS[next() % WORDS.len()]).collect();
        let account = next() % ACCOUNTS;
        let folder = if next() % 2 == 0 { 0 } else { next() % FOLDERS_PER_ACCOUNT };
        let (account_id, folder_id) = folders[account * FOLDERS_PER_ACCOUNT + folder].clone();
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
                account_id,
                folder_id,
                uid: Some(i as u32 + 1),
                raw: raw.into_bytes(),
                has_body: true,
                size: 500,
                ..Default::default()
            })
            .unwrap();
    }
    eprintln!("inserted {MESSAGES} in {:?}", t.elapsed());
}

fn search_levels(store: &Store, folders: &[(String, i64)]) {

    let (account_id, inbox) = folders[0].clone();
    let levels = [
        ("alle mail", SearchScope::All),
        ("dit postvak", SearchScope::Account { account_id }),
        ("deze map", SearchScope::Folder { folder_id: inbox.to_string() }),
    ];
    let mut slowest = 0;
    let mut too_slow = Vec::new();
    for (level, scope) in &levels {
        for q in ["oma", "verj", "factuur bank", "persoon 17", "kerst taart bloemen", "zz"] {
            let t = Instant::now();
            let hits = store.search(q, scope, 50).unwrap();
            let took = t.elapsed();
            // The same search again: the index pages are in memory now, as while typing.
            let t = Instant::now();
            store.search(q, scope, 50).unwrap();
            let again = t.elapsed();
            eprintln!("{level}: {q:?}: {} hits in {took:?} (again {again:?})", hits.len());
            if took.as_millis() >= TARGET_MS {
                too_slow.push(format!("{level}: {q:?} took {took:?}"));
            }
            slowest = slowest.max(took.as_millis());
        }
    }
    let t = Instant::now();
    let page = store.messages_in_folder(inbox, 0, 100).unwrap();
    eprintln!("first page of {} in {:?}", page.len(), t.elapsed());
    eprintln!("slowest search: {slowest} ms (target {TARGET_MS} ms)");
    assert!(t.elapsed().as_millis() < 100);
    assert!(too_slow.is_empty(), "slower than {TARGET_MS} ms: {too_slow:#?}");
}
