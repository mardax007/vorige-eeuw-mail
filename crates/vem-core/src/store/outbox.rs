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
}
