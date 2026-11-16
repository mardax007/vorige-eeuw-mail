//! Outlook contacts in a PST ("IPM.Contact" items) for the address book (phase 3: PST contacts).
//! Only a name, the e-mail addresses and one phone number are kept, as the address book has.

use outlook_pst::ltp::prop_context::PropertyValue;
use outlook_pst::messaging::named_prop::{NamedPropertyGuid, NamedPropertyId};
use outlook_pst::messaging::store::Store;

/// A contact read from the PST, before it goes into the address book.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PstContact {
    pub name: String,
    pub emails: Vec<String>,
    pub phone: Option<String>,
}

const PR_SUBJECT: u16 = 0x0037;
const PR_DISPLAY_NAME: u16 = 0x3001;
const PR_GIVEN_NAME: u16 = 0x3A06;
const PR_SURNAME: u16 = 0x3A11;
/// Mobile first: that is the number the family calls.
const PHONES: [u16; 4] = [0x3A1C, 0x3A09, 0x3A1A, 0x3A08];

/// PidLidEmail1EmailAddress, Email2, Email3 and their "original display name" twins, which hold
/// the SMTP address when the first one is an Exchange address ([MS-OXOCNTC] 2.2.1.2).
const EMAIL_LIDS: [u32; 6] = [0x8083, 0x8093, 0x80A3, 0x8084, 0x8094, 0x80A4];

/// Whether a message class is a contact (not a distribution list).
pub fn is_contact_class(class: &str) -> bool {
    let class = class.trim().to_ascii_lowercase();
    class == "ipm.contact" || class.starts_with("ipm.contact.")
}

/// The property ids this PST uses for the contact e-mail fields. They are named properties in
/// PSETID_Address; numbered named properties outside the two standard sets are matched by number,
/// because the crate's GUID lookup for them is not reliable.
pub fn email_prop_ids(store: &dyn Store) -> Vec<u16> {
    let Ok(map) = store.named_property_map() else { return Vec::new() };
    let Ok(entries) = map.properties().stream_entry() else { return Vec::new() };
    let mut ids: Vec<(usize, u16)> = entries
        .into_iter()
        .filter_map(|entry| {
            let NamedPropertyId::Number(lid) = entry.id() else { return None };
            if matches!(entry.guid(), NamedPropertyGuid::None | NamedPropertyGuid::Mapi | NamedPropertyGuid::PublicStrings) {
                return None;
            }
            EMAIL_LIDS.iter().position(|l| *l == lid).map(|order| (order, entry.prop_id()))
        })
        .collect();
    ids.sort();
    ids.into_iter().map(|(_, id)| id).collect()
}

/// Build the contact from its properties. `text` reads a string property. None when it has
/// neither a name nor an address.
pub fn contact(text: &dyn Fn(u16) -> Option<String>, email_ids: &[u16]) -> Option<PstContact> {
    let mut emails: Vec<String> = Vec::new();
    for id in email_ids {
        if let Some(e) = text(*id).and_then(|v| smtp_address(&v))
            && !emails.contains(&e)
        {
            emails.push(e);
        }
    }
    let given_and_surname = [text(PR_GIVEN_NAME), text(PR_SURNAME)]
        .into_iter()
        .flatten()
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let name = [text(PR_DISPLAY_NAME), Some(given_and_surname), text(PR_SUBJECT)]
        .into_iter()
        .flatten()
        .map(|s| s.trim().to_string())
        .find(|s| !s.is_empty())
        .unwrap_or_default();
    let phone = PHONES.iter().filter_map(|id| text(*id)).map(|p| p.trim().to_string()).find(|p| !p.is_empty());
    if emails.is_empty() {
        return None; // The address book is for mail: a name without an address is not kept.
    }
    let name = if name.is_empty() { emails[0].clone() } else { name };
    Some(PstContact { name, emails, phone })
}

/// An e-mail address out of a field that may hold "Name (adres@x.nl)" or an Exchange path.
fn smtp_address(value: &str) -> Option<String> {
    let v = value.trim();
    let inner = match (v.rfind('('), v.rfind(')')) {
        (Some(a), Some(b)) if a < b => &v[a + 1..b],
        _ => v,
    };
    let inner = inner.trim();
    let inner = inner.strip_prefix("SMTP:").or_else(|| inner.strip_prefix("smtp:")).unwrap_or(inner).trim();
    let at = inner.find('@')?;
    let ok = at > 0 && inner[at + 1..].contains('.') && !inner.contains(char::is_whitespace) && !inner.starts_with('/');
    ok.then(|| inner.to_lowercase())
}

/// Read a string property for [`contact`], Unicode or String8 through `decode8`.
pub fn text_of(value: Option<&PropertyValue>, decode8: &dyn Fn(&[u8]) -> String) -> Option<String> {
    let s = match value? {
        PropertyValue::String8(v) => decode8(v.buffer()),
        PropertyValue::Unicode(v) => String::from_utf16_lossy(v.buffer()),
        _ => return None,
    };
    Some(s.trim_end_matches('\0').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn read(fields: &[(u16, &str)]) -> impl Fn(u16) -> Option<String> {
        let map: HashMap<u16, String> = fields.iter().map(|(k, v)| (*k, v.to_string())).collect();
        move |id| map.get(&id).cloned()
    }

    #[test]
    fn contact_classes() {
        assert!(is_contact_class("IPM.Contact"));
        assert!(is_contact_class("ipm.contact.custom"));
        assert!(!is_contact_class("IPM.DistList"));
        assert!(!is_contact_class("IPM.Note"));
    }

    #[test]
    fn name_addresses_and_mobile_number() {
        let text = read(&[
            (PR_DISPLAY_NAME, "Tante Truus"),
            (0x8001, "TRUUS@Voorbeeld.nl"),
            (0x8002, "Truus (truus@voorbeeld.nl)"),
            (0x8003, "/o=Exchange/ou=First/cn=Recipients/cn=truus"),
            (0x8004, "Truus (t.werk@bedrijf-voorbeeld.nl)"),
            (0x3A08, "010-1234567"),
            (0x3A1C, "06-12345678"),
        ]);
        let c = contact(&text, &[0x8001, 0x8002, 0x8003, 0x8004]).unwrap();
        assert_eq!(c.name, "Tante Truus");
        assert_eq!(c.emails, vec!["truus@voorbeeld.nl", "t.werk@bedrijf-voorbeeld.nl"]);
        assert_eq!(c.phone.as_deref(), Some("06-12345678"));
    }

    #[test]
    fn name_from_given_and_surname_or_the_address() {
        let c = contact(&read(&[(PR_GIVEN_NAME, "Kees"), (PR_SURNAME, "Jansen"), (0x8001, "kees@voorbeeld.nl")]), &[0x8001]).unwrap();
        assert_eq!(c.name, "Kees Jansen");
        let c = contact(&read(&[(0x8001, "bep@voorbeeld.nl")]), &[0x8001]).unwrap();
        assert_eq!(c.name, "bep@voorbeeld.nl");
        assert!(contact(&read(&[(PR_DISPLAY_NAME, "Wim"), (0x3A1C, "06-1")]), &[]).is_none(), "no address: not kept");
    }

    #[test]
    fn exchange_paths_are_not_addresses() {
        assert_eq!(smtp_address("/o=Org/cn=jan@x"), None);
        assert_eq!(smtp_address("SMTP:Jan@X.nl"), Some("jan@x.nl".into()));
        assert_eq!(smtp_address("geen adres"), None);
    }
}
