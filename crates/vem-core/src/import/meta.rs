//! Read state, the star and tags of an imported message (phase 3: "APPEND keeps the original
//! date and read state", "Outlook categories become tags").
//!
//! Every reader hands over plain RFC 5322 bytes; what the old program knew about the message
//! travels in its header block:
//! - `X-VEM-Import-Status: unread, flagged`: our own line, written by the PST reader and removed
//!   again here, so it never reaches the index or a mailbox.
//! - `Keywords:` (RFC 5322): Outlook writes categories there; the PST reader does too.
//! - `X-Mozilla-Status` and `X-Mozilla-Keys`: Thunderbird's read state, star and tags in mbox.
//! - `X-Gmail-Labels`: Gmail Takeout's labels in mbox.
//!
//! A message without any of these is old mail that was read, without a star or tags.

/// Our own line, removed before the message is stored.
pub const STATUS_HEADER: &str = "X-VEM-Import-Status";

/// Thunderbird's five old default tags (`$label1`–`$label5`), with the names Thunderbird shows.
const THUNDERBIRD_LABELS: [(&str, &str); 5] =
    [("$label1", "Important"), ("$label2", "Work"), ("$label3", "Personal"), ("$label4", "To Do"), ("$label5", "Later")];

/// Gmail labels that describe a state or a system folder, never a tag.
const GMAIL_SYSTEM_LABELS: [&str; 12] =
    ["inbox", "sent", "opened", "unread", "starred", "important", "archived", "spam", "trash", "drafts", "chat", "draft"];

/// Thunderbird's message flags (nsMsgMessageFlags).
const MOZ_READ: u32 = 0x0001;
const MOZ_MARKED: u32 = 0x0004;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Meta {
    pub unread: bool,
    pub important: bool,
    pub tags: Vec<String>,
}

impl Meta {
    fn add_tag(&mut self, name: &str) {
        let name = name.trim();
        if !name.is_empty() && !self.tags.iter().any(|t| t.eq_ignore_ascii_case(name)) {
            self.tags.push(name.to_string());
        }
    }
}

/// The header line for a PST message's state, or None when there is nothing to say.
pub fn status_line(unread: bool, flagged: bool) -> Option<String> {
    let words: Vec<&str> = [(unread, "unread"), (flagged, "flagged")]
        .into_iter()
        .filter_map(|(on, word)| on.then_some(word))
        .collect();
    (!words.is_empty()).then(|| format!("{STATUS_HEADER}: {}\r\n", words.join(", ")))
}

/// A `Keywords:` line for tags or Outlook categories (encoded words where not ASCII), or None.
/// Names holding a comma or a line break are left out: they cannot be told apart again.
pub fn keywords_line<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<String> {
    use base64::Engine;
    let encoded: Vec<String> = names
        .into_iter()
        .map(str::trim)
        .filter(|n| !n.is_empty() && !n.contains([',', '\r', '\n']))
        .map(|n| {
            if n.is_ascii() {
                n.to_string()
            } else {
                format!("=?utf-8?B?{}?=", base64::engine::general_purpose::STANDARD.encode(n))
            }
        })
        .collect();
    (!encoded.is_empty()).then(|| format!("Keywords: {}\r\n", encoded.join(", ")))
}

/// The message without our status line, and what its headers say about it.
pub fn take(raw: Vec<u8>) -> (Vec<u8>, Meta) {
    let head_end = header_end(&raw);
    let fields = unfolded_fields(&raw[..head_end]);
    let mut meta = Meta::default();
    let mut has_status = false;
    for (name, value) in &fields {
        match name.to_ascii_lowercase().as_str() {
            "x-vem-import-status" => {
                has_status = true;
                for word in value.split(',').map(|w| w.trim().to_ascii_lowercase()) {
                    match word.as_str() {
                        "unread" => meta.unread = true,
                        "flagged" => meta.important = true,
                        _ => {}
                    }
                }
            }
            "keywords" => {
                for k in decode_words(value).split(',') {
                    meta.add_tag(k);
                }
            }
            "x-mozilla-status" => {
                if let Ok(flags) = u32::from_str_radix(value.trim(), 16) {
                    meta.unread = flags & MOZ_READ == 0;
                    meta.important |= flags & MOZ_MARKED != 0;
                }
            }
            "x-mozilla-keys" => {
                for k in value.split_whitespace() {
                    if let Some((_, shown)) = THUNDERBIRD_LABELS.iter().find(|(key, _)| key.eq_ignore_ascii_case(k)) {
                        meta.add_tag(shown);
                    } else if !k.starts_with('$') && !matches!(k.to_ascii_lowercase().as_str(), "junk" | "nonjunk") {
                        meta.add_tag(k);
                    }
                }
            }
            "x-gmail-labels" => gmail_labels(&decode_words(value), &mut meta),
            _ => {}
        }
    }
    let raw = if has_status { without_status(raw, head_end) } else { raw };
    (raw, meta)
}

fn gmail_labels(value: &str, meta: &mut Meta) {
    for label in split_quoted(value) {
        let lower = label.to_ascii_lowercase();
        match lower.as_str() {
            "unread" => meta.unread = true,
            "starred" => meta.important = true,
            _ if GMAIL_SYSTEM_LABELS.contains(&lower.as_str()) || lower.starts_with("category ") => {}
            _ => meta.add_tag(&label),
        }
    }
}

/// Comma-separated values, where a value in double quotes may hold a comma.
fn split_quoted(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in value.chars() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

/// RFC 2047 encoded words decoded, through mail-parser.
fn decode_words(value: &str) -> String {
    if !value.contains("=?") {
        return value.to_string();
    }
    let raw = format!("Subject: {value}\r\n\r\n");
    mail_parser::MessageParser::default()
        .parse_headers(raw.as_bytes())
        .and_then(|m| m.subject().map(str::to_string))
        .unwrap_or_else(|| value.to_string())
}

/// Where the header block ends (the blank line), or the whole message if there is none.
fn header_end(raw: &[u8]) -> usize {
    let crlf = raw.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 2);
    let lf = raw.windows(2).position(|w| w == b"\n\n").map(|p| p + 1);
    match (crlf, lf) {
        (Some(a), Some(b)) => a.min(b),
        (a, b) => a.or(b).unwrap_or(raw.len()),
    }
}

fn unfolded_fields(head: &[u8]) -> Vec<(String, String)> {
    let text = String::from_utf8_lossy(head);
    let mut fields: Vec<(String, String)> = Vec::new();
    for line in text.split('\n').map(|l| l.trim_end_matches('\r')) {
        if line.starts_with([' ', '\t']) {
            if let Some((_, v)) = fields.last_mut() {
                v.push(' ');
                v.push_str(line.trim());
            }
        } else if let Some((name, value)) = line.split_once(':') {
            fields.push((name.trim().to_string(), value.trim().to_string()));
        }
    }
    fields
}

/// The message with every `X-VEM-Import-Status` line (and its folded lines) taken out of the
/// header block. The body is not touched.
fn without_status(raw: Vec<u8>, head_end: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut skipping = false;
    let mut at = 0;
    while at < head_end {
        let end = raw[at..head_end].iter().position(|&b| b == b'\n').map(|p| at + p + 1).unwrap_or(head_end);
        let line = &raw[at..end];
        let folded = line.first().is_some_and(|b| *b == b' ' || *b == b'\t');
        if !folded {
            skipping = line.len() > STATUS_HEADER.len()
                && line[..STATUS_HEADER.len()].eq_ignore_ascii_case(STATUS_HEADER.as_bytes())
                && line[STATUS_HEADER.len()] == b':';
        }
        if !skipping {
            out.extend_from_slice(line);
        }
        at = end;
    }
    out.extend_from_slice(&raw[head_end..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta_of(head: &str) -> (String, Meta) {
        let raw = format!("{head}Subject: Hallo\r\n\r\nTekst\r\n");
        let (out, meta) = take(raw.into_bytes());
        (String::from_utf8(out).unwrap(), meta)
    }

    #[test]
    fn plain_old_mail_is_read_without_star_or_tags() {
        let (out, meta) = meta_of("From: a@b.nl\r\n");
        assert_eq!(meta, Meta::default());
        assert!(out.starts_with("From: a@b.nl\r\nSubject: Hallo"));
    }

    #[test]
    fn our_status_line_is_read_and_removed() {
        let line = status_line(true, true).unwrap();
        let (out, meta) = meta_of(&line);
        assert!(meta.unread && meta.important);
        assert!(!out.contains("X-VEM"), "{out}");
        assert!(out.ends_with("\r\n\r\nTekst\r\n"));
        assert_eq!(status_line(false, false), None);
    }

    #[test]
    fn status_line_in_the_body_is_left_alone() {
        let raw = "Subject: x\r\n\r\nX-VEM-Import-Status: unread\r\n";
        let (out, meta) = take(raw.as_bytes().to_vec());
        assert_eq!(String::from_utf8(out).unwrap(), raw);
        assert!(!meta.unread);
    }

    #[test]
    fn keywords_become_tags_also_encoded_ones() {
        let (_, meta) = meta_of("Keywords: Familie, =?utf-8?Q?Priv=C3=A9?=, familie\r\n");
        assert_eq!(meta.tags, vec!["Familie", "Privé"]);
    }

    #[test]
    fn thunderbird_status_and_keys() {
        let (_, meta) = meta_of("X-Mozilla-Status: 0004\r\nX-Mozilla-Keys: $label1 vakantie $Forwarded Junk\r\n");
        assert!(meta.unread, "read bit not set");
        assert!(meta.important);
        assert_eq!(meta.tags, vec!["Important", "vakantie"]);
        let (_, read) = meta_of("X-Mozilla-Status: 0001\r\n");
        assert!(!read.unread && !read.important);
    }

    #[test]
    fn gmail_labels_give_state_and_tags() {
        let (_, meta) = meta_of("X-Gmail-Labels: Inbox,Unread,Starred,Category Updates,\"Oma, opa\",Rekeningen\r\n");
        assert!(meta.unread && meta.important);
        assert_eq!(meta.tags, vec!["Oma, opa", "Rekeningen"]);
    }

    #[test]
    fn folded_status_line_is_removed_whole() {
        let raw = "X-VEM-Import-Status: unread,\r\n flagged\r\nSubject: x\r\n\r\nb";
        let (out, meta) = take(raw.as_bytes().to_vec());
        assert!(meta.unread && meta.important);
        assert_eq!(String::from_utf8(out).unwrap(), "Subject: x\r\n\r\nb");
    }

    #[test]
    fn lf_only_messages_work() {
        let (out, meta) = take(b"X-VEM-Import-Status: flagged\nSubject: x\n\nb\n".to_vec());
        assert!(meta.important);
        assert_eq!(out, b"Subject: x\n\nb\n");
    }
}
