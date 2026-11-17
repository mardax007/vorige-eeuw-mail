//! The "Bijwerken" button on Android (decision 22, phase 5 task 8).
//!
//! Android never installs an update silently from outside a store. The app reads `android.json`
//! from the latest GitHub release (next to the Windows `latest.json`); when it names a newer
//! version, the screens show "Bijwerken". Pressing it downloads the APK from GitHub, checks it
//! against the update key that also signs the Windows update, and hands it to Android's own
//! install screen (the shell does that last step).

use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::error::{Code, Result, VemError};

/// The APK comes from GitHub only (CLAUDE.md, Network): the release page, which redirects to
/// GitHub's own download host.
const APK_HOST_PREFIX: &str = "https://github.com/";
/// A whole APK over a slow phone connection.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// `android.json`, written by the release workflow (`scripts/android-release.mjs`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidRelease {
    pub version: String,
    /// What is new, one line per point, as in the Windows release text.
    #[serde(default)]
    pub notes: String,
    pub url: String,
    /// The update key's signature over the APK, as `tauri signer sign` writes it.
    pub signature: String,
}

/// `android.json` sits next to the Windows `latest.json` in the same release.
pub fn release_url(windows_url: &str) -> Option<String> {
    windows_url.strip_suffix("latest.json").map(|base| format!("{base}android.json"))
}

/// "0.2.0" is newer than "0.1.9". Anything after `-` or `+` is ignored; a version that does not
/// read as three numbers is never newer.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (numbers(candidate), numbers(current)) {
        (Some(c), Some(now)) => c > now,
        _ => false,
    }
}

fn numbers(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.trim().trim_start_matches('v').split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let n = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(n)
}

/// Checks the APK against the update key. Both come base64-encoded, as Tauri's updater uses them.
pub fn verify(bytes: &[u8], signature_b64: &str, pubkey_b64: &str) -> Result<()> {
    let bad = |what: &str| VemError::new(Code::UpdateGet01).with_detail(what.to_string());
    let decode = |s: &str| {
        base64::engine::general_purpose::STANDARD
            .decode(s.trim())
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
    };
    let key_text = decode(pubkey_b64).ok_or_else(|| bad("update key"))?;
    let sig_text = decode(signature_b64).ok_or_else(|| bad("signature"))?;
    let key = minisign_verify::PublicKey::decode(&key_text).map_err(|_| bad("update key"))?;
    let sig = minisign_verify::Signature::decode(&sig_text).map_err(|_| bad("signature"))?;
    key.verify(bytes, &sig, true).map_err(|_| bad("signature does not match"))
}

/// The release named by `android.json`, when it is newer than this app.
pub async fn check(url: &str) -> Result<Option<AndroidRelease>> {
    let get = |e: reqwest::Error| VemError::new(Code::UpdateGet01).with_detail(e.to_string());
    let release: AndroidRelease = crate::lookup::http()?
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(get)?
        .json()
        .await
        .map_err(get)?;
    Ok(is_newer(&release.version, crate::version()).then_some(release))
}

/// Downloads the APK and checks it; only a checked APK comes back.
pub async fn download(release: &AndroidRelease, pubkey_b64: &str) -> Result<Vec<u8>> {
    if !release.url.starts_with(APK_HOST_PREFIX) {
        return Err(VemError::new(Code::UpdateGet01).with_detail("not a GitHub address"));
    }
    let get = |e: reqwest::Error| VemError::new(Code::UpdateGet01).with_detail(e.to_string());
    let bytes = crate::lookup::http_for(DOWNLOAD_TIMEOUT)?
        .get(&release.url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(get)?
        .bytes()
        .await
        .map_err(get)?;
    verify(&bytes, &release.signature, pubkey_b64)?;
    Ok(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    // A throwaway key made only for this test (`tauri signer generate`, no password); it signs
    // nothing real. FIXTURE is the file it signed.
    const TEST_PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDM3MTEwOUNCNEM5MTA5NjkKUldScENaRk15d2tSTitqQUlyYm5rYWx2Sk84Q0NpZkdMcndMQitHRk55b0QvMUFiNVpzdFY4WUUK";
    const TEST_SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVScENaRk15d2tSTndiN0IrUUNDVng2MTJ6amd2c2liQ3BOa3FvVHpybGtxS2VYV3JlSjdsdHAzQ3loOVZ5MHF2YUJwTCt4Y25YWEVBVlc4SE5zdXltN3B1YVFnN3g0NWdVPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0OTEzNDU2CWZpbGU6Zml4dHVyZS5iaW4KOXZ5b2czM05qak5sbkwwa2FRZnlpK0RLem1KNktUMG8wTGl4YU9keHZiZWhRVnVMNXd4d2RzTDlsWlc4OU52dzZyZ240UmNqWmdXRFZnR3ZIWTNBQ3c9PQo=";
    const FIXTURE: &[u8] = b"Vorige Eeuw Mail test APK";

    #[test]
    fn a_signed_apk_passes_and_a_changed_one_does_not() {
        assert!(verify(FIXTURE, TEST_SIGNATURE, TEST_PUBKEY).is_ok());
        let err = verify(b"Vorige Eeuw Mail test APX", TEST_SIGNATURE, TEST_PUBKEY).unwrap_err();
        assert_eq!(err.code, "VEM-UPDATE-GET-01");
        assert!(verify(FIXTURE, "geen handtekening", TEST_PUBKEY).is_err());
        assert!(verify(FIXTURE, TEST_SIGNATURE, "geen sleutel").is_err());
    }

    #[test]
    fn versions_compare_as_numbers() {
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(is_newer("v1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
        assert!(!is_newer("0.2", "0.1.0"), "two numbers is not a version");
        assert!(!is_newer("nieuw", "0.1.0"));
        assert!(is_newer("0.2.0-beta", "0.1.0"));
    }

    #[test]
    fn android_json_sits_next_to_latest_json() {
        assert_eq!(
            release_url("https://github.com/o/r/releases/latest/download/latest.json").as_deref(),
            Some("https://github.com/o/r/releases/latest/download/android.json")
        );
        assert_eq!(release_url("https://example.test/other.json"), None);
    }

    #[test]
    fn android_json_reads_without_notes() {
        let r: AndroidRelease = serde_json::from_str(r#"{"version":"0.2.0","url":"https://github.com/a.apk","signature":"x"}"#).unwrap();
        assert_eq!(r.version, "0.2.0");
        assert_eq!(r.notes, "");
    }

    #[tokio::test]
    async fn an_apk_from_elsewhere_is_refused_before_downloading() {
        let r = AndroidRelease {
            version: "9.9.9".into(),
            notes: String::new(),
            url: "https://example.test/vem.apk".into(),
            signature: TEST_SIGNATURE.into(),
        };
        let err = download(&r, TEST_PUBKEY).await.unwrap_err();
        assert_eq!(err.detail.as_deref(), Some("not a GitHub address"));
    }
}
