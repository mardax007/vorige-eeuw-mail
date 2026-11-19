//! Turn raw RFC 5322 bytes into the fields the index and the reading pane need.

use mail_parser::{AddressList, MessageParser, PartKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Addr {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub email: String,
}

impl Addr {
    pub fn new(name: Option<&str>, email: &str) -> Self {
        Addr {
            name: name.map(str::trim).filter(|n| !n.is_empty()).map(str::to_string),
            email: email.trim().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentInfo {
    /// Index in the attachment list of the message; stable for the same bytes.
    pub index: usize,
    pub name: String,
    pub size: usize,
    pub mime: String,
    pub content_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedMail {
    pub subject: String,
    pub from: Addr,
    pub to: Vec<Addr>,
    pub cc: Vec<Addr>,
    pub reply_to: Vec<Addr>,
    /// Unix seconds; 0 when the message has no usable date.
    pub date: i64,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub text: Option<String>,
    pub html: Option<String>,
    pub preview: String,
    pub attachments: Vec<AttachmentInfo>,
    /// Lower-case addresses from Delivered-To, X-Original-To and the like: where the server
    /// delivered the message, also when that address is not in To or Cc (BCC, lists).
    pub delivered_to: Vec<String>,
}

/// Headers in which mail servers note the address a message was delivered to.
const DELIVERY_HEADERS: [&str; 5] = ["delivered-to", "x-original-to", "x-delivered-to", "envelope-to", "x-envelope-to"];

/// Every address in the delivery headers, lower case, without duplicates.
fn delivered_to(msg: &mail_parser::Message<'_>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for h in msg.headers().iter() {
        if !DELIVERY_HEADERS.iter().any(|n| h.raw_name().eq_ignore_ascii_case(n)) {
            continue;
        }
        let value = String::from_utf8_lossy(h.raw_value());
        for word in value.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
            let a = word.trim_matches(|c: char| matches!(c, '<' | '>' | '"' | '\'')).to_lowercase();
            if a.contains('@') && crate::compose::valid_email(&a) && !out.contains(&a) {
                out.push(a);
            }
        }
    }
    out
}

pub const PREVIEW_LEN: usize = 200;

fn addrs(list: Option<AddressList<'_>>) -> Vec<Addr> {
    list.map(|l| {
        l.mailboxes()
            .filter_map(|m| m.address().map(|a| Addr::new(m.name(), a)))
            .collect()
    })
    .unwrap_or_default()
}

/// Parse a message. Never fails: damaged mail still gives whatever could be read.
pub fn parse(raw: &[u8]) -> ParsedMail {
    let parser = MessageParser::new();
    let Some(msg) = parser.parse(raw) else {
        return ParsedMail::default();
    };
    let from = addrs(msg.from()).into_iter().next().unwrap_or_default();
    let html = msg
        .html_body()
        .find(|p| matches!(p.kind(), PartKind::Html))
        .and_then(|p| p.text())
        .map(|c| c.into_owned());
    let text = {
        let parts: Vec<String> = msg
            .text_body()
            .filter(|p| matches!(p.kind(), PartKind::Text))
            .filter_map(|p| p.text().map(|c| c.into_owned()))
            .collect();
        if parts.is_empty() { None } else { Some(parts.join("\n")) }
    };
    let preview = msg
        .root()
        .body_preview(PREVIEW_LEN * 2)
        .map(|p| collapse_space(&p, PREVIEW_LEN))
        .unwrap_or_default();
    let attachments = msg
        .attachments()
        .enumerate()
        .map(|(index, part)| {
            let mime = part
                .content_type()
                .map(|ct| match ct.subtype() {
                    Some(sub) => format!("{}/{}", ct.ctype(), sub),
                    None => ct.ctype().to_string(),
                })
                .unwrap_or_else(|| "application/octet-stream".into())
                .to_ascii_lowercase();
            let name = part
                .attachment_name()
                .map(str::to_string)
                .or_else(|| part.nested().and_then(|n| n.subject().map(|s| format!("{s}.eml"))))
                .unwrap_or_else(|| format!("bijlage-{}", index + 1));
            AttachmentInfo {
                index,
                name: safe_file_name(&name),
                size: part.decoded_len(),
                mime,
                content_id: part.content_id().map(|c| c.trim_matches(['<', '>']).to_string()),
            }
        })
        .collect();
    ParsedMail {
        subject: msg.subject().unwrap_or_default().trim().to_string(),
        from,
        to: addrs(msg.to()),
        cc: addrs(msg.cc()),
        reply_to: addrs(msg.reply_to()),
        date: msg.date().map(|d| d.to_timestamp()).unwrap_or(0),
        message_id: msg.message_id().map(str::to_string),
        in_reply_to: msg.in_reply_to().and_then(|l| l.first().map(str::to_string)),
        references: msg
            .references()
            .map(|l| l.iter().map(str::to_string).collect())
            .unwrap_or_default(),
        text,
        html,
        preview,
        attachments,
        delivered_to: delivered_to(&msg),
    }
}

/// The decoded bytes of one attachment, by its index in [`ParsedMail::attachments`].
pub fn attachment_bytes(raw: &[u8], index: usize) -> Option<Vec<u8>> {
    let parser = MessageParser::new();
    let msg = parser.parse(raw)?;
    let part = msg.attachments().nth(index)?;
    if part.is_message() {
        // A forwarded message: hand it out as the original .eml bytes.
        return part.nested().map(|n| n.raw().to_vec());
    }
    Some(part.decoded().into_owned())
}

/// Inline images referenced by `cid:`, for the reading pane: (content id, mime, bytes).
pub fn inline_images(raw: &[u8]) -> Vec<(String, String, Vec<u8>)> {
    let parser = MessageParser::new();
    let Some(msg) = parser.parse(raw) else { return Vec::new() };
    msg.parts()
        .filter_map(|p| {
            let cid = p.content_id()?.trim_matches(['<', '>']).to_string();
            let ct = p.content_type()?;
            if !ct.ctype().eq_ignore_ascii_case("image") {
                return None;
            }
            let mime = format!("image/{}", ct.subtype().unwrap_or("png")).to_ascii_lowercase();
            Some((cid, mime, p.decoded().into_owned()))
        })
        .collect()
}

fn collapse_space(s: &str, max_chars: usize) -> String {
    let mut out = String::with_capacity(max_chars);
    let mut last_space = true;
    for ch in s.chars() {
        if out.chars().count() >= max_chars {
            break;
        }
        if ch.is_whitespace() {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.push(ch);
            last_space = false;
        }
    }
    out.trim_end().to_string()
}

/// A file name that is safe to write on Windows: no path parts, no reserved characters.
pub fn safe_file_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base
        .chars()
        .map(|c| if c.is_control() || r#"<>:"/\|?*"#.contains(c) { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches(['.', ' ']).to_string();
    let reserved = ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "LPT1", "LPT2"];
    let stem = trimmed.split('.').next().unwrap_or("").to_ascii_uppercase();
    if trimmed.is_empty() || reserved.contains(&stem.as_str()) {
        format!("bijlage_{trimmed}")
    } else {
        trimmed.chars().take(150).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_headers_give_the_address_the_mail_came_in_on() {
        let raw = b"Delivered-To: <Info@Familie.nl>
X-Original-To: info@familie.nl, Oma@familie.nl
To: lijst@club.nl
Subject: x

hoi
";
        assert_eq!(parse(raw).delivered_to, ["info@familie.nl", "oma@familie.nl"]);
        assert!(parse(b"To: a@b.nl

x").delivered_to.is_empty());
    }

    const SAMPLE: &[u8] = b"From: =?utf-8?q?Jan_de_Vr=C3=AFes?= <jan@example.nl>\r\n\
To: Ann <ann@example.com>, bob@example.com\r\n\
Subject: Verjaardag\r\n\
Date: Mon, 16 Nov 2026 10:00:00 +0100\r\n\
Message-ID: <abc@example.nl>\r\n\
References: <r1@x> <r2@x>\r\n\
In-Reply-To: <r2@x>\r\n\
MIME-Version: 1.0\r\n\
Content-Type: multipart/mixed; boundary=b\r\n\
\r\n\
--b\r\n\
Content-Type: text/plain; charset=utf-8\r\n\
\r\n\
Hallo   allemaal,\r\nTot zaterdag!\r\n\
--b\r\n\
Content-Type: image/jpeg; name=\"../foto:1.jpg\"\r\n\
Content-Disposition: attachment; filename=\"../foto:1.jpg\"\r\n\
Content-Transfer-Encoding: base64\r\n\
\r\n\
/9j/4AAQ\r\n\
--b--\r\n";

    #[test]
    fn parses_headers_and_body() {
        let p = parse(SAMPLE);
        assert_eq!(p.subject, "Verjaardag");
        assert_eq!(p.from, Addr::new(Some("Jan de Vrïes"), "jan@example.nl"));
        assert_eq!(p.to.len(), 2);
        assert_eq!(p.to[1].email, "bob@example.com");
        assert_eq!(p.message_id.as_deref(), Some("abc@example.nl"));
        assert_eq!(p.in_reply_to.as_deref(), Some("r2@x"));
        assert_eq!(p.references, vec!["r1@x", "r2@x"]);
        assert!(p.text.unwrap().contains("Tot zaterdag"));
        assert_eq!(p.preview, "Hallo allemaal, Tot zaterdag!");
        assert!(p.date > 1_700_000_000);
    }

    #[test]
    fn attachments_get_safe_names_and_bytes() {
        let p = parse(SAMPLE);
        assert_eq!(p.attachments.len(), 1);
        assert_eq!(p.attachments[0].name, "foto_1.jpg");
        assert_eq!(p.attachments[0].mime, "image/jpeg");
        let bytes = attachment_bytes(SAMPLE, 0).unwrap();
        assert_eq!(&bytes[..3], &[0xff, 0xd8, 0xff]);
    }

    #[test]
    fn garbage_does_not_panic() {
        let p = parse(b"\x00\x01 not a mail");
        assert!(p.subject.is_empty() || !p.subject.is_empty());
    }

    #[test]
    fn reserved_names_are_prefixed() {
        assert_eq!(safe_file_name("CON.txt"), "bijlage_CON.txt");
        assert_eq!(safe_file_name("a\\b\\c.pdf"), "c.pdf");
    }
}
