//! Sending over SMTP (RFC 5321) with the server's replies kept exactly as sent.
//!
//! A small client of our own instead of `lettre`: lettre joins multi-line replies and drops
//! repeated status codes (docs/research.md), and the error rule needs every line verbatim.

use base64::Engine;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

use crate::error::{Code, Result, VemError};
use crate::imap::Auth;
use crate::net::{self, Proto, Security, Trust};

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub security: Security,
    pub username: String,
    pub auth: Auth,
    pub trust: Trust,
}

/// One reply: the code and every line exactly as the server sent it (joined with CRLF).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub code: u16,
    pub raw: String,
}

impl Reply {
    fn ok(&self) -> bool {
        (200..400).contains(&self.code)
    }
    fn has(&self, word: &str) -> bool {
        self.raw.lines().any(|l| l.get(4..).is_some_and(|rest| rest.to_ascii_uppercase().starts_with(word)))
    }
    fn mentions(&self, word: &str) -> bool {
        self.raw.to_ascii_uppercase().contains(word)
    }
}

struct Conn<S> {
    io: BufReader<S>,
    host: String,
}

impl<S: AsyncRead + AsyncWrite + Unpin> Conn<S> {
    async fn reply(&mut self) -> Result<Reply> {
        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            let n = tokio::time::timeout(net::CONNECT_TIMEOUT * 3, self.io.read_line(&mut line))
                .await
                .map_err(|_| VemError::new(Code::SmtpConnect01).with_detail(self.host.clone()))?
                .map_err(|e| VemError::new(Code::SmtpConnect01).with_detail(e.kind().to_string()))?;
            if n == 0 {
                return Err(VemError::new(Code::SmtpConnect01)
                    .with_detail(self.host.clone())
                    .with_reply(lines.join("\r\n")));
            }
            let line = line.trim_end_matches(['\r', '\n']).to_string();
            let last = line.len() < 4 || line.as_bytes()[3] != b'-';
            lines.push(line);
            if last {
                break;
            }
        }
        let code = lines.last().and_then(|l| l.get(..3)).and_then(|c| c.parse().ok()).unwrap_or(0);
        Ok(Reply { code, raw: lines.join("\r\n") })
    }

    async fn send(&mut self, line: &str) -> Result<Reply> {
        self.io
            .get_mut()
            .write_all(format!("{line}\r\n").as_bytes())
            .await
            .map_err(|e| VemError::new(Code::SmtpConnect01).with_detail(e.kind().to_string()))?;
        self.reply().await
    }

    async fn ehlo(&mut self) -> Result<Reply> {
        let r = self.send("EHLO vorige-eeuw-mail.localhost").await?;
        if r.ok() {
            return Ok(r);
        }
        let r = self.send("HELO vorige-eeuw-mail.localhost").await?;
        if r.ok() { Ok(r) } else { Err(VemError::new(Code::SmtpConnect01).with_reply(r.raw)) }
    }

    async fn login(&mut self, ehlo: &Reply, user: &str, auth: &Auth) -> Result<()> {
        let b64 = base64::engine::general_purpose::STANDARD;
        let r = match auth {
            Auth::OAuth(token) => {
                let s = format!("user={user}\x01auth=Bearer {token}\x01\x01");
                self.send(&format!("AUTH XOAUTH2 {}", b64.encode(s))).await?
            }
            Auth::Password(pw) if ehlo.mentions("PLAIN") || !ehlo.mentions("LOGIN") => {
                let s = format!("\0{user}\0{pw}");
                self.send(&format!("AUTH PLAIN {}", b64.encode(s))).await?
            }
            Auth::Password(pw) => {
                let r = self.send("AUTH LOGIN").await?;
                if r.code != 334 {
                    return Err(VemError::new(Code::SmtpLogin01).with_reply(r.raw));
                }
                let r = self.send(&b64.encode(user)).await?;
                if r.code != 334 {
                    return Err(VemError::new(Code::SmtpLogin01).with_reply(r.raw));
                }
                self.send(&b64.encode(pw)).await?
            }
        };
        if r.code == 334 {
            // XOAUTH2 failure: the server sends a challenge with details, then expects an empty line.
            let r2 = self.send("").await?;
            return Err(VemError::new(Code::SmtpLogin01).with_reply(format!("{}\r\n{}", r.raw, r2.raw)));
        }
        if r.code != 235 {
            return Err(VemError::new(Code::SmtpLogin01).with_reply(r.raw));
        }
        Ok(())
    }

    /// MAIL, RCPT, DATA. A refused recipient stops the send: nobody gets half a message.
    async fn transaction(&mut self, from: &str, to: &[String], raw: &[u8], ehlo: &Reply) -> Result<()> {
        let size = if ehlo.mentions("SIZE") { format!(" SIZE={}", raw.len()) } else { String::new() };
        let r = self.send(&format!("MAIL FROM:<{from}>{size}")).await?;
        if !r.ok() {
            return Err(VemError::new(refusal_code(&r)).with_reply(r.raw));
        }
        for rcpt in to {
            let r = self.send(&format!("RCPT TO:<{rcpt}>")).await?;
            if !r.ok() {
                let _ = self.send("RSET").await;
                let code = if r.code == 552 || r.raw.contains("5.3.4") { Code::SmtpCmd02 } else { Code::SmtpCmd01 };
                return Err(VemError::new(code).with_reply(r.raw).with_detail(rcpt.clone()));
            }
        }
        let r = self.send("DATA").await?;
        if r.code != 354 {
            return Err(VemError::new(refusal_code(&r)).with_reply(r.raw));
        }
        let w = self.io.get_mut();
        let body = dot_stuff(raw);
        w.write_all(&body).await.map_err(|e| VemError::new(Code::SmtpConnect01).with_detail(e.kind().to_string()))?;
        w.write_all(b".\r\n").await.map_err(|e| VemError::new(Code::SmtpConnect01).with_detail(e.kind().to_string()))?;
        let r = self.reply().await?;
        if !r.ok() {
            return Err(VemError::new(refusal_code(&r)).with_reply(r.raw));
        }
        Ok(())
    }
}

fn refusal_code(r: &Reply) -> Code {
    if r.code == 552 || r.raw.contains("5.3.4") || r.raw.contains("5.2.3") {
        Code::SmtpCmd02
    } else {
        Code::SmtpCmd03
    }
}

/// Lines starting with "." get an extra "." (RFC 5321 §4.5.2); line endings become CRLF and the
/// message ends with CRLF.
pub fn dot_stuff(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() + 64);
    let mut at_line_start = true;
    let mut i = 0;
    while i < raw.len() {
        let b = raw[i];
        if at_line_start && b == b'.' {
            out.push(b'.');
        }
        if b == b'\n' && (i == 0 || raw[i - 1] != b'\r') {
            out.push(b'\r');
        }
        out.push(b);
        at_line_start = b == b'\n';
        i += 1;
    }
    if !out.ends_with(b"\r\n") {
        out.extend_from_slice(b"\r\n");
    }
    out
}

async fn run<S: AsyncRead + AsyncWrite + Unpin>(
    mut c: Conn<S>,
    cfg: &SmtpConfig,
    greeted: bool,
    from: &str,
    to: &[String],
    raw: &[u8],
) -> Result<()> {
    if !greeted {
        let g = c.reply().await?;
        if g.code != 220 {
            return Err(VemError::new(Code::SmtpConnect01).with_reply(g.raw));
        }
    }
    let ehlo = c.ehlo().await?;
    c.login(&ehlo, &cfg.username, &cfg.auth).await?;
    c.transaction(from, to, raw, &ehlo).await?;
    let _ = c.send("QUIT").await;
    Ok(())
}

/// Send one message. `to` holds every recipient (To, Cc and Bcc); `raw` has no Bcc header.
pub async fn send(cfg: &SmtpConfig, from: &str, to: &[String], raw: &[u8]) -> Result<()> {
    if to.is_empty() {
        return Err(VemError::new(Code::SmtpCmd01));
    }
    let tcp = net::tcp(&cfg.host, cfg.port, Proto::Smtp).await?;
    match cfg.security {
        Security::Tls => {
            let tls = net::upgrade(tcp, &cfg.host, cfg.trust, Proto::Smtp).await?;
            run(Conn { io: BufReader::new(tls), host: cfg.host.clone() }, cfg, false, from, to, raw).await
        }
        Security::Starttls => {
            let mut plain = Conn { io: BufReader::new(tcp), host: cfg.host.clone() };
            let g = plain.reply().await?;
            if g.code != 220 {
                return Err(VemError::new(Code::SmtpConnect01).with_reply(g.raw));
            }
            let ehlo = plain.ehlo().await?;
            if !ehlo.has("STARTTLS") {
                // Required STARTTLS that is not offered stops the send (CLAUDE.md, Network).
                return Err(VemError::new(Code::SmtpTls02).with_detail(cfg.host.clone()));
            }
            let r = plain.send("STARTTLS").await?;
            if r.code != 220 {
                return Err(VemError::new(Code::SmtpTls02).with_reply(r.raw));
            }
            let tcp = plain.io.into_inner();
            let tls = net::upgrade(tcp, &cfg.host, cfg.trust, Proto::Smtp).await?;
            run(Conn { io: BufReader::new(tls), host: cfg.host.clone() }, cfg, true, from, to, raw).await
        }
    }
}

/// Sign in only, for the account setup test.
pub async fn check_login(cfg: &SmtpConfig) -> Result<()> {
    let tcp = net::tcp(&cfg.host, cfg.port, Proto::Smtp).await?;
    async fn finish<S: AsyncRead + AsyncWrite + Unpin>(mut c: Conn<S>, cfg: &SmtpConfig, greeted: bool) -> Result<()> {
        if !greeted {
            let g = c.reply().await?;
            if g.code != 220 {
                return Err(VemError::new(Code::SmtpConnect01).with_reply(g.raw));
            }
        }
        let ehlo = c.ehlo().await?;
        c.login(&ehlo, &cfg.username, &cfg.auth).await?;
        let _ = c.send("QUIT").await;
        Ok(())
    }
    match cfg.security {
        Security::Tls => {
            let tls = net::upgrade(tcp, &cfg.host, cfg.trust, Proto::Smtp).await?;
            finish(Conn { io: BufReader::new(tls), host: cfg.host.clone() }, cfg, false).await
        }
        Security::Starttls => {
            let mut plain = Conn { io: BufReader::new(tcp), host: cfg.host.clone() };
            let g = plain.reply().await?;
            if g.code != 220 {
                return Err(VemError::new(Code::SmtpConnect01).with_reply(g.raw));
            }
            let ehlo = plain.ehlo().await?;
            if !ehlo.has("STARTTLS") {
                return Err(VemError::new(Code::SmtpTls02).with_detail(cfg.host.clone()));
            }
            let r = plain.send("STARTTLS").await?;
            if r.code != 220 {
                return Err(VemError::new(Code::SmtpTls02).with_reply(r.raw));
            }
            let tls = net::upgrade(plain.io.into_inner(), &cfg.host, cfg.trust, Proto::Smtp).await?;
            finish(Conn { io: BufReader::new(tls), host: cfg.host.clone() }, cfg, true).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    #[test]
    fn dot_stuffing() {
        assert_eq!(dot_stuff(b".a\n..b\r\nc."), b"..a\r\n...b\r\nc.\r\n".to_vec());
    }

    /// A scripted server: answers each client line with the next canned reply.
    async fn fake_server(script: Vec<&'static str>) -> (Conn<tokio::io::DuplexStream>, tokio::task::JoinHandle<Vec<String>>) {
        let (client, server) = duplex(64 * 1024);
        let handle = tokio::spawn(async move {
            let mut io = BufReader::new(server);
            let mut seen = Vec::new();
            let mut replies = script.into_iter();
            if let Some(g) = replies.next() {
                io.get_mut().write_all(g.as_bytes()).await.unwrap();
            }
            let mut in_data = false;
            loop {
                let mut line = String::new();
                if io.read_line(&mut line).await.unwrap_or(0) == 0 {
                    break;
                }
                let l = line.trim_end().to_string();
                if in_data {
                    if l == "." {
                        in_data = false;
                    } else {
                        continue;
                    }
                }
                if l == "DATA" {
                    in_data = true;
                }
                seen.push(l);
                match replies.next() {
                    Some(r) => io.get_mut().write_all(r.as_bytes()).await.unwrap(),
                    None => break,
                }
                if in_data && seen.last().is_some_and(|s| s != "DATA") {
                    in_data = false;
                }
            }
            seen
        });
        (Conn { io: BufReader::new(client), host: "test".into() }, handle)
    }

    fn cfg() -> SmtpConfig {
        SmtpConfig {
            host: "test".into(),
            port: 0,
            security: Security::Tls,
            username: "jan@x.nl".into(),
            auth: Auth::Password("geheim".into()),
            trust: Trust::Platform,
        }
    }

    #[tokio::test]
    async fn refused_recipient_keeps_reply_verbatim() {
        let (conn, server) = fake_server(vec![
            "220 hi\r\n",
            "250-x\r\n250-AUTH PLAIN LOGIN\r\n250 SIZE 1000\r\n",
            "235 ok\r\n",
            "250 ok\r\n",
            "550-5.1.1 The email account that you tried to reach does not exist.\r\n550-5.1.1  Please try\r\n550 5.1.1 double-checking\r\n",
            "250 reset\r\n",
        ])
        .await;
        let e = run(conn, &cfg(), false, "jan@x.nl", &["nobody@y.nl".into()], b"Subject: x\r\n\r\nhoi\r\n")
            .await
            .unwrap_err();
        assert_eq!(e.code, "VEM-SMTP-CMD-01");
        assert_eq!(
            e.server_reply.as_deref(),
            Some("550-5.1.1 The email account that you tried to reach does not exist.\r\n550-5.1.1  Please try\r\n550 5.1.1 double-checking")
        );
        let seen = server.await.unwrap();
        assert!(seen[1].starts_with("AUTH PLAIN "));
        assert!(!seen.iter().any(|l| l.contains("geheim")), "password must not be sent in clear text");
        assert_eq!(seen[2], "MAIL FROM:<jan@x.nl> SIZE=19");
    }

    #[tokio::test]
    async fn login_refusal_is_smtp_login() {
        let (conn, _server) = fake_server(vec![
            "220 hi\r\n",
            "250 AUTH LOGIN\r\n",
            "334 VXNlcm5hbWU6\r\n",
            "334 UGFzc3dvcmQ6\r\n",
            "535 5.7.8 Error: authentication failed: UGFzc3dvcmQ6\r\n",
        ])
        .await;
        let e = run(conn, &cfg(), false, "jan@x.nl", &["a@b.nl".into()], b"x").await.unwrap_err();
        assert_eq!(e.code, "VEM-SMTP-LOGIN-01");
        assert_eq!(e.server_reply.as_deref(), Some("535 5.7.8 Error: authentication failed: UGFzc3dvcmQ6"));
    }

    #[tokio::test]
    async fn too_big_is_its_own_code() {
        let (conn, _server) = fake_server(vec![
            "220 hi\r\n",
            "250 AUTH PLAIN\r\n",
            "235 ok\r\n",
            "552 5.3.4 Message size exceeds fixed limit\r\n",
        ])
        .await;
        let e = run(conn, &cfg(), false, "jan@x.nl", &["a@b.nl".into()], b"x").await.unwrap_err();
        assert_eq!(e.code, "VEM-SMTP-CMD-02");
    }
}
