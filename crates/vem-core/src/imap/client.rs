//! One IMAP connection: connect over TLS (or required STARTTLS), sign in, and the commands sync
//! needs. Every failure carries the server's exact last line (docs/errors.md).

use std::collections::HashSet;

use async_imap::types::{Fetch, Flag, NameAttribute};
use async_imap::{Client, Session};
use futures::TryStreamExt;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::utf7;
use crate::error::{Code, Result, VemError};
use crate::model::{FolderRole, Quota};
use crate::net::{self, BoxStream, Proto, Recorder, Security, Tap, Trust};

/// How to sign in. The secret is held only for the call and never logged.
#[derive(Clone)]
pub enum Auth {
    Password(String),
    /// An OAuth access token (Google, Microsoft), sent with XOAUTH2.
    OAuth(String),
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Auth::Password(_) => "Password(..)",
            Auth::OAuth(_) => "OAuth(..)",
        })
    }
}

#[derive(Debug, Clone)]
pub struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub security: Security,
    pub username: String,
    pub auth: Auth,
    pub trust: Trust,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFolder {
    /// The name as the server knows it (modified UTF-7), used in commands.
    pub name: String,
    pub delimiter: Option<String>,
    pub role: Option<FolderRole>,
    pub selectable: bool,
    /// Gmail's All Mail and Important repeat every message; sync skips them (docs/sync.md).
    pub skip: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MailboxState {
    pub uidvalidity: Option<u32>,
    pub uidnext: Option<u32>,
    pub exists: u32,
    pub highest_modseq: Option<u64>,
    /// `\*` in PERMANENTFLAGS: new keywords (our tags) can be stored.
    pub keywords_ok: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RemoteMessage {
    pub uid: u32,
    pub seen: bool,
    pub flagged: bool,
    pub keywords: Vec<String>,
    pub size: u64,
    pub internal_date: Option<i64>,
    /// The header block (or the whole message when fetched with the body).
    pub raw: Vec<u8>,
    pub gmail_labels: Vec<String>,
}

struct XOAuth2 {
    user: String,
    token: String,
}

impl async_imap::Authenticator for XOAuth2 {
    type Response = String;
    fn process(&mut self, _challenge: &[u8]) -> String {
        format!("user={}\x01auth=Bearer {}\x01\x01", self.user, self.token)
    }
}

type Stream = Tap<BoxStream>;

pub struct Imap {
    session: Session<Stream>,
    rec: Recorder,
    caps: HashSet<String>,
}

impl std::fmt::Debug for Imap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Imap").field("caps", &self.caps).finish()
    }
}

/// Read one CRLF line during the plain part of a STARTTLS connection.
async fn read_line<R: tokio::io::AsyncBufRead + Unpin>(r: &mut R, host: &str) -> Result<String> {
    let mut line = String::new();
    match tokio::time::timeout(net::CONNECT_TIMEOUT, r.read_line(&mut line)).await {
        Ok(Ok(0)) => Err(VemError::new(Code::ImapConnect01).with_detail(host.to_string())),
        Ok(Ok(_)) => Ok(line.trim_end_matches(['\r', '\n']).to_string()),
        Ok(Err(e)) => Err(VemError::new(Code::ImapConnect01).with_detail(format!("{host}: {}", e.kind()))),
        Err(_) => Err(VemError::new(Code::ImapConnect02).with_detail(host.to_string())),
    }
}

/// Plain greeting, CAPABILITY, STARTTLS. STARTTLS not offered stops here (CLAUDE.md, Network).
async fn starttls(tcp: tokio::net::TcpStream, host: &str) -> Result<tokio::net::TcpStream> {
    let mut r = BufReader::new(tcp);
    let greeting = read_line(&mut r, host).await?;
    if !greeting.starts_with("* OK") {
        return Err(VemError::new(Code::ImapConnect01).with_reply(greeting));
    }
    r.get_mut().write_all(b"s1 CAPABILITY\r\n").await.map_err(|_| VemError::new(Code::ImapConnect01))?;
    let mut offered = greeting.to_ascii_uppercase().contains("STARTTLS");
    loop {
        let line = read_line(&mut r, host).await?;
        if line.starts_with("* ") && line.to_ascii_uppercase().contains("STARTTLS") {
            offered = true;
        }
        if line.starts_with("s1 ") {
            break;
        }
    }
    if !offered {
        return Err(VemError::new(Code::ImapTls02).with_detail(host.to_string()));
    }
    r.get_mut().write_all(b"s2 STARTTLS\r\n").await.map_err(|_| VemError::new(Code::ImapConnect01))?;
    loop {
        let line = read_line(&mut r, host).await?;
        if let Some(rest) = line.strip_prefix("s2 ") {
            if rest.starts_with("OK") {
                break;
            }
            return Err(VemError::new(Code::ImapTls02).with_reply(line));
        }
    }
    Ok(r.into_inner())
}

fn role_from(attrs: &[NameAttribute<'_>], name: &str) -> (Option<FolderRole>, bool) {
    let mut skip = false;
    for a in attrs {
        let role = match a {
            NameAttribute::Sent => Some(FolderRole::Sent),
            NameAttribute::Trash => Some(FolderRole::Trash),
            NameAttribute::Drafts => Some(FolderRole::Drafts),
            NameAttribute::Junk => Some(FolderRole::Junk),
            NameAttribute::Archive => Some(FolderRole::Archive),
            NameAttribute::All | NameAttribute::Flagged => {
                skip = true;
                None
            }
            NameAttribute::Extension(e) if e.eq_ignore_ascii_case("\\Important") => {
                skip = true;
                None
            }
            _ => None,
        };
        if role.is_some() {
            return (role, false);
        }
    }
    if name.eq_ignore_ascii_case("INBOX") {
        return (Some(FolderRole::Inbox), false);
    }
    (guess_role(&utf7::decode(name)), skip)
}

/// Servers without SPECIAL-USE: recognise the usual names (Dutch and English).
pub fn guess_role(name: &str) -> Option<FolderRole> {
    let last = name.rsplit(['/', '.']).next().unwrap_or(name).to_lowercase();
    let is = |list: &[&str]| list.iter().any(|n| last == *n);
    if is(&["sent", "sent items", "sent messages", "verzonden", "verzonden items", "verzonden berichten"]) {
        Some(FolderRole::Sent)
    } else if is(&["trash", "deleted", "deleted items", "deleted messages", "prullenbak", "verwijderde items", "verwijderd", "bin"]) {
        Some(FolderRole::Trash)
    } else if is(&["drafts", "draft", "concepten", "concept"]) {
        Some(FolderRole::Drafts)
    } else if is(&["junk", "spam", "junk e-mail", "junk email", "ongewenste e-mail", "ongewenst"]) {
        Some(FolderRole::Junk)
    } else if is(&["archive", "archief"]) {
        Some(FolderRole::Archive)
    } else {
        None
    }
}

fn fetch_to_remote(f: &Fetch) -> Option<RemoteMessage> {
    let uid = f.uid?;
    let mut m = RemoteMessage { uid, size: f.size.unwrap_or(0) as u64, ..Default::default() };
    for flag in f.flags() {
        match flag {
            Flag::Seen => m.seen = true,
            Flag::Flagged => m.flagged = true,
            Flag::Custom(k) if !k.starts_with('\\') => m.keywords.push(k.to_string()),
            _ => {}
        }
    }
    m.internal_date = f.internal_date().map(|d| d.timestamp());
    m.raw = f.body().or_else(|| f.header()).map(<[u8]>::to_vec).unwrap_or_default();
    if let Some(labels) = f.gmail_labels() {
        m.gmail_labels = labels.iter().map(|l| l.to_string()).collect();
    }
    Some(m)
}

/// A quoted IMAP string; folder names come from the server or our own fixed names.
fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

impl Imap {
    pub async fn connect(cfg: &ImapConfig) -> Result<Imap> {
        let tcp = net::tcp(&cfg.host, cfg.port, Proto::Imap).await?;
        let rec = Recorder::default();
        let stream: BoxStream = match cfg.security {
            Security::Tls => Box::new(net::upgrade(tcp, &cfg.host, cfg.trust, Proto::Imap).await?),
            Security::Starttls => {
                let tcp = starttls(tcp, &cfg.host).await?;
                Box::new(net::upgrade(tcp, &cfg.host, cfg.trust, Proto::Imap).await?)
            }
        };
        let mut client = Client::new(Tap::new(stream, rec.clone()));
        if cfg.security == Security::Tls {
            match tokio::time::timeout(net::CONNECT_TIMEOUT, client.read_response()).await {
                Ok(Ok(Some(_))) => {}
                Ok(_) => return Err(VemError::new(Code::ImapConnect01).with_reply(rec.last_line())),
                Err(_) => return Err(VemError::new(Code::ImapConnect02).with_detail(cfg.host.clone())),
            }
        }
        rec.clear();
        let login = match &cfg.auth {
            Auth::Password(p) => client.login(&cfg.username, p).await.map_err(|(e, _)| e),
            Auth::OAuth(t) => client
                .authenticate("XOAUTH2", XOAuth2 { user: cfg.username.clone(), token: t.clone() })
                .await
                .map_err(|(e, _)| e),
        };
        let session = login.map_err(|e| login_error(&e, &rec))?;
        let mut imap = Imap { session, rec, caps: HashSet::new() };
        imap.refresh_capabilities().await?;
        Ok(imap)
    }


    async fn refresh_capabilities(&mut self) -> Result<()> {
        let rec = self.rec.clone();
        let caps = self.session.capabilities().await.map_err(|e| fail(&rec, Code::ImapCmd01, e))?;
        self.caps = caps
            .iter()
            .filter_map(|c| match c {
                async_imap::types::Capability::Atom(a) => Some(a.to_ascii_uppercase()),
                async_imap::types::Capability::Imap4rev1 => Some("IMAP4REV1".into()),
                async_imap::types::Capability::Auth(a) => Some(format!("AUTH={}", a.to_ascii_uppercase())),
            })
            .collect();
        Ok(())
    }

    pub fn has(&self, cap: &str) -> bool {
        self.caps.contains(&cap.to_ascii_uppercase())
    }

    pub fn capabilities(&self) -> Vec<String> {
        let mut v: Vec<String> = self.caps.iter().cloned().collect();
        v.sort();
        v
    }

    pub fn is_gmail(&self) -> bool {
        self.has("X-GM-EXT-1")
    }

    pub async fn list_folders(&mut self) -> Result<Vec<RemoteFolder>> {
        let rec = self.rec.clone();
        let names: Vec<_> = {
            let stream = self.session.list(Some(""), Some("*")).await.map_err(|e| fail(&rec, Code::ImapCmd01, e))?;
            stream.try_collect().await.map_err(|e| VemError::new(Code::ImapCmd01).with_detail(e.to_string()))?
        };
        Ok(names
            .iter()
            .map(|n| {
                let attrs = n.attributes();
                let (role, skip) = role_from(attrs, n.name());
                RemoteFolder {
                    name: n.name().to_string(),
                    delimiter: n.delimiter().map(str::to_string),
                    role,
                    selectable: !attrs.iter().any(|a| matches!(a, NameAttribute::NoSelect)),
                    skip,
                }
            })
            .collect())
    }

    /// SELECT, or EXAMINE when `read_only` (spike checks on real accounts: nothing changes).
    pub async fn open(&mut self, name: &str, read_only: bool) -> Result<MailboxState> {
        let rec = self.rec.clone();
        let mb = if read_only {
            self.session.examine(name).await
        } else if self.has("CONDSTORE") {
            self.session.select_condstore(name).await
        } else {
            self.session.select(name).await
        };
        let mb = mb.map_err(|e| fail(&rec, Code::ImapCmd01, e))?;
        Ok(MailboxState {
            uidvalidity: mb.uid_validity,
            uidnext: mb.uid_next,
            exists: mb.exists,
            highest_modseq: mb.highest_modseq,
            keywords_ok: mb.permanent_flags.iter().any(|f| matches!(f, Flag::MayCreate)),
        })
    }

    fn fetch_items(&self, body: bool) -> String {
        let part = if body { "BODY.PEEK[]" } else { "BODY.PEEK[HEADER]" };
        let gm = if self.is_gmail() { " X-GM-LABELS" } else { "" };
        format!("(UID FLAGS RFC822.SIZE INTERNALDATE {part}{gm})")
    }

    /// Headers (or whole messages) for a UID set such as `"1:*"` or `"5,7,9"`.
    pub async fn fetch(&mut self, uid_set: &str, body: bool) -> Result<Vec<RemoteMessage>> {
        let rec = self.rec.clone();
        let items = self.fetch_items(body);
        let fetches: Vec<Fetch> = {
            let stream = self.session.uid_fetch(uid_set, &items).await.map_err(|e| fail(&rec, Code::ImapCmd01, e))?;
            stream.try_collect().await.map_err(|e| VemError::new(Code::ImapCmd02).with_detail(e.to_string()))?
        };
        Ok(fetches.iter().filter_map(fetch_to_remote).collect())
    }

    /// FLAGS only, to see what changed elsewhere (docs/sync.md, Finding changes).
    pub async fn fetch_flags(&mut self, uid_set: &str) -> Result<Vec<RemoteMessage>> {
        let rec = self.rec.clone();
        let gm = if self.is_gmail() { " X-GM-LABELS" } else { "" };
        let fetches: Vec<Fetch> = {
            let stream = self
                .session
                .uid_fetch(uid_set, format!("(UID FLAGS{gm})"))
                .await
                .map_err(|e| fail(&rec, Code::ImapCmd01, e))?;
            stream.try_collect().await.map_err(|e| VemError::new(Code::ImapCmd02).with_detail(e.to_string()))?
        };
        Ok(fetches.iter().filter_map(fetch_to_remote).collect())
    }

    pub async fn uids(&mut self) -> Result<HashSet<u32>> {
        let rec = self.rec.clone();
        self.session.uid_search("ALL").await.map_err(|e| fail(&rec, Code::ImapCmd01, e))
    }

    pub async fn store(&mut self, uids: &[u32], add: bool, flags: &str) -> Result<()> {
        let rec = self.rec.clone();
        if uids.is_empty() {
            return Ok(());
        }
        let set = uid_set(uids);
        let op = if add { "+FLAGS.SILENT" } else { "-FLAGS.SILENT" };
        let stream = self.session.uid_store(&set, format!("{op} ({flags})")).await.map_err(|e| fail(&rec, Code::ImapCmd01, e))?;
        let _: Vec<Fetch> = stream.try_collect().await.map_err(|e| VemError::new(Code::ImapCmd01).with_detail(e.to_string()))?;
        Ok(())
    }

    pub async fn gmail_label(&mut self, uids: &[u32], add: bool, label: &str) -> Result<()> {
        let rec = self.rec.clone();
        let op = if add { "+X-GM-LABELS" } else { "-X-GM-LABELS" };
        let cmd = format!("UID STORE {} {op} ({})", uid_set(uids), quote(label));
        self.session.run_command_and_check_ok(cmd).await.map_err(|e| fail(&rec, Code::ImapCmd01, e))
    }

    /// Move messages to another folder: MOVE where offered, else COPY and delete.
    pub async fn move_to(&mut self, uids: &[u32], target: &str) -> Result<()> {
        let rec = self.rec.clone();
        if uids.is_empty() {
            return Ok(());
        }
        let set = uid_set(uids);
        if self.has("MOVE") {
            return self.session.uid_mv(&set, target).await.map_err(|e| fail(&rec, Code::ImapCmd01, e));
        }
        self.session.uid_copy(&set, target).await.map_err(|e| fail(&rec, Code::ImapCmd01, e))?;
        self.expunge_uids(uids).await
    }

    /// Mark deleted and remove only these messages (UID EXPUNGE where the server has UIDPLUS).
    pub async fn expunge_uids(&mut self, uids: &[u32]) -> Result<()> {
        let rec = self.rec.clone();
        self.store(uids, true, "\\Deleted").await?;
        let set = uid_set(uids);
        if self.has("UIDPLUS") {
            let s = self.session.uid_expunge(&set).await.map_err(|e| fail(&rec, Code::ImapCmd01, e))?;
            let _: Vec<_> = s.try_collect().await.map_err(|e| VemError::new(Code::ImapCmd01).with_detail(e.to_string()))?;
        } else {
            let s = self.session.expunge().await.map_err(|e| fail(&rec, Code::ImapCmd01, e))?;
            let _: Vec<_> = s.try_collect().await.map_err(|e| VemError::new(Code::ImapCmd01).with_detail(e.to_string()))?;
        }
        Ok(())
    }

    pub async fn append(&mut self, folder: &str, seen: bool, raw: &[u8]) -> Result<()> {
        let rec = self.rec.clone();
        let flags = if seen { Some("(\\Seen)") } else { None };
        self.session.append(folder, flags, None, raw).await.map_err(|e| fail(&rec, Code::ImapCmd01, e))
    }

    pub async fn create_folder(&mut self, name: &str) -> Result<()> {
        let rec = self.rec.clone();
        self.session.create(name).await.map_err(|e| fail(&rec, Code::ImapCmd01, e))
    }

    /// Mailbox space from IMAP QUOTA (RFC 9208), in KB. `None` when the server does not say.
    pub async fn quota(&mut self) -> Result<Option<Quota>> {
        if !self.has("QUOTA") {
            return Ok(None);
        }
        let Ok((_, quotas)) = self.session.get_quota_root("INBOX").await else { return Ok(None) };
        for q in quotas {
            for r in q.resources {
                if matches!(r.name, async_imap::types::QuotaResourceName::Storage) {
                    return Ok(Some(Quota { used_kb: r.usage, limit_kb: Some(r.limit) }));
                }
            }
        }
        Ok(None)
    }

    pub async fn noop(&mut self) -> Result<()> {
        let rec = self.rec.clone();
        self.session.noop().await.map_err(|e| fail(&rec, Code::ImapCmd02, e))
    }

    /// Wait for new mail on the selected folder with IDLE (or sleep, then NOOP, without it).
    /// `wake` ends the wait early and cleanly, so the connection stays usable. Returns `true`
    /// when the server reported a change.
    pub async fn wait_for_change(self, timeout: std::time::Duration, wake: &tokio::sync::Notify) -> Result<(Imap, bool)> {
        let Imap { session, rec, caps } = self;
        if !caps.contains("IDLE") {
            tokio::select! {
                _ = tokio::time::sleep(timeout) => {}
                _ = wake.notified() => {}
            }
            let mut me = Imap { session, rec, caps };
            me.noop().await?;
            return Ok((me, true));
        }
        let mut idle = session.idle();
        idle.init().await.map_err(|e| VemError::new(Code::ImapCmd02).with_detail(e.to_string()))?;
        let changed = {
            let (wait, stop) = idle.wait_with_timeout(timeout);
            tokio::pin!(wait);
            let mut stop = Some(stop);
            let first = tokio::select! {
                r = &mut wait => Some(r),
                _ = wake.notified() => None,
            };
            let r = match first {
                Some(r) => r,
                None => {
                    drop(stop.take());
                    (&mut wait).await
                }
            };
            drop(stop);
            matches!(r, Ok(async_imap::extensions::idle::IdleResponse::NewData(_)))
        };
        let session = idle.done().await.map_err(|e| VemError::new(Code::ImapCmd02).with_detail(e.to_string()))?;
        Ok((Imap { session, rec, caps }, changed))
    }

    pub async fn logout(mut self) {
        let _ = self.session.logout().await;
    }
}

fn fail(rec: &Recorder, code: Code, e: async_imap::error::Error) -> VemError {
    let code = match e {
        async_imap::error::Error::Io(_) | async_imap::error::Error::ConnectionLost => Code::ImapCmd02,
        _ => code,
    };
    VemError::new(code).with_reply(rec.last_line())
}

fn login_error(e: &async_imap::error::Error, rec: &Recorder) -> VemError {
    let reply = rec.last_line();
    let lower = reply.to_ascii_lowercase();
    let code = match e {
        async_imap::error::Error::Io(_) | async_imap::error::Error::ConnectionLost => Code::ImapConnect01,
        _ if lower.contains("application-specific password")
            || lower.contains("web login required")
            || lower.contains("please log in via your web browser")
            || lower.contains("app password") =>
        {
            Code::ImapLogin02
        }
        _ => Code::ImapLogin01,
    };
    VemError::new(code).with_reply(reply)
}

/// A compact UID set: "1:3,7,9:10".
pub fn uid_set(uids: &[u32]) -> String {
    let mut v: Vec<u32> = uids.to_vec();
    v.sort_unstable();
    v.dedup();
    let mut out = Vec::new();
    let mut i = 0;
    while i < v.len() {
        let start = v[i];
        let mut end = start;
        while i + 1 < v.len() && v[i + 1] == end + 1 {
            i += 1;
            end = v[i];
        }
        out.push(if start == end { start.to_string() } else { format!("{start}:{end}") });
        i += 1;
    }
    out.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uid_sets_compress() {
        assert_eq!(uid_set(&[9, 1, 2, 3, 7, 10, 2]), "1:3,7,9:10");
        assert_eq!(uid_set(&[5]), "5");
    }

    #[test]
    fn guesses_dutch_and_english_names() {
        assert_eq!(guess_role("Verzonden items"), Some(FolderRole::Sent));
        assert_eq!(guess_role("INBOX.Trash"), Some(FolderRole::Trash));
        assert_eq!(guess_role("[Gmail]/Prullenbak"), Some(FolderRole::Trash));
        assert_eq!(guess_role("Concepten"), Some(FolderRole::Drafts));
        assert_eq!(guess_role("Vakantie"), None);
    }

    #[test]
    fn auth_debug_hides_secret() {
        assert_eq!(format!("{:?}", Auth::Password("geheim".into())), "Password(..)");
    }

    #[test]
    fn quoting() {
        assert_eq!(quote(r#"a"b\c"#), r#""a\"b\\c""#);
    }
}
