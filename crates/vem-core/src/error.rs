//! The one error type of the core and the single source of the `VEM-…` codes (docs/errors.md).
//!
//! The screens look up the sentence and the next step by code. The server's reply is kept raw,
//! exactly as it came off the wire, and is never passed through the catalogue.

use serde::Serialize;
use std::fmt;

/// Every code the core can emit. Codes never change once a release has shipped them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Code {
    LookupFind01,
    ImapFind01,
    ImapConnect01,
    ImapConnect02,
    ImapTls01,
    ImapTls02,
    ImapLogin01,
    ImapLogin02,
    ImapCmd01,
    ImapCmd02,
    SmtpFind01,
    SmtpConnect01,
    SmtpTls01,
    SmtpTls02,
    SmtpLogin01,
    SmtpCmd01,
    SmtpCmd02,
    SmtpCmd03,
    AuthLogin01,
    AuthLogin02,
    ImportRead01,
    ImportRead02,
    ImportRead03,
    ImportRead04,
    ImportRead05,
    ImportSpace01,
    ImportSpace02,
    StoreWrite01,
    StoreRead01,
    StoreSecret01,
    UpdateCmd01,
    ReportCmd01,
    AppCmd01,
}

impl Code {
    pub const ALL: &'static [Code] = &[
        Code::LookupFind01,
        Code::ImapFind01,
        Code::ImapConnect01,
        Code::ImapConnect02,
        Code::ImapTls01,
        Code::ImapTls02,
        Code::ImapLogin01,
        Code::ImapLogin02,
        Code::ImapCmd01,
        Code::ImapCmd02,
        Code::SmtpFind01,
        Code::SmtpConnect01,
        Code::SmtpTls01,
        Code::SmtpTls02,
        Code::SmtpLogin01,
        Code::SmtpCmd01,
        Code::SmtpCmd02,
        Code::SmtpCmd03,
        Code::AuthLogin01,
        Code::AuthLogin02,
        Code::ImportRead01,
        Code::ImportRead02,
        Code::ImportRead03,
        Code::ImportRead04,
        Code::ImportRead05,
        Code::ImportSpace01,
        Code::ImportSpace02,
        Code::StoreWrite01,
        Code::StoreRead01,
        Code::StoreSecret01,
        Code::UpdateCmd01,
        Code::ReportCmd01,
        Code::AppCmd01,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Code::LookupFind01 => "VEM-LOOKUP-FIND-01",
            Code::ImapFind01 => "VEM-IMAP-FIND-01",
            Code::ImapConnect01 => "VEM-IMAP-CONNECT-01",
            Code::ImapConnect02 => "VEM-IMAP-CONNECT-02",
            Code::ImapTls01 => "VEM-IMAP-TLS-01",
            Code::ImapTls02 => "VEM-IMAP-TLS-02",
            Code::ImapLogin01 => "VEM-IMAP-LOGIN-01",
            Code::ImapLogin02 => "VEM-IMAP-LOGIN-02",
            Code::ImapCmd01 => "VEM-IMAP-CMD-01",
            Code::ImapCmd02 => "VEM-IMAP-CMD-02",
            Code::SmtpFind01 => "VEM-SMTP-FIND-01",
            Code::SmtpConnect01 => "VEM-SMTP-CONNECT-01",
            Code::SmtpTls01 => "VEM-SMTP-TLS-01",
            Code::SmtpTls02 => "VEM-SMTP-TLS-02",
            Code::SmtpLogin01 => "VEM-SMTP-LOGIN-01",
            Code::SmtpCmd01 => "VEM-SMTP-CMD-01",
            Code::SmtpCmd02 => "VEM-SMTP-CMD-02",
            Code::SmtpCmd03 => "VEM-SMTP-CMD-03",
            Code::AuthLogin01 => "VEM-AUTH-LOGIN-01",
            Code::AuthLogin02 => "VEM-AUTH-LOGIN-02",
            Code::ImportRead01 => "VEM-IMPORT-READ-01",
            Code::ImportRead02 => "VEM-IMPORT-READ-02",
            Code::ImportRead03 => "VEM-IMPORT-READ-03",
            Code::ImportRead04 => "VEM-IMPORT-READ-04",
            Code::ImportRead05 => "VEM-IMPORT-READ-05",
            Code::ImportSpace01 => "VEM-IMPORT-SPACE-01",
            Code::ImportSpace02 => "VEM-IMPORT-SPACE-02",
            Code::StoreWrite01 => "VEM-STORE-WRITE-01",
            Code::StoreRead01 => "VEM-STORE-READ-01",
            Code::StoreSecret01 => "VEM-STORE-SECRET-01",
            Code::UpdateCmd01 => "VEM-UPDATE-CMD-01",
            Code::ReportCmd01 => "VEM-REPORT-CMD-01",
            Code::AppCmd01 => "VEM-APP-CMD-01",
        }
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What the screens receive. `server_reply` is exact; `detail` is plain data (a host, a path).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VemError {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_reply: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip)]
    pub kind: Option<Code>,
}

impl VemError {
    pub fn new(code: Code) -> Self {
        VemError {
            code: code.as_str().to_string(),
            server_reply: None,
            detail: None,
            kind: Some(code),
        }
    }

    pub fn with_reply(mut self, reply: impl Into<String>) -> Self {
        let reply = reply.into();
        if !reply.is_empty() {
            self.server_reply = Some(reply);
        }
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn code(&self) -> Option<Code> {
        self.kind
    }
}

impl fmt::Display for VemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code)?;
        if let Some(d) = &self.detail {
            write!(f, " ({d})")?;
        }
        if let Some(r) = &self.server_reply {
            write!(f, ": {r}")?;
        }
        Ok(())
    }
}

impl std::error::Error for VemError {}

pub type Result<T> = std::result::Result<T, VemError>;

impl From<rusqlite::Error> for VemError {
    fn from(e: rusqlite::Error) -> Self {
        VemError::new(Code::StoreRead01).with_detail(e.to_string())
    }
}

impl From<std::io::Error> for VemError {
    fn from(e: std::io::Error) -> Self {
        VemError::new(Code::StoreWrite01).with_detail(e.kind().to_string())
    }
}

/// Replace every address other than the allowed ones by `<adres>` (P1 in docs/DECISIONS.md).
/// Used for the emailed report and the log; the screen keeps the exact reply.
pub fn mask_addresses(text: &str, allowed: &[&str]) -> String {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(
            r"[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?(?:\.[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?)+",
        )
        .expect("address pattern is valid")
    });
    re.replace_all(text, |caps: &regex::Captures| {
        let found = &caps[0];
        if allowed.iter().any(|a| a.eq_ignore_ascii_case(found)) {
            found.to_string()
        } else {
            "<adres>".to_string()
        }
    })
    .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique_and_follow_scheme() {
        let mut seen = std::collections::HashSet::new();
        let re = regex::Regex::new(r"^VEM-[A-Z]+-[A-Z]+-\d\d$").unwrap();
        for c in Code::ALL {
            assert!(seen.insert(c.as_str()), "duplicate {c}");
            assert!(re.is_match(c.as_str()), "bad scheme {c}");
        }
    }

    #[test]
    fn masking_keeps_allowed_and_rest_of_reply() {
        let reply = "550 5.1.1 <someone@example.nl>: Recipient address rejected (from me@home.nl)";
        let masked = mask_addresses(reply, &["me@home.nl"]);
        assert_eq!(
            masked,
            "550 5.1.1 <<adres>>: Recipient address rejected (from me@home.nl)"
        );
    }

    #[test]
    fn masking_is_case_insensitive_for_allowed() {
        assert_eq!(mask_addresses("Me@Home.NL", &["me@home.nl"]), "Me@Home.NL");
    }

    #[test]
    fn error_serialises_camel_case_without_kind() {
        let e = VemError::new(Code::ImapLogin01).with_reply("NO [AUTHENTICATIONFAILED] x");
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(
            json,
            r#"{"code":"VEM-IMAP-LOGIN-01","serverReply":"NO [AUTHENTICATIONFAILED] x"}"#
        );
    }
}
