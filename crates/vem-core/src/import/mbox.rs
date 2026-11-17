//! mbox files (Thunderbird, Eudora, exports): messages separated by "From " lines (mboxrd).
//! Read as a stream, so a file of several GB never sits in memory at once.

use std::io::{BufRead, BufReader};
use std::path::Path;

use super::{Emit, Skipped};

/// Undo mboxrd quoting: ">From " becomes "From ", ">>From " becomes ">From ".
fn unquote(line: &[u8]) -> &[u8] {
    let gt = line.iter().take_while(|b| **b == b'>').count();
    if gt > 0 && line[gt..].starts_with(b"From ") { &line[1..] } else { line }
}

/// One mbox file; every message goes to `folder`.
pub fn read_file(path: &Path, folder: &[String], emit: &mut Emit<'_>) -> Result<Skipped, String> {
    let f = std::fs::File::open(path).map_err(|e| e.kind().to_string())?;
    let mut r = BufReader::with_capacity(256 * 1024, f);
    let mut current: Option<Vec<u8>> = None;
    let mut prev_blank = true;
    let mut line = Vec::new();
    let mut skipped = Skipped::new();
    // Text before the first "From " line: none in a sound file. Damage at the start of the file
    // lands here; what follows is still read.
    let mut lost_start = false;
    let mut empty = 0u64;
    let mut give = |msg: Vec<u8>, empty: &mut u64| -> Result<(), String> {
        if msg.iter().all(u8::is_ascii_whitespace) {
            *empty += 1;
            return Ok(());
        }
        emit(folder, trim_end(msg))
    };
    loop {
        line.clear();
        let n = r.read_until(b'\n', &mut line).map_err(|e| e.kind().to_string())?;
        if n == 0 {
            break;
        }
        if prev_blank && line.starts_with(b"From ") {
            if let Some(msg) = current.take() {
                give(msg, &mut empty)?;
            }
            current = Some(Vec::new());
            prev_blank = false;
            continue;
        }
        prev_blank = line == b"\n" || line == b"\r\n";
        match current.as_mut() {
            Some(msg) => msg.extend_from_slice(unquote(&line)),
            None => lost_start |= !line.iter().all(u8::is_ascii_whitespace),
        }
    }
    let found_any = current.is_some() || empty > 0;
    if let Some(msg) = current.take() {
        give(msg, &mut empty)?;
    }
    if lost_start && !found_any {
        return Err("not an mbox file: no \"From \" line found (damaged, or another kind of file)".into());
    }
    if lost_start {
        skipped.push((folder.join("/"), "the start of the file is damaged; what came before the first message is lost".into()));
    }
    if empty > 0 {
        skipped.push((folder.join("/"), format!("{empty} empty message(s)")));
    }
    Ok(skipped)
}

/// The blank line before the next "From " belongs to the separator, not the message.
fn trim_end(mut m: Vec<u8>) -> Vec<u8> {
    if m.ends_with(b"\r\n\r\n") {
        m.truncate(m.len() - 2);
    } else if m.ends_with(b"\n\n") {
        m.truncate(m.len() - 1);
    }
    m
}

fn is_mbox_file(p: &Path) -> bool {
    match p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()) {
        None => true,
        Some(e) => e == "mbox" || e == "mbx",
    }
}

/// A file, or a Thunderbird folder tree (files without extension, subfolders in "Name.sbd").
pub fn read_path(path: &Path, emit: &mut Emit<'_>) -> Result<Skipped, String> {
    if path.is_file() {
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        return read_file(path, &[stem], emit);
    }
    let mut skipped = Vec::new();
    walk(path, &mut Vec::new(), emit, &mut skipped)?;
    Ok(skipped)
}

fn walk(dir: &Path, folder: &mut Vec<String>, emit: &mut Emit<'_>, skipped: &mut Skipped) -> Result<(), String> {
    let mut entries: Vec<_> = std::fs::read_dir(dir).map_err(|e| e.kind().to_string())?.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if p.is_dir() {
            let shown = name.strip_suffix(".sbd").unwrap_or(&name).to_string();
            folder.push(shown);
            walk(&p, folder, emit, skipped)?;
            folder.pop();
        } else if is_mbox_file(&p) && !name.starts_with('.') {
            let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            folder.push(stem);
            match read_file(&p, folder, emit) {
                Ok(s) => skipped.extend(s),
                Err(e) if e == "cancelled" || e == "store" => return Err(e),
                Err(e) => skipped.push((folder.join("/"), e)),
            }
            folder.pop();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_and_unquotes() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("Inbox");
        std::fs::write(
            &f,
            "From a@b Mon Jan  1 00:00:00 2001\nSubject: een\n\nRegel\n>From hier\n>>From daar\n\nFrom c@d Mon Jan  1 00:00:00 2001\nSubject: twee\n\nTekst\nFrom middenin zonder lege regel ervoor\n",
        )
        .unwrap();
        let mut got: Vec<(Vec<String>, String)> = Vec::new();
        read_path(d.path(), &mut |p, raw| {
            got.push((p.to_vec(), String::from_utf8(raw).unwrap()));
            Ok(())
        })
        .unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, vec!["Inbox"]);
        assert_eq!(got[0].1, "Subject: een\n\nRegel\nFrom hier\n>From daar\n");
        assert!(got[1].1.contains("From middenin"));
    }
}
