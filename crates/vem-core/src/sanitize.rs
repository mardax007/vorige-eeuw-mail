//! Making HTML mail safe to show, and the warnings shown above a message (docs/screens.md).
//!
//! The frame on screen is sandboxed with a strict CSP as well; this is the first wall, not the only.
//! Here: scripts, forms, frames and event handlers go; pictures from the internet are removed
//! until the user allows them; pictures inside the message (cid:) become data: URLs; links open
//! in a new window, which the shell catches and sends to the system browser.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use base64::Engine;

use crate::model::{MailWarning, WarningKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanHtml {
    pub html: String,
    pub has_remote_images: bool,
}

const EXTRA_TAGS: &[&str] = &["center", "font", "span", "div", "table", "thead", "tbody", "tfoot", "tr", "td", "th", "caption", "col", "colgroup"];
const GENERIC_ATTRS: &[&str] = &["style", "align", "valign", "bgcolor", "width", "height", "border", "dir", "class"];
const STYLE_PROPERTIES: &[&str] = &[
    "color", "background-color", "font", "font-family", "font-size", "font-style", "font-weight",
    "text-align", "text-decoration", "text-indent", "text-transform", "line-height", "letter-spacing",
    "white-space", "vertical-align", "margin", "margin-top", "margin-right", "margin-bottom",
    "margin-left", "padding", "padding-top", "padding-right", "padding-bottom", "padding-left",
    "border", "border-top", "border-right", "border-bottom", "border-left", "border-color",
    "border-style", "border-width", "border-radius", "border-collapse", "border-spacing", "width",
    "height", "max-width", "min-width", "max-height", "display", "table-layout", "list-style-type",
    "direction",
];

/// Clean an HTML body. `inline` is the message's cid images: (content id, mime, bytes).
pub fn clean_html(html: &str, inline: &[(String, String, Vec<u8>)], allow_remote: bool) -> CleanHtml {
    let b64 = base64::engine::general_purpose::STANDARD;
    let cids: HashMap<String, String> = inline
        .iter()
        .map(|(cid, mime, bytes)| (cid.to_ascii_lowercase(), format!("data:{mime};base64,{}", b64.encode(bytes))))
        .collect();
    let remote_seen = Arc::new(Mutex::new(false));
    let seen = remote_seen.clone();

    let mut builder = ammonia::Builder::default();
    builder
        .add_tags(EXTRA_TAGS)
        .add_generic_attributes(GENERIC_ATTRS)
        .add_tag_attributes("font", &["color", "face", "size"])
        .add_tag_attributes("td", &["colspan", "rowspan", "nowrap"])
        .add_tag_attributes("th", &["colspan", "rowspan"])
        .add_tag_attributes("table", &["cellpadding", "cellspacing"])
        .add_tag_attributes("img", &["width", "height"])
        .url_schemes(HashSet::from(["http", "https", "mailto", "cid", "data"]))
        .url_relative(ammonia::UrlRelative::Deny)
        .filter_style_properties(STYLE_PROPERTIES.iter().copied().collect())
        .link_rel(Some("noopener noreferrer"))
        .set_tag_attribute_value("a", "target", "_blank")
        .attribute_filter(move |element, attribute, value| {
            let v = value.trim();
            let lower = v.to_ascii_lowercase();
            match (element, attribute) {
                ("img", "src") => {
                    if let Some(cid) = lower.strip_prefix("cid:") {
                        return cids.get(cid).map(|d| d.clone().into());
                    }
                    if lower.starts_with("data:image/") {
                        return Some(value.into());
                    }
                    if lower.starts_with("https://") || lower.starts_with("http://") {
                        *seen.lock().unwrap_or_else(|e| e.into_inner()) = true;
                        // Plain http pictures would load without TLS; the frame only allows https.
                        return (allow_remote && lower.starts_with("https://")).then(|| value.into());
                    }
                    None
                }
                ("a", "href") => {
                    (lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with("mailto:"))
                        .then(|| value.into())
                }
                (_, "src" | "href") => None,
                (_, "class") => None,
                _ => Some(value.into()),
            }
        });
    let html = builder.clean(html).to_string();
    let has_remote_images = *remote_seen.lock().unwrap_or_else(|e| e.into_inner());
    CleanHtml { html, has_remote_images }
}

/// The part of a host that a person recognises: "ing.nl" for "www.mijn.ing.nl".
pub fn registrable_domain(host: &str) -> String {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let labels: Vec<&str> = host.split('.').filter(|l| !l.is_empty()).collect();
    if labels.len() <= 2 {
        return labels.join(".");
    }
    let second = labels[labels.len() - 2];
    let take = if ["co", "com", "org", "net", "gov", "ac", "edu"].contains(&second) && labels.len() >= 3 { 3 } else { 2 };
    labels[labels.len() - take..].join(".")
}

fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let host = rest.split(['/', '?', '#']).next()?;
    let host = host.rsplit('@').next()?.split(':').next()?;
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
}

/// Links whose visible text names another site than where they really go.
pub fn link_warnings(clean_html: &str) -> Vec<MailWarning> {
    let anchor = regex::Regex::new(r#"(?is)<a\s[^>]*href="([^"]+)"[^>]*>(.*?)</a>"#).expect("valid regex");
    let domain_in_text =
        regex::Regex::new(r"(?i)\b((?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z]{2,})\b").expect("valid regex");
    let mut out: Vec<MailWarning> = Vec::new();
    for cap in anchor.captures_iter(clean_html) {
        let href = cap[1].replace("&amp;", "&");
        let Some(real) = host_of(&href) else { continue };
        let text = strip_tags(&cap[2]);
        let Some(shown) = domain_in_text.captures(&text).map(|c| c[1].to_ascii_lowercase()) else { continue };
        if shown.contains('@') || !shown.contains('.') {
            continue;
        }
        if registrable_domain(&shown) != registrable_domain(&real) {
            let w = MailWarning { kind: WarningKind::LinkMismatch, shown: Some(shown), actual: Some(real) };
            if !out.contains(&w) {
                out.push(w);
            }
        }
    }
    out
}

/// Words that point at a request for money; used only for senders the user never wrote with.
pub fn mentions_money(text: &str) -> bool {
    let t = text.to_lowercase();
    const WORDS: &[&str] = &[
        "iban", "rekeningnummer", "bankrekening", "overmaken", "overboeken", "betaalverzoek", "tikkie",
        "bitcoin", "crypto", "cadeaukaart", "giftcard", "gift card", "western union", "moneygram",
        "inloggegevens", "pincode", "wachtwoord bevestigen", "uw rekening is geblokkeerd",
        "bank account", "wire transfer", "payment",
    ];
    WORDS.iter().any(|w| t.contains(w)) || regex::Regex::new(r"(€|eur)\s?\d").expect("valid regex").is_match(&t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_and_handlers_go() {
        let c = clean_html(
            r#"<p onclick="x()">Hoi<script>alert(1)</script></p><iframe src="https://x.nl"></iframe><form><input></form><a href="javascript:alert(1)">klik</a>"#,
            &[],
            false,
        );
        assert!(!c.html.contains("script") && !c.html.contains("onclick") && !c.html.contains("iframe"));
        assert!(!c.html.contains("javascript") && !c.html.contains("<form") && !c.html.contains("<input"));
        assert!(c.html.contains("Hoi"));
    }

    #[test]
    fn remote_images_blocked_until_allowed() {
        let html = r#"<img src="https://tracker.example/p.gif"><img src="http://plain.example/a.png">"#;
        let c = clean_html(html, &[], false);
        assert!(c.has_remote_images);
        assert!(!c.html.contains("tracker.example") && !c.html.contains("plain.example"));
        let c = clean_html(html, &[], true);
        assert!(c.html.contains("https://tracker.example/p.gif"));
        assert!(!c.html.contains("http://plain.example"), "pictures without TLS never load");
    }

    #[test]
    fn cid_images_become_data_urls() {
        let c = clean_html(r#"<img src="cid:Foto1@x">"#, &[("foto1@x".into(), "image/png".into(), vec![1, 2, 3])], false);
        assert!(c.html.contains("data:image/png;base64,AQID"));
        assert!(!c.has_remote_images);
    }

    #[test]
    fn links_open_outside_and_styles_are_filtered() {
        let c = clean_html(
            r#"<a href="https://ing.nl">ING</a><div style="color:red;background:url(https://t.example/x)">x</div>"#,
            &[],
            false,
        );
        assert!(c.html.contains(r#"target="_blank""#) && c.html.contains("noopener"));
        assert!(c.html.contains("color:red") && !c.html.contains("t.example"));
    }

    #[test]
    fn mismatched_link_is_warned() {
        let c = clean_html(
            r#"<a href="https://inloggen-ing.example.ru/x">https://mijn.ing.nl</a> <a href="https://www.ing.nl/a">www.ing.nl</a> <a href="https://x.nl">Klik hier</a>"#,
            &[],
            false,
        );
        let w = link_warnings(&c.html);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].shown.as_deref(), Some("mijn.ing.nl"));
        assert_eq!(w[0].actual.as_deref(), Some("inloggen-ing.example.ru"));
        assert_eq!(registrable_domain("www.bbc.co.uk"), "bbc.co.uk");
    }

    #[test]
    fn money_words() {
        assert!(mentions_money("Kunt u even € 450 overmaken naar dit IBAN?"));
        assert!(mentions_money("Koop een cadeaukaart"));
        assert!(!mentions_money("Tot zaterdag bij oma!"));
    }
}
