//! "Map exporteren": a folder as one mbox file (mboxrd), the backup of mail kept on this
//! computer only (docs/PLAN.md section 4). Thunderbird and most mail programs read it.

use std::io::{BufWriter, Write};
use std::path::Path;

use crate::error::{Code, Result, VemError};
use crate::store::Store;
use crate::store::messages::parse_message_key;

const PAGE: u32 = 500;

fn quote_from_lines(raw: &[u8], out: &mut impl Write) -> std::io::Result<()> {
    for line in raw.split_inclusive(|b| *b == b'\n') {
        let gt = line.iter().take_while(|b| **b == b'>').count();
        if line[gt..].starts_with(b"From ") {
            out.write_all(b">")?;
        }
        out.write_all(line)?;
    }
    if !raw.ends_with(b"\n") {
        out.write_all(b"\n")?;
    }
    Ok(())
}

/// Write every message of the folder whose body is on this computer. Returns how many.
pub fn export_folder(store: &Store, folder_id: i64, out_path: &Path) -> Result<u64> {
    let tmp = out_path.with_extension("tmp");
    let file = std::fs::File::create(&tmp).map_err(|e| VemError::new(Code::StoreWrite01).with_detail(e.kind().to_string()))?;
    let mut w = BufWriter::new(file);
    let mut n = 0u64;
    let mut offset = 0;
    loop {
        let page = store.messages_in_folder(folder_id, offset, PAGE)?;
        if page.is_empty() {
            break;
        }
        offset += PAGE;
        for s in page {
            let id = parse_message_key(&s.id)?;
            let Some(raw) = store.read_eml(id)? else { continue };
            let date = chrono::DateTime::parse_from_rfc3339(&s.date)
                .map(|d| d.format("%a %b %e %H:%M:%S %Y").to_string())
                .unwrap_or_else(|_| "Thu Jan  1 00:00:00 1970".into());
            let res = writeln!(w, "From MAILER-DAEMON {date}").and_then(|_| quote_from_lines(&raw, &mut w)).and_then(|_| w.write_all(b"\n"));
            res.map_err(|e| VemError::new(Code::StoreWrite01).with_detail(e.kind().to_string()))?;
            n += 1;
        }
    }
    w.flush().map_err(|e| VemError::new(Code::StoreWrite01).with_detail(e.kind().to_string()))?;
    drop(w);
    std::fs::rename(&tmp, out_path).map_err(|e| VemError::new(Code::StoreWrite01).with_detail(e.kind().to_string()))?;
    crate::log::event("export.done", &[("folder", &folder_id.to_string()), ("messages", &n.to_string())]);
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FolderRole;
    use crate::store::NewMessage;
    use crate::store::testutil::store;

    #[test]
    fn export_then_import_gives_the_same_messages() {
        let (_d, s) = store();
        let a = s.local_account("Oud").unwrap();
        let f = s.ensure_local_folder(&a.id, &[], Some(FolderRole::Inbox)).unwrap();
        for (i, body) in ["Hallo\nFrom de zolder\n", "Tweede"].iter().enumerate() {
            let raw = format!("Subject: m{i}\r\nMessage-ID: <{i}@x>\r\nDate: Tue, 3 Feb 2004 10:00:00 +0100\r\n\r\n{body}").into_bytes();
            s.insert_message(&NewMessage { account_id: a.id.clone(), folder_id: f, raw, has_body: true, ..Default::default() }).unwrap();
        }
        let out = tempfile::tempdir().unwrap();
        let path = out.path().join("Postvak IN.mbox");
        assert_eq!(export_folder(&s, f, &path).unwrap(), 2);
        let mut back = Vec::new();
        super::super::mbox::read_file(&path, &[], &mut |_, raw| {
            back.push(String::from_utf8(raw).unwrap());
            Ok(())
        })
        .unwrap();
        assert_eq!(back.len(), 2);
        assert!(back.iter().any(|m| m.contains("\nFrom de zolder")), "From lines survive the round trip");
    }
}
