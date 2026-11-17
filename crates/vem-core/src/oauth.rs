//! Signing in with Google or Microsoft (decisions 8 and 9): OAuth 2 with PKCE and a loopback
//! redirect on 127.0.0.1, in the user's own browser. The tokens go to the secrets file only.
//!
//! The client ids come from build-time secrets; without them the sign-in is not offered.

use std::time::Duration;

use base64::Engine;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::error::{Code, Result, VemError};
use crate::model::{SignIn, now};
use crate::secrets::Secret;

pub struct ProviderInfo {
    auth_url: &'static str,
    token_url: &'static str,
    scope: &'static str,
    client_id: Option<&'static str>,
    client_secret: Option<&'static str>,
    redirect_host: &'static str,
}

pub fn provider(sign_in: SignIn) -> Option<ProviderInfo> {
    match sign_in {
        SignIn::Password => None,
        SignIn::Google => Some(ProviderInfo {
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth",
            token_url: "https://oauth2.googleapis.com/token",
            scope: "https://mail.google.com/",
            client_id: option_env!("VEM_GOOGLE_CLIENT_ID"),
            // Google's "desktop app" clients have a secret that is not secret (RFC 8252 §8.5).
            client_secret: option_env!("VEM_GOOGLE_CLIENT_SECRET"),
            redirect_host: "127.0.0.1",
        }),
        SignIn::Microsoft => Some(ProviderInfo {
            auth_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
            token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
            scope: "https://outlook.office.com/IMAP.AccessAsUser.All https://outlook.office.com/SMTP.Send offline_access",
            client_id: option_env!("VEM_MICROSOFT_CLIENT_ID"),
            client_secret: None,
            redirect_host: "localhost",
        }),
    }
}

/// Whether this build can sign in this way.
pub fn available(sign_in: SignIn) -> bool {
    let has_client = provider(sign_in).is_some_and(|p| p.client_id.is_some());
    offered(sign_in, cfg!(target_os = "android"), has_client)
}

/// Google blocks the loopback redirect for apps on Android and wants its Android library there,
/// tied to the signing key; a Gmail mailbox on a phone takes an app password instead (phase 5
/// task 3). Microsoft accepts `http://localhost` from any public client, so it stays.
fn offered(sign_in: SignIn, on_android: bool, has_client: bool) -> bool {
    match sign_in {
        SignIn::Password => false,
        SignIn::Google => has_client && !on_android,
        SignIn::Microsoft => has_client,
    }
}

pub struct Pending {
    info: ProviderInfo,
    listener: TcpListener,
    verifier: String,
    state: String,
    redirect: String,
    pub url: String,
}

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn dec(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                let h = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
                match h {
                    Some(v) => {
                        out.push(v);
                        i += 3;
                        continue;
                    }
                    None => out.push(b'%'),
                }
            }
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Start: listen on a free local port and build the page to open in the browser.
pub async fn begin(sign_in: SignIn, email: &str) -> Result<Pending> {
    let info = provider(sign_in).ok_or_else(|| VemError::new(Code::AuthLogin01).with_detail("no provider"))?;
    let client_id = info.client_id.ok_or_else(|| VemError::new(Code::AuthLogin01).with_detail("not in this build"))?;
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.map_err(|e| VemError::new(Code::AuthLogin01).with_detail(e.kind().to_string()))?;
    let port = listener.local_addr().map_err(|e| VemError::new(Code::AuthLogin01).with_detail(e.kind().to_string()))?.port();
    let redirect = format!("http://{}:{port}/", info.redirect_host);
    let verifier = b64url(&rand::random::<[u8; 32]>());
    let challenge = b64url(&Sha256::digest(verifier.as_bytes()));
    let state = b64url(&rand::random::<[u8; 16]>());
    let url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={challenge}&code_challenge_method=S256&state={state}&login_hint={}&access_type=offline&prompt=consent",
        info.auth_url,
        enc(client_id),
        enc(&redirect),
        enc(info.scope),
        enc(email)
    );
    Ok(Pending { info, listener, verifier, state, redirect, url })
}

#[derive(Deserialize)]
struct TokenReply {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

fn query_param(target: &str, key: &str) -> Option<String> {
    let q = target.split_once('?')?.1;
    q.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then(|| dec(v))
    })
}

impl Pending {
    /// Wait for the browser to come back (at most `timeout`), answer it with `done_page`, and
    /// trade the code for tokens.
    pub async fn finish(self, timeout: Duration, done_page: &str) -> Result<Secret> {
        let (mut sock, _) = tokio::time::timeout(timeout, self.listener.accept())
            .await
            .map_err(|_| VemError::new(Code::AuthLogin01).with_detail("timeout"))?
            .map_err(|e| VemError::new(Code::AuthLogin01).with_detail(e.kind().to_string()))?;
        let mut buf = vec![0u8; 8192];
        let n = tokio::time::timeout(Duration::from_secs(10), sock.read(&mut buf))
            .await
            .map_err(|_| VemError::new(Code::AuthLogin01).with_detail("timeout"))?
            .map_err(|e| VemError::new(Code::AuthLogin01).with_detail(e.kind().to_string()))?;
        let req = String::from_utf8_lossy(&buf[..n]).to_string();
        let target = req.lines().next().and_then(|l| l.split_whitespace().nth(1)).unwrap_or("").to_string();
        let page = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<!doctype html><meta charset=utf-8><body style=\"font:20px Segoe UI,sans-serif;margin:40px\">{done_page}</body>"
        );
        let _ = sock.write_all(page.as_bytes()).await;
        let _ = sock.shutdown().await;

        if query_param(&target, "state").as_deref() != Some(self.state.as_str()) {
            return Err(VemError::new(Code::AuthLogin01).with_detail("state"));
        }
        if let Some(err) = query_param(&target, "error") {
            return Err(VemError::new(Code::AuthLogin02).with_reply(err));
        }
        let code = query_param(&target, "code").ok_or_else(|| VemError::new(Code::AuthLogin01).with_detail("no code"))?;
        let mut form = vec![
            ("grant_type", "authorization_code".to_string()),
            ("code", code),
            ("redirect_uri", self.redirect.clone()),
            ("code_verifier", self.verifier.clone()),
            ("client_id", self.info.client_id.unwrap_or_default().to_string()),
        ];
        if let Some(s) = self.info.client_secret {
            form.push(("client_secret", s.to_string()));
        }
        token_request(self.info.token_url, &form, None).await
    }
}

async fn token_request(url: &str, form: &[(&str, String)], old_refresh: Option<&str>) -> Result<Secret> {
    let body: String = form.iter().map(|(k, v)| format!("{k}={}", enc(v))).collect::<Vec<_>>().join("&");
    let resp = crate::lookup::http()?
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|e| VemError::new(Code::AuthLogin01).with_detail(if e.is_timeout() { "timeout" } else { "connect" }))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|_| VemError::new(Code::AuthLogin01).with_detail("read"))?;
    if !status.is_success() {
        return Err(VemError::new(Code::AuthLogin02).with_reply(text));
    }
    let t: TokenReply = serde_json::from_str(&text).map_err(|_| VemError::new(Code::AuthLogin01).with_detail("format"))?;
    Ok(Secret::OAuth {
        access_token: t.access_token,
        refresh_token: t.refresh_token.or(old_refresh.map(str::to_string)),
        expires_at: now() + t.expires_in.unwrap_or(3600),
    })
}

/// A fresh access token when the old one is (nearly) expired.
pub async fn refresh(sign_in: SignIn, secret: &Secret) -> Result<Option<Secret>> {
    let Secret::OAuth { refresh_token, expires_at, .. } = secret else { return Ok(None) };
    if *expires_at > now() + 120 {
        return Ok(None);
    }
    let Some(rt) = refresh_token else { return Err(VemError::new(Code::AuthLogin02).with_detail("no refresh token")) };
    let info = provider(sign_in).ok_or_else(|| VemError::new(Code::AuthLogin01))?;
    let mut form = vec![
        ("grant_type", "refresh_token".to_string()),
        ("refresh_token", rt.clone()),
        ("client_id", info.client_id.unwrap_or_default().to_string()),
    ];
    if let Some(s) = info.client_secret {
        form.push(("client_secret", s.to_string()));
    }
    token_request(info.token_url, &form, Some(rt)).await.map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_coding_round_trip() {
        let s = "https://mail.google.com/ a&b=c";
        assert_eq!(dec(&enc(s)), s);
        assert_eq!(query_param("/?code=4%2F0Ab&state=xyz", "code").as_deref(), Some("4/0Ab"));
    }

    #[test]
    fn password_has_no_provider() {
        assert!(provider(SignIn::Password).is_none());
    }

    #[test]
    fn google_takes_an_app_password_on_a_phone() {
        assert!(!offered(SignIn::Google, true, true), "Google blocks the loopback flow on Android");
        assert!(offered(SignIn::Google, false, true));
        assert!(offered(SignIn::Microsoft, true, true), "Microsoft allows http://localhost there");
        assert!(!offered(SignIn::Microsoft, true, false), "no client id, no browser sign-in");
        assert!(!offered(SignIn::Password, false, true));
    }
}
