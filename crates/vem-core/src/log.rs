//! The local log (docs/PLAN.md section 5): rotating files plus the last lines in memory.
//!
//! Callers pass an event name and typed fields (account number, host, code, reply); every line
//! is masked for addresses before it is written. Never pass passwords, tokens or mail text.

use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::error::mask_addresses;

const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const KEEP_FILES: usize = 3;
const MEMORY_LINES: usize = 500;

struct Inner {
    dir: Option<PathBuf>,
    recent: VecDeque<String>,
}

fn inner() -> &'static Mutex<Inner> {
    static LOG: OnceLock<Mutex<Inner>> = OnceLock::new();
    LOG.get_or_init(|| Mutex::new(Inner { dir: None, recent: VecDeque::new() }))
}

/// Write log files into `<data>/logs` from now on.
pub fn init(data_dir: &Path) {
    let dir = data_dir.join("logs");
    let _ = std::fs::create_dir_all(&dir);
    inner().lock().unwrap_or_else(|e| e.into_inner()).dir = Some(dir);
}

fn file(dir: &Path, n: usize) -> PathBuf {
    if n == 0 { dir.join("vem.log") } else { dir.join(format!("vem.{n}.log")) }
}

fn rotate(dir: &Path) {
    let current = file(dir, 0);
    if std::fs::metadata(&current).map(|m| m.len() < MAX_FILE_BYTES).unwrap_or(true) {
        return;
    }
    for n in (1..KEEP_FILES).rev() {
        let _ = std::fs::rename(file(dir, n - 1), file(dir, n));
    }
}

/// One line: `2026-11-16T10:00:00Z event key=value …`. Values are masked and kept on one line.
pub fn event(name: &str, fields: &[(&str, &str)]) {
    let mut line = format!("{} {name}", chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ"));
    for (k, v) in fields {
        let v = mask_addresses(v, &[]).replace(['\r', '\n'], " ");
        line.push_str(&format!(" {k}={v:?}"));
    }
    let mut g = inner().lock().unwrap_or_else(|e| e.into_inner());
    if g.recent.len() >= MEMORY_LINES {
        g.recent.pop_front();
    }
    g.recent.push_back(line.clone());
    if let Some(dir) = g.dir.clone() {
        rotate(&dir);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(file(&dir, 0)) {
            let _ = writeln!(f, "{line}");
        }
    }
}

pub fn error(name: &str, e: &crate::VemError) {
    event(
        name,
        &[
            ("code", &e.code),
            ("detail", e.detail.as_deref().unwrap_or("")),
            ("reply", e.server_reply.as_deref().unwrap_or("")),
        ],
    );
}

/// The last `n` lines, oldest first.
pub fn tail(n: usize) -> Vec<String> {
    let g = inner().lock().unwrap_or_else(|e| e.into_inner());
    g.recent.iter().skip(g.recent.len().saturating_sub(n)).cloned().collect()
}

/// Every log file's text, oldest first, for a log request.
pub fn full() -> String {
    let dir = inner().lock().unwrap_or_else(|e| e.into_inner()).dir.clone();
    let Some(dir) = dir else { return tail(MEMORY_LINES).join("\n") };
    (0..KEEP_FILES).rev().filter_map(|n| std::fs::read_to_string(file(&dir, n)).ok()).collect::<Vec<_>>().join("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_masked_and_kept() {
        event("test.masked", &[("reply", "550 no such user piet@ziggo.nl\r\nbye"), ("host", "imap.ziggo.nl")]);
        let last = tail(1).pop().unwrap();
        assert!(last.contains("test.masked"));
        assert!(last.contains("<adres>") && !last.contains("piet@ziggo.nl"));
        assert!(!last.contains('\n') && last.contains("imap.ziggo.nl"));
    }
}
