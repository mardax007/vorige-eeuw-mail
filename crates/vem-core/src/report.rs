//! Error reports, wishes and the help report for Martijn (docs/PLAN.md section 5).
//!
//! Reports are ordinary emails sent through the user's own account. They hold no mail content,
//! passwords or tokens; the only addresses in them are the sender's and Martijn's.

use serde::{Deserialize, Serialize};

use crate::compose;
use crate::error::{Result, VemError, mask_addresses};
use crate::log;
use crate::model::{Addr, Draft, now};
use crate::store::Store;

/// Martijn's report address, set at build time from the `VEM_SUPPORT_ADDRESS` secret.
/// Without it (a local build) reports are only written to the log.
pub fn support_address() -> Option<&'static str> {
    option_env!("VEM_SUPPORT_ADDRESS").filter(|a| compose::valid_email(a))
}

pub const SUBJECT_PREFIX: &str = "[Vorige Eeuw Mail]";
pub const MAX_PER_DAY: u32 = 10;
const LOG_LINES_IN_REPORT: usize = 50;
const LOG_LINES_IN_HELP: usize = 200;
const STATE_KEY: &str = "report_state";

#[derive(Debug, Clone)]
pub struct AppFacts {
    pub version: String,
    pub os: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ReportState {
    day: String,
    sent_today: u32,
    /// Codes reported today.
    codes_today: Vec<String>,
    /// Errors held back by the limits, per code, told in the next report of that code.
    held: Vec<(String, u32)>,
}

fn today() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

/// Decide whether this error goes out now. Returns the number of earlier, unreported times the
/// same code happened (for "also 3 times before"), or None when the limits hold it back.
pub fn admit(store: &Store, code: &str) -> Result<Option<u32>> {
    let mut s: ReportState = store.kv_get(STATE_KEY)?.unwrap_or_default();
    let day = today();
    if s.day != day {
        s = ReportState { day, held: std::mem::take(&mut s.held), ..Default::default() };
    }
    let earlier = s.held.iter().find(|(c, _)| c == code).map(|(_, n)| *n).unwrap_or(0);
    let allowed = s.sent_today < MAX_PER_DAY && !s.codes_today.iter().any(|c| c == code);
    if allowed {
        s.sent_today += 1;
        s.codes_today.push(code.to_string());
        s.held.retain(|(c, _)| c != code);
    } else {
        match s.held.iter_mut().find(|(c, _)| c == code) {
            Some((_, n)) => *n += 1,
            None => s.held.push((code.to_string(), 1)),
        }
    }
    store.kv_set(STATE_KEY, &s)?;
    Ok(allowed.then_some(earlier))
}

fn facts_block(store: &Store, facts: &AppFacts) -> Result<String> {
    Ok(format!(
        "Versie: {}\nWindows: {}\nInstallatienummer: {}\n",
        facts.version,
        facts.os,
        store.install_id()?
    ))
}

/// The error report as a draft from `sender` to Martijn.
pub fn error_report(store: &Store, facts: &AppFacts, sender: &Addr, account_id: &str, err: &VemError, earlier: u32) -> Result<Option<Draft>> {
    let Some(support) = support_address() else { return Ok(None) };
    let allowed = [sender.email.as_str(), support];
    let reply = err.server_reply.as_deref().map(|r| mask_addresses(r, &allowed)).unwrap_or_default();
    let detail = err.detail.as_deref().map(|d| mask_addresses(d, &allowed)).unwrap_or_default();
    let mut body = format!("Foutcode: {}\n", err.code);
    if earlier > 0 {
        body.push_str(&format!("Eerder vandaag niet gemeld: {earlier} keer\n"));
    }
    body.push_str(&facts_block(store, facts)?);
    if !detail.is_empty() {
        body.push_str(&format!("Details: {detail}\n"));
    }
    body.push_str(&format!("\nAntwoord van de server:\n{reply}\n\nLaatste regels van het logboek:\n"));
    body.push_str(&log::tail(LOG_LINES_IN_REPORT).join("\n"));
    Ok(Some(Draft {
        account_id: account_id.to_string(),
        to: vec![Addr::new(None, support)],
        subject: format!("{SUBJECT_PREFIX} Fout {}", err.code),
        body_text: body,
        ..Default::default()
    }))
}

/// "Wens of idee": the user's own words, with the version added.
pub fn wish(store: &Store, facts: &AppFacts, account_id: &str, text: &str) -> Result<Option<Draft>> {
    let Some(support) = support_address() else { return Ok(None) };
    Ok(Some(Draft {
        account_id: account_id.to_string(),
        to: vec![Addr::new(None, support)],
        subject: format!("{SUBJECT_PREFIX} Wens of idee"),
        body_text: format!("{}\n\n--\n{}", text.trim(), facts_block(store, facts)?),
        ..Default::default()
    }))
}

/// The full log for a log request from Martijn.
pub fn log_answer(store: &Store, facts: &AppFacts, sender: &Addr, account_id: &str, request: u64) -> Result<Option<Draft>> {
    let Some(support) = support_address() else { return Ok(None) };
    let log = mask_addresses(&log::full(), &[sender.email.as_str(), support]);
    Ok(Some(Draft {
        account_id: account_id.to_string(),
        to: vec![Addr::new(None, support)],
        subject: format!("{SUBJECT_PREFIX} Logboek (verzoek {request})"),
        body_text: format!("{}\n{log}", facts_block(store, facts)?),
        ..Default::default()
    }))
}

/// "Hulprapport kopiëren": the same facts for the clipboard, without any address.
pub fn help_report(store: &Store, facts: &AppFacts) -> Result<String> {
    let mut out = facts_block(store, facts)?;
    for a in store.accounts()? {
        let s = a.settings.as_ref();
        out.push_str(&format!(
            "\nPostvak {}: {} {}:{} / {}:{}\n",
            &a.id[..a.id.len().min(8)],
            if a.kind == crate::model::AccountKind::Local { "lokaal" } else { "imap" },
            s.map(|s| s.imap_host.as_str()).unwrap_or("-"),
            s.map(|s| s.imap_port).unwrap_or(0),
            s.map(|s| s.smtp_host.as_str()).unwrap_or("-"),
            s.map(|s| s.smtp_port).unwrap_or(0),
        ));
        out.push_str(&format!("Mogelijkheden: {}\n", store.capabilities(&a.id)?.join(" ")));
        if let Some(e) = &a.last_error {
            out.push_str(&format!("Laatste fout: {} {}\n", e.code, mask_addresses(e.server_reply.as_deref().unwrap_or(""), &[])));
        }
        for f in store.folder_infos(&a.id)? {
            out.push_str(&format!(
                "  map {}: uidvalidity {:?} uidnext {:?}\n",
                f.id, f.sync.uidvalidity, f.sync.uidnext
            ));
        }
    }
    out.push_str(&format!("\nGemaakt: {}\n\nLogboek:\n", crate::model::iso(now())));
    out.push_str(&log::tail(LOG_LINES_IN_HELP).join("\n"));
    Ok(mask_addresses(&out, &[]))
}

/// `log-requests.json` in the release repository: `{"requests":[{"install":"…","request":3}]}`.
#[derive(Debug, Deserialize)]
pub struct LogRequests {
    #[serde(default)]
    pub requests: Vec<LogRequest>,
}

#[derive(Debug, Deserialize)]
pub struct LogRequest {
    pub install: String,
    pub request: u64,
}

/// The request number to answer now, if this install is asked and has not answered yet.
pub fn pending_log_request(store: &Store, list: &LogRequests) -> Result<Option<u64>> {
    let me = store.install_id()?;
    let answered: u64 = store.kv_get("log_request_answered")?.unwrap_or(0);
    Ok(list.requests.iter().filter(|r| r.install == me && r.request > answered).map(|r| r.request).max())
}

pub fn mark_log_request_answered(store: &Store, request: u64) -> Result<()> {
    store.kv_set("log_request_answered", &request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::testutil::store;

    #[test]
    fn one_report_per_code_per_day_and_counts_the_rest() {
        let (_d, s) = store();
        assert_eq!(admit(&s, "VEM-IMAP-CONNECT-01").unwrap(), Some(0));
        assert_eq!(admit(&s, "VEM-IMAP-CONNECT-01").unwrap(), None);
        assert_eq!(admit(&s, "VEM-IMAP-CONNECT-01").unwrap(), None);
        for n in 0..9 {
            assert!(admit(&s, &format!("VEM-X-{n}")).unwrap().is_some());
        }
        assert_eq!(admit(&s, "VEM-Y-1").unwrap(), None, "at most ten a day");
        // Next day: the held count goes along with the first report of that code.
        let mut st: ReportState = s.kv_get(STATE_KEY).unwrap().unwrap();
        st.day = "2000-01-01".into();
        s.kv_set(STATE_KEY, &st).unwrap();
        assert_eq!(admit(&s, "VEM-IMAP-CONNECT-01").unwrap(), Some(2));
    }

    #[test]
    fn help_report_has_no_addresses() {
        let (_d, s) = store();
        s.add_account("Jan", "jan@ziggo.nl", crate::model::AccountKind::Imap, None).unwrap();
        crate::log::event("t", &[("reply", "no user piet@kpn.nl")]);
        let r = help_report(&s, &AppFacts { version: "0.1.0".into(), os: "Windows 11".into() }).unwrap();
        assert!(!r.contains("jan@ziggo.nl") && !r.contains("piet@kpn.nl"));
        assert!(r.contains("Installatienummer") && r.contains("0.1.0"));
    }

    #[test]
    fn log_requests_are_answered_once() {
        let (_d, s) = store();
        let me = s.install_id().unwrap();
        let list: LogRequests =
            serde_json::from_str(&format!(r#"{{"requests":[{{"install":"{me}","request":2}},{{"install":"other","request":9}}]}}"#)).unwrap();
        assert_eq!(pending_log_request(&s, &list).unwrap(), Some(2));
        mark_log_request_answered(&s, 2).unwrap();
        assert_eq!(pending_log_request(&s, &list).unwrap(), None);
    }
}
