//! Damaged old mail files (the user's request of 17 November 2026: "corrupt data to see
//! recovery"). Every reader is fed truncated and trampled copies of a file it can read. The rule
//! for each: the import ends, never hangs and never takes the app down; it either brings in what
//! it can read (and lists what it skipped) or stops with an import code the screen explains; the
//! damaged file is left exactly as it was.
//!
//! Self-made damage on copies in a temporary folder only; no real mailbox is involved.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::time::Duration;

use super::*;
use crate::store::testutil::store;

/// A reader that has not finished by then is taken to hang on the damage.
const PER_CASE: Duration = Duration::from_secs(120);

/// What one damaged import did.
#[derive(Debug)]
enum Outcome {
    // `skipped` shows in the printed outcome (Debug), which records what each case did.
    #[allow(dead_code)]
    Imported { added: u64, skipped: usize },
    Refused(String),
}

/// Deterministic noise without a random-number crate (a 64-bit LCG, Knuth's constants).
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// The ways a file gets damaged: cut short, a run of bytes overwritten, single bits flipped.
fn damaged_copies(good: &[u8], keep_header: usize, seed: u64) -> Vec<(String, Vec<u8>)> {
    let len = good.len();
    let mut out = Vec::new();
    for cut in [0, 1, 7, keep_header.min(len), len / 4, len / 2, len * 3 / 4, len.saturating_sub(1)] {
        out.push((format!("cut at {cut}"), good[..cut.min(len)].to_vec()));
    }
    let mut noise = Noise(seed);
    let body = len.saturating_sub(keep_header).max(1);
    for round in 0..6 {
        let mut bytes = good.to_vec();
        let start = keep_header + noise.below(body);
        let run = 1 + noise.below(4096);
        let end = (start + run).min(len);
        let fill = if round % 2 == 0 { 0x00 } else { 0xFF };
        for b in &mut bytes[start.min(len)..end] {
            *b = fill;
        }
        out.push((format!("{run} bytes of {fill:#04x} at {start}"), bytes));
    }
    for round in 0..6 {
        let mut bytes = good.to_vec();
        for _ in 0..(1 + round * 8) {
            let at = keep_header + noise.below(body);
            if at < len {
                bytes[at] ^= 1 << noise.below(8);
            }
        }
        out.push((format!("{} flipped bits", 1 + round * 8), bytes));
    }
    out
}

/// Import one damaged file in its own thread, so a hang shows up as a failure with the case name.
fn import_damaged(name: &str, bytes: Vec<u8>, kind_ext: &str) -> Outcome {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(format!("kapot.{kind_ext}"));
    std::fs::write(&file, &bytes).unwrap();
    let (tx, rx) = mpsc::channel();
    let path = file.clone();
    std::thread::spawn(move || {
        let (_d, s) = store();
        let outcome = match source_for(&path) {
            Err(e) => Outcome::Refused(e.code),
            Ok(source) => match run(&s, "j", &source, &ImportTarget::Local, &|_| {}, &AtomicBool::new(false)) {
                Ok(r) => Outcome::Imported { added: r.added, skipped: r.skipped.len() },
                Err(e) => Outcome::Refused(e.code),
            },
        };
        let _ = tx.send(outcome);
    });
    let outcome = rx.recv_timeout(PER_CASE).unwrap_or_else(|_| panic!("{name}: the import did not end"));
    if let Outcome::Refused(code) = &outcome {
        assert!(
            code.starts_with("VEM-IMPORT-READ-"),
            "{name}: refused with {code}, not an import code the screen explains"
        );
    }
    assert_eq!(std::fs::read(&file).unwrap(), bytes, "{name}: the damaged file was changed");
    outcome
}

fn run_all(label: &str, good: &[u8], keep_header: usize, ext: &str, seed: u64) -> Vec<(String, Outcome)> {
    damaged_copies(good, keep_header, seed)
        .into_iter()
        .map(|(how, bytes)| {
            let name = format!("{label}, {how}");
            let outcome = import_damaged(&name, bytes, ext);
            eprintln!("{name}: {outcome:?}");
            (name, outcome)
        })
        .collect()
}

#[test]
fn a_damaged_empty_pst_ends_cleanly() {
    let good = std::fs::read(super::pst::tests::empty_pst()).unwrap();
    let results = run_all("Empty.pst", &good, 512, "pst", 1);
    assert!(results.iter().any(|(_, o)| matches!(o, Outcome::Refused(_))), "a PST cut to nothing is refused");
}

/// A real PST (`VEM_RESCUE_PST`, the Enron sample): damage in the middle must not lose the
/// whole import silently. Either the reader stops with an import code (the screen then offers
/// the rescue tool) or it brings in what it could.
#[test]
fn a_damaged_real_pst_ends_cleanly() {
    let Some(src) = std::env::var_os("VEM_RESCUE_PST").map(PathBuf::from) else {
        eprintln!("note: VEM_RESCUE_PST not set, no real PST damaged");
        return;
    };
    let good = std::fs::read(&src).unwrap();
    run_all("real PST", &good, 512, "pst", 2);
}

/// The recovery the screen offers after VEM-IMPORT-READ-01: the rescue tool on a real PST that
/// was cut short (a copy broken off halfway, the commonest damage). It must end, leave the file
/// unchanged and report what it got; how much a cut file still gives is recorded, not required.
#[cfg(windows)]
#[test]
fn the_rescue_tool_on_a_cut_real_pst() {
    let (Some(src), Some(tool)) =
        (std::env::var_os("VEM_RESCUE_PST").map(PathBuf::from), std::env::var_os("VEM_READPST").map(PathBuf::from))
    else {
        eprintln!("note: VEM_RESCUE_PST or VEM_READPST not set, rescue of a cut PST not tried");
        return;
    };
    let good = std::fs::read(&src).unwrap();
    for part in [2usize, 3] {
        let cut = good.len() * part / 4;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("afgebroken.pst");
        std::fs::write(&file, &good[..cut]).unwrap();
        let source = source_for(&file).unwrap();
        let (tx, rx) = mpsc::channel();
        let (path_tool, src_clone) = (tool.clone(), source.clone());
        std::thread::spawn(move || {
            let (_d, s) = store();
            let r = run_rescue(&s, "j", &src_clone, &ImportTarget::Local, &|_| {}, &AtomicBool::new(false), &path_tool);
            let _ = tx.send(r.map(|r| (r.added, r.skipped.len())).map_err(|e| e.code));
        });
        let outcome = rx.recv_timeout(PER_CASE).unwrap_or_else(|_| panic!("rescue of a PST cut at {cut} did not end"));
        eprintln!("rescue, real PST cut at {cut} of {}: {outcome:?}", good.len());
        if let Err(code) = &outcome {
            assert!(code.starts_with("VEM-IMPORT-READ-"), "rescue refused with {code}");
        }
        assert_eq!(std::fs::read(&file).unwrap(), &good[..cut], "the cut file was changed");
    }
}

#[test]
fn a_damaged_real_dbx_ends_cleanly() {
    let Some(src) = std::env::var_os("VEM_REAL_DBX").map(PathBuf::from) else {
        eprintln!("note: VEM_REAL_DBX not set, no real DBX damaged");
        return;
    };
    let good = std::fs::read(&src).unwrap();
    run_all("real DBX", &good, 0x24bc, "dbx", 3);
}

/// A self-made Outlook Express folder of 40 messages: damage in the index or the message blocks
/// must still bring most of them in (the reader falls back to scanning for message blocks).
#[test]
fn a_damaged_self_made_dbx_keeps_most_messages() {
    let texts: Vec<Vec<u8>> = (0..40)
        .map(|i| {
            format!("From: Oma <oma@x.nl>\r\nSubject: Brief {i}\r\nMessage-ID: <d{i}@x>\r\n\r\nTekst van brief {i}\r\n{}\r\n", "regel ".repeat(60))
                .into_bytes()
        })
        .collect();
    let refs: Vec<&[u8]> = texts.iter().map(Vec::as_slice).collect();
    let good = super::dbx::tests::sample(&refs);
    let header = 0x24BC;
    for (name, outcome) in run_all("self-made DBX", &good, header, "dbx", 6) {
        let Some(cut) = name.strip_prefix("self-made DBX, cut at ").map(|n| n.parse::<usize>().unwrap()) else {
            // Overwritten runs and flipped bits: at most a few messages may go.
            let Outcome::Imported { added, .. } = outcome else { panic!("{name}: still read") };
            assert!(added >= 30, "{name}: only {added} of 40 messages came in");
            continue;
        };
        // Each message takes about 550 bytes after the header; all whole ones before the cut
        // must come back, less one for the block the cut runs through.
        if cut >= good.len() / 2 {
            let Outcome::Imported { added, .. } = outcome else { panic!("{name}: a cut file is still read") };
            let whole = ((cut - header) / 560) as u64;
            assert!(added + 1 >= whole, "{name}: only {added} of about {whole} whole messages came in");
        }
    }
}

fn sample_mbox() -> Vec<u8> {
    let mut s = String::new();
    for i in 0..20 {
        s.push_str(&format!(
            "From oma@x.nl Tue Feb  3 10:00:00 2004\r\nFrom: Oma <oma@x.nl>\r\nSubject: Brief {i}\r\nMessage-ID: <m{i}@x>\r\nDate: Tue, 3 Feb 2004 10:00:00 +0100\r\n\r\nTekst van brief {i}\r\n{}\r\n\r\n",
            "regel ".repeat(40)
        ));
    }
    s.into_bytes()
}

#[test]
fn a_damaged_mbox_keeps_the_messages_it_can_read() {
    let good = sample_mbox();
    for (name, outcome) in run_all("mbox", &good, 0, "mbox", 4) {
        // Overwritten runs land inside one or two messages; the rest of the twenty still count.
        if name.contains("bytes of") {
            let Outcome::Imported { added, .. } = outcome else { panic!("{name}: an mbox with a bad patch is still read") };
            assert!(added >= 10, "{name}: only {added} of 20 messages came in");
        }
    }
}

/// An EML folder with broken files among good ones: the good ones come in, nothing hangs.
#[test]
fn an_eml_folder_with_broken_files_imports_the_rest() {
    let (_d, s) = store();
    let src = tempfile::tempdir().unwrap();
    let inbox = src.path().join("Postvak IN");
    std::fs::create_dir_all(&inbox).unwrap();
    let good = |i: usize| format!("From: Oma <oma@x.nl>\r\nSubject: Goed {i}\r\nMessage-ID: <g{i}@x>\r\n\r\nTekst\r\n");
    for i in 0..5 {
        std::fs::write(inbox.join(format!("goed{i}.eml")), good(i)).unwrap();
    }
    let mut noise = Noise(5);
    let junk: Vec<u8> = (0..20_000).map(|_| noise.next() as u8).collect();
    std::fs::write(inbox.join("ruis.eml"), &junk).unwrap();
    std::fs::write(inbox.join("leeg.eml"), b"").unwrap();
    std::fs::write(inbox.join("half.eml"), &good(9).as_bytes()[..20]).unwrap();
    std::fs::write(inbox.join("nul.eml"), vec![0u8; 4096]).unwrap();
    let before: Vec<(PathBuf, Vec<u8>)> = std::fs::read_dir(&inbox)
        .unwrap()
        .flatten()
        .map(|e| (e.path(), std::fs::read(e.path()).unwrap()))
        .collect();

    let source = ImportSource { kind: ImportKind::Eml, path: src.path().to_string_lossy().into(), size: None, label: "Map".into(), protected: false };
    let r = run(&s, "j", &source, &ImportTarget::Local, &|_| {}, &AtomicBool::new(false)).unwrap();
    assert!(r.added >= 5, "the five good messages came in (added {})", r.added);
    let hits = s.search("goed", &crate::model::SearchScope::All, 20).unwrap();
    assert_eq!(hits.len(), 5, "each good message is findable");
    for (path, bytes) in before {
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "{} was changed", display(&path));
    }
}

fn display(p: &Path) -> String {
    p.display().to_string()
}
