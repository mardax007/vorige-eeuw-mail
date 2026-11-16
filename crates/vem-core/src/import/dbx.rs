//! Reading an Outlook Express 5/6 folder file (.dbx), read-only.
//!
//! One file is one folder; its name is the file name without ".dbx". The format is not
//! documented by Microsoft. The layout here follows UnDBX (github.com/ZungBang/undbx,
//! dbxread.c), which is based on DbxConv:
//!
//! - offset 0: 16-byte signature (mail folder files; Folders.dbx and Offline.dbx differ);
//! - 0xC4: number of messages; 0xE4: file offset of the root node of the message index tree;
//! - index node: u32 self, u32, u32 left child, u32, u8, u8 entry count (at +17), u16,
//!   u32 messages under the left child (at +20); then entries of 12 bytes: u32 message info
//!   offset, u32 child node, u32 messages under the child;
//! - message info: u32 self, u32 size, u16, u8 attribute count (at +10), u8; then 4-byte
//!   attributes (type in the low byte, value in the high 24 bits). Type 0x84 holds the offset
//!   of the message text directly; type 0x04 points to it (at info + 12 + value + 4 * count);
//! - message text: a chain of blocks, each u32 self offset, u32 0x200 (block capacity),
//!   u16 data length (the next 16 bits are not used), u32 next block (0 = last), then the data.
//!
//! The message text is the message exactly as it was received (RFC 5322). When the index tree
//! is damaged, the file is scanned for block headers instead (UnDBX's recovery mode): chains
//! start at blocks that no other block points to. Blocks marked as deleted are not recovered.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// What `read` calls for every message: folder path and RFC 5322 bytes.
pub type Emit<'a> = dyn FnMut(&[String], Vec<u8>) -> Result<(), String> + 'a;

pub struct ReadStats {
    pub messages: u64,
    /// (folder path joined with "/", short reason without mail content)
    pub skipped: Vec<(String, String)>,
}

/// Signature of a folder file that holds mail (UnDBX's DBX_TYPE_EMAIL).
const MAIL_SIGNATURE: [u8; 16] =
    [0xCF, 0xAD, 0x12, 0xFE, 0xC5, 0xFD, 0x74, 0x6F, 0x66, 0xE3, 0xD1, 0x11, 0x9A, 0x4E, 0x00, 0xC0];
const ITEM_COUNT: u64 = 0xC4;
const INDEX_POINTER: u64 = 0xE4;
const NODE_HEADER: u64 = 24;
const NODE_ENTRY: u64 = 12;
const INFO_HEADER: u64 = 12;
const ATTR_MESSAGE_OFFSET_DIRECT: u8 = 0x84;
const ATTR_MESSAGE_OFFSET_POINTER: u8 = 0x04;
const BLOCK_HEADER: u64 = 16;
const BLOCK_CAPACITY: u32 = 0x200;
const MAX_TREE_DEPTH: usize = 64;
const SCAN_CHUNK: usize = 1 << 20;

/// Read every message in the folder file and hand it to `emit` with the folder path.
/// A file that is not a mail folder (Folders.dbx, Offline.dbx, anything else) gives 0 messages.
pub fn read(
    path: &Path,
    emit: &mut Emit<'_>,
) -> Result<ReadStats, String> {
    let mut stats = ReadStats { messages: 0, skipped: Vec::new() };
    let file = File::open(path).map_err(|e| format!("DBX cannot be opened: {e}"))?;
    let len = file.metadata().map_err(|e| format!("DBX cannot be read: {e}"))?.len();
    let mut dbx = Dbx { file: BufReader::new(file), len };
    let mut signature = [0u8; 16];
    if len < INDEX_POINTER + 4 || dbx.read_exact_at(0, &mut signature).is_err() || signature != MAIL_SIGNATURE {
        return Ok(stats);
    }
    let folder = vec![path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()];
    let skip = |stats: &mut ReadStats, reason: &str| stats.skipped.push((folder.join("/"), reason.to_string()));

    let count = dbx.u32_at(ITEM_COUNT).map_err(|e| format!("DBX cannot be read: {e}"))?;
    if count == 0 {
        return Ok(stats);
    }
    let root = dbx.u32_at(INDEX_POINTER).map_err(|e| format!("DBX cannot be read: {e}"))?;
    let mut infos = Vec::new();
    let tree_ok = dbx.index_tree(u64::from(root), 0, &mut HashSet::new(), &mut infos).is_ok();
    if tree_ok {
        for info in infos {
            let text = match dbx.message_offset(info) {
                Ok(0) => Err("a message was never downloaded (only its header line is in the index)"),
                Ok(offset) => dbx.chain(offset).map_err(|_| "a message is damaged"),
                Err(_) => Err("a message index entry is damaged"),
            };
            match text {
                Ok(raw) => {
                    emit(&folder, raw)?;
                    stats.messages += 1;
                }
                Err(reason) => skip(&mut stats, reason),
            }
        }
        return Ok(stats);
    }

    skip(&mut stats, "the folder index is damaged; messages were recovered by scanning the file");
    for chain in dbx.scan().map_err(|e| format!("DBX cannot be read: {e}"))? {
        match chain {
            Ok(raw) if looks_like_message(&raw) => {
                emit(&folder, raw)?;
                stats.messages += 1;
            }
            Ok(_) => {}
            Err(()) => skip(&mut stats, "a damaged message could not be recovered"),
        }
    }
    Ok(stats)
}

struct Dbx {
    file: BufReader<File>,
    len: u64,
}

#[derive(Debug, Clone, Copy)]
struct Fragment {
    offset: u64,
    len: u64,
    next: u64,
}

impl Dbx {
    fn read_exact_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        if offset.saturating_add(buf.len() as u64) > self.len {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "beyond the end of the file"));
        }
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.read_exact(buf)
    }

    fn u32_at(&mut self, offset: u64) -> io::Result<u32> {
        let mut b = [0u8; 4];
        self.read_exact_at(offset, &mut b)?;
        Ok(u32::from_le_bytes(b))
    }

    fn damaged() -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, "damaged index")
    }

    /// Walk the index tree in order and collect the message info offsets.
    fn index_tree(
        &mut self,
        pos: u64,
        depth: usize,
        seen: &mut HashSet<u64>,
        out: &mut Vec<u64>,
    ) -> io::Result<()> {
        if pos == 0 || pos >= self.len || depth > MAX_TREE_DEPTH || !seen.insert(pos) {
            return Err(Self::damaged());
        }
        let mut node = [0u8; NODE_HEADER as usize];
        self.read_exact_at(pos, &mut node)?;
        let word = |at: usize| u32::from_le_bytes([node[at], node[at + 1], node[at + 2], node[at + 3]]);
        let entries = u64::from(node[17]);
        if u64::from(word(0)) != pos || entries == 0 {
            return Err(Self::damaged());
        }
        if word(20) > 0 {
            self.index_tree(u64::from(word(8)), depth + 1, seen, out)?;
        }
        for k in 0..entries {
            let mut entry = [0u8; NODE_ENTRY as usize];
            self.read_exact_at(pos + NODE_HEADER + k * NODE_ENTRY, &mut entry)?;
            let word = |at: usize| u32::from_le_bytes([entry[at], entry[at + 1], entry[at + 2], entry[at + 3]]);
            out.push(u64::from(word(0)));
            if word(8) > 0 {
                self.index_tree(u64::from(word(4)), depth + 1, seen, out)?;
            }
        }
        Ok(())
    }

    /// The file offset of the message text, or 0 when only the header line was downloaded.
    fn message_offset(&mut self, info: u64) -> io::Result<u64> {
        let mut head = [0u8; INFO_HEADER as usize];
        self.read_exact_at(info, &mut head)?;
        let count = u64::from(head[10]);
        for k in 0..count {
            let attr = self.u32_at(info + INFO_HEADER + 4 * k)?;
            let (kind, value) = ((attr & 0xFF) as u8, u64::from(attr >> 8));
            if kind == ATTR_MESSAGE_OFFSET_DIRECT {
                return Ok(value);
            }
            if kind == ATTR_MESSAGE_OFFSET_POINTER {
                return Ok(u64::from(self.u32_at(info + INFO_HEADER + value + 4 * count)?));
            }
        }
        Ok(0)
    }

    /// Join the data of a block chain.
    fn chain(&mut self, first: u64) -> io::Result<Vec<u8>> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut at = first;
        while at != 0 {
            if !seen.insert(at) {
                return Err(Self::damaged());
            }
            let mut head = [0u8; BLOCK_HEADER as usize];
            self.read_exact_at(at, &mut head)?;
            let size = u64::from(u16::from_le_bytes([head[8], head[9]]));
            if size == 0 || size > u64::from(BLOCK_CAPACITY) {
                return Err(Self::damaged());
            }
            let start = out.len();
            out.resize(start + size as usize, 0);
            self.read_exact_at(at + BLOCK_HEADER, &mut out[start..])?;
            at = u64::from(u32::from_le_bytes([head[12], head[13], head[14], head[15]]));
        }
        Ok(out)
    }

    /// Recovery: find every block header in the file and join the chains, in file order.
    /// `Err(())` is a chain whose next block is missing.
    fn scan(&mut self) -> io::Result<Vec<Result<Vec<u8>, ()>>> {
        let fragments = self.scan_fragments()?;
        let by_offset: HashMap<u64, Fragment> = fragments.iter().map(|f| (f.offset, *f)).collect();
        let pointed: HashSet<u64> = fragments.iter().map(|f| f.next).filter(|n| by_offset.contains_key(n)).collect();
        let mut used = HashSet::new();
        let mut chains = Vec::new();
        for head in fragments.iter().filter(|f| !pointed.contains(&f.offset)) {
            let mut data = Vec::new();
            let mut current = Some(*head);
            let mut complete = true;
            while let Some(f) = current {
                if !used.insert(f.offset) {
                    complete = false;
                    break;
                }
                let start = data.len();
                data.resize(start + f.len as usize, 0);
                self.read_exact_at(f.offset + BLOCK_HEADER, &mut data[start..])?;
                current = match f.next {
                    0 => None,
                    next => match by_offset.get(&next) {
                        Some(n) => Some(*n),
                        None => {
                            complete = false;
                            None
                        }
                    },
                };
            }
            chains.push(if complete { Ok(data) } else { Err(()) });
        }
        Ok(chains)
    }

    /// Block headers as UnDBX recognises them: the first word is the block's own offset, the
    /// second 0x200, the third a length of 1..=0x200, the fourth a 4-aligned offset inside the
    /// file that is not the block itself.
    fn scan_fragments(&mut self) -> io::Result<Vec<Fragment>> {
        let mut fragments = Vec::new();
        let mut buf = vec![0u8; SCAN_CHUNK + BLOCK_HEADER as usize];
        let mut pos: u64 = 0;
        let (mut chunk_start, mut loaded_end): (u64, u64) = (0, 0);
        while pos + BLOCK_HEADER <= self.len {
            if pos < chunk_start || pos + BLOCK_HEADER > loaded_end {
                chunk_start = pos;
                let n = (self.len - pos).min(buf.len() as u64) as usize;
                self.read_exact_at(pos, &mut buf[..n])?;
                loaded_end = pos + n as u64;
            }
            let at = (pos - chunk_start) as usize;
            let word = |k: usize| u64::from(u32::from_le_bytes([buf[at + k], buf[at + k + 1], buf[at + k + 2], buf[at + k + 3]]));
            let (own, capacity, len, next) = (word(0), word(4), word(8), word(12));
            let is_block = own == pos
                && capacity == u64::from(BLOCK_CAPACITY)
                && (1..=u64::from(BLOCK_CAPACITY)).contains(&len)
                && next < self.len
                && next % 4 == 0
                && next != own;
            if is_block {
                fragments.push(Fragment { offset: pos, len, next });
                pos += BLOCK_HEADER + u64::from(BLOCK_CAPACITY);
            } else {
                pos += 4;
            }
        }
        Ok(fragments)
    }
}

/// A recovered chain is kept only when it starts like a header field ("Name: ...").
fn looks_like_message(raw: &[u8]) -> bool {
    let line = raw.split(|&b| b == b'\n').next().unwrap_or_default();
    match line.iter().position(|&b| b == b':') {
        Some(colon) => colon > 0 && line[..colon].iter().all(|b| b.is_ascii_graphic()),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FOLDERS_SIGNATURE: [u8; 16] =
        [0xCF, 0xAD, 0x12, 0xFE, 0xC6, 0xFD, 0x74, 0x6F, 0x66, 0xE3, 0xD1, 0x11, 0x9A, 0x4E, 0x00, 0xC0];
    const HEADER_SIZE: usize = 0x24BC;

    fn put(buf: &mut Vec<u8>, at: usize, value: u32) {
        if buf.len() < at + 4 {
            buf.resize(at + 4, 0);
        }
        buf[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn align(buf: &mut Vec<u8>) -> usize {
        while !buf.len().is_multiple_of(4) {
            buf.push(0);
        }
        buf.len()
    }

    /// Store `text` as a block chain at the end of `buf`; returns the first block's offset.
    fn add_chain(buf: &mut Vec<u8>, text: &[u8]) -> usize {
        let mut offsets = Vec::new();
        for chunk in text.chunks(BLOCK_CAPACITY as usize) {
            let at = align(buf);
            offsets.push(at);
            put(buf, at, at as u32);
            put(buf, at + 4, BLOCK_CAPACITY);
            put(buf, at + 8, chunk.len() as u32);
            put(buf, at + 12, 0);
            buf.extend_from_slice(chunk);
            buf.resize(at + 16 + BLOCK_CAPACITY as usize, 0);
        }
        for pair in offsets.windows(2) {
            put(buf, pair[0] + 12, pair[1] as u32);
        }
        offsets[0]
    }

    /// Message info with one attribute: direct (0x84) or through a pointer (0x04).
    fn add_info(buf: &mut Vec<u8>, message: usize, direct: bool) -> usize {
        let at = align(buf);
        put(buf, at, at as u32);
        put(buf, at + 4, 8);
        put(buf, at + 8, 1 << 16); // attribute count 1 at +10
        if direct {
            put(buf, at + 12, ((message as u32) << 8) | u32::from(ATTR_MESSAGE_OFFSET_DIRECT));
        } else {
            put(buf, at + 12, u32::from(ATTR_MESSAGE_OFFSET_POINTER));
            put(buf, at + 16, message as u32);
        }
        at
    }

    fn sample(messages: &[&[u8]]) -> Vec<u8> {
        let mut buf = vec![0u8; HEADER_SIZE];
        buf[..16].copy_from_slice(&MAIL_SIGNATURE);
        let infos: Vec<usize> = messages
            .iter()
            .enumerate()
            .map(|(n, m)| {
                let chain = add_chain(&mut buf, m);
                add_info(&mut buf, chain, n % 2 == 0)
            })
            .collect();
        let node = align(&mut buf);
        put(&mut buf, node, node as u32);
        buf.resize(node + NODE_HEADER as usize, 0);
        buf[node + 17] = infos.len() as u8;
        for info in infos {
            let at = buf.len();
            put(&mut buf, at, info as u32);
            put(&mut buf, at + 4, 0);
            put(&mut buf, at + 8, 0);
        }
        put(&mut buf, ITEM_COUNT as usize, messages.len() as u32);
        put(&mut buf, INDEX_POINTER as usize, node as u32);
        buf
    }

    fn first() -> Vec<u8> {
        let mut m = b"From: Oma <oma@example.nl>\r\nSubject: Lang bericht\r\n\r\n".to_vec();
        m.extend(std::iter::repeat_n(b'x', 900));
        m.extend_from_slice(b"\r\nEinde\r\n");
        m
    }

    const SECOND: &[u8] = b"From: Opa <opa@example.nl>\r\nSubject: Kort\r\n\r\nHoi\r\n";

    type Got = Vec<(Vec<String>, Vec<u8>)>;

    fn read_all(path: &Path) -> (Result<ReadStats, String>, Got) {
        let mut got = Vec::new();
        let stats = read(path, &mut |folder, raw| {
            got.push((folder.to_vec(), raw));
            Ok(())
        });
        (stats, got)
    }

    #[test]
    fn reads_both_messages_through_the_index() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Postvak IN.dbx");
        std::fs::write(&path, sample(&[&first(), SECOND])).unwrap();
        let (stats, got) = read_all(&path);
        let stats = stats.unwrap();
        assert_eq!(stats.messages, 2);
        assert!(stats.skipped.is_empty());
        assert_eq!(got[0].0, vec!["Postvak IN".to_string()]);
        assert_eq!(got[0].1, first());
        assert_eq!(got[1].1, SECOND);
    }

    #[test]
    fn recovers_by_scanning_when_the_index_is_damaged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Verzonden items.dbx");
        let mut file = sample(&[&first(), SECOND]);
        put(&mut file, INDEX_POINTER as usize, 0x0FFF_FFF0);
        std::fs::write(&path, file).unwrap();
        let (stats, got) = read_all(&path);
        let stats = stats.unwrap();
        assert_eq!(stats.messages, 2);
        assert_eq!(stats.skipped.len(), 1);
        assert_eq!(stats.skipped[0].0, "Verzonden items");
        assert_eq!(got[0].1, first());
        assert_eq!(got[1].1, SECOND);
    }

    #[test]
    fn a_message_without_text_is_skipped_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Nieuws.dbx");
        let mut file = sample(&[SECOND, SECOND]);
        // Point the first info's direct attribute at offset 0 (header only, not downloaded).
        let node = u32::from_le_bytes(file[0xE4..0xE8].try_into().unwrap()) as usize;
        let info = u32::from_le_bytes(file[node + 24..node + 28].try_into().unwrap()) as usize;
        put(&mut file, info + 12, u32::from(ATTR_MESSAGE_OFFSET_DIRECT));
        std::fs::write(&path, file).unwrap();
        let (stats, got) = read_all(&path);
        let stats = stats.unwrap();
        assert_eq!(stats.messages, 1);
        assert_eq!(stats.skipped.len(), 1);
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn folders_dbx_and_other_files_give_no_messages() {
        let dir = tempfile::tempdir().unwrap();
        let folders = dir.path().join("Folders.dbx");
        let mut file = sample(&[SECOND]);
        file[..16].copy_from_slice(&FOLDERS_SIGNATURE);
        std::fs::write(&folders, file).unwrap();
        let other = dir.path().join("brief.dbx");
        std::fs::write(&other, b"geen outlook express").unwrap();
        for path in [folders, other] {
            let (stats, got) = read_all(&path);
            assert_eq!(stats.unwrap().messages, 0);
            assert!(got.is_empty());
        }
    }

    #[test]
    fn an_error_from_emit_stops_the_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Postvak IN.dbx");
        std::fs::write(&path, sample(&[SECOND, SECOND])).unwrap();
        let mut calls = 0;
        let result = read(&path, &mut |_, _| {
            calls += 1;
            Err("schijf vol".to_string())
        });
        assert_eq!(result.err().as_deref(), Some("schijf vol"));
        assert_eq!(calls, 1);
    }

    #[test]
    fn a_looping_chain_is_damaged_not_endless() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Lus.dbx");
        let mut file = sample(&[SECOND]);
        let node = u32::from_le_bytes(file[0xE4..0xE8].try_into().unwrap()) as usize;
        let info = u32::from_le_bytes(file[node + 24..node + 28].try_into().unwrap()) as usize;
        let block = (u32::from_le_bytes(file[info + 12..info + 16].try_into().unwrap()) >> 8) as usize;
        put(&mut file, block + 12, block as u32); // the block points to itself
        std::fs::write(&path, file).unwrap();
        let (stats, _) = read_all(&path);
        let stats = stats.unwrap();
        assert_eq!(stats.messages, 0);
        assert_eq!(stats.skipped.len(), 1);
    }
}
