//! The hostile HTML set (tests/fixtures/hostile.html) through the first wall, the sanitiser.
//! The frame's sandbox and CSP are the second and third walls (src/ui/mail/MailFrame.tsx); the
//! whole chain was run in the app on the Android emulator (docs/PROGRESS.md).

use vem_core::sanitize::clean_html;

const HOST: &str = "hostile.invalid";

fn sample() -> String {
    include_str!("fixtures/hostile.html").replace("HOST", HOST)
}

/// Things that must never reach the frame, whatever the user allows.
const NEVER: &[&str] = &[
    "<script", "<style", "<meta", "<base", "<link", "<iframe", "<object", "<embed", "<form", "<input",
    "<button", "<video", "<audio", "<svg", "<source", "srcset", "srcdoc", "javascript:", "data:text",
    "file:", "intent:", "tauri:", "expression", "@import", "__TAURI", "postMessage", "onload", "onerror",
    "onfocus", "autofocus", "localhost", "127.0.0.1",
];

fn assert_safe(html: &str) {
    let lower = html.to_ascii_lowercase();
    for bad in NEVER {
        assert!(!lower.contains(&bad.to_ascii_lowercase()), "{bad:?} got through:\n{html}");
    }
    for plain in ["http://hostile", "url("] {
        assert!(!lower.contains(plain), "{plain:?} got through:\n{html}");
    }
}

#[test]
fn nothing_hostile_survives_and_nothing_loads_unasked() {
    let c = clean_html(&sample(), &[], false);
    assert_safe(&c.html);
    assert!(!c.html.contains(HOST), "no address of the hostile host before Afbeeldingen tonen:\n{}", c.html);
    assert!(c.has_remote_images, "the screen offers Afbeeldingen tonen");
    assert!(c.html.contains("Hostile test message"), "the text itself stays readable");
}

#[test]
fn allowing_pictures_lets_only_https_pictures_through() {
    let c = clean_html(&sample(), &[], true);
    assert_safe(&c.html);
    assert!(c.html.contains(&format!("https://{HOST}/tls.png")));
    let others = c.html.matches(HOST).count();
    assert_eq!(others, 1, "only the https picture names the host:\n{}", c.html);
}

#[test]
fn links_keep_only_web_and_mail_and_open_in_a_new_window() {
    let c = clean_html(&sample(), &[], false);
    assert!(c.html.contains(r#"href="https://example.org/link-without-target""#));
    assert!(c.html.contains(r#"href="mailto:piet@example.test?subject=Hoi""#));
    for a in c.html.split("<a ").skip(1) {
        let tag = &a[..a.find('>').unwrap()];
        if tag.contains("href=") {
            assert!(tag.contains(r#"target="_blank""#) && tag.contains("noopener"), "link without a new window: {tag}");
        }
    }
}
