//! The background work: one loop per mailbox (sync, bodies, IDLE), the send queue with its
//! undo delay, error reports to Martijn, and housekeeping. The Tauri shell owns one `Engine`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::Notify;
use tokio::task::JoinHandle;

use crate::compose;
use crate::contacts_sync;
use crate::error::{Code, Result, VemError};
use crate::imap::ops::Op;
use crate::imap::{Auth, Imap, ImapConfig, sync};
use crate::log;
use crate::model::{
    Account, AccountKind, Addr, Draft, FolderRole, MessageSummary, OutboxItem, ServerSettings, SignIn, now,
};
use crate::net::Trust;
use crate::oauth;
use crate::report::{self, AppFacts};
use crate::secrets::{Secret, Secrets};
use crate::smtp::{self, SmtpConfig};
use crate::store::{NewMessage, Store};

/// Longest IDLE before the connection is checked again (servers drop IDLE after 29 minutes).
const IDLE_MAX: Duration = Duration::from_secs(20 * 60);
/// While actions wait for the server, look again this often.
const OPS_POLL: Duration = Duration::from_secs(30);
const BODIES_PER_ROUND: u32 = 100;
const RETRY_FIRST: Duration = Duration::from_secs(30);
const RETRY_MAX: Duration = Duration::from_secs(15 * 60);
/// A send that failed on the connection is tried again after this long.
const SEND_RETRY_SECS: i64 = 5 * 60;
const REPORT_QUEUE: &str = "report_queue";

/// What the engine tells the screens. The shell turns these into Tauri events and notifications.
pub trait Sink: Send + Sync + 'static {
    fn mail_changed(&self, account_id: &str, folder_id: Option<String>);
    fn sync_state(&self, account_id: &str, busy: bool, error: Option<VemError>);
    fn outbox_changed(&self, items: Vec<OutboxItem>);
    fn new_mail(&self, account_id: &str, messages: Vec<MessageSummary>);
}

pub struct Engine {
    pub store: Arc<Store>,
    pub secrets: Arc<Secrets>,
    sink: Arc<dyn Sink>,
    trust: Trust,
    pub facts: AppFacts,
    wakes: Mutex<HashMap<String, Arc<Notify>>>,
    loops: Mutex<HashMap<String, JoinHandle<()>>>,
    /// The send queue and housekeeping loops, so `pause` can stop them.
    background: Mutex<Vec<JoinHandle<()>>>,
    /// Held while the send queue sends: `pause` waits for it, so nothing goes out twice.
    sending: tokio::sync::Mutex<()>,
    outbox_wake: Arc<Notify>,
}

impl Engine {
    pub fn new(store: Arc<Store>, secrets: Arc<Secrets>, sink: Arc<dyn Sink>, trust: Trust, facts: AppFacts) -> Arc<Engine> {
        Arc::new(Engine {
            store,
            secrets,
            sink,
            trust,
            facts,
            wakes: Mutex::new(HashMap::new()),
            loops: Mutex::new(HashMap::new()),
            background: Mutex::new(Vec::new()),
            sending: tokio::sync::Mutex::new(()),
            outbox_wake: Arc::new(Notify::new()),
        })
    }

    /// Start every loop, inside the Tokio runtime. Also starts again after `pause`.
    pub fn start(self: &Arc<Self>) -> Result<()> {
        self.store.reset_sending()?;
        for a in self.store.accounts()? {
            if a.kind == AccountKind::Imap {
                self.start_account(&a.id);
            }
        }
        let me = self.clone();
        let outbox = tokio::spawn(async move { me.outbox_loop().await });
        let me = self.clone();
        let housekeeping = tokio::spawn(async move { me.housekeeping_loop().await });
        let old = std::mem::replace(&mut *self.background.lock().unwrap_or_else(|e| e.into_inner()), vec![outbox, housekeeping]);
        old.into_iter().for_each(|h| h.abort());
        Ok(())
    }

    /// Stop all work: the mailbox loops, the send queue and housekeeping. A message that is going
    /// out is sent first. `start` begins again. On Android only one process may work on the
    /// mailboxes at a time: the open app, or the background service of decision 24.
    pub async fn pause(&self) {
        let _sending = self.sending.lock().await;
        let background = std::mem::take(&mut *self.background.lock().unwrap_or_else(|e| e.into_inner()));
        background.into_iter().for_each(|h| h.abort());
        let loops = std::mem::take(&mut *self.loops.lock().unwrap_or_else(|e| e.into_inner()));
        loops.into_values().for_each(|h| h.abort());
        log::event("engine.paused", &[]);
    }

    fn wake_for(&self, account_id: &str) -> Arc<Notify> {
        self.wakes.lock().unwrap_or_else(|e| e.into_inner()).entry(account_id.to_string()).or_default().clone()
    }

    pub fn start_account(self: &Arc<Self>, account_id: &str) {
        let wake = self.wake_for(account_id);
        let me = self.clone();
        let id = account_id.to_string();
        let handle = tokio::spawn(async move { me.account_loop(id, wake).await });
        if let Some(old) = self.loops.lock().unwrap_or_else(|e| e.into_inner()).insert(account_id.to_string(), handle) {
            old.abort();
        }
    }

    pub fn stop_account(&self, account_id: &str) {
        if let Some(h) = self.loops.lock().unwrap_or_else(|e| e.into_inner()).remove(account_id) {
            h.abort();
        }
    }

    /// Look at the server now ("Mail ophalen", or after an action). None wakes every mailbox.
    pub fn wake(&self, account_id: Option<&str>) {
        let wakes = self.wakes.lock().unwrap_or_else(|e| e.into_inner());
        match account_id {
            Some(id) => {
                if let Some(w) = wakes.get(id) {
                    w.notify_one();
                }
            }
            None => wakes.values().for_each(|w| w.notify_one()),
        }
    }

    // ---- connections -------------------------------------------------------------------

    async fn auth(&self, account: &Account) -> Result<Auth> {
        let settings = account.settings.as_ref().ok_or_else(|| VemError::new(Code::StoreSecret01).with_detail("settings"))?;
        let secret = self.secrets.get(&account.id)?.ok_or_else(|| VemError::new(Code::StoreSecret01).with_detail("missing"))?;
        let secret = match oauth::refresh(settings.sign_in, &secret).await? {
            Some(fresh) => {
                self.secrets.put(&account.id, &fresh)?;
                fresh
            }
            None => secret,
        };
        Ok(match secret {
            Secret::Password { password } => Auth::Password(password),
            Secret::OAuth { access_token, .. } => Auth::OAuth(access_token),
            Secret::SyncKey { .. } => return Err(VemError::new(Code::StoreSecret01).with_detail("kind")),
        })
    }

    pub fn imap_config(settings: &ServerSettings, auth: Auth, trust: Trust) -> ImapConfig {
        ImapConfig {
            host: settings.imap_host.clone(),
            port: settings.imap_port,
            security: settings.imap_tls,
            username: settings.username.clone(),
            auth,
            trust,
        }
    }

    pub fn smtp_config(settings: &ServerSettings, auth: Auth, trust: Trust) -> SmtpConfig {
        SmtpConfig {
            host: settings.smtp_host.clone(),
            port: settings.smtp_port,
            security: settings.smtp_tls,
            username: settings.username.clone(),
            auth,
            trust,
        }
    }

    async fn connect(&self, account_id: &str) -> Result<Imap> {
        let account = self.store.account(account_id)?;
        let settings = account.settings.clone().ok_or_else(|| VemError::new(Code::StoreSecret01).with_detail("settings"))?;
        let auth = self.auth(&account).await?;
        Imap::connect(&Self::imap_config(&settings, auth, self.trust)).await
    }

    /// Account setup: sign in to both servers before anything is saved.
    pub async fn add_account(
        self: &Arc<Self>,
        display_name: &str,
        email: &str,
        secret: Secret,
        settings: ServerSettings,
    ) -> Result<Account> {
        let auth = match &secret {
            Secret::Password { password } => Auth::Password(password.clone()),
            Secret::OAuth { access_token, .. } => Auth::OAuth(access_token.clone()),
            Secret::SyncKey { .. } => return Err(VemError::new(Code::AppCmd01)),
        };
        let imap = Imap::connect(&Self::imap_config(&settings, auth.clone(), self.trust)).await?;
        imap.logout().await;
        smtp::check_login(&Self::smtp_config(&settings, auth, self.trust)).await?;
        let account = self.store.add_account(display_name, email, AccountKind::Imap, Some(&settings))?;
        if let Err(e) = self.secrets.put(&account.id, &secret) {
            self.store.remove_account(&account.id)?;
            return Err(e);
        }
        // Gmail and Outlook.com keep a copy of what goes out over SMTP themselves.
        let saves = matches!(settings.sign_in, SignIn::Google | SignIn::Microsoft)
            || settings.smtp_host.ends_with("gmail.com")
            || settings.smtp_host.contains("outlook");
        self.store.set_server_saves_sent(&account.id, saves)?;
        log::event("account.added", &[("account", &account.id), ("imap", &settings.imap_host), ("smtp", &settings.smtp_host)]);
        self.start_account(&account.id);
        Ok(account)
    }

    /// A new password or a fresh Google/Microsoft sign-in for an existing mailbox. It is tried
    /// against both servers first; only then is it saved and the mailbox reconnected.
    pub async fn update_secret(self: &Arc<Self>, account_id: &str, secret: Secret) -> Result<()> {
        let account = self.store.account(account_id)?;
        let settings = account.settings.clone().ok_or_else(|| VemError::new(Code::StoreSecret01).with_detail("settings"))?;
        let auth = match &secret {
            Secret::Password { password } => Auth::Password(password.clone()),
            Secret::OAuth { access_token, .. } => Auth::OAuth(access_token.clone()),
            Secret::SyncKey { .. } => return Err(VemError::new(Code::AppCmd01)),
        };
        let imap = Imap::connect(&Self::imap_config(&settings, auth.clone(), self.trust)).await?;
        imap.logout().await;
        smtp::check_login(&Self::smtp_config(&settings, auth, self.trust)).await?;
        self.secrets.put(account_id, &secret)?;
        log::event("account.secret_updated", &[("account", account_id)]);
        // Both servers just accepted it, so the old error is over; the screens reload right away.
        self.store.set_last_error(account_id, None)?;
        self.stop_account(account_id);
        self.start_account(account_id);
        self.sink.sync_state(account_id, false, None);
        Ok(())
    }

    pub fn remove_account(&self, account_id: &str) -> Result<()> {
        self.stop_account(account_id);
        self.secrets.delete(account_id)?;
        if contacts_sync::paired_account(&self.store)?.as_deref() == Some(account_id) {
            contacts_sync::unpair(&self.store, &self.secrets)?;
        }
        self.store.remove_account(account_id)?;
        log::event("account.removed", &[("account", account_id)]);
        Ok(())
    }

    // ---- the mailbox loop --------------------------------------------------------------

    async fn account_loop(self: Arc<Self>, id: String, wake: Arc<Notify>) {
        let mut conn: Option<Imap> = None;
        let mut retry = RETRY_FIRST;
        let mut first_run = self.store.folder_infos(&id).map(|f| f.is_empty()).unwrap_or(true);
        loop {
            self.sink.sync_state(&id, true, None);
            let result = async {
                let mut imap = match conn.take() {
                    Some(i) => i,
                    None => self.connect(&id).await?,
                };
                self.round(&id, &mut imap, first_run).await?;
                Ok::<Imap, VemError>(imap)
            }
            .await;
            match result {
                Ok(imap) => {
                    first_run = false;
                    if retry != RETRY_FIRST {
                        // The connection is back after a failure: what waits in Postvak UIT goes now.
                        if self.store.retry_waiting_now(&id, now()).unwrap_or(0) > 0 {
                            self.outbox_changed();
                            self.outbox_wake.notify_one();
                        }
                    }
                    retry = RETRY_FIRST;
                    let _ = self.store.set_last_error(&id, None);
                    self.sink.sync_state(&id, false, None);
                    if self.store.count_without_body(&id).unwrap_or(0) > 0 {
                        // More bodies to fetch: go again at once, unless asked to stop.
                        conn = Some(imap);
                        continue;
                    }
                    conn = self.idle(&id, imap, &wake).await;
                }
                Err(e) => {
                    log::error("sync.failed", &e);
                    let reported = self.report(Some(&id), &e).await;
                    let e = e.with_reported(reported);
                    let _ = self.store.set_last_error(&id, Some(&e));
                    self.sink.sync_state(&id, false, Some(e.clone()));
                    tokio::select! {
                        _ = tokio::time::sleep(retry) => {}
                        _ = wake.notified() => {}
                    }
                    retry = (retry * 2).min(RETRY_MAX);
                }
            }
        }
    }

    /// One pass: actions, folders, new mail, then a batch of bodies.
    async fn round(&self, id: &str, imap: &mut Imap, first_run: bool) -> Result<()> {
        let report = sync::sync_account(&self.store, imap, id, first_run).await?;
        for f in &report.changed_folders {
            self.sink.mail_changed(id, Some(f.to_string()));
        }
        if report.folder_list_changed {
            // No folder id: the screens read the whole folder list again.
            self.sink.mail_changed(id, None);
        }
        if !report.new_in_inbox.is_empty() {
            let msgs: Vec<MessageSummary> =
                report.new_in_inbox.iter().filter_map(|m| self.store.message(*m).ok().map(|s| s.summary)).collect();
            self.sink.new_mail(id, msgs);
        }
        let n = sync::fetch_bodies(&self.store, imap, id, BODIES_PER_ROUND).await?;
        if n > 0 {
            self.sink.mail_changed(id, None);
        }
        match contacts_sync::sync(&self.store, &self.secrets, imap, id).await {
            Ok(_) => {}
            Err(e) => log::error("contacts.sync_failed", &e),
        }
        Ok(())
    }

    /// Wait on Postvak IN until something changes, an action is due, or someone wakes us.
    async fn idle(&self, id: &str, mut imap: Imap, wake: &Notify) -> Option<Imap> {
        let ops_waiting = !self.store.ops::<Op>(id).map(|o| o.is_empty()).unwrap_or(true);
        let timeout = if ops_waiting { OPS_POLL } else { IDLE_MAX };
        if imap.open("INBOX", true).await.is_err() {
            return None;
        }
        match imap.wait_for_change(timeout, wake).await {
            Ok((imap, _)) => Some(imap),
            Err(e) => {
                log::error("idle.ended", &e);
                None
            }
        }
    }

    /// Fetch one message's body now, when the reading pane needs it before the background did.
    pub async fn ensure_body(&self, message_id: i64) -> Result<()> {
        // A message file removed behind our back is downloaded again here (mailbox mail only).
        if self.store.forget_lost_body(message_id)? {
            log::event("store.body_lost", &[("message", &message_id.to_string())]);
        }
        let m = self.store.message(message_id)?;
        if m.has_body {
            return Ok(());
        }
        let Some(uid) = m.uid else { return Ok(()) };
        let folder = self.store.folder(m.folder_id)?;
        let mut imap = self.connect(&m.summary.account_id).await?;
        imap.open(&folder.name, true).await?;
        if let Some(r) = imap.fetch(&uid.to_string(), true).await?.into_iter().find(|r| r.uid == uid) {
            self.store.set_body(message_id, &r.raw)?;
        }
        imap.logout().await;
        Ok(())
    }

    /// Download the text of every message of an account (read-only, BODY.PEEK), before its mail
    /// is copied to another mailbox. Stops between batches when `cancel` is set.
    pub async fn download_bodies(&self, account_id: &str, cancel: &std::sync::atomic::AtomicBool) -> Result<()> {
        let mut imap = self.connect(account_id).await?;
        let result = loop {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                break Ok(());
            }
            match sync::fetch_bodies(&self.store, &mut imap, account_id, BODIES_PER_ROUND).await {
                Ok(0) => break Ok(()),
                Ok(_) => self.sink.mail_changed(account_id, None),
                Err(e) => break Err(e),
            }
        };
        imap.logout().await;
        result
    }

    // ---- sending -----------------------------------------------------------------------

    /// "Verzenden": into Postvak UIT; it goes after the undo delay.
    pub fn send(&self, draft: Draft) -> Result<OutboxItem> {
        let draft = compose::clean_draft(draft);
        let account = self.store.account(&draft.account_id)?;
        let from = Addr::new(Some(account.display_name.as_str()), &account.send_from(draft.from_address.as_deref()));
        // Check the message can be built before promising it will go.
        compose::build(&draft, &from, now())?;
        let delay = i64::from(self.store.settings()?.undo_send_seconds);
        let item = self.store.queue_send(&draft, now() + delay)?;
        if let Some(draft_id) = draft.id.as_deref().and_then(|d| d.parse::<i64>().ok())
            && self.store.message(draft_id).map(|m| m.is_draft).unwrap_or(false) {
                self.store.delete_message(draft_id)?;
                self.sink.mail_changed(&draft.account_id, None);
            }
        self.outbox_changed();
        self.outbox_wake.notify_one();
        Ok(item)
    }

    pub fn cancel_send(&self, outbox_id: &str) -> Result<Draft> {
        let d = self.store.cancel_send(outbox_id)?;
        self.outbox_changed();
        Ok(d)
    }

    fn outbox_changed(&self) {
        if let Ok(items) = self.store.outbox() {
            self.sink.outbox_changed(items);
        }
    }

    /// Send what is due, then the waiting error reports. Each message holds `sending` while it
    /// goes out, so `pause` waits for that message only, not for the whole queue.
    async fn send_due(&self) {
        loop {
            let _sending = self.sending.lock().await;
            let Ok(Some(row)) = self.store.take_due(now()) else { break };
            self.outbox_changed();
            match self.deliver(&row.draft).await {
                Ok(()) => {
                    let _ = self.store.sent(&row.id);
                    log::event("send.done", &[("outbox", &row.id)]);
                }
                Err(e) => {
                    log::error("send.failed", &e);
                    let retry = matches!(e.code(), Some(Code::SmtpConnect01) | Some(Code::SmtpTls01))
                        .then(|| now() + SEND_RETRY_SECS);
                    let reported = self.report(Some(&row.account_id), &e).await;
                    let _ = self.store.send_failed(&row.id, &e.with_reported(reported), retry);
                }
            }
            self.outbox_changed();
        }
        let _sending = self.sending.lock().await;
        self.flush_reports().await;
    }

    async fn outbox_loop(self: Arc<Self>) {
        loop {
            self.send_due().await;
            let next = self.store.outbox().ok().and_then(|items| {
                items
                    .iter()
                    .filter(|i| i.state == crate::model::OutboxState::Waiting)
                    .filter_map(|i| chrono::DateTime::parse_from_rfc3339(&i.send_at).ok())
                    .map(|t| t.timestamp())
                    .min()
            });
            let wait = next.map(|t| Duration::from_secs((t - now()).clamp(0, 300) as u64)).unwrap_or(Duration::from_secs(300));
            tokio::select! {
                _ = tokio::time::sleep(wait) => {}
                _ = self.outbox_wake.notified() => {}
            }
        }
    }

    /// Send one message, then keep a copy in Verzonden items and learn the addresses.
    async fn deliver(&self, draft: &Draft) -> Result<()> {
        let account = self.store.account(&draft.account_id)?;
        let settings = account.settings.clone().ok_or_else(|| VemError::new(Code::StoreSecret01).with_detail("settings"))?;
        // The address the message goes out from, also in MAIL FROM: iCloud refuses another one.
        let sender = account.send_from(draft.from_address.as_deref());
        let from = Addr::new(Some(account.display_name.as_str()), &sender);
        let built = compose::build(draft, &from, now())?;
        let auth = self.auth(&account).await?;
        smtp::send(&Self::smtp_config(&settings, auth, self.trust), &sender, &built.recipients, &built.raw).await?;
        for a in draft.to.iter().chain(&draft.cc).chain(&draft.bcc) {
            let _ = self.store.learn_contact(a.name.as_deref(), &a.email);
        }
        let is_report = draft.subject.starts_with(report::SUBJECT_PREFIX);
        if let Ok(Some(sent)) = self.store.folder_by_role(&account.id, FolderRole::Sent) {
            let server_saves = self.store.server_saves_sent(&account.id)?.unwrap_or(false);
            if !server_saves && !is_report {
                let id = self.store.insert_message(&NewMessage {
                    account_id: account.id.clone(),
                    folder_id: sent,
                    uid: None,
                    raw: built.raw.clone(),
                    has_body: true,
                    unread: false,
                    important: false,
                    size: built.raw.len() as u64,
                    fallback_date: Some(now()),
                })?;
                self.store.push_op(&account.id, &Op::Append { message_id: id, folder: sent, seen: true, flagged: false })?;
                self.sink.mail_changed(&account.id, Some(sent.to_string()));
                self.wake(Some(&account.id));
            }
        }
        Ok(())
    }

    // ---- reports -----------------------------------------------------------------------

    fn report_sender(&self, account_id: Option<&str>) -> Option<Account> {
        let accounts = self.store.accounts().ok()?;
        account_id
            .and_then(|id| accounts.iter().find(|a| a.id == id && a.kind == AccountKind::Imap).cloned())
            .or_else(|| accounts.into_iter().find(|a| a.kind == AccountKind::Imap))
    }

    /// Queues a report about `e` for Martijn, within the daily limits. Returns whether one was
    /// queued, so the screen can say "Martijn krijgt hier vanzelf bericht van" only when true.
    pub async fn report(&self, account_id: Option<&str>, e: &VemError) -> bool {
        if report::support_address().is_none() {
            return false;
        }
        let Some(sender) = self.report_sender(account_id) else { return false };
        let earlier = match report::admit(&self.store, &e.code) {
            Ok(Some(n)) => n,
            _ => return false,
        };
        let from = Addr::new(Some(sender.display_name.as_str()), &sender.email);
        match report::error_report(&self.store, &self.facts, &from, &sender.id, e, earlier) {
            Ok(Some(d)) => {
                self.queue_report(d);
                self.outbox_wake.notify_one();
                true
            }
            _ => false,
        }
    }

    pub fn queue_report(&self, d: Draft) {
        let mut q: Vec<Draft> = self.store.kv_get(REPORT_QUEUE).ok().flatten().unwrap_or_default();
        q.push(d);
        let _ = self.store.kv_set(REPORT_QUEUE, &q);
    }

    /// Send waiting reports; any that fail wait for the next round.
    async fn flush_reports(&self) {
        let q: Vec<Draft> = self.store.kv_get(REPORT_QUEUE).ok().flatten().unwrap_or_default();
        if q.is_empty() {
            return;
        }
        let mut keep = Vec::new();
        for d in q {
            if let Err(e) = self.deliver(&d).await {
                log::error("report.failed", &e);
                keep.push(d);
            }
        }
        let _ = self.store.kv_set(REPORT_QUEUE, &keep);
    }

    pub fn send_wish(&self, text: &str) -> Result<()> {
        let sender = self.report_sender(None).ok_or_else(|| VemError::new(Code::ReportCmd01).with_detail("no account"))?;
        let d = report::wish(&self.store, &self.facts, &sender.id, text)?
            .ok_or_else(|| VemError::new(Code::ReportCmd01).with_detail("no address in this build"))?;
        // A wish is the user's own message: it goes through Postvak UIT like any other.
        self.send(d).map(|_| ())
    }

    /// Answer a log request from `log-requests.json`, if one is waiting for this install.
    pub fn answer_log_request(&self, list: &report::LogRequests) -> Result<()> {
        let Some(request) = report::pending_log_request(&self.store, list)? else { return Ok(()) };
        let Some(sender) = self.report_sender(None) else { return Ok(()) };
        let from = Addr::new(Some(sender.display_name.as_str()), &sender.email);
        if let Some(d) = report::log_answer(&self.store, &self.facts, &from, &sender.id, request)? {
            self.queue_report(d);
            report::mark_log_request_answered(&self.store, request)?;
            self.outbox_wake.notify_one();
        }
        Ok(())
    }

    // ---- housekeeping ------------------------------------------------------------------

    async fn housekeeping_loop(self: Arc<Self>) {
        loop {
            match crate::actions::purge_due(&self.store) {
                Ok(n) if n > 0 => {
                    for a in self.store.accounts().unwrap_or_default() {
                        self.sink.mail_changed(&a.id, None);
                    }
                }
                Err(e) => log::error("purge.failed", &e),
                _ => {}
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }

    /// Whether an update may install now: nothing being sent (docs/PLAN.md section 6). The
    /// shell adds what only it knows (a Write screen open, an import running).
    pub fn idle_for_update(&self) -> bool {
        !self.store.is_sending().unwrap_or(true)
            && self.store.outbox().map(|o| o.is_empty()).unwrap_or(false)
    }
}
