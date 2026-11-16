//! Turning a draft from the Write screen into a MIME message (RFC 5322).
//!
//! The screen fills in the signature and the quoted text of a reply; this module only builds
//! the message. Attachments are read from the user's files, read-only.

use std::path::Path;

use mail_builder::MessageBuilder;
use mail_builder::headers::address::Address;
use mail_builder::headers::message_id::MessageId;

use crate::error::{Code, Result, VemError};
use crate::model::{Addr, Draft};

/// Photos larger than this are made smaller when "Foto's kleiner maken" is on (C1).
pub const SHRINK_ABOVE_BYTES: usize = 1024 * 1024;
/// The long side of a shrunk photo, in pixels.
pub const SHRINK_LONG_SIDE: u32 = 1600;
const SHRINK_JPEG_QUALITY: u8 = 80;

#[derive(Debug, Clone)]
pub struct Built {
    pub raw: Vec<u8>,
    /// Every envelope recipient: To, Cc and Bcc (Bcc is not in the headers).
    pub recipients: Vec<String>,
    pub message_id: String,
}

/// A plain address check: one "@", no spaces, no characters that could break a header.
pub fn valid_email(email: &str) -> bool {
    let e = email.trim();
    let Some((local, domain)) = e.split_once('@') else { return false };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains('@')
        && !e.chars().any(|c| c.is_whitespace() || c.is_control() || "<>,;\"()[]\\".contains(c))
}

fn clean_name(name: &Option<String>) -> Option<String> {
    name.as_ref()
        .map(|n| n.chars().filter(|c| !c.is_control()).collect::<String>().trim().to_string())
        .filter(|n| !n.is_empty())
}

fn address_list(list: &[Addr]) -> Address<'static> {
    Address::new_list(
        list.iter().map(|a| Address::new_address(clean_name(&a.name), a.email.trim().to_string())).collect(),
    )
}

/// The id inside a Message-ID header, without the angle brackets.
fn bare_id(id: &str) -> String {
    id.trim().trim_start_matches('<').trim_end_matches('>').to_string()
}

pub fn new_message_id(from_email: &str) -> String {
    let domain = from_email.rsplit_once('@').map(|(_, d)| d).filter(|d| valid_email(&format!("x@{d}"))).unwrap_or("vem.local");
    format!("{}@{domain}", uuid::Uuid::new_v4().simple())
}

/// Build the message. `date` is Unix seconds. Fails when a recipient address is not valid or an
/// attachment cannot be read.
pub fn build(draft: &Draft, from: &Addr, date: i64) -> Result<Built> {
    let all: Vec<&Addr> = draft.to.iter().chain(&draft.cc).chain(&draft.bcc).collect();
    if let Some(bad) = all.iter().find(|a| !valid_email(&a.email)) {
        return Err(VemError::new(Code::SmtpCmd01).with_detail(bad.email.clone()));
    }
    let mut recipients: Vec<String> = Vec::new();
    for a in &all {
        let e = a.email.trim().to_string();
        if !recipients.iter().any(|r| r.eq_ignore_ascii_case(&e)) {
            recipients.push(e);
        }
    }
    let message_id = new_message_id(&from.email);
    let subject: String = draft.subject.chars().filter(|c| !c.is_control()).collect();
    let body = draft.body_text.replace("\r\n", "\n");

    let mut b = MessageBuilder::new()
        .from(Address::new_address(clean_name(&from.name), from.email.trim().to_string()))
        .subject(subject)
        .date(date)
        .message_id(message_id.clone())
        .header("X-Mailer", mail_builder::headers::raw::Raw::new("Vorige Eeuw Mail"))
        .text_body(body);
    if !draft.to.is_empty() {
        b = b.to(address_list(&draft.to));
    }
    if !draft.cc.is_empty() {
        b = b.cc(address_list(&draft.cc));
    }
    if let Some(irt) = draft.in_reply_to.as_deref().filter(|s| !s.trim().is_empty()) {
        b = b.in_reply_to(MessageId::new(bare_id(irt)));
    }
    let refs: Vec<String> = draft.references.iter().map(|r| bare_id(r)).filter(|r| !r.is_empty()).collect();
    if !refs.is_empty() {
        b = b.references(MessageId::from(refs));
    }
    for path in &draft.attachments {
        let (bytes, name, mime) = read_attachment(Path::new(path), draft.shrink_photos)?;
        b = b.attachment(mime, name, bytes);
    }
    let raw = b.write_to_vec().map_err(|e| VemError::new(Code::AppCmd01).with_detail(e.kind().to_string()))?;
    Ok(Built { raw, recipients, message_id: format!("<{message_id}>") })
}

fn read_attachment(path: &Path, shrink: bool) -> Result<(Vec<u8>, String, String)> {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "bijlage".into());
    let bytes = std::fs::read(path).map_err(|e| {
        VemError::new(Code::StoreRead01).with_detail(format!("{name}: {}", e.kind()))
    })?;
    let mime = mime_for(&name).to_string();
    if shrink {
        if let Some(small) = shrink_photo(&bytes) {
            let jpg_name = match name.rsplit_once('.') {
                Some((stem, _)) => format!("{stem}.jpg"),
                None => format!("{name}.jpg"),
            };
            return Ok((small, jpg_name, "image/jpeg".into()));
        }
    }
    Ok((bytes, name, mime))
}

pub fn mime_for(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "heic" => "image/heic",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "zip" => "application/zip",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
}

/// A smaller JPEG of a large photo, turned upright, or None when it is small already, not a
/// JPEG or PNG, or would not get smaller.
pub fn shrink_photo(bytes: &[u8]) -> Option<Vec<u8>> {
    use image::{ImageDecoder, ImageReader};
    if bytes.len() <= SHRINK_ABOVE_BYTES {
        return None;
    }
    let reader = ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().ok()?;
    if !matches!(reader.format(), Some(image::ImageFormat::Jpeg | image::ImageFormat::Png)) {
        return None;
    }
    let mut decoder = reader.into_decoder().ok()?;
    let orientation = decoder.orientation().ok();
    let mut img = image::DynamicImage::from_decoder(decoder).ok()?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    if img.width().max(img.height()) > SHRINK_LONG_SIDE {
        img = img.resize(SHRINK_LONG_SIDE, SHRINK_LONG_SIDE, image::imageops::FilterType::Triangle);
    }
    let rgb = img.to_rgb8();
    let mut out = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, SHRINK_JPEG_QUALITY);
    rgb.write_with_encoder(enc).ok()?;
    (out.len() < bytes.len()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn draft() -> Draft {
        Draft {
            account_id: "a".into(),
            to: vec![Addr::new(Some("Oma Jansen".into()), "oma@example.nl")],
            cc: vec![Addr::new(None, "tante@example.nl")],
            bcc: vec![Addr::new(None, "geheim@example.nl"), Addr::new(None, "OMA@example.nl")],
            subject: "Foto's van zaterdag\r\nBcc: evil@x.nl".into(),
            body_text: "Hoi oma,\n\nHier zijn de foto's.\n.\nGroet".into(),
            in_reply_to: Some("<abc@example.nl>".into()),
            references: vec!["<root@example.nl>".into(), "<abc@example.nl>".into()],
            ..Default::default()
        }
    }

    #[test]
    fn builds_a_message_that_parses_back() {
        let b = build(&draft(), &Addr::new(Some("Jan".into()), "jan@example.nl"), 1_790_000_000).unwrap();
        let p = parse::parse(&b.raw);
        assert_eq!(p.subject, "Foto's van zaterdagBcc: evil@x.nl");
        assert_eq!(p.from.email, "jan@example.nl");
        assert_eq!(p.to[0].name.as_deref(), Some("Oma Jansen"));
        assert_eq!(p.cc[0].email, "tante@example.nl");
        assert_eq!(p.in_reply_to.as_deref(), Some("abc@example.nl"));
        assert!(p.text.unwrap().contains("Hier zijn de foto's."));
        let raw = String::from_utf8_lossy(&b.raw);
        assert!(!raw.contains("geheim@example.nl"), "Bcc must not be in the headers");
        assert!(!raw.lines().any(|l| l.starts_with("Bcc:")));
        assert_eq!(b.recipients, vec!["oma@example.nl", "tante@example.nl", "geheim@example.nl"]);
        assert!(b.message_id.ends_with("@example.nl>"));
    }

    #[test]
    fn refuses_bad_addresses() {
        let mut d = draft();
        d.to.push(Addr::new(None, "jan@x.nl>\r\nBcc: y@z.nl"));
        let e = build(&d, &Addr::new(None, "jan@example.nl"), 0).unwrap_err();
        assert_eq!(e.code, "VEM-SMTP-CMD-01");
        assert!(!valid_email("geen-apenstaartje"));
        assert!(!valid_email("a@b"));
        assert!(valid_email("piet.de.vries+mail@ziggo.nl"));
    }

    #[test]
    fn attachments_are_read_and_named() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("brief.pdf");
        std::fs::write(&path, b"%PDF-1.4 test").unwrap();
        let mut d = draft();
        d.attachments = vec![path.to_string_lossy().into()];
        let b = build(&d, &Addr::new(None, "jan@example.nl"), 0).unwrap();
        let p = parse::parse(&b.raw);
        assert_eq!(p.attachments.len(), 1);
        assert_eq!(p.attachments[0].name, "brief.pdf");
        assert_eq!(p.attachments[0].mime, "application/pdf");
        assert_eq!(parse::attachment_bytes(&b.raw, 0).unwrap(), b"%PDF-1.4 test");
        assert!(path.exists(), "the user's file is left alone");

        d.attachments = vec![dir.path().join("weg.pdf").to_string_lossy().into()];
        assert_eq!(build(&d, &Addr::new(None, "jan@example.nl"), 0).unwrap_err().code, "VEM-STORE-READ-01");
    }

    #[test]
    fn large_photo_is_shrunk() {
        // Noise does not compress, so a 2000x1500 PNG is well over 1 MB.
        let mut seed: u32 = 7;
        let img = image::RgbImage::from_fn(2000, 1500, |_, _| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            image::Rgb([(seed >> 16) as u8, (seed >> 8) as u8, seed as u8])
        });
        let mut png = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        assert!(png.len() > SHRINK_ABOVE_BYTES);
        let small = shrink_photo(&png).expect("shrunk");
        let back = image::load_from_memory(&small).unwrap();
        assert_eq!((back.width(), back.height()), (1600, 1200));
        assert!(shrink_photo(b"small").is_none());
    }
}
