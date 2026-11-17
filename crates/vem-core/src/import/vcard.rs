//! Contacts from vCard files (phase 3 task 6). Windows Live Mail exports its contacts as one
//! `.vcf` file per person (vCard 2.1, sometimes quoted-printable); other programs write several
//! cards in one file. Only the name, the e-mail addresses and one phone number are kept.

use encoding_rs::WINDOWS_1252;

use super::pst_contact::OldContact;

/// Every card in the text that has an e-mail address.
pub fn parse(bytes: &[u8]) -> Vec<OldContact> {
    let text = match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => WINDOWS_1252.decode_without_bom_handling(bytes).0.into_owned(),
    };
    let text = text.trim_start_matches('\u{feff}');
    let mut cards = Vec::new();
    let mut current: Option<Card> = None;
    for line in unfold(text) {
        let Some((head, value)) = line.split_once(':') else { continue };
        let mut parts = head.split(';');
        let name = parts.next().unwrap_or("").trim();
        let name = name.rsplit('.').next().unwrap_or(name).to_ascii_uppercase(); // "item1.EMAIL"
        let params: Vec<String> = parts.map(|p| p.trim().to_ascii_uppercase()).collect();
        match name.as_str() {
            "BEGIN" if value.trim().eq_ignore_ascii_case("VCARD") => current = Some(Card::default()),
            "END" if value.trim().eq_ignore_ascii_case("VCARD") => {
                if let Some(c) = current.take().and_then(Card::finish) {
                    cards.push(c);
                }
            }
            _ => {
                if let Some(card) = current.as_mut() {
                    card.add(&name, &params, &decode_value(value, &params));
                }
            }
        }
    }
    cards
}

#[derive(Default)]
struct Card {
    full_name: Option<String>,
    structured_name: Option<String>,
    emails: Vec<String>,
    /// (is a mobile number, number)
    phones: Vec<(bool, String)>,
}

impl Card {
    fn add(&mut self, name: &str, params: &[String], value: &str) {
        let value = value.trim();
        if value.is_empty() {
            return;
        }
        match name {
            "FN" => self.full_name = Some(value.to_string()),
            "N" => {
                // Family;Given;Additional;Prefix;Suffix
                let f: Vec<&str> = split_unescaped(value);
                let given = [f.get(3), f.get(1), f.get(2), f.first(), f.get(4)];
                let joined = given.into_iter().flatten().map(|s| s.trim()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
                if !joined.is_empty() {
                    self.structured_name = Some(joined);
                }
            }
            "EMAIL" => {
                let e = value.to_lowercase();
                if e.contains('@') && !e.contains(char::is_whitespace) && !self.emails.contains(&e) {
                    self.emails.push(e);
                }
            }
            "TEL" => {
                let mobile = params.iter().any(|p| p.contains("CELL"));
                self.phones.push((mobile, value.to_string()));
            }
            _ => {}
        }
    }

    fn finish(self) -> Option<OldContact> {
        if self.emails.is_empty() {
            return None; // The address book is for mail.
        }
        let name = self.full_name.or(self.structured_name).unwrap_or_else(|| self.emails[0].clone());
        let phone = self.phones.iter().find(|(m, _)| *m).or(self.phones.first()).map(|(_, p)| p.clone());
        Some(OldContact { name: unescape(&name), emails: self.emails, phone })
    }
}

/// Lines with their continuations joined: a line starting with a space or tab goes on the
/// previous one, and a quoted-printable line ending in "=" goes on with the next.
fn unfold(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut soft_break = false;
    for raw in text.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let upper = line.to_ascii_uppercase();
        let card_edge = upper.starts_with("BEGIN:") || upper.starts_with("END:");
        if soft_break && !card_edge && let Some(last) = out.last_mut() {
            last.push_str(line);
        } else if (line.starts_with(' ') || line.starts_with('\t')) && let Some(last) = out.last_mut() {
            last.push_str(&line[1..]);
        } else {
            out.push(line.to_string());
        }
        let last = out.last().map(String::as_str).unwrap_or("");
        soft_break = last.to_ascii_uppercase().contains("QUOTED-PRINTABLE") && last.ends_with('=');
        if soft_break && let Some(l) = out.last_mut() {
            l.pop();
        }
    }
    out
}

fn decode_value(value: &str, params: &[String]) -> String {
    let qp = params.iter().any(|p| p == "QUOTED-PRINTABLE" || p == "ENCODING=QUOTED-PRINTABLE");
    if !qp {
        return value.to_string();
    }
    let bytes = quoted_printable(value);
    let latin = params.iter().any(|p| p.starts_with("CHARSET=") && !p.contains("UTF-8"));
    match std::str::from_utf8(&bytes) {
        Ok(s) if !latin => s.to_string(),
        _ => WINDOWS_1252.decode_without_bom_handling(&bytes).0.into_owned(),
    }
}

fn quoted_printable(value: &str) -> Vec<u8> {
    let b = value.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'='
            && i + 2 < b.len()
            && b[i + 1].is_ascii_hexdigit()
            && b[i + 2].is_ascii_hexdigit()
            && let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// Split a structured value on ";" that is not escaped as "\;".
fn split_unescaped(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let bytes = value.as_bytes();
    for (i, c) in bytes.iter().enumerate() {
        if *c == b';' && (i == 0 || bytes[i - 1] != b'\\') {
            parts.push(&value[start..i]);
            start = i + 1;
        }
    }
    parts.push(&value[start..]);
    parts
}

fn unescape(value: &str) -> String {
    value.replace("\\,", ",").replace("\\;", ";").replace("\\n", " ").replace("\\N", " ").replace("\\\\", "\\")
}

/// The address book as one vCard 3.0 file in UTF-8 (phase 2b task 3: "never trapped in the
/// app"). Outlook, Gmail, phones and this program itself read it back.
pub fn write(contacts: &[crate::model::Contact]) -> String {
    let mut out = String::new();
    for c in contacts {
        out.push_str("BEGIN:VCARD\r\nVERSION:3.0\r\n");
        out.push_str(&format!("FN:{}\r\n", escape(&c.name)));
        out.push_str(&format!("N:{};;;;\r\n", escape(&c.name)));
        for e in &c.emails {
            out.push_str(&format!("EMAIL;TYPE=INTERNET:{}\r\n", escape(e)));
        }
        if let Some(p) = c.phone.as_deref().filter(|p| !p.trim().is_empty()) {
            out.push_str(&format!("TEL:{}\r\n", escape(p)));
        }
        if let Some(n) = c.note.as_deref().filter(|n| !n.trim().is_empty()) {
            out.push_str(&format!("NOTE:{}\r\n", escape(n)));
        }
        out.push_str("END:VCARD\r\n");
    }
    out
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace(',', "\\,").replace(';', "\\;").replace("\r\n", "\\n").replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_address_book_survives_export_and_import() {
        let contact = |name: &str, emails: &[&str], phone: Option<&str>| crate::model::Contact {
            id: String::new(),
            name: name.into(),
            emails: emails.iter().map(|e| e.to_string()).collect(),
            phone: phone.map(Into::into),
            note: Some("Buurvrouw; heeft de sleutel,\nbel eerst".into()),
        };
        let book = vec![
            contact("Renée de Bakker-Ünal", &["renee@voorbeeld.nl", "r.bakker@werk.nl"], Some("06-12345678")),
            contact("Bakker, Sanne", &["sanne@voorbeeld.nl"], None),
        ];
        let back = parse(write(&book).as_bytes());
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].name, "Renée de Bakker-Ünal");
        assert_eq!(back[0].emails, vec!["renee@voorbeeld.nl", "r.bakker@werk.nl"]);
        assert_eq!(back[0].phone.as_deref(), Some("06-12345678"));
        assert_eq!(back[1].name, "Bakker, Sanne");
        assert_eq!(back[1].phone, None);
    }

    #[test]
    fn live_mail_card_with_quoted_printable_name() {
        let card = "BEGIN:VCARD\r\nVERSION:2.1\r\nN;CHARSET=Windows-1252;ENCODING=QUOTED-PRINTABLE:M=FCller;J=F6rg\r\nFN;CHARSET=Windows-1252;ENCODING=QUOTED-PRINTABLE:J=F6rg M=FCller\r\nEMAIL;PREF;INTERNET:Joerg@Voorbeeld.de\r\nTEL;HOME;VOICE:030 123\r\nTEL;CELL;VOICE:0170 456\r\nEND:VCARD\r\n";
        let c = parse(card.as_bytes());
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].name, "Jörg Müller");
        assert_eq!(c[0].emails, vec!["joerg@voorbeeld.de"]);
        assert_eq!(c[0].phone.as_deref(), Some("0170 456"));
    }

    #[test]
    fn several_cards_folded_lines_and_name_from_n() {
        let text = "BEGIN:VCARD\nVERSION:3.0\nN:Visser;Marja;;;\nEMAIL;TYPE=INTERNET:marja@voor\n beeld.nl\nitem1.EMAIL:marja.werk@voorbeeld.nl\nEND:VCARD\nBEGIN:VCARD\nFN:Zonder adres\nTEL:06-1\nEND:VCARD\nBEGIN:VCARD\nFN:Bakker\\, Sanne\nEMAIL:sanne@voorbeeld.nl\nEND:VCARD\n";
        let c = parse(text.as_bytes());
        assert_eq!(c.len(), 2, "a card without an address is not kept");
        assert_eq!(c[0].name, "Marja Visser");
        assert_eq!(c[0].emails, vec!["marja@voorbeeld.nl", "marja.werk@voorbeeld.nl"]);
        assert_eq!(c[1].name, "Bakker, Sanne");
    }

    #[test]
    fn quoted_printable_soft_line_breaks_and_windows_1252_files() {
        let text = "BEGIN:VCARD\r\nFN;ENCODING=QUOTED-PRINTABLE:Ren=C3=A9e van=\r\n der Berg\r\nEMAIL:renee@x.nl\r\nEND:VCARD\r\n";
        assert_eq!(parse(text.as_bytes())[0].name, "Renée van der Berg");
        let cut = "BEGIN:VCARD\r\nFN;ENCODING=QUOTED-PRINTABLE:Half=\r\nEND:VCARD\r\nBEGIN:VCARD\r\nFN:Heel\r\nEMAIL:heel@x.nl\r\nEND:VCARD\r\n";
        assert_eq!(parse(cut.as_bytes()).len(), 1, "a broken card does not swallow the next one");
        assert_eq!(quoted_printable("=+1"), b"=+1");
        let latin = b"BEGIN:VCARD\r\nFN:Ren\xe9e\r\nEMAIL:renee@x.nl\r\nEND:VCARD\r\n";
        assert_eq!(parse(latin)[0].name, "Renée");
    }
}
