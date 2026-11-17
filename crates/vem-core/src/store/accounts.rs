use rusqlite::{OptionalExtension, Row, params};

use super::{Store, json, new_id};
use crate::error::{Result, VemError};
use crate::model::{Account, AccountKind, Quota, ServerSettings, now};

fn row_to_account(r: &Row<'_>) -> rusqlite::Result<Account> {
    let kind: String = r.get("kind")?;
    let settings_json: Option<String> = r.get("settings_json")?;
    let settings = settings_json.and_then(|s| serde_json::from_str::<ServerSettings>(&s).ok());
    let used: Option<i64> = r.get("quota_used_kb")?;
    let limit: Option<i64> = r.get("quota_limit_kb")?;
    let err: Option<String> = r.get("last_error_json")?;
    Ok(Account {
        id: r.get("id")?,
        display_name: r.get("display_name")?,
        email: r.get("email")?,
        kind: if kind == "local" { AccountKind::Local } else { AccountKind::Imap },
        quota: used.map(|u| Quota { used_kb: u as u64, limit_kb: limit.map(|l| l as u64) }),
        signature: r.get("signature")?,
        last_error: err.and_then(|e| serde_json::from_str::<StoredError>(&e).ok()).map(Into::into),
        sign_in: settings.as_ref().map(|s| s.sign_in),
        settings,
    })
}

/// VemError without the skipped field, for storing in the index.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredError {
    code: String,
    server_reply: Option<String>,
    detail: Option<String>,
}

impl From<StoredError> for VemError {
    fn from(s: StoredError) -> Self {
        VemError { code: s.code, server_reply: s.server_reply, detail: s.detail, kind: None }
    }
}

impl Store {
    pub fn add_account(
        &self,
        display_name: &str,
        email: &str,
        kind: AccountKind,
        settings: Option<&ServerSettings>,
    ) -> Result<Account> {
        let id = new_id();
        let db = self.db();
        let pos: i64 = db.query_row("SELECT COALESCE(MAX(position), -1) + 1 FROM accounts", [], |r| r.get(0))?;
        db.execute(
            "INSERT INTO accounts (id, display_name, email, kind, settings_json, position, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                display_name.trim(),
                email.trim(),
                if kind == AccountKind::Local { "local" } else { "imap" },
                settings.map(json),
                pos,
                now()
            ],
        )?;
        drop(db);
        self.account(&id)
    }

    pub fn accounts(&self) -> Result<Vec<Account>> {
        let db = self.db();
        let mut st = db.prepare("SELECT * FROM accounts ORDER BY position")?;
        let rows = st.query_map([], row_to_account)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn account(&self, id: &str) -> Result<Account> {
        self.db()
            .query_row("SELECT * FROM accounts WHERE id = ?1", [id], row_to_account)
            .optional()?
            .ok_or_else(|| VemError::new(crate::Code::StoreRead01).with_detail("account"))
    }

    pub fn remove_account(&self, id: &str) -> Result<()> {
        let dir = self.mail_dir().join(id);
        {
            let db = self.db();
            db.execute(
                "DELETE FROM messages_fts WHERE rowid IN (SELECT id FROM messages WHERE account_id = ?1)",
                [id],
            )?;
            db.execute("DELETE FROM accounts WHERE id = ?1", [id])?;
            db.execute("DELETE FROM outbox WHERE account_id = ?1", [id])?;
            db.execute("DELETE FROM pending_ops WHERE account_id = ?1", [id])?;
        }
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn set_signature(&self, id: &str, signature: Option<&str>) -> Result<()> {
        self.db().execute(
            "UPDATE accounts SET signature = ?2 WHERE id = ?1",
            params![id, signature.map(str::trim).filter(|s| !s.is_empty())],
        )?;
        Ok(())
    }

    pub fn set_account_settings(&self, id: &str, settings: &ServerSettings) -> Result<()> {
        self.db()
            .execute("UPDATE accounts SET settings_json = ?2 WHERE id = ?1", params![id, json(settings)])?;
        Ok(())
    }

    pub fn set_quota(&self, id: &str, quota: Option<&Quota>) -> Result<()> {
        self.db().execute(
            "UPDATE accounts SET quota_used_kb = ?2, quota_limit_kb = ?3 WHERE id = ?1",
            params![id, quota.map(|q| q.used_kb as i64), quota.and_then(|q| q.limit_kb.map(|l| l as i64))],
        )?;
        Ok(())
    }

    pub fn set_last_error(&self, id: &str, error: Option<&VemError>) -> Result<()> {
        let stored = error.map(|e| StoredError {
            code: e.code.clone(),
            server_reply: e.server_reply.clone(),
            detail: e.detail.clone(),
        });
        self.db().execute(
            "UPDATE accounts SET last_error_json = ?2 WHERE id = ?1",
            params![id, stored.as_ref().map(json)],
        )?;
        Ok(())
    }

    pub fn set_capabilities(&self, id: &str, caps: &[String]) -> Result<()> {
        self.db()
            .execute("UPDATE accounts SET capabilities = ?2 WHERE id = ?1", params![id, caps.join(" ")])?;
        Ok(())
    }

    pub fn capabilities(&self, id: &str) -> Result<Vec<String>> {
        let s: Option<String> = self
            .db()
            .query_row("SELECT capabilities FROM accounts WHERE id = ?1", [id], |r| r.get(0))
            .optional()?
            .flatten();
        Ok(s.map(|s| s.split(' ').filter(|c| !c.is_empty()).map(str::to_string).collect()).unwrap_or_default())
    }

    /// Whether the server files a sent message in Sent by itself (Gmail does). `None`: unknown.
    pub fn server_saves_sent(&self, id: &str) -> Result<Option<bool>> {
        let v: Option<i64> = self
            .db()
            .query_row("SELECT server_saves_sent FROM accounts WHERE id = ?1", [id], |r| r.get(0))
            .optional()?
            .flatten();
        Ok(v.map(|v| v != 0))
    }

    pub fn set_server_saves_sent(&self, id: &str, saves: bool) -> Result<()> {
        self.db()
            .execute("UPDATE accounts SET server_saves_sent = ?2 WHERE id = ?1", params![id, saves as i64])?;
        Ok(())
    }

    /// The local account that holds imported old mail, made on first use.
    pub fn local_account(&self, label: &str) -> Result<Account> {
        let existing: Option<String> = self
            .db()
            // A copy of a PC's local folders (docs/pc-to-phone.md) is not where this device's own
            // imports go: the PC may remove that copy.
            .query_row(
                "SELECT id FROM accounts WHERE kind = 'local' AND display_name = ?1
                 AND id NOT IN (SELECT account_id FROM lan_accounts)",
                [label],
                |r| r.get(0),
            )
            .optional()?;
        match existing {
            Some(id) => self.account(&id),
            None => self.add_account(label, "", AccountKind::Local, None),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{AccountKind, SignIn};
    use crate::net::Security;
    use crate::store::testutil::store;

    fn settings() -> crate::model::ServerSettings {
        crate::model::ServerSettings {
            imap_host: "imap.solcon.nl".into(),
            imap_port: 993,
            imap_tls: Security::Tls,
            smtp_host: "mail.solcon.nl".into(),
            smtp_port: 587,
            smtp_tls: Security::Starttls,
            username: "jan".into(),
            source: "table".into(),
            needs_confirmation: false,
            sign_in: SignIn::Password,
        }
    }

    #[test]
    fn add_list_remove() {
        let (_d, s) = store();
        let a = s.add_account("Jan", "jan@solcon.nl", AccountKind::Imap, Some(&settings())).unwrap();
        let b = s.add_account("Oud", "", AccountKind::Local, None).unwrap();
        let all = s.accounts().unwrap();
        assert_eq!(all.iter().map(|a| a.id.clone()).collect::<Vec<_>>(), vec![a.id.clone(), b.id.clone()]);
        assert_eq!(all[0].settings.as_ref().unwrap().imap_host, "imap.solcon.nl");
        s.remove_account(&a.id).unwrap();
        assert_eq!(s.accounts().unwrap().len(), 1);
    }

    #[test]
    fn errors_and_quota_round_trip() {
        let (_d, s) = store();
        let a = s.add_account("Jan", "jan@x.nl", AccountKind::Imap, None).unwrap();
        let e = crate::VemError::new(crate::Code::ImapLogin01).with_reply("a1 NO nope");
        s.set_last_error(&a.id, Some(&e)).unwrap();
        s.set_quota(&a.id, Some(&crate::model::Quota { used_kb: 10, limit_kb: Some(100) })).unwrap();
        let a = s.account(&a.id).unwrap();
        assert_eq!(a.last_error.unwrap().server_reply.as_deref(), Some("a1 NO nope"));
        assert_eq!(a.quota.unwrap().limit_kb, Some(100));
    }

    #[test]
    fn local_account_is_reused() {
        let (_d, s) = store();
        let a = s.local_account("Oude mail uit Outlook").unwrap();
        let b = s.local_account("Oude mail uit Outlook").unwrap();
        assert_eq!(a.id, b.id);
    }
}
