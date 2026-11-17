//! Postvak UIT: messages waiting to be sent, with the undo-send delay (C3).

use rusqlite::{OptionalExtension, params};

use super::{Store, json, new_id};
use crate::error::{Code, Result, VemError};
use crate::model::{Draft, OutboxItem, OutboxState, iso};

#[derive(Debug, Clone)]
pub struct OutboxRow {
    pub id: String,
    pub account_id: String,
    pub draft: Draft,
    pub send_at: i64,
    pub state: OutboxState,
    pub attempts: u32,
}

fn state_str(s: OutboxState) -> &'static str {
    match s {
        OutboxState::Waiting => "waiting",
        OutboxState::Sending => "sending",
        OutboxState::Failed => "failed",
    }
}

fn parse_state(s: &str) -> OutboxState {
    match s {
        "sending" => OutboxState::Sending,
        "failed" => OutboxState::Failed,
        _ => OutboxState::Waiting,
    }
}

impl Store {
    pub fn queue_send(&self, draft: &Draft, send_at: i64) -> Result<OutboxItem> {
        let id = new_id();
        self.db().execute(
            "INSERT INTO outbox (id, account_id, draft_json, subject, send_at, state) VALUES (?1, ?2, ?3, ?4, ?5, 'waiting')",
            params![id, draft.account_id, json(draft), draft.subject, send_at],
        )?;
        Ok(OutboxItem { id, subject: draft.subject.clone(), send_at: iso(send_at), state: OutboxState::Waiting, error: None })
    }

    pub fn outbox(&self) -> Result<Vec<OutboxItem>> {
        let db = self.db();
        let mut st = db.prepare("SELECT id, subject, send_at, state, error_json FROM outbox ORDER BY send_at")?;
        let rows = st
            .query_map([], |r| {
                let err: Option<String> = r.get(4)?;
                Ok(OutboxItem {
                    id: r.get(0)?,
                    subject: r.get(1)?,
                    send_at: iso(r.get(2)?),
                    state: parse_state(&r.get::<_, String>(3)?),
                    error: err.and_then(|e| serde_json::from_str::<serde_json::Value>(&e).ok()).map(|v| VemError {
                        code: v["code"].as_str().unwrap_or("VEM-APP-CMD-01").to_string(),
                        server_reply: v["serverReply"].as_str().map(str::to_string),
                        detail: v["detail"].as_str().map(str::to_string),
                        kind: None,
                        reported: v["reported"].as_bool().unwrap_or(false),
                    }),
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    /// Take one message whose time has come and mark it as being sent.
    pub fn take_due(&self, now: i64) -> Result<Option<OutboxRow>> {
        let db = self.db();
        let row = db
            .query_row(
                "SELECT id, account_id, draft_json, send_at, state, attempts FROM outbox
                 WHERE state = 'waiting' AND send_at <= ?1 ORDER BY send_at LIMIT 1",
                [now],
                |r| {
                    Ok(OutboxRow {
                        id: r.get(0)?,
                        account_id: r.get(1)?,
                        draft: serde_json::from_str(&r.get::<_, String>(2)?).unwrap_or_default(),
                        send_at: r.get(3)?,
                        state: parse_state(&r.get::<_, String>(4)?),
                        attempts: r.get::<_, i64>(5)? as u32,
                    })
                },
            )
            .optional()?;
        if let Some(r) = &row {
            db.execute("UPDATE outbox SET state = 'sending', attempts = attempts + 1 WHERE id = ?1", [&r.id])?;
        }
        Ok(row)
    }

    /// Back to waiting after a failure, with the error shown and a later retry time.
    pub fn send_failed(&self, id: &str, error: &VemError, retry_at: Option<i64>) -> Result<()> {
        let state = if retry_at.is_some() { OutboxState::Waiting } else { OutboxState::Failed };
        let err = serde_json::json!({"code": error.code, "serverReply": error.server_reply, "detail": error.detail, "reported": error.reported});
        self.db().execute(
            "UPDATE outbox SET state = ?2, error_json = ?3, send_at = COALESCE(?4, send_at) WHERE id = ?1",
            params![id, state_str(state), err.to_string(), retry_at],
        )?;
        Ok(())
    }

    /// The connection is back: messages of this account that wait for a retry go now instead of
    /// at their retry time. Returns how many.
    pub fn retry_waiting_now(&self, account_id: &str, now: i64) -> Result<usize> {
        Ok(self.db().execute(
            "UPDATE outbox SET send_at = ?2
             WHERE account_id = ?1 AND state = 'waiting' AND error_json IS NOT NULL AND send_at > ?2",
            params![account_id, now],
        )?)
    }

    pub fn sent(&self, id: &str) -> Result<()> {
        self.db().execute("DELETE FROM outbox WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Undo send: only while still waiting. Returns the draft to put back in the Write screen.
    pub fn cancel_send(&self, id: &str) -> Result<Draft> {
        let db = self.db();
        let draft: Option<String> = db
            .query_row("SELECT draft_json FROM outbox WHERE id = ?1 AND state != 'sending'", [id], |r| r.get(0))
            .optional()?;
        let Some(d) = draft else {
            return Err(VemError::new(Code::AppCmd01).with_detail("already sending"));
        };
        db.execute("DELETE FROM outbox WHERE id = ?1", [id])?;
        Ok(serde_json::from_str(&d).unwrap_or_default())
    }

    /// After a crash, anything left "sending" goes back to waiting.
    pub fn reset_sending(&self) -> Result<()> {
        self.db().execute("UPDATE outbox SET state = 'waiting' WHERE state = 'sending'", [])?;
        Ok(())
    }

    pub fn is_sending(&self) -> Result<bool> {
        let n: i64 = self.db().query_row("SELECT COUNT(*) FROM outbox WHERE state = 'sending'", [], |r| r.get(0))?;
        Ok(n > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::testutil::store;

    #[test]
    fn queue_take_cancel() {
        let (_d, s) = store();
        let d = Draft { account_id: "a".into(), subject: "Hoi".into(), ..Default::default() };
        let item = s.queue_send(&d, 100).unwrap();
        assert!(s.take_due(99).unwrap().is_none());
        let back = s.cancel_send(&item.id).unwrap();
        assert_eq!(back.subject, "Hoi");
        assert!(s.outbox().unwrap().is_empty());

        let item = s.queue_send(&d, 100).unwrap();
        let row = s.take_due(100).unwrap().unwrap();
        assert_eq!(row.id, item.id);
        assert!(s.cancel_send(&item.id).is_err());
        s.send_failed(&item.id, &VemError::new(Code::SmtpConnect01), Some(200)).unwrap();
        assert_eq!(s.outbox().unwrap()[0].error.as_ref().unwrap().code, "VEM-SMTP-CONNECT-01");
        assert!(s.take_due(150).unwrap().is_none());
        assert!(s.take_due(200).unwrap().is_some());
        s.sent(&item.id).unwrap();
        assert!(s.outbox().unwrap().is_empty());
    }

    #[test]
    fn a_refused_message_is_kept_with_every_line_of_the_reply() {
        let (_d, s) = store();
        let d = Draft { account_id: "a".into(), subject: "Uitnodiging".into(), body_text: "Komt u?".into(), ..Default::default() };
        let item = s.queue_send(&d, 100).unwrap();
        s.take_due(100).unwrap().unwrap();
        let reply = "550-5.1.1 The email account that you tried to reach does not exist.\r\n550-5.1.1  Please try\r\n550 5.1.1 double-checking";
        s.send_failed(&item.id, &VemError::new(Code::SmtpCmd01).with_reply(reply.to_string()), None).unwrap();
        let shown = s.outbox().unwrap();
        assert_eq!(shown[0].state, OutboxState::Failed);
        assert_eq!(shown[0].error.as_ref().unwrap().server_reply.as_deref(), Some(reply), "every line, as sent");
        assert!(s.take_due(i64::MAX).unwrap().is_none(), "not tried again by itself");
        let back = s.cancel_send(&item.id).unwrap();
        assert_eq!((back.subject.as_str(), back.body_text.as_str()), ("Uitnodiging", "Komt u?"), "the message is kept");
    }

    #[test]
    fn a_returning_connection_brings_retries_forward_only_for_that_account() {
        let (_d, s) = store();
        let mine = s.queue_send(&Draft { account_id: "a".into(), ..Default::default() }, 100).unwrap();
        let other = s.queue_send(&Draft { account_id: "b".into(), ..Default::default() }, 100).unwrap();
        let fresh = s.queue_send(&Draft { account_id: "a".into(), ..Default::default() }, 500).unwrap();
        for _ in 0..2 {
            let row = s.take_due(100).unwrap().unwrap();
            s.send_failed(&row.id, &VemError::new(Code::SmtpConnect01), Some(400)).unwrap();
        }
        assert!(s.outbox().unwrap().iter().filter(|i| i.id != fresh.id).all(|i| i.error.is_some()));
        let _ = other;
        assert_eq!(s.retry_waiting_now("a", 150).unwrap(), 1, "only the failed one of account a");
        assert_eq!(s.take_due(150).unwrap().unwrap().id, mine.id);
        assert!(s.take_due(150).unwrap().is_none(), "b and the undo delay of the fresh one are kept");
    }
}
