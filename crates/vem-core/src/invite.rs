//! An invitation in a message, shown readably above the text (C11, decision 29).
//!
//! Reads the first event of a `text/calendar` part or an `.ics` attachment: what, when, where,
//! who invites, and whether it was cancelled. There is no calendar in the app and no reply is
//! ever sent; the `.ics` stays in the attachments to open in a calendar program.

use mail_parser::MessageParser;
use serde::Serialize;

use crate::parse::Addr;

/// Longer texts are cut: an invitation box is a summary, the message itself has the rest.
const MAX_TEXT: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Invite {
    pub summary: Option<String>,
    pub start: Option<InviteTime>,
    pub end: Option<InviteTime>,
    pub location: Option<String>,
    pub organizer: Option<Addr>,
    /// The organiser called it off (METHOD:CANCEL or STATUS:CANCELLED).
    pub cancelled: bool,
    /// The event comes back (an RRULE); the box says so, without spelling out the rule.
    pub repeats: bool,
}

/// A moment as the invitation wrote it. The screen turns it into the device's own time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteTime {
    /// "2026-11-20T14:00:00", or "2026-11-20" for a whole day.
    pub wall: String,
    /// "UTC", an IANA zone ("Europe/Amsterdam"), or the invitation's own zone name when it is
    /// none of those. None: a "floating" time, the same clock time everywhere.
    pub zone: Option<String>,
    pub all_day: bool,
}

/// The invitation in this message, if it has one that asks or tells something. A reply to an
/// invitation (METHOD:REPLY) is not shown as one.
pub fn find(raw: &[u8]) -> Option<Invite> {
    let msg = MessageParser::new().parse(raw)?;
    msg.parts().find_map(|part| {
        let is_calendar = part.content_type().is_some_and(|ct| {
            let sub = ct.subtype().unwrap_or("").to_ascii_lowercase();
            let main = ct.ctype().to_ascii_lowercase();
            (main == "text" && sub == "calendar") || (main == "application" && sub == "ics")
        }) || part.attachment_name().is_some_and(|n| n.to_ascii_lowercase().ends_with(".ics"));
        if !is_calendar {
            return None;
        }
        let text = match part.text() {
            Some(t) => t.into_owned(),
            None => String::from_utf8_lossy(&part.decoded()).into_owned(),
        };
        parse_ics(&text)
    })
}

/// Read the first VEVENT of an iCalendar text (RFC 5545), as far as the box needs it.
pub fn parse_ics(text: &str) -> Option<Invite> {
    let unfolded = text.replace("\r\n ", "").replace("\r\n\t", "").replace("\n ", "").replace("\n\t", "");
    let mut method = None;
    let mut in_event = false;
    let mut seen_event = false;
    let mut nested = 0usize;
    let mut invite = Invite {
        summary: None,
        start: None,
        end: None,
        location: None,
        organizer: None,
        cancelled: false,
        repeats: false,
    };
    for line in unfolded.lines() {
        let Some((name, params, value)) = split_line(line.trim_end_matches('\r')) else { continue };
        match (name.as_str(), value.trim().to_ascii_uppercase().as_str()) {
            ("BEGIN", "VEVENT") if !seen_event => {
                in_event = true;
                seen_event = true;
                continue;
            }
            ("END", "VEVENT") if in_event && nested == 0 => break,
            ("BEGIN", _) if in_event => {
                nested += 1;
                continue;
            }
            ("END", _) if in_event && nested > 0 => {
                nested -= 1;
                continue;
            }
            _ => {}
        }
        if !in_event {
            if name == "METHOD" {
                method = Some(value.trim().to_ascii_uppercase());
            }
            continue;
        }
        if nested > 0 {
            continue; // An alarm or similar inside the event.
        }
        match name.as_str() {
            "SUMMARY" => invite.summary = text_value(&value),
            "LOCATION" => invite.location = text_value(&value),
            "DTSTART" => invite.start = time_value(&params, &value),
            "DTEND" => invite.end = time_value(&params, &value),
            "ORGANIZER" => invite.organizer = organizer(&params, &value),
            "STATUS" if value.trim().eq_ignore_ascii_case("CANCELLED") => invite.cancelled = true,
            "RRULE" => invite.repeats = true,
            _ => {}
        }
    }
    if !seen_event || method.as_deref() == Some("REPLY") {
        return None;
    }
    if method.as_deref() == Some("CANCEL") {
        invite.cancelled = true;
    }
    if invite.summary.is_none() && invite.start.is_none() {
        return None;
    }
    Some(invite)
}

/// Parameters of one content line, upper-case names.
type Params = Vec<(String, String)>;

/// "NAME;P1=a;P2="b;c":value" into the upper-case name, its parameters and the value.
fn split_line(line: &str) -> Option<(String, Params, String)> {
    let mut quoted = false;
    let colon = line.char_indices().find_map(|(i, c)| {
        if c == '"' {
            quoted = !quoted;
        }
        (c == ':' && !quoted).then_some(i)
    })?;
    let (head, value) = (&line[..colon], &line[colon + 1..]);
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in head.chars() {
        match c {
            '"' => quoted = !quoted,
            ';' if !quoted => parts.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    parts.push(current);
    let name = parts.remove(0).trim().to_ascii_uppercase();
    let params = parts
        .into_iter()
        .filter_map(|p| p.split_once('=').map(|(k, v)| (k.trim().to_ascii_uppercase(), v.trim().to_string())))
        .collect();
    Some((name, params, value.to_string()))
}

fn param<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
    params.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// Undo the TEXT escapes of RFC 5545 and cut long texts.
fn text_value(value: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => out.push('\n'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(match trimmed.char_indices().nth(MAX_TEXT) {
        Some((i, _)) => format!("{}…", &trimmed[..i]),
        None => trimmed.to_string(),
    })
}

fn is_date(s: &str) -> bool {
    s.len() == 8 && s.bytes().all(|b| b.is_ascii_digit())
}

fn date_text(s: &str) -> String {
    format!("{}-{}-{}", &s[..4], &s[4..6], &s[6..8])
}

fn time_value(params: &[(String, String)], value: &str) -> Option<InviteTime> {
    let v = value.trim();
    if param(params, "VALUE").is_some_and(|p| p.eq_ignore_ascii_case("DATE")) || is_date(v) {
        return is_date(v).then(|| InviteTime { wall: date_text(v), zone: None, all_day: true });
    }
    let (d, rest) = v.split_once('T')?;
    let utc = rest.ends_with(['Z', 'z']);
    let t = rest.trim_end_matches(['Z', 'z']);
    if !is_date(d) || !(t.len() == 4 || t.len() == 6) || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let sec = if t.len() == 6 { &t[4..6] } else { "00" };
    let wall = format!("{}T{}:{}:{}", date_text(d), &t[..2], &t[2..4], sec);
    let zone = if utc { Some("UTC".to_string()) } else { param(params, "TZID").map(zone_name) };
    Some(InviteTime { wall, zone, all_day: false })
}

/// Outlook writes Windows zone names; the screen understands IANA names. The common ones for
/// the family are mapped; any other name is passed on as written and shown next to the time.
fn zone_name(tzid: &str) -> String {
    let id = tzid.trim().trim_matches('"').trim_start_matches('/');
    let mapped = match id {
        "W. Europe Standard Time" => "Europe/Amsterdam",
        "Romance Standard Time" => "Europe/Paris",
        "Central Europe Standard Time" => "Europe/Budapest",
        "Central European Standard Time" => "Europe/Warsaw",
        "GMT Standard Time" => "Europe/London",
        "Greenwich Standard Time" => "Atlantic/Reykjavik",
        "E. Europe Standard Time" => "Europe/Chisinau",
        "FLE Standard Time" => "Europe/Kiev",
        "GTB Standard Time" => "Europe/Bucharest",
        "Eastern Standard Time" => "America/New_York",
        "Pacific Standard Time" => "America/Los_Angeles",
        "UTC" | "Coordinated Universal Time" => "UTC",
        other => other,
    };
    mapped.to_string()
}

fn organizer(params: &[(String, String)], value: &str) -> Option<Addr> {
    let v = value.trim();
    let email = match v.get(..7) {
        Some(p) if p.eq_ignore_ascii_case("mailto:") => &v[7..],
        _ => v,
    };
    if !email.contains('@') {
        return None;
    }
    Some(Addr::new(param(params, "CN").map(|n| n.trim_matches('"')), email))
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUTLOOK: &str = "BEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nPRODID:Microsoft Exchange Server 2010\r\n\
BEGIN:VTIMEZONE\r\nTZID:W. Europe Standard Time\r\nBEGIN:STANDARD\r\nDTSTART:16010101T030000\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\n\
BEGIN:VEVENT\r\nORGANIZER;CN=\"Jansen, Kees\":mailto:kees.jansen@voorbeeld.nl\r\n\
SUMMARY;LANGUAGE=nl-NL:Verjaardag oma\\, met taart\r\n\
DTSTART;TZID=W. Europe Standard Time:20261120T140000\r\nDTEND;TZID=W. Europe Standard Time:20261120T170000\r\n\
LOCATION;LANGUAGE=nl-NL:Dorpsstraat 1\\, Ommen\r\n\
DESCRIPTION:Lange tekst\r\n die doorloopt\r\n\
BEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:REMINDER\r\nTRIGGER;RELATED=START:-PT15M\r\nEND:VALARM\r\n\
END:VEVENT\r\nEND:VCALENDAR\r\n";

    #[test]
    fn reads_an_outlook_invitation() {
        let i = parse_ics(OUTLOOK).unwrap();
        assert_eq!(i.summary.as_deref(), Some("Verjaardag oma, met taart"));
        assert_eq!(i.location.as_deref(), Some("Dorpsstraat 1, Ommen"));
        let start = i.start.unwrap();
        assert_eq!(start.wall, "2026-11-20T14:00:00");
        assert_eq!(start.zone.as_deref(), Some("Europe/Amsterdam"));
        assert!(!start.all_day);
        assert_eq!(i.end.unwrap().wall, "2026-11-20T17:00:00");
        let org = i.organizer.unwrap();
        assert_eq!(org.name.as_deref(), Some("Jansen, Kees"));
        assert_eq!(org.email, "kees.jansen@voorbeeld.nl");
        assert!(!i.cancelled && !i.repeats);
    }

    #[test]
    fn reads_google_utc_times_whole_days_and_cancellations() {
        let google = "BEGIN:VCALENDAR\nMETHOD:REQUEST\nBEGIN:VEVENT\nDTSTART:20261201T090000Z\nDTEND:20261201T100000Z\n\
RRULE:FREQ=WEEKLY\nSUMMARY:Koor\nORGANIZER;CN=Koor:mailto:koor@voorbeeld.nl\nEND:VEVENT\nEND:VCALENDAR\n";
        let i = parse_ics(google).unwrap();
        assert_eq!(i.start.as_ref().unwrap().zone.as_deref(), Some("UTC"));
        assert!(i.repeats);

        let day = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART;VALUE=DATE:20261224\nDTEND;VALUE=DATE:20261227\nSUMMARY:Kerst\nEND:VEVENT\nEND:VCALENDAR\n";
        let i = parse_ics(day).unwrap();
        assert_eq!(i.start.unwrap(), InviteTime { wall: "2026-12-24".into(), zone: None, all_day: true });

        let cancel = "BEGIN:VCALENDAR\nMETHOD:CANCEL\nBEGIN:VEVENT\nDTSTART:20261201T090000Z\nSUMMARY:Koor\nEND:VEVENT\nEND:VCALENDAR\n";
        assert!(parse_ics(cancel).unwrap().cancelled);
    }

    #[test]
    fn a_reply_or_an_empty_calendar_is_no_invitation() {
        let reply = "BEGIN:VCALENDAR\nMETHOD:REPLY\nBEGIN:VEVENT\nDTSTART:20261201T090000Z\nSUMMARY:Koor\nEND:VEVENT\nEND:VCALENDAR\n";
        assert!(parse_ics(reply).is_none());
        assert!(parse_ics("BEGIN:VCALENDAR\nEND:VCALENDAR\n").is_none());
        assert!(parse_ics("niets").is_none());
    }

    #[test]
    fn an_unknown_zone_is_passed_on_and_bad_times_are_dropped() {
        let ics = "BEGIN:VCALENDAR\nBEGIN:VEVENT\nDTSTART;TZID=\"Mars Standard Time\":20261201T0900\nDTEND:gisteren\nSUMMARY:x\nEND:VEVENT\nEND:VCALENDAR\n";
        let i = parse_ics(ics).unwrap();
        let s = i.start.unwrap();
        assert_eq!(s.wall, "2026-12-01T09:00:00");
        assert_eq!(s.zone.as_deref(), Some("Mars Standard Time"));
        assert!(i.end.is_none());
    }

    #[test]
    fn finds_the_calendar_part_in_a_message() {
        let raw = format!(
            "From: kees@voorbeeld.nl\r\nSubject: Uitnodiging\r\nMIME-Version: 1.0\r\n\
Content-Type: multipart/alternative; boundary=b\r\n\r\n--b\r\nContent-Type: text/plain\r\n\r\nKom je?\r\n\
--b\r\nContent-Type: text/calendar; method=REQUEST; charset=utf-8\r\n\r\n{OUTLOOK}\r\n--b--\r\n"
        );
        let i = find(raw.as_bytes()).unwrap();
        assert_eq!(i.summary.as_deref(), Some("Verjaardag oma, met taart"));
        assert!(find(b"Subject: x\r\n\r\ngeen uitnodiging\r\n").is_none());
    }
}
