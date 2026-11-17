//! Find the server settings for an email address (PLAN.md §3, Setup flow).
//!
//! Order: our own provider table, the domain's autoconfig file, Thunderbird's ISPDB, the provider
//! behind the domain's MX host, SRV records, then a guess under the user's own domain. Web lookups
//! are HTTPS only (CLAUDE.md, Network). Hosts from the table, autoconfig or ISPDB count as looked
//! up; anything else outside the user's domain (from MX, SRV or a guess) must be confirmed by the
//! user before a password goes to it.

use std::time::Duration;

use hickory_resolver::TokioResolver;
use hickory_resolver::proto::rr::RData;
use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::{Code, Result, VemError};
use crate::model::{ServerSettings, SignIn};
use crate::net::{self, Proto, Security, Trust};

struct Provider {
    domains: &'static [&'static str],
    imap: (&'static str, u16, Security),
    smtp: (&'static str, u16, Security),
    /// Username is the whole address (true) or the part before @ (false).
    full_address: bool,
    sign_in: SignIn,
    /// Mailbox size in KB, when the provider has one size for everybody.
    limit_kb: Option<u64>,
}

const GB: u64 = 1024 * 1024;

/// Solcon first (PLAN.md §3): nothing can be looked up for it.
const PROVIDERS: &[Provider] = &[
    Provider {
        domains: &["solcon.nl"],
        imap: ("imap.solcon.nl", 993, Security::Tls),
        smtp: ("mail.solcon.nl", 587, Security::Starttls),
        full_address: true,
        sign_in: SignIn::Password,
        limit_kb: None,
    },
    Provider {
        domains: &["gmail.com", "googlemail.com"],
        imap: ("imap.gmail.com", 993, Security::Tls),
        smtp: ("smtp.gmail.com", 465, Security::Tls),
        full_address: true,
        sign_in: SignIn::Google,
        limit_kb: Some(15 * GB),
    },
    Provider {
        domains: &["outlook.com", "hotmail.com", "live.com", "msn.com", "outlook.nl", "hotmail.nl", "live.nl"],
        imap: ("outlook.office365.com", 993, Security::Tls),
        smtp: ("smtp-mail.outlook.com", 587, Security::Starttls),
        full_address: true,
        sign_in: SignIn::Microsoft,
        limit_kb: Some(15 * GB),
    },
    Provider {
        domains: &["ziggo.nl", "home.nl", "casema.nl", "quicknet.nl", "chello.nl", "upcmail.nl", "zeelandnet.nl"],
        imap: ("imap.ziggo.nl", 993, Security::Tls),
        smtp: ("smtp.ziggo.nl", 587, Security::Starttls),
        full_address: true,
        sign_in: SignIn::Password,
        limit_kb: None,
    },
    Provider {
        domains: &["kpnmail.nl", "planet.nl", "hetnet.nl", "xs4all.nl", "kpnplanet.nl"],
        imap: ("imap.kpnmail.nl", 993, Security::Tls),
        smtp: ("smtp.kpnmail.nl", 587, Security::Starttls),
        full_address: true,
        sign_in: SignIn::Password,
        limit_kb: Some(5 * GB),
    },
    Provider {
        domains: &["yahoo.com", "yahoo.nl", "ymail.com", "aol.com"],
        imap: ("imap.mail.yahoo.com", 993, Security::Tls),
        smtp: ("smtp.mail.yahoo.com", 465, Security::Tls),
        full_address: true,
        sign_in: SignIn::Password,
        limit_kb: None,
    },
    Provider {
        domains: &["icloud.com", "me.com", "mac.com"],
        imap: ("imap.mail.me.com", 993, Security::Tls),
        smtp: ("smtp.mail.me.com", 587, Security::Starttls),
        // iCloud: IMAP username without the domain; SMTP with it (PLAN.md §3). We sign in to
        // SMTP with the address and to IMAP with the local part.
        full_address: false,
        sign_in: SignIn::Password,
        limit_kb: Some(5 * GB),
    },
];

pub fn domain_of(email: &str) -> Option<String> {
    let (local, domain) = email.trim().rsplit_once('@')?;
    let domain = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    if local.is_empty() || !domain.contains('.') || domain.contains(['/', '\\', ' ', ':', '?', '#']) {
        return None;
    }
    Some(domain)
}

fn from_table(email: &str, domain: &str) -> Option<ServerSettings> {
    let p = PROVIDERS.iter().find(|p| p.domains.contains(&domain))?;
    let local = email.split('@').next().unwrap_or(email);
    Some(ServerSettings {
        imap_host: p.imap.0.into(),
        imap_port: p.imap.1,
        imap_tls: p.imap.2,
        smtp_host: p.smtp.0.into(),
        smtp_port: p.smtp.1,
        smtp_tls: p.smtp.2,
        username: if p.full_address { email.trim().to_string() } else { local.to_string() },
        source: "table".into(),
        needs_confirmation: false,
        sign_in: p.sign_in,
    })
}

/// The provider's fixed mailbox size, when it has one (mailbox space without QUOTA).
pub fn known_limit_kb(email: &str) -> Option<u64> {
    let d = domain_of(email)?;
    PROVIDERS.iter().find(|p| p.domains.contains(&d.as_str()))?.limit_kb
}

#[derive(Default, Debug)]
struct XmlServer {
    kind: String,
    host: String,
    port: u16,
    socket: String,
    username: String,
}

/// Read a Thunderbird autoconfig file (the same format for the domain's own file and ISPDB).
pub fn parse_autoconfig(xml: &str, email: &str, source: &str) -> Option<ServerSettings> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut servers: Vec<(bool, XmlServer)> = Vec::new();
    let mut current: Option<(bool, XmlServer)> = None;
    let mut field = String::new();
    loop {
        match reader.read_event().ok()? {
            Event::Start(e) => {
                let name = e.local_name().as_ref().to_string();
                if name == "incomingServer" || name == "outgoingServer" {
                    let mut s = XmlServer::default();
                    for a in e.attributes().flatten() {
                        if a.key.as_ref() == "type" {
                            s.kind = a.value.as_ref().to_ascii_lowercase();
                        }
                    }
                    current = Some((name == "incomingServer", s));
                } else {
                    field = name;
                }
            }
            Event::Text(t) => {
                if let Some((_, s)) = current.as_mut() {
                    let v = t.as_ref().trim().to_string();
                    match field.as_str() {
                        "hostname" => s.host = v,
                        "port" => s.port = v.parse().unwrap_or(0),
                        "socketType" => s.socket = v.to_ascii_uppercase(),
                        "username" => s.username = v,
                        _ => {}
                    }
                }
            }
            Event::End(e) => {
                let name = e.local_name().as_ref().to_string();
                if (name == "incomingServer" || name == "outgoingServer")
                    && let Some(s) = current.take() {
                        servers.push(s);
                    }
                field.clear();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    // Only TLS or STARTTLS; a plain server is never used (CLAUDE.md, Network).
    let pick = |incoming: bool, kind: &str| {
        servers
            .iter()
            .filter(|(i, s)| *i == incoming && s.kind == kind && (s.socket == "SSL" || s.socket == "STARTTLS"))
            .map(|(_, s)| s)
            .min_by_key(|s| if s.socket == "SSL" { 0 } else { 1 })
    };
    let imap = pick(true, "imap")?;
    let smtp = pick(false, "smtp")?;
    let sec = |s: &XmlServer| if s.socket == "SSL" { Security::Tls } else { Security::Starttls };
    let local = email.split('@').next().unwrap_or(email);
    let domain = domain_of(email).unwrap_or_default();
    let username = imap
        .username
        .replace("%EMAILADDRESS%", email.trim())
        .replace("%EMAILLOCALPART%", local)
        .replace("%EMAILDOMAIN%", &domain);
    let sign_in = if imap.host.ends_with("gmail.com") || imap.host.ends_with("googlemail.com") {
        SignIn::Google
    } else if imap.host.ends_with("office365.com") || imap.host.ends_with("outlook.com") {
        SignIn::Microsoft
    } else {
        SignIn::Password
    };
    Some(ServerSettings {
        imap_host: imap.host.clone(),
        imap_port: if imap.port == 0 { 993 } else { imap.port },
        imap_tls: sec(imap),
        smtp_host: smtp.host.clone(),
        smtp_port: if smtp.port == 0 { 587 } else { smtp.port },
        smtp_tls: sec(smtp),
        username: if username.is_empty() { email.trim().to_string() } else { username },
        source: source.into(),
        needs_confirmation: false,
        sign_in,
    })
}

pub fn http() -> Result<reqwest::Client> {
    http_for(Duration::from_secs(10))
}

/// The same HTTPS-only client with another time limit (a whole APK takes longer than a file).
pub fn http_for(timeout: Duration) -> Result<reqwest::Client> {
    let tls = net::tls_config(Trust::Platform)?;
    reqwest::Client::builder()
        .use_preconfigured_tls((*tls).clone())
        .https_only(true)
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::limited(3))
        .user_agent(concat!("VorigeEeuwMail/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| VemError::new(Code::LookupFind01).with_detail(e.to_string()))
}

async fn fetch_xml(client: &reqwest::Client, url: &str) -> Option<String> {
    let resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let text = resp.text().await.ok()?;
    (text.len() < 256 * 1024).then_some(text)
}

/// Is there something listening on this host and port? No password is sent.
async fn reachable(host: &str, port: u16) -> bool {
    tokio::time::timeout(Duration::from_secs(5), net::tcp(host, port, Proto::Imap)).await.is_ok_and(|r| r.is_ok())
}

pub async fn lookup(email: &str) -> Result<ServerSettings> {
    let domain = domain_of(email).ok_or_else(|| VemError::new(Code::LookupFind01))?;
    if let Some(s) = from_table(email, &domain) {
        return Ok(s);
    }
    let client = http()?;
    let urls = [
        (format!("https://autoconfig.{domain}/mail/config-v1.1.xml"), "autoconfig"),
        (format!("https://{domain}/.well-known/autoconfig/mail/config-v1.1.xml"), "autoconfig"),
        (format!("https://autoconfig.thunderbird.net/v1.1/{domain}"), "ispdb"),
    ];
    for (url, source) in &urls {
        if let Some(xml) = fetch_xml(&client, url).await
            && let Some(s) = parse_autoconfig(&xml, email, source) {
                return Ok(s);
            }
    }
    // DNS answers are unsigned: hosts from them outside the user's domain need confirmation.
    let resolver = TokioResolver::builder_tokio().ok().and_then(|b| b.build().ok());
    if let Some(r) = &resolver {
        if let Some(s) = from_mx(&client, r, email, &domain).await {
            return Ok(confirm_if_foreign(email, s));
        }
        if let Some(s) = from_srv(r, email, &domain).await {
            return Ok(confirm_if_foreign(email, s));
        }
    }
    guess(email, &domain).await
}

fn confirm_if_foreign(email: &str, s: ServerSettings) -> ServerSettings {
    let needs = needs_confirmation(email, &s);
    ServerSettings { needs_confirmation: needs, ..s }
}

const DNS_TIMEOUT: Duration = Duration::from_secs(5);

/// MX hosts of big providers whose own table entry has another domain (Google Workspace).
const MX_ALIASES: &[(&str, &str)] = &[("google.com", "gmail.com"), ("googlemail.com", "gmail.com")];

/// Second-level labels under which a country domain hands out names (`example.co.uk`).
const SECOND_LEVEL: &[&str] = &["co", "com", "org", "net", "ac", "gov", "edu"];

/// The domain an MX host belongs to, the way Thunderbird looks it up: `mx1.mail.transip.nl`
/// gives `transip.nl`, `mx.example.co.uk` gives `example.co.uk`.
fn mx_base_domain(host: &str) -> Option<String> {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    let labels: Vec<&str> = host.split('.').filter(|l| !l.is_empty()).collect();
    if labels.len() < 2 {
        return None;
    }
    let tld = labels[labels.len() - 1];
    let second = labels[labels.len() - 2];
    let keep = if labels.len() >= 3 && tld.len() == 2 && SECOND_LEVEL.contains(&second) { 3 } else { 2 };
    Some(labels[labels.len() - keep..].join("."))
}

/// Step 4: the provider behind the domain's mail servers, from our table or ISPDB.
async fn from_mx(client: &reqwest::Client, r: &TokioResolver, email: &str, domain: &str) -> Option<ServerSettings> {
    let found = tokio::time::timeout(DNS_TIMEOUT, r.mx_lookup(format!("{domain}."))).await.ok()?.ok()?;
    let mut hosts: Vec<(u16, String)> = found
        .answers()
        .iter()
        .filter_map(|rec| match &rec.data {
            RData::MX(mx) => Some((mx.preference, mx.exchange.to_ascii())),
            _ => None,
        })
        .collect();
    hosts.sort();
    let base = mx_base_domain(&hosts.first()?.1)?;
    if base == domain {
        return None; // The domain's own entries were tried already.
    }
    let table_domain = MX_ALIASES.iter().find(|(mx, _)| *mx == base).map_or(base.as_str(), |(_, d)| d);
    let s = match from_table(email, table_domain) {
        Some(s) => s,
        None => {
            let xml = fetch_xml(client, &format!("https://autoconfig.thunderbird.net/v1.1/{base}")).await?;
            parse_autoconfig(&xml, email, "mx")?
        }
    };
    Some(ServerSettings { source: "mx".into(), ..s })
}

/// The best SRV answer: lowest priority, then highest weight. A target of "." means the domain
/// says it does not offer the service (RFC 2782).
fn pick_srv(records: &[(u16, u16, u16, String)]) -> Option<(String, u16)> {
    let (_, _, port, target) = records.iter().min_by_key(|(prio, weight, _, _)| (*prio, std::cmp::Reverse(*weight)))?;
    let target = target.trim_end_matches('.').to_ascii_lowercase();
    (!target.is_empty() && *port > 0).then_some((target, *port))
}

async fn srv(r: &TokioResolver, name: String) -> Option<(String, u16)> {
    let found = tokio::time::timeout(DNS_TIMEOUT, r.srv_lookup(name)).await.ok()?.ok()?;
    let records: Vec<(u16, u16, u16, String)> = found
        .answers()
        .iter()
        .filter_map(|rec| match &rec.data {
            RData::SRV(s) => Some((s.priority, s.weight, s.port, s.target.to_ascii())),
            _ => None,
        })
        .collect();
    pick_srv(&records)
}

/// Step 5: SRV records (RFC 6186, implicit TLS first per RFC 8314). Never a plain service.
async fn from_srv(r: &TokioResolver, email: &str, domain: &str) -> Option<ServerSettings> {
    let imap = match srv(r, format!("_imaps._tcp.{domain}.")).await {
        Some((h, p)) => (h, p, Security::Tls),
        None => srv(r, format!("_imap._tcp.{domain}.")).await.map(|(h, p)| (h, p, Security::Starttls))?,
    };
    let smtp = match srv(r, format!("_submissions._tcp.{domain}.")).await {
        Some((h, p)) => (h, p, Security::Tls),
        None => srv(r, format!("_submission._tcp.{domain}.")).await.map(|(h, p)| (h, p, Security::Starttls))?,
    };
    Some(srv_settings(email, imap, smtp))
}

fn srv_settings(email: &str, imap: (String, u16, Security), smtp: (String, u16, Security)) -> ServerSettings {
    ServerSettings {
        imap_host: imap.0,
        imap_port: imap.1,
        imap_tls: imap.2,
        smtp_host: smtp.0,
        smtp_port: smtp.1,
        smtp_tls: smtp.2,
        username: email.trim().to_string(),
        source: "srv".into(),
        needs_confirmation: false,
        sign_in: SignIn::Password,
    }
}

/// Last step: the usual names under the user's own domain. Being under that domain, they do
/// not need confirmation; the login test decides.
async fn guess(email: &str, domain: &str) -> Result<ServerSettings> {
    for imap in [format!("imap.{domain}"), format!("mail.{domain}")] {
        if !reachable(&imap, 993).await {
            continue;
        }
        let mut smtp = (format!("smtp.{domain}"), 587, Security::Starttls);
        if !reachable(&smtp.0, 587).await {
            smtp = (format!("mail.{domain}"), 587, Security::Starttls);
            if !reachable(&smtp.0, 587).await {
                smtp = (imap.clone(), 465, Security::Tls);
            }
        }
        return Ok(ServerSettings {
            imap_host: imap,
            imap_port: 993,
            imap_tls: Security::Tls,
            smtp_host: smtp.0,
            smtp_port: smtp.1,
            smtp_tls: smtp.2,
            username: email.trim().to_string(),
            source: "guess".into(),
            needs_confirmation: false,
            sign_in: SignIn::Password,
        });
    }
    Err(VemError::new(Code::LookupFind01).with_detail(domain.to_string()))
}

fn host_under(domain: &str, host: &str) -> bool {
    host.is_empty() || host == domain || host.ends_with(&format!(".{domain}"))
}

/// Settings typed or changed by hand: a host outside the user's own domain that did not come
/// from a lookup must be confirmed by the user (CLAUDE.md, Network).
pub fn needs_confirmation(email: &str, s: &ServerSettings) -> bool {
    if matches!(s.source.as_str(), "table" | "autoconfig" | "ispdb") {
        return false;
    }
    let domain = domain_of(email).unwrap_or_default();
    !(host_under(&domain, &s.imap_host) && host_under(&domain, &s.smtp_host))
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0"?>
<clientConfig version="1.1"><emailProvider id="example.nl">
<incomingServer type="pop3"><hostname>pop.example.nl</hostname><port>995</port><socketType>SSL</socketType></incomingServer>
<incomingServer type="imap"><hostname>plain.example.nl</hostname><port>143</port><socketType>plain</socketType><username>%EMAILADDRESS%</username></incomingServer>
<incomingServer type="imap"><hostname>imap.example.nl</hostname><port>143</port><socketType>STARTTLS</socketType><username>%EMAILLOCALPART%</username></incomingServer>
<incomingServer type="imap"><hostname>imaps.example.nl</hostname><port>993</port><socketType>SSL</socketType><username>%EMAILLOCALPART%</username></incomingServer>
<outgoingServer type="smtp"><hostname>smtp.example.nl</hostname><port>587</port><socketType>STARTTLS</socketType></outgoingServer>
</emailProvider></clientConfig>"#;

    #[test]
    fn solcon_from_table() {
        let s = from_table("jan@solcon.nl", "solcon.nl").unwrap();
        assert_eq!((s.imap_host.as_str(), s.imap_port, s.imap_tls), ("imap.solcon.nl", 993, Security::Tls));
        assert_eq!((s.smtp_host.as_str(), s.smtp_port, s.smtp_tls), ("mail.solcon.nl", 587, Security::Starttls));
        assert!(!s.needs_confirmation);
    }

    #[test]
    fn gmail_is_google_sign_in() {
        assert_eq!(from_table("a@gmail.com", "gmail.com").unwrap().sign_in, SignIn::Google);
        assert_eq!(known_limit_kb("a@gmail.com"), Some(15 * GB));
    }

    #[test]
    fn autoconfig_prefers_tls_and_skips_plain() {
        let s = parse_autoconfig(XML, "jan@example.nl", "ispdb").unwrap();
        assert_eq!(s.imap_host, "imaps.example.nl");
        assert_eq!(s.imap_tls, Security::Tls);
        assert_eq!(s.username, "jan");
        assert_eq!(s.smtp_tls, Security::Starttls);
    }

    #[test]
    fn autoconfig_without_secure_server_gives_none() {
        let xml = XML.replace("SSL", "plain").replace("STARTTLS", "plain");
        assert!(parse_autoconfig(&xml, "jan@example.nl", "ispdb").is_none());
    }

    #[test]
    fn domains_are_validated() {
        assert_eq!(domain_of(" Jan@Example.NL "), Some("example.nl".into()));
        assert_eq!(domain_of("jan@example.nl/evil"), None);
        assert_eq!(domain_of("@x.nl"), None);
        assert_eq!(domain_of("geen"), None);
    }

    #[test]
    fn mx_hosts_map_to_their_provider_domain() {
        assert_eq!(mx_base_domain("mx1.mail.transip.nl."), Some("transip.nl".into()));
        assert_eq!(mx_base_domain("ASPMX.L.GOOGLE.COM"), Some("google.com".into()));
        assert_eq!(mx_base_domain("mx.example.co.uk"), Some("example.co.uk".into()));
        assert_eq!(mx_base_domain("familie-nl.mail.protection.outlook.com"), Some("outlook.com".into()));
        assert_eq!(mx_base_domain("localhost"), None);
    }

    #[test]
    fn settings_from_mx_or_srv_are_confirmed_when_foreign() {
        let gmail = from_table("jan@familie.nl", "gmail.com").unwrap();
        let s = confirm_if_foreign("jan@familie.nl", ServerSettings { source: "mx".into(), ..gmail });
        assert!(s.needs_confirmation, "a Workspace domain found through MX shows Google's hosts first");
        assert_eq!(s.username, "jan@familie.nl");
        let own = srv_settings("jan@familie.nl", ("imap.familie.nl".into(), 993, Security::Tls), ("smtp.familie.nl".into(), 465, Security::Tls));
        assert!(!confirm_if_foreign("jan@familie.nl", own).needs_confirmation);
        let hosted = srv_settings("jan@familie.nl", ("mail.hoster.nl".into(), 993, Security::Tls), ("mail.hoster.nl".into(), 587, Security::Starttls));
        assert!(confirm_if_foreign("jan@familie.nl", hosted).needs_confirmation);
    }

    #[test]
    fn srv_picks_priority_then_weight_and_honours_dot() {
        let r = vec![(10, 5, 993, "b.x.nl.".to_string()), (0, 1, 993, "a.x.nl.".into()), (0, 9, 993, "c.x.nl.".into())];
        assert_eq!(pick_srv(&r), Some(("c.x.nl".into(), 993)));
        assert_eq!(pick_srv(&[(0, 0, 0, ".".into())]), None, "a dot target: not offered");
        assert_eq!(pick_srv(&[]), None);
    }

    /// Real DNS, read only: `cargo test -p vem-core --lib live_dns -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn live_dns_mx_and_srv() {
        let r = TokioResolver::builder_tokio().unwrap().build().unwrap();
        let client = http().unwrap();
        let mx = from_mx(&client, &r, "jan@googlemail.com", "googlemail.com").await.unwrap();
        assert_eq!((mx.imap_host.as_str(), mx.source.as_str()), ("imap.gmail.com", "mx"));
        let srv = from_srv(&r, "jan@gmail.com", "gmail.com").await.unwrap();
        assert_eq!(srv.imap_host, "imap.gmail.com");
        assert_eq!(srv.imap_tls, Security::Tls);
    }

    #[test]
    fn confirmation_for_foreign_hand_typed_hosts() {
        let mut s = from_table("jan@solcon.nl", "solcon.nl").unwrap();
        s.source = "manual".into();
        assert!(needs_confirmation("jan@familie.nl", &s));
        assert!(!needs_confirmation("jan@solcon.nl", &s));
        s.imap_host = "imap.familie.nl".into();
        s.smtp_host = "familie.nl".into();
        assert!(!needs_confirmation("jan@familie.nl", &s));
    }
}
