//! Compressed RTF ([MS-OXRTFCP]) and the HTML or text encapsulated in it ([MS-OXRTFEX]),
//! as Outlook keeps message bodies in a PST.

use encoding_rs::{Encoding, WINDOWS_1252};

use super::pst::encoding_for;


const RTF_COMPRESSED: u32 = 0x7546_5A4C;
const RTF_UNCOMPRESSED: u32 = 0x414C_454D;
const RTF_HEADER: usize = 16;

/// Decompressed RTF as bytes, or None. The header is checked first because the decompressor
/// slices without checks, and the app is built with panic = "abort".
pub(crate) fn decompress_rtf(data: &[u8]) -> Option<Vec<u8>> {
    let word = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().unwrap_or_default());
    if data.len() < RTF_HEADER || word(0) as usize + 4 != data.len() {
        return None;
    }
    match word(8) {
        RTF_COMPRESSED => {}
        RTF_UNCOMPRESSED if (word(4) as usize).checked_add(RTF_HEADER)? <= data.len() => {}
        _ => return None,
    }
    let text = compressed_rtf::decompress_rtf(data).ok()?;
    // The crate maps every byte to the char of the same value.
    Some(text.chars().map(|c| c as u32 as u8).collect())
}

#[derive(Debug, PartialEq)]
pub(crate) enum RtfBody {
    Html(String),
    Text(String),
}

/// Destinations whose content is never text.
const RTF_SKIP_DESTINATIONS: [&str; 26] = [
    "fonttbl", "colortbl", "stylesheet", "info", "pict", "object", "header", "footer", "headerl",
    "headerr", "headerf", "footerl", "footerr", "footerf", "ftnsep", "ftnsepc", "aftnsep",
    "listtable", "listoverridetable", "revtbl", "rsidtbl", "generator", "themedata",
    "datastore", "latentstyles", "fldinst",
];

#[derive(Clone, Copy)]
struct RtfGroup {
    skip: bool,
    htmlrtf: bool,
    uc: usize,
}

struct RtfParser<'a> {
    rtf: &'a [u8],
    at: usize,
    is_html: bool,
    stack: Vec<RtfGroup>,
    /// The next control word names the destination of a group that just opened.
    at_group_start: bool,
    /// `\*` was seen: the destination is ignorable.
    star: bool,
    text: String,
    pending: Vec<u8>,
    encoding: &'static Encoding,
    skip_chars: usize,
}

/// HTML when the RTF says `\fromhtml1`, otherwise the plain text in it.
pub(crate) fn rtf_to_body(rtf: &[u8]) -> RtfBody {
    let mut p = RtfParser {
        rtf,
        at: 0,
        is_html: rtf.windows(10).any(|w| w == b"\\fromhtml1"),
        stack: vec![RtfGroup { skip: false, htmlrtf: false, uc: 1 }],
        at_group_start: false,
        star: false,
        text: String::new(),
        pending: Vec::new(),
        encoding: WINDOWS_1252,
        skip_chars: 0,
    };
    while p.at < rtf.len() {
        p.step();
    }
    p.flush();
    let text = p.text.replace("\r\n", "\n");
    if p.is_html { RtfBody::Html(text) } else { RtfBody::Text(text.trim().to_string()) }
}

impl RtfParser<'_> {
    fn group(&mut self) -> &mut RtfGroup {
        let last = self.stack.len() - 1; // the outer group is never popped
        &mut self.stack[last]
    }

    fn visible(&mut self) -> bool {
        let g = *self.group();
        !g.skip && !g.htmlrtf
    }

    fn step(&mut self) {
        let b = self.rtf[self.at];
        self.at += 1;
        let more = self.at < self.rtf.len();
        match b {
            b'{' => {
                let top = *self.group();
                self.stack.push(top);
                (self.at_group_start, self.star) = (true, false);
            }
            b'}' => {
                if self.stack.len() > 1 {
                    self.stack.pop();
                }
                (self.at_group_start, self.star) = (false, false);
            }
            b'\r' | b'\n' => {}
            b'\\' if more && self.rtf[self.at].is_ascii_alphabetic() => self.control_word(),
            b'\\' if more => self.control_symbol(),
            other => {
                self.byte(other);
                self.at_group_start = false;
            }
        }
    }

    fn control_symbol(&mut self) {
        let symbol = self.rtf[self.at];
        self.at += 1;
        match symbol {
            b'*' => {
                self.star = true;
                return;
            }
            b'\'' => {
                let hex = self.rtf.get(self.at..self.at + 2).and_then(|h| std::str::from_utf8(h).ok());
                if let Some(byte) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    self.byte(byte);
                    self.at += 2;
                }
            }
            b'~' => self.char('\u{a0}'),
            b'_' => self.char('\u{2011}'),
            b'\r' | b'\n' => self.char('\n'),
            b'-' => {}
            other => self.byte(other),
        }
        self.at_group_start = false;
    }

    fn control_word(&mut self) {
        let rtf = self.rtf;
        let mut end = self.at;
        while end < rtf.len() && rtf[end].is_ascii_alphabetic() {
            end += 1;
        }
        let word = std::str::from_utf8(&rtf[self.at..end]).unwrap_or_default();
        let digits = end;
        if end < rtf.len() && rtf[end] == b'-' {
            end += 1;
        }
        while end < rtf.len() && rtf[end].is_ascii_digit() {
            end += 1;
        }
        let param: Option<i64> = std::str::from_utf8(&rtf[digits..end]).ok().and_then(|n| n.parse().ok());
        if end < rtf.len() && rtf[end] == b' ' {
            end += 1;
        }
        self.at = end;
        if self.at_group_start {
            let (star, is_html) = (self.star, self.is_html);
            let group = self.group();
            if star && is_html && word.starts_with("htmltag") {
                group.htmlrtf = false;
            } else if star || RTF_SKIP_DESTINATIONS.contains(&word) {
                group.skip = true;
            }
            (self.at_group_start, self.star) = (false, false);
        }
        self.apply(word, param);
    }

    fn apply(&mut self, word: &str, param: Option<i64>) {
        let number = param.and_then(|p| usize::try_from(p).ok());
        match word {
            "htmlrtf" => self.group().htmlrtf = param != Some(0),
            "uc" => self.group().uc = number.unwrap_or(1),
            "ansicpg" => {
                self.flush();
                self.encoding = param.and_then(|p| u16::try_from(p).ok()).map(encoding_for).unwrap_or(WINDOWS_1252);
            }
            "u" => {
                let code = param.unwrap_or(0);
                let code = if code < 0 { code + 65_536 } else { code };
                self.char(u32::try_from(code).ok().and_then(char::from_u32).unwrap_or('\u{fffd}'));
                self.skip_chars = self.group().uc;
            }
            "bin" => self.at = self.at.saturating_add(number.unwrap_or(0)),
            "par" | "line" => self.char('\n'),
            "tab" => self.char('\t'),
            "lquote" => self.char('\u{2018}'),
            "rquote" => self.char('\u{2019}'),
            "ldblquote" => self.char('\u{201c}'),
            "rdblquote" => self.char('\u{201d}'),
            "bullet" => self.char('\u{2022}'),
            "endash" => self.char('\u{2013}'),
            "emdash" => self.char('\u{2014}'),
            _ => {}
        }
    }

    /// A byte of text in the document code page; after `\u` it may be the fallback to skip.
    fn byte(&mut self, b: u8) {
        if self.skip_chars > 0 {
            self.skip_chars -= 1;
        } else if self.visible() {
            self.pending.push(b);
        }
    }

    fn char(&mut self, c: char) {
        if self.visible() {
            self.flush();
            self.text.push(c);
        }
    }

    fn flush(&mut self) {
        if !self.pending.is_empty() {
            self.text.push_str(&self.encoding.decode_without_bom_handling(&self.pending).0);
            self.pending.clear();
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressed_rtf_with_html_gives_the_html() {
        let rtf = "{\\rtf1\\ansi\\ansicpg1252\\fromhtml1 \\deff0{\\fonttbl{\\f0\\fswiss Arial;}}\r\n\
                   {\\*\\htmltag19 <html>}{\\*\\htmltag34 <body>}\\htmlrtf {\\f0 \\htmlrtf0 \
                   Caf\\'e9 \\u8364?\\htmlrtf }\\htmlrtf0 {\\*\\htmltag244 <br>}\\par\
                   {\\*\\htmltag42 </body>}{\\*\\htmltag27 </html>}}";
        let compressed = compressed_rtf::compress_rtf(rtf).unwrap();
        let bytes = decompress_rtf(&compressed).expect("decompresses");
        match rtf_to_body(&bytes) {
            RtfBody::Html(h) => {
                assert!(h.starts_with("<html><body>"), "{h:?}");
                assert!(h.contains("Café €"), "{h:?}");
                assert!(h.contains("<br>\n</body></html>"), "{h:?}");
                assert!(!h.contains("Arial"));
            }
            other => panic!("expected html, got {other:?}"),
        }
    }

    #[test]
    fn plain_rtf_gives_the_text() {
        let rtf = b"{\\rtf1\\ansi{\\fonttbl{\\f0 Times;}}{\\*\\generator Word;}\\f0 Hallo \\b oma\\b0\\par Tot ziens}";
        assert_eq!(rtf_to_body(rtf), RtfBody::Text("Hallo oma\nTot ziens".into()));
    }

    #[test]
    fn broken_rtf_is_none_not_a_panic() {
        assert_eq!(decompress_rtf(&[1, 2, 3]), None);
        let mut bad = vec![0u8; 20];
        bad[0] = 16; // size matches, length field says 4 GB
        bad[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        bad[8..12].copy_from_slice(&RTF_UNCOMPRESSED.to_le_bytes());
        assert_eq!(decompress_rtf(&bad), None);
    }
}
