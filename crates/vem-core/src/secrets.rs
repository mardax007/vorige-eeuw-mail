//! Passwords and sign-in tokens, encrypted for the current Windows user with DPAPI
//! (`CryptProtectData`), one file per account in `<data>/secrets/` (CLAUDE.md, Secrets).
//!
//! Nothing here is ever logged; `Secret`'s Debug output hides the values.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Code, Result, VemError};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Secret {
    Password { password: String },
    #[serde(rename_all = "camelCase")]
    OAuth { access_token: String, refresh_token: Option<String>, expires_at: i64 },
    /// The key for the contact sync message (docs/PLAN.md, C10); random, made on pairing.
    #[serde(rename_all = "camelCase")]
    SyncKey { key: Vec<u8> },
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            Secret::Password { .. } => "Password",
            Secret::OAuth { .. } => "OAuth",
            Secret::SyncKey { .. } => "SyncKey",
        };
        write!(f, "Secret::{kind}(<hidden>)")
    }
}

/// Mixed into DPAPI so another program of the same user cannot decrypt our files by accident.
#[cfg(windows)]
const ENTROPY: &[u8] = b"tech.mardax.vem/secrets/v1";

pub struct Secrets {
    dir: PathBuf,
}

impl Secrets {
    pub fn new(data_dir: &Path) -> Secrets {
        Secrets { dir: data_dir.join("secrets") }
    }

    fn path(&self, name: &str) -> Result<PathBuf> {
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(VemError::new(Code::StoreSecret01).with_detail("name"));
        }
        Ok(self.dir.join(format!("{name}.bin")))
    }

    pub fn put(&self, name: &str, secret: &Secret) -> Result<()> {
        let path = self.path(name)?;
        std::fs::create_dir_all(&self.dir).map_err(secret_io)?;
        let plain = serde_json::to_vec(secret).map_err(|_| VemError::new(Code::StoreSecret01))?;
        let sealed = protect(&plain)?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, sealed).map_err(secret_io)?;
        std::fs::rename(&tmp, &path).map_err(secret_io)?;
        Ok(())
    }

    pub fn get(&self, name: &str) -> Result<Option<Secret>> {
        let path = self.path(name)?;
        let sealed = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(secret_io(e)),
        };
        let plain = unprotect(&sealed)?;
        serde_json::from_slice(&plain).map(Some).map_err(|_| VemError::new(Code::StoreSecret01).with_detail("format"))
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        match std::fs::remove_file(self.path(name)?) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(secret_io(e)),
            _ => Ok(()),
        }
    }
}

fn secret_io(e: std::io::Error) -> VemError {
    VemError::new(Code::StoreSecret01).with_detail(e.kind().to_string())
}

#[cfg(windows)]
fn protect(plain: &[u8]) -> Result<Vec<u8>> {
    dpapi::call(plain, true)
}

#[cfg(windows)]
fn unprotect(sealed: &[u8]) -> Result<Vec<u8>> {
    dpapi::call(sealed, false)
}

/// Android: the file holds the secret sealed with an AES key that lives in the Android Keystore
/// (made on first use, never leaves the keystore). Same file layout as Windows, so the damage
/// handling and tests above hold on both.
#[cfg(target_os = "android")]
fn protect(plain: &[u8]) -> Result<Vec<u8>> {
    keystore::seal(plain)
}

#[cfg(target_os = "android")]
fn unprotect(sealed: &[u8]) -> Result<Vec<u8>> {
    keystore::open(sealed)
}

#[cfg(not(any(windows, target_os = "android")))]
fn protect(_plain: &[u8]) -> Result<Vec<u8>> {
    // Only Windows and Android are targets (CLAUDE.md, Stack); no plain-text fallback.
    Err(VemError::new(Code::StoreSecret01).with_detail("unsupported"))
}

#[cfg(not(any(windows, target_os = "android")))]
fn unprotect(_sealed: &[u8]) -> Result<Vec<u8>> {
    Err(VemError::new(Code::StoreSecret01).with_detail("unsupported"))
}

/// Sealing with a 32-byte key (XChaCha20-Poly1305, random nonce in front). Used on Android with
/// the key from the keystore; plain Rust, so the tests run it on Windows too.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod sealing {
    use chacha20poly1305::aead::{Aead, KeyInit};
    use chacha20poly1305::{XChaCha20Poly1305, XNonce};

    use crate::error::{Code, Result, VemError};

    const NONCE: usize = 24;

    pub fn seal(key: &[u8; 32], plain: &[u8]) -> Result<Vec<u8>> {
        let nonce: [u8; NONCE] = rand::random();
        let ct = XChaCha20Poly1305::new(key.into())
            .encrypt(&XNonce::from(nonce), plain)
            .map_err(|_| VemError::new(Code::StoreSecret01).with_detail("protect"))?;
        let mut out = nonce.to_vec();
        out.extend_from_slice(&ct);
        Ok(out)
    }

    pub fn open(key: &[u8; 32], sealed: &[u8]) -> Result<Vec<u8>> {
        let unprotect = || VemError::new(Code::StoreSecret01).with_detail("unprotect");
        if sealed.len() < NONCE {
            return Err(unprotect());
        }
        let (n, ct) = sealed.split_at(NONCE);
        let mut nonce = [0u8; NONCE];
        nonce.copy_from_slice(n);
        XChaCha20Poly1305::new(key.into()).decrypt(&XNonce::from(nonce), ct).map_err(|_| unprotect())
    }
}

/// The sealing key, kept in the Android Keystore through `android-native-keyring-store`
/// (SharedPreferences encrypted with a key that never leaves the keystore). Made once, on the
/// first password saved; Tauri sets up the Android context the crate needs.
#[cfg(target_os = "android")]
mod keystore {
    use std::sync::Mutex;

    use android_native_keyring_store::by_store::store::Store;
    use keyring_core::api::CredentialStoreApi;

    use crate::error::{Code, Result, VemError};

    const SERVICE: &str = "tech.mardax.vem";
    const USER: &str = "secrets-key-v1";

    /// Held after the first read; the lock also stops two threads from each making a key.
    static KEY: Mutex<Option<[u8; 32]>> = Mutex::new(None);

    fn failed(step: &'static str) -> VemError {
        // Only a fixed word: the keystore's own error text never reaches a report.
        VemError::new(Code::StoreSecret01).with_detail(step)
    }

    fn key() -> Result<[u8; 32]> {
        let mut held = KEY.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(k) = *held {
            return Ok(k);
        }
        let store = Store::new().map_err(|_| failed("keystore"))?;
        let entry = store.build(SERVICE, USER, None).map_err(|_| failed("keystore"))?;
        let key = match entry.get_secret() {
            Ok(bytes) => <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| failed("keystore key"))?,
            Err(keyring_core::Error::NoEntry) => {
                let k: [u8; 32] = rand::random();
                entry.set_secret(&k).map_err(|_| failed("keystore"))?;
                k
            }
            Err(_) => return Err(failed("keystore")),
        };
        *held = Some(key);
        Ok(key)
    }

    pub fn seal(plain: &[u8]) -> Result<Vec<u8>> {
        super::sealing::seal(&key()?, plain)
    }

    pub fn open(sealed: &[u8]) -> Result<Vec<u8>> {
        super::sealing::open(&key()?, sealed)
    }
}

#[cfg(windows)]
mod dpapi {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
    };

    use super::ENTROPY;
    use crate::error::{Code, Result, VemError};

    pub fn call(input: &[u8], encrypt: bool) -> Result<Vec<u8>> {
        let len = u32::try_from(input.len()).map_err(|_| VemError::new(Code::StoreSecret01).with_detail("size"))?;
        let data_in = CRYPT_INTEGER_BLOB { cbData: len, pbData: input.as_ptr() as *mut u8 };
        let entropy = CRYPT_INTEGER_BLOB { cbData: ENTROPY.len() as u32, pbData: ENTROPY.as_ptr() as *mut u8 };
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        // SAFETY: every pointer refers to a live buffer of the stated length for the whole call;
        // DPAPI only reads the inputs and allocates `out`, which we free with LocalFree below.
        let ok = unsafe {
            if encrypt {
                CryptProtectData(
                    &data_in,
                    std::ptr::null(),
                    &entropy,
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut out,
                )
            } else {
                CryptUnprotectData(
                    &data_in,
                    std::ptr::null_mut(),
                    &entropy,
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut out,
                )
            }
        };
        if ok == 0 || out.pbData.is_null() {
            return Err(VemError::new(Code::StoreSecret01).with_detail(if encrypt { "protect" } else { "unprotect" }));
        }
        // SAFETY: DPAPI returned a buffer of `cbData` bytes at `pbData`.
        let bytes = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
        // SAFETY: the buffer was allocated by DPAPI with LocalAlloc and is not used after this.
        unsafe {
            LocalFree(out.pbData as _);
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_not_plain_on_disk() {
        let d = tempfile::tempdir().unwrap();
        let s = Secrets::new(d.path());
        assert_eq!(s.get("acc1").unwrap(), None);
        let secret = Secret::Password { password: "Zomer2026!geheim".into() };
        s.put("acc1", &secret).unwrap();
        let disk = std::fs::read(d.path().join("secrets/acc1.bin")).unwrap();
        assert!(!disk.windows(6).any(|w| w == b"geheim"), "password must not be readable on disk");
        assert_eq!(s.get("acc1").unwrap(), Some(secret));
        s.delete("acc1").unwrap();
        assert_eq!(s.get("acc1").unwrap(), None);
        s.delete("acc1").unwrap();
    }

    /// A damaged or emptied password file gives VEM-STORE-SECRET-01 (the screen then asks for
    /// the password again), never a panic and never a wrong password sent to the server.
    #[test]
    fn a_damaged_secret_file_asks_for_the_password_again() {
        let d = tempfile::tempdir().unwrap();
        let s = Secrets::new(d.path());
        s.put("acc1", &Secret::Password { password: "Zomer2026!geheim".into() }).unwrap();
        let path = d.path().join("secrets/acc1.bin");
        let good = std::fs::read(&path).unwrap();
        let mut flipped = good.clone();
        let mid = flipped.len() / 2;
        flipped[mid] ^= 0x40;
        for damaged in [flipped, good[..good.len() / 2].to_vec(), Vec::new(), b"geen geheim".to_vec()] {
            std::fs::write(&path, &damaged).unwrap();
            let err = s.get("acc1").expect_err("a damaged file is not read as a secret");
            assert_eq!(err.code(), Some(Code::StoreSecret01));
        }
        s.put("acc1", &Secret::Password { password: "nieuw".into() }).unwrap();
        assert_eq!(s.get("acc1").unwrap(), Some(Secret::Password { password: "nieuw".into() }), "typing it again repairs it");
    }

    /// The Android sealing: a round trip works, the password is not on disk in the clear, and a
    /// damaged copy or another key is refused instead of read as a wrong password.
    #[test]
    fn the_android_sealing_refuses_damage_and_other_keys() {
        let key: [u8; 32] = rand::random();
        let plain = br#"{"kind":"password","password":"Zomer2026!geheim"}"#;
        let sealed = sealing::seal(&key, plain).unwrap();
        assert!(!sealed.windows(6).any(|w| w == b"geheim"));
        assert_eq!(sealing::open(&key, &sealed).unwrap(), plain);
        let mut flipped = sealed.clone();
        let last = flipped.len() - 1;
        flipped[last] ^= 1;
        let other: [u8; 32] = rand::random();
        for (k, s) in [(&key, flipped), (&key, sealed[..10].to_vec()), (&key, Vec::new()), (&other, sealed.clone())] {
            let err = sealing::open(k, &s).expect_err("not read as a secret");
            assert_eq!(err.code(), Some(Code::StoreSecret01));
        }
    }

    #[test]
    fn debug_hides_values() {
        let s = Secret::OAuth { access_token: "ya29.abc".into(), refresh_token: Some("1//r".into()), expires_at: 0 };
        let shown = format!("{s:?}");
        assert!(!shown.contains("ya29") && !shown.contains("1//r"));
    }

    #[test]
    fn rejects_path_tricks() {
        let d = tempfile::tempdir().unwrap();
        let s = Secrets::new(d.path());
        assert!(s.put("../x", &Secret::Password { password: "p".into() }).is_err());
    }
}
