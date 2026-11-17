//! Contacts on PC and phone (decision 19, C13): one encrypted message in the folder
//! "Vorige Eeuw Mail" of the user's own mailbox. The key travels once, by QR code.

use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};

use crate::error::{Code, Result, VemError};
use crate::imap::Imap;
use crate::log;
use crate::model::{AccountKind, Addr, now};
use crate::secrets::{Secret, Secrets};
use crate::store::Store;
use crate::store::contacts::SyncContact;

pub const FOLDER: &str = "Vorige Eeuw Mail";
pub const SUBJECT: &str = "Vorige Eeuw Mail: contactpersonen";
const SECRET_NAME: &str = "contact-sync";
const PAIR_KEY: &str = "contact_sync_account";
const MARK_HEADER: &str = "X-Vem-Contacts";
const ATTACHMENT: &str = "contactpersonen.vem";

#[derive(Debug, Serialize, Deserialize)]
struct Payload {
    v: u32,
    written_at: i64,
    contacts: Vec<SyncContact>,
}

/// What the phone reads from the QR code. Never a password.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairCode {
    pub v: u32,
    pub account: String,
    pub key: String,
}

/// The pairing key, also used to copy the PC's local folders to the phone (docs/pc-to-phone.md).
pub(crate) fn key(secrets: &Secrets) -> Result<Option<[u8; 32]>> {
    match secrets.get(SECRET_NAME)? {
        Some(Secret::SyncKey { key }) if key.len() == 32 => {
            let mut k = [0u8; 32];
            k.copy_from_slice(&key);
            Ok(Some(k))
        }
        _ => Ok(None),
    }
}

/// The account whose mailbox carries the contacts, once paired.
pub fn paired_account(store: &Store) -> Result<Option<String>> {
    store.kv_get(PAIR_KEY)
}

/// The PC side: make a new key (a new pairing replaces the old) and the code for the phone.
pub fn pair_code(store: &Store, secrets: &Secrets, account_id: &str) -> Result<PairCode> {
    let account = store.account(account_id)?;
    let k: [u8; 32] = rand::random();
    secrets.put(SECRET_NAME, &Secret::SyncKey { key: k.to_vec() })?;
    store.kv_set(PAIR_KEY, &account_id.to_string())?;
    log::event("contacts.paired", &[("account", account_id)]);
    Ok(PairCode {
        v: 1,
        account: account.email,
        key: base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(k),
    })
}

/// Pair on the PC and return the code as a QR picture (SVG).
pub fn pair(store: &Store, secrets: &Secrets, account_id: &str) -> Result<String> {
    let code = pair_code(store, secrets, account_id)?;
    let text = serde_json::to_string(&code).map_err(|_| VemError::new(Code::AppCmd01))?;
    let qr = qrcode::QrCode::new(text.as_bytes()).map_err(|_| VemError::new(Code::AppCmd01).with_detail("qr"))?;
    Ok(qr.render::<qrcode::render::svg::Color>().min_dimensions(280, 280).quiet_zone(true).build())
}

/// The phone side: the text of the QR code the PC showed. The mailbox in the code must already be
/// on this phone (the contacts travel through it). Keeps the key and returns that mailbox's id.
pub fn pair_from_code(store: &Store, secrets: &Secrets, text: &str) -> Result<String> {
    let bad = || VemError::new(Code::PairRead01);
    let code: PairCode = serde_json::from_str(text.trim()).map_err(|_| bad())?;
    if code.v != 1 {
        return Err(bad());
    }
    let key = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(code.key.as_bytes())
        .ok()
        .filter(|k| k.len() == 32)
        .ok_or_else(bad)?;
    let account = store
        .accounts()?
        .into_iter()
        .find(|a| a.kind == AccountKind::Imap && a.email.eq_ignore_ascii_case(code.account.trim()))
        .ok_or_else(|| VemError::new(Code::PairFind01).with_detail(code.account.trim().to_string()))?;
    secrets.put(SECRET_NAME, &Secret::SyncKey { key })?;
    store.kv_set(PAIR_KEY, &account.id)?;
    log::event("contacts.paired_phone", &[("account", &account.id)]);
    Ok(account.id)
}

pub fn unpair(store: &Store, secrets: &Secrets) -> Result<()> {
    secrets.delete(SECRET_NAME)?;
    store.kv_set::<Option<String>>(PAIR_KEY, &None)?;
    Ok(())
}

pub fn seal(key: &[u8; 32], contacts: Vec<SyncContact>) -> Result<Vec<u8>> {
    let plain = serde_json::to_vec(&Payload { v: 1, written_at: now(), contacts })
        .map_err(|_| VemError::new(Code::AppCmd01))?;
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce_bytes: [u8; 24] = rand::random();
    let nonce = XNonce::from(nonce_bytes);
    let ct = cipher.encrypt(&nonce, plain.as_ref()).map_err(|_| VemError::new(Code::AppCmd01).with_detail("seal"))?;
    let mut out = nonce_bytes.to_vec();
    out.extend_from_slice(&ct);
    Ok(out)
}

pub fn open(key: &[u8; 32], sealed: &[u8]) -> Option<Vec<SyncContact>> {
    if sealed.len() < 24 {
        return None;
    }
    let (n, ct) = sealed.split_at(24);
    let mut nb = [0u8; 24];
    nb.copy_from_slice(n);
    let cipher = XChaCha20Poly1305::new(key.into());
    let plain = cipher.decrypt(&XNonce::from(nb), ct).ok()?;
    serde_json::from_slice::<Payload>(&plain).ok().map(|p| p.contacts)
}

fn message(from: &str, sealed: &[u8]) -> Result<Vec<u8>> {
    mail_builder::MessageBuilder::new()
        .from(Addr::new(None, from).email.clone())
        .to(from.to_string())
        .subject(SUBJECT)
        .date(now())
        .header(MARK_HEADER, mail_builder::headers::raw::Raw::new("1"))
        .text_body(
            "Dit bericht houdt de contactpersonen van Vorige Eeuw Mail gelijk op uw computer en telefoon.\n\
             Het is versleuteld. Laat het staan; de app ruimt oude versies zelf op.\n",
        )
        .attachment("application/octet-stream", ATTACHMENT, sealed.to_vec())
        .write_to_vec()
        .map_err(|e| VemError::new(Code::AppCmd01).with_detail(e.kind().to_string()))
}

/// One sync round on an open connection: read what the phone wrote, merge, write back.
/// Returns true when local contacts changed.
pub async fn sync(store: &Store, secrets: &Secrets, imap: &mut Imap, account_id: &str) -> Result<bool> {
    if paired_account(store)?.as_deref() != Some(account_id) {
        return Ok(false);
    }
    let Some(k) = key(secrets)? else { return Ok(false) };
    let email = store.account(account_id)?.email;
    let folders = imap.list_folders().await?;
    if !folders.iter().any(|f| f.name == FOLDER) {
        imap.create_folder(FOLDER).await?;
    }
    imap.open(FOLDER, false).await?;
    let uids: Vec<u32> = imap.uids().await?.into_iter().collect();
    let mut ours = Vec::new();
    let mut changed = false;
    if !uids.is_empty() {
        let set = crate::imap::client::uid_set(&uids);
        for m in imap.fetch(&set, true).await? {
            let p = crate::parse::parse(&m.raw);
            if p.subject != SUBJECT {
                continue; // Something else the user put here: leave it alone.
            }
            ours.push(m.uid);
            let Some(a) = p.attachments.iter().find(|a| a.name == ATTACHMENT) else { continue };
            let Some(bytes) = crate::parse::attachment_bytes(&m.raw, a.index) else { continue };
            if let Some(remote) = open(&k, &bytes) {
                changed |= store.merge_contacts(&remote)?;
            }
        }
    }
    let sealed = seal(&k, store.contacts_for_sync()?)?;
    imap.append(FOLDER, true, false, &message(&email, &sealed)?).await?;
    if !ours.is_empty() {
        imap.open(FOLDER, false).await?;
        imap.expunge_uids(&ours).await?;
    }
    log::event("contacts.synced", &[("account", account_id), ("changed", if changed { "1" } else { "0" })]);
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Contact;

    #[test]
    fn seal_and_open() {
        let k: [u8; 32] = rand::random();
        let c = SyncContact {
            contact: Contact { id: "c1".into(), name: "Oma".into(), emails: vec!["oma@x.nl".into()], phone: None, note: None },
            changed_at: 5,
            deleted: false,
        };
        let sealed = seal(&k, vec![c.clone()]).unwrap();
        assert!(!sealed.windows(3).any(|w| w == b"Oma"), "contacts must not be readable");
        assert_eq!(open(&k, &sealed).unwrap(), vec![c]);
        let other: [u8; 32] = rand::random();
        assert!(open(&other, &sealed).is_none());
        let mut broken = sealed.clone();
        *broken.last_mut().unwrap() ^= 1;
        assert!(open(&k, &broken).is_none());
    }

    #[test]
    fn pairing_makes_a_qr_without_password() {
        let (d, s) = crate::store::testutil::store();
        let secrets = Secrets::new(d.path());
        let a = s.add_account("Jan", "jan@x.nl", crate::model::AccountKind::Imap, None).unwrap();
        let svg = pair(&s, &secrets, &a.id).unwrap();
        assert!(svg.starts_with("<?xml") || svg.starts_with("<svg"));
        assert_eq!(paired_account(&s).unwrap().as_deref(), Some(a.id.as_str()));
        assert!(key(&secrets).unwrap().is_some());
        unpair(&s, &secrets).unwrap();
        assert_eq!(paired_account(&s).unwrap(), None);
        assert!(key(&secrets).unwrap().is_none());
    }

    #[test]
    fn the_phone_takes_the_key_for_its_own_mailbox() {
        let (pd, pc) = crate::store::testutil::store();
        let pc_secrets = Secrets::new(pd.path());
        let a = pc.add_account("Jan", "jan@x.nl", crate::model::AccountKind::Imap, None).unwrap();
        let text = serde_json::to_string(&pair_code(&pc, &pc_secrets, &a.id).unwrap()).unwrap();

        let (fd, phone) = crate::store::testutil::store();
        let phone_secrets = Secrets::new(fd.path());
        phone.add_account("Jan", "Jan@X.nl", crate::model::AccountKind::Imap, None).unwrap();
        let id = pair_from_code(&phone, &phone_secrets, &format!(" {text}
")).unwrap();
        assert_eq!(paired_account(&phone).unwrap().as_deref(), Some(id.as_str()));
        assert_eq!(key(&phone_secrets).unwrap(), key(&pc_secrets).unwrap(), "both hold the same key");
    }

    #[test]
    fn a_code_for_a_mailbox_not_on_the_phone_says_which_one() {
        let (pd, pc) = crate::store::testutil::store();
        let a = pc.add_account("Jan", "jan@x.nl", crate::model::AccountKind::Imap, None).unwrap();
        let text = serde_json::to_string(&pair_code(&pc, &Secrets::new(pd.path()), &a.id).unwrap()).unwrap();
        let (fd, phone) = crate::store::testutil::store();
        let secrets = Secrets::new(fd.path());
        phone.add_account("Piet", "piet@x.nl", crate::model::AccountKind::Imap, None).unwrap();
        let e = pair_from_code(&phone, &secrets, &text).unwrap_err();
        assert_eq!(e.code, "VEM-PAIR-FIND-01");
        assert_eq!(e.detail.as_deref(), Some("jan@x.nl"));
        assert_eq!(paired_account(&phone).unwrap(), None, "nothing kept");
        assert!(key(&secrets).unwrap().is_none());
    }

    #[test]
    fn something_else_than_our_code_is_refused() {
        let (fd, phone) = crate::store::testutil::store();
        let secrets = Secrets::new(fd.path());
        phone.add_account("Jan", "jan@x.nl", crate::model::AccountKind::Imap, None).unwrap();
        for text in [
            "https://example.test/",
            "",
            r#"{"v":2,"account":"jan@x.nl","key":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}"#,
            r#"{"v":1,"account":"jan@x.nl","key":"kort"}"#,
        ] {
            assert_eq!(pair_from_code(&phone, &secrets, text).unwrap_err().code, "VEM-PAIR-READ-01", "{text}");
        }
        assert_eq!(paired_account(&phone).unwrap(), None);
    }
}
