//! The formatted text of the Write screen (decision 33): what may be in it, and turning the
//! photos in it into parts of the message.
//!
//! The screen edits the text as HTML in the app's own window, so whatever comes back from it, or
//! from a saved draft, is cleaned here before it is kept or sent: only simple formatting and
//! photos stored inside the text (`data:` URLs) stay. Scripts, links, frames and outside pictures
//! never get in.

use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use base64::Engine;

/// The formatting the toolbar makes, and the plain blocks the editor writes around lines.
const TAGS: &[&str] = &["p", "div", "br", "b", "strong", "i", "em", "u", "ul", "ol", "li", "span", "font", "img"];
const STYLE_PROPERTIES: &[&str] =
    &["color", "font-size", "font-weight", "font-style", "text-decoration", "text-decoration-line"];
/// Photo types every mail program shows.
const PHOTO_TYPES: &[&str] = &["image/jpeg", "image/png", "image/gif"];

/// A photo in the text, sent as a part of its own and shown in the text through `cid:`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Photo {
    pub cid: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}

/// The HTML part of a formatted message, with its photos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RichBody {
    pub html: String,
    pub photos: Vec<Photo>,
}

/// The allow-list both functions below share; each adds its own filter for a photo's address.
fn builder() -> ammonia::Builder<'static> {
    let mut b = ammonia::Builder::empty();
    b.add_tags(TAGS)
        .add_generic_attributes(&["style"])
        .add_tag_attributes("font", &["color", "size"])
        .add_tag_attributes("img", &["src", "alt"])
        .url_schemes(HashSet::from(["data"]))
        .url_relative(ammonia::UrlRelative::Deny)
        .filter_style_properties(STYLE_PROPERTIES.iter().copied().collect());
    b
}

/// Only what the Write screen itself can make.
pub fn clean(html: &str) -> String {
    let mut b = builder();
    b.attribute_filter(|element, attribute, value| match (element, attribute) {
        ("img", "src") => photo_data(value).map(|_| Cow::Owned(value.trim().to_string())),
        _ => Some(value.into()),
    });
    b.clean(html).to_string()
}

/// The type and bytes of a photo stored in the text, or None when it is something else.
fn photo_data(src: &str) -> Option<(String, Vec<u8>)> {
    let rest = src.trim().strip_prefix("data:")?;
    let (mime, data) = rest.split_once(";base64,")?;
    let mime = mime.trim().to_ascii_lowercase();
    if !PHOTO_TYPES.contains(&mime.as_str()) {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.trim()).ok()?;
    (!bytes.is_empty()).then_some((mime, bytes))
}

/// Clean the text and give each photo in it a content id under `domain`; the same photo twice is
/// sent once. The photos are taken from the `src` of real pictures only, never from text that
/// happens to look like one.
pub fn with_photo_parts(html: &str, domain: &str) -> RichBody {
    let photos: Arc<Mutex<Vec<Photo>>> = Arc::default();
    let found = photos.clone();
    let domain = domain.to_string();
    let mut b = builder();
    b.attribute_filter(move |element, attribute, value| match (element, attribute) {
        ("img", "src") => {
            let (mime, bytes) = photo_data(value)?;
            let mut list = found.lock().unwrap_or_else(|e| e.into_inner());
            let cid = match list.iter().find(|p| p.bytes == bytes) {
                Some(p) => p.cid.clone(),
                None => {
                    let cid = format!("{}@{domain}", uuid::Uuid::new_v4().simple());
                    list.push(Photo { cid: cid.clone(), mime, bytes });
                    cid
                }
            };
            Some(Cow::Owned(format!("cid:{cid}")))
        }
        _ => Some(value.into()),
    });
    let body = b.clean(html).to_string();
    drop(b);
    let photos = std::mem::take(&mut *photos.lock().unwrap_or_else(|e| e.into_inner()));
    RichBody {
        html: format!("<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"></head><body>{body}</body></html>"),
        photos,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOT: &str = "data:image/png;base64,iVBORw0KGgo=";

    #[test]
    fn keeps_the_formatting_and_drops_the_rest() {
        let html = format!(
            "<div><b>Vet</b> <i>schuin</i> <u>onder</u></div><ul><li>een</li></ul>\
             <span style=\"color: rgb(200, 0, 0); position: fixed\">rood</span>\
             <script>alert(1)</script><img src=\"x\" onerror=\"alert(1)\">\
             <img src=\"https://spion.example/p.gif\"><a href=\"https://x.nl\">link</a>\
             <iframe src=\"https://x.nl\"></iframe><img src=\"{DOT}\" alt=\"foto\">"
        );
        let out = clean(&html);
        assert!(out.contains("<b>Vet</b>") && out.contains("<i>schuin</i>") && out.contains("<u>onder</u>"));
        assert!(out.contains("<ul><li>een</li></ul>"));
        assert!(out.contains("color:rgb(200, 0, 0)") || out.contains("color: rgb(200, 0, 0)"), "{out}");
        for gone in ["script", "alert", "onerror", "spion", "position", "iframe", "href", "<a"] {
            assert!(!out.contains(gone), "{gone} in {out}");
        }
        assert!(out.contains(DOT));
        assert!(out.contains("link"), "the words of a link stay");
    }

    #[test]
    fn photos_become_parts_with_content_ids() {
        let html = format!("<p>Kijk:</p><img src=\"{DOT}\"><img src=\"{DOT}\"><img src=\"data:text/html;base64,PHA+\">");
        let body = with_photo_parts(&html, "example.nl");
        assert_eq!(body.photos.len(), 1, "the same photo is sent once");
        let p = &body.photos[0];
        assert_eq!(p.mime, "image/png");
        assert!(p.cid.ends_with("@example.nl"));
        assert_eq!(body.html.matches(&format!("src=\"cid:{}\"", p.cid)).count(), 2);
        assert!(!body.html.contains("data:"), "{}", body.html);
        assert!(body.html.starts_with("<!DOCTYPE html>"));
    }

    #[test]
    fn text_that_looks_like_a_photo_stays_text() {
        let html = format!("<p>Typ dit: src=\"{DOT}\"</p>");
        let body = with_photo_parts(&html, "example.nl");
        assert!(body.photos.is_empty());
        assert!(!body.html.contains("cid:"), "{}", body.html);
    }
}
