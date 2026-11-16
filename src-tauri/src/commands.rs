//! The commands the screens call (src/api/commands.ts). Names are snake_case, arguments
//! camelCase. Errors are `VemError`s, so the screen shows the plain sentence and the exact reply.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use vem_core::engine::Engine;
use vem_core::error::{Code, VemError};
use vem_core::model::*;
use vem_core::store::folders::parse_folder_key;
use vem_core::store::messages::parse_message_key;
use vem_core::{actions, contacts_sync, import, lookup, parse};

use crate::AppState;
use crate::shell::{self, ShellTexts};

type R<T> = std::result::Result<T, VemError>;

/// The id the screens use for "Alle postvakken" (src/api/commands.ts).
const ALL_INBOXES_ID: &str = "all-inboxes";
/// Never opened straight from a mail: these run programs.
const BLOCKED_EXTENSIONS: &[&str] = &[
    "exe", "com", "bat", "cmd", "scr", "pif", "msi", "msp", "js", "jse", "vbs", "vbe", "wsf", "wsh", "ps1", "hta",
    "cpl", "lnk", "reg", "jar", "msc", "appx", "msix", "iso", "img", "vhd", "vhdx",
];

fn notify_touched(app: &AppHandle, state: &AppState, touched: &actions::Touched) {
    for a in touched {
        let _ = app.emit("mail:changed", serde_json::json!({ "accountId": a, "folderId": null }));
        state.engine.wake(Some(a));
    }
    crate::shell::update_badge(app);
}

fn app_error(detail: impl Into<String>) -> VemError {
    VemError::new(Code::AppCmd01).with_detail(detail.into())
}

// ---- app and accounts --------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    updated_to: Option<String>,
    whats_new: Option<Vec<String>>,
}

#[tauri::command]
fn app_info(state: State<'_, AppState>) -> R<AppInfo> {
    let u = crate::updates::recent_update(&state.engine.store);
    Ok(AppInfo {
        version: vem_core::version().to_string(),
        updated_to: u.as_ref().map(|u| u.version.clone()),
        whats_new: u.map(|u| u.notes),
    })
}

#[tauri::command]
fn set_shell_texts(app: AppHandle, texts: ShellTexts) -> R<()> {
    shell::apply_texts(&app, texts).map_err(|e| app_error(e.to_string()))
}

#[tauri::command]
fn list_accounts(state: State<'_, AppState>) -> R<Vec<Account>> {
    state.engine.store.accounts()
}

#[tauri::command]
async fn lookup_settings(email: String) -> R<ServerSettings> {
    let mut s = lookup::lookup(&email).await?;
    s.needs_confirmation = lookup::needs_confirmation(&email, &s);
    Ok(s)
}

#[tauri::command]
async fn add_account(
    state: State<'_, AppState>,
    display_name: String,
    email: String,
    password: String,
    settings: ServerSettings,
) -> R<Account> {
    let secret = vem_core::secrets::Secret::Password { password };
    state.engine.add_account(&display_name, &email, secret, settings).await
}

/// "Inloggen met Google / Microsoft": the browser opens, the user signs in there.
#[tauri::command]
async fn add_account_oauth(app: AppHandle, state: State<'_, AppState>, display_name: String, email: String, settings: ServerSettings, done_page: String) -> R<Account> {
    let secret = oauth_secret(&app, &state, settings.sign_in, &email, &done_page).await?;
    state.engine.add_account(&display_name, &email, secret, settings).await
}

/// Opens the browser for Google or Microsoft and waits for the sign-in, at most five minutes,
/// or until "Stoppen" (`cancel_sign_in`).
async fn oauth_secret(app: &AppHandle, state: &AppState, sign_in: SignIn, email: &str, done_page: &str) -> R<vem_core::secrets::Secret> {
    let pending = vem_core::oauth::begin(sign_in, email).await?;
    let stop = Arc::new(tokio::sync::Notify::new());
    *state.sign_in_cancel.lock().unwrap_or_else(|e| e.into_inner()) = Some(stop.clone());
    app.opener().open_url(&pending.url, None::<&str>).map_err(|e| app_error(e.to_string()))?;
    let result = tokio::select! {
        r = pending.finish(std::time::Duration::from_secs(300), done_page) => r,
        _ = stop.notified() => Err(VemError::new(Code::AuthLogin01).with_detail("stopped")),
    };
    *state.sign_in_cancel.lock().unwrap_or_else(|e| e.into_inner()) = None;
    result
}

/// "Stoppen" while the browser sign-in is open.
#[tauri::command]
fn cancel_sign_in(state: State<'_, AppState>) {
    if let Some(stop) = state.sign_in_cancel.lock().unwrap_or_else(|e| e.into_inner()).take() {
        stop.notify_one();
    }
}

/// A new password for an existing mailbox (after it was changed at the provider).
#[tauri::command]
async fn update_password(state: State<'_, AppState>, account_id: String, password: String) -> R<()> {
    state.engine.update_secret(&account_id, vem_core::secrets::Secret::Password { password }).await
}

/// "Opnieuw inloggen" for a Google or Microsoft mailbox.
#[tauri::command]
async fn reauth_account(app: AppHandle, state: State<'_, AppState>, account_id: String, done_page: String) -> R<()> {
    let account = state.engine.store.account(&account_id)?;
    let sign_in = account.settings.as_ref().map(|s| s.sign_in).ok_or_else(|| VemError::new(Code::StoreSecret01).with_detail("settings"))?;
    let secret = oauth_secret(&app, &state, sign_in, &account.email, &done_page).await?;
    state.engine.update_secret(&account_id, secret).await
}

#[tauri::command]
fn oauth_available(sign_in: SignIn) -> bool {
    vem_core::oauth::available(sign_in)
}

#[tauri::command]
fn remove_account(state: State<'_, AppState>, account_id: String) -> R<()> {
    state.engine.remove_account(&account_id)
}

#[tauri::command]
fn set_signature(state: State<'_, AppState>, account_id: String, signature: String) -> R<()> {
    let s = signature.trim_end();
    state.engine.store.set_signature(&account_id, (!s.is_empty()).then_some(s))
}

// ---- folders and messages ----------------------------------------------------------

#[tauri::command]
fn list_folders(state: State<'_, AppState>, account_id: Option<String>) -> R<Vec<Folder>> {
    state.engine.store.folders(account_id.as_deref())
}

#[tauri::command]
fn list_messages(state: State<'_, AppState>, folder_id: String, offset: u32, limit: u32) -> R<Vec<MessageSummary>> {
    let limit = limit.clamp(1, 500);
    if folder_id == ALL_INBOXES_ID {
        return state.engine.store.messages_in_all_inboxes(offset, limit);
    }
    state.engine.store.messages_in_folder(parse_folder_key(&folder_id)?, offset, limit)
}

#[tauri::command]
async fn get_message(state: State<'_, AppState>, message_id: String, show_remote_images: bool) -> R<MessageDetail> {
    let id = parse_message_key(&message_id)?;
    if let Err(e) = state.engine.ensure_body(id).await {
        state.engine.report(None, &e).await;
        return Err(e);
    }
    actions::message_detail(&state.engine.store, &message_id, show_remote_images)
}

#[tauri::command]
fn set_read(app: AppHandle, state: State<'_, AppState>, message_ids: Vec<String>, read: bool) -> R<()> {
    let t = actions::set_read(&state.engine.store, &message_ids, read)?;
    notify_touched(&app, &state, &t);
    Ok(())
}

#[tauri::command]
fn set_important(app: AppHandle, state: State<'_, AppState>, message_ids: Vec<String>, important: bool) -> R<()> {
    let t = actions::set_important(&state.engine.store, &message_ids, important)?;
    notify_touched(&app, &state, &t);
    Ok(())
}

#[tauri::command]
fn delete_messages(app: AppHandle, state: State<'_, AppState>, message_ids: Vec<String>) -> R<UndoToken> {
    let (tok, t) = actions::delete_messages(&state.engine.store, &message_ids)?;
    notify_touched(&app, &state, &t);
    Ok(tok)
}

#[tauri::command]
fn move_messages(app: AppHandle, state: State<'_, AppState>, message_ids: Vec<String>, folder_id: String) -> R<UndoToken> {
    let (tok, t) = actions::move_messages(&state.engine.store, &message_ids, &folder_id)?;
    notify_touched(&app, &state, &t);
    Ok(tok)
}

#[tauri::command]
fn undo(app: AppHandle, state: State<'_, AppState>, token: String) -> R<()> {
    let t = actions::undo(&state.engine.store, &token)?;
    notify_touched(&app, &state, &t);
    Ok(())
}

#[tauri::command]
fn empty_trash(app: AppHandle, state: State<'_, AppState>, account_id: String) -> R<()> {
    actions::empty_trash(&state.engine.store, &account_id)?;
    notify_touched(&app, &state, &[account_id].into_iter().collect());
    Ok(())
}

#[tauri::command]
fn search(state: State<'_, AppState>, query: String, scope: SearchScope, limit: u32) -> R<Vec<SearchHit>> {
    let scope = match scope {
        SearchScope::Folder { folder_id } if folder_id == ALL_INBOXES_ID => SearchScope::All,
        other => other,
    };
    state.engine.store.search(&query, &scope, limit.clamp(1, 500))
}

// ---- tags and contacts -------------------------------------------------------------

#[tauri::command]
fn list_tags(state: State<'_, AppState>) -> R<Vec<Tag>> {
    state.engine.store.tags()
}

#[tauri::command]
fn create_tag(state: State<'_, AppState>, name: String, color: String) -> R<Tag> {
    state.engine.store.create_tag(&name, &color)
}

#[tauri::command]
fn delete_tag(app: AppHandle, state: State<'_, AppState>, tag_id: String) -> R<()> {
    state.engine.store.delete_tag(&tag_id)?;
    for a in state.engine.store.accounts()? {
        let _ = app.emit("mail:changed", serde_json::json!({ "accountId": a.id, "folderId": null }));
    }
    Ok(())
}

#[tauri::command]
fn set_message_tags(app: AppHandle, state: State<'_, AppState>, message_ids: Vec<String>, tag_id: String, on: bool) -> R<()> {
    let t = actions::set_message_tag(&state.engine.store, &message_ids, &tag_id, on)?;
    notify_touched(&app, &state, &t);
    Ok(())
}

#[tauri::command]
fn list_tag_messages(state: State<'_, AppState>, tag_id: String) -> R<Vec<MessageSummary>> {
    state.engine.store.messages_with_tag(&tag_id, 500)
}

#[tauri::command]
fn list_contacts(state: State<'_, AppState>, query: String) -> R<Vec<Contact>> {
    state.engine.store.contacts(&query)
}

#[tauri::command]
fn save_contact(state: State<'_, AppState>, contact: Contact) -> R<Contact> {
    let c = state.engine.store.save_contact(&contact)?;
    state.engine.wake(contacts_sync::paired_account(&state.engine.store)?.as_deref());
    Ok(c)
}

#[tauri::command]
fn delete_contact(state: State<'_, AppState>, contact_id: String) -> R<()> {
    state.engine.store.delete_contact(&contact_id)?;
    state.engine.wake(contacts_sync::paired_account(&state.engine.store)?.as_deref());
    Ok(())
}

#[tauri::command]
fn contact_messages(state: State<'_, AppState>, contact_id: String) -> R<Vec<MessageSummary>> {
    actions::contact_messages(&state.engine.store, &contact_id)
}

// ---- writing and sending -----------------------------------------------------------

fn drafts_folder(state: &AppState, account_id: &str) -> R<i64> {
    let store = &state.engine.store;
    if let Some(f) = store.folder_by_role(account_id, FolderRole::Drafts)? {
        return Ok(f);
    }
    // No Concepten on the server (or a local account): keep drafts on this computer.
    let local = store.local_account("Op deze computer")?;
    store.ensure_local_folder(&local.id, &["Concepten".to_string()], Some(FolderRole::Drafts))
}

#[tauri::command]
fn save_draft(app: AppHandle, state: State<'_, AppState>, draft: Draft) -> R<String> {
    state.last_draft_save.store(now(), Ordering::Relaxed);
    let account = state.engine.store.account(&draft.account_id)?;
    let folder = drafts_folder(&state, &account.id)?;
    // A half-typed address must not stop the save: build with the valid ones, keep all in the draft.
    let keep = |l: &[Addr]| l.iter().filter(|a| vem_core::compose::valid_email(&a.email)).cloned().collect::<Vec<_>>();
    let buildable = Draft { to: keep(&draft.to), cc: keep(&draft.cc), bcc: keep(&draft.bcc), attachments: vec![], ..draft.clone() };
    let from = Addr::new(Some(account.display_name.as_str()), &account.email);
    let built = vem_core::compose::build(&buildable, &from, now())?;
    let id = state.engine.store.save_draft(&draft, folder, &built.raw)?;
    let _ = app.emit("mail:changed", serde_json::json!({ "accountId": account.id, "folderId": folder.to_string() }));
    Ok(id.to_string())
}

#[tauri::command]
fn delete_draft(state: State<'_, AppState>, draft_id: String) -> R<()> {
    let id = parse_message_key(&draft_id)?;
    if state.engine.store.message(id)?.is_draft {
        state.engine.store.delete_message(id)?;
    }
    Ok(())
}

#[tauri::command]
fn load_draft(state: State<'_, AppState>, message_id: String) -> R<Draft> {
    let m = state.engine.store.message(parse_message_key(&message_id)?)?;
    m.draft.ok_or_else(|| VemError::new(Code::StoreRead01).with_detail("draft"))
}

#[tauri::command]
fn send_message(state: State<'_, AppState>, draft: Draft) -> R<OutboxItem> {
    state.engine.send(draft)
}

#[tauri::command]
fn cancel_send(state: State<'_, AppState>, outbox_id: String) -> R<Draft> {
    state.engine.cancel_send(&outbox_id)
}

#[tauri::command]
fn list_outbox(state: State<'_, AppState>) -> R<Vec<OutboxItem>> {
    state.engine.store.outbox()
}

#[tauri::command]
async fn pick_attachments(app: AppHandle) -> R<Vec<String>> {
    let picked = tauri::async_runtime::spawn_blocking(move || app.dialog().file().blocking_pick_files())
        .await
        .map_err(|e| app_error(e.to_string()))?;
    Ok(picked
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .collect())
}

fn attachment(state: &AppState, message_id: &str, attachment_id: &str) -> R<(String, Vec<u8>)> {
    let id = parse_message_key(message_id)?;
    let raw = state.engine.store.read_eml(id)?.ok_or_else(|| VemError::new(Code::StoreRead01).with_detail("body"))?;
    let index: usize = attachment_id.parse().map_err(|_| VemError::new(Code::StoreRead01).with_detail("attachment"))?;
    let name = parse::parse(&raw)
        .attachments
        .into_iter()
        .find(|a| a.index == index)
        .map(|a| parse::safe_file_name(&a.name))
        .ok_or_else(|| VemError::new(Code::StoreRead01).with_detail("attachment"))?;
    let bytes = parse::attachment_bytes(&raw, index).ok_or_else(|| VemError::new(Code::StoreRead01).with_detail("attachment"))?;
    Ok((name, bytes))
}

#[tauri::command]
fn open_attachment(app: AppHandle, state: State<'_, AppState>, message_id: String, attachment_id: String) -> R<()> {
    let (name, bytes) = attachment(&state, &message_id, &attachment_id)?;
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    if BLOCKED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(app_error(format!("blocked .{ext}")));
    }
    let dir = std::env::temp_dir().join("vorige-eeuw-mail").join(uuid::Uuid::new_v4().simple().to_string());
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(&name);
    std::fs::write(&path, bytes)?;
    app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|e| app_error(e.to_string()))
}

#[tauri::command]
async fn save_attachment(app: AppHandle, state: State<'_, AppState>, message_id: String, attachment_id: String) -> R<()> {
    let (name, bytes) = attachment(&state, &message_id, &attachment_id)?;
    let target = tauri::async_runtime::spawn_blocking(move || app.dialog().file().set_file_name(name).blocking_save_file())
        .await
        .map_err(|e| app_error(e.to_string()))?;
    if let Some(path) = target.and_then(|p| p.into_path().ok()) {
        std::fs::write(path, bytes)?;
    }
    Ok(())
}

// ---- sync and settings -------------------------------------------------------------

#[tauri::command]
fn sync_now(state: State<'_, AppState>, account_id: Option<String>) -> R<()> {
    state.engine.wake(account_id.as_deref());
    Ok(())
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> R<Settings> {
    state.engine.store.settings()
}

#[tauri::command]
fn save_settings(app: AppHandle, state: State<'_, AppState>, settings: Settings) -> R<()> {
    let s = state.engine.store.save_settings(&settings)?;
    let auto = app.autolaunch();
    let on = auto.is_enabled().unwrap_or(false);
    if s.start_with_windows != on {
        let r = if s.start_with_windows { auto.enable() } else { auto.disable() };
        if let Err(e) = r {
            vem_core::log::event("autostart.failed", &[("detail", &e.to_string())]);
        }
    }
    Ok(())
}

// ---- import and export -------------------------------------------------------------

#[tauri::command]
fn find_import_sources() -> R<Vec<ImportSource>> {
    Ok(import::find_sources())
}

#[tauri::command]
async fn pick_import_file(app: AppHandle, kind: ImportKind) -> R<Option<ImportSource>> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let d = app.dialog().file();
        match kind {
            ImportKind::Eml | ImportKind::Wlm => d.blocking_pick_folder(),
            ImportKind::Pst => d.add_filter("Outlook", &["pst"]).blocking_pick_file(),
            ImportKind::Dbx => d.add_filter("Outlook Express", &["dbx"]).blocking_pick_file(),
            ImportKind::Mbox => d.blocking_pick_file(),
        }
    })
    .await
    .map_err(|e| app_error(e.to_string()))?;
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else { return Ok(None) };
    let mut source = import::source_for(&path)?;
    if path.is_dir() {
        source.kind = kind;
    }
    Ok(Some(source))
}

#[tauri::command]
fn start_import(app: AppHandle, state: State<'_, AppState>, source: ImportSource, target: ImportTarget) -> R<String> {
    spawn_import(app, &state, target.clone(), move |engine, job, emit, cancel| {
        import::run(&engine.store, job, &source, &target, emit, cancel)
    })
}

/// "Uit een ander postvak": download the old mailbox's mail (read-only), then copy it.
#[tauri::command]
fn import_from_account(app: AppHandle, state: State<'_, AppState>, source_account_id: String, target: ImportTarget) -> R<String> {
    spawn_import(app, &state, target.clone(), move |engine, job, emit, cancel| {
        emit(ImportProgress { job_id: job.to_string(), ..Default::default() });
        tauri::async_runtime::block_on(engine.download_bodies(&source_account_id, cancel))?;
        import::account::run(&engine.store, job, &source_account_id, &target, emit, cancel)
    })
}

/// Run an import in the background with progress events, a stop switch and a final event.
fn spawn_import(
    app: AppHandle,
    state: &AppState,
    target: ImportTarget,
    work: impl FnOnce(&Engine, &str, &dyn Fn(ImportProgress), &AtomicBool) -> vem_core::Result<import::ImportReport> + Send + 'static,
) -> R<String> {
    let job = uuid::Uuid::new_v4().simple().to_string();
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut running = state.imports.lock().unwrap_or_else(|e| e.into_inner());
        // One import at a time: a second press of "Ophalen" must not start a second copy.
        if !running.is_empty() {
            return Err(app_error("an import is running"));
        }
        running.insert(job.clone(), cancel.clone());
    }
    let engine = state.engine.clone();
    let job2 = job.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let emit = |p: ImportProgress| {
            if let Some(state) = tauri::Manager::try_state::<AppState>(&app) {
                *state.last_import.lock().unwrap_or_else(|e| e.into_inner()) = Some(p.clone());
            }
            let _ = app.emit("import:progress", p);
        };
        let result = work(&engine, &job2, &emit, &cancel);
        let (report, error) = match result {
            Ok(r) => (r, None),
            Err(e) => {
                vem_core::log::error("import.failed", &e);
                (import::ImportReport::default(), Some(e))
            }
        };
        let skipped = report.skipped.iter().map(|(folder, reason)| ImportSkip { folder: folder.clone(), reason: reason.clone() }).collect();
        emit(ImportProgress {
            job_id: job2.clone(),
            done: report.added,
            total: Some(report.added),
            finished: true,
            error: error.clone(),
            already_there: report.already_there,
            contacts_added: report.contacts,
            all_marked_read: report.all_marked_read,
            skipped,
            ..Default::default()
        });
        if let ImportTarget::Mailbox { account_id } = &target {
            engine.wake(Some(account_id));
        }
        let accounts = engine.store.accounts().unwrap_or_default();
        for a in accounts {
            let _ = app.emit("mail:changed", serde_json::json!({ "accountId": a.id, "folderId": null }));
        }
        if let Some(e) = error {
            tauri::async_runtime::block_on(engine.report(None, &e));
        }
        if let Some(state) = tauri::Manager::try_state::<AppState>(&app) {
            state.imports.lock().unwrap_or_else(|e| e.into_inner()).remove(&job2);
        }
    });
    Ok(job)
}

/// The running import's latest progress, or the last finished one, for when the Import screen
/// opens again.
#[tauri::command]
fn import_status(state: State<'_, AppState>) -> Option<ImportProgress> {
    state.last_import.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

#[tauri::command]
fn list_originals(state: State<'_, AppState>) -> R<Vec<OriginalCopy>> {
    state.engine.store.originals()
}

#[tauri::command]
fn delete_original(state: State<'_, AppState>, id: String) -> R<()> {
    state.engine.store.delete_original(&id)
}

#[tauri::command]
async fn export_folder(app: AppHandle, state: State<'_, AppState>, folder_id: String) -> R<Option<String>> {
    let fid = parse_folder_key(&folder_id)?;
    let info = state.engine.store.folder(fid)?;
    let shown = vem_core::imap::utf7::decode(&info.name).replace(['/', '\\', '.'], " ");
    let name = format!("{}.mbox", parse::safe_file_name(shown.trim()));
    let target = tauri::async_runtime::spawn_blocking(move || {
        app.dialog().file().set_file_name(name).add_filter("mbox", &["mbox"]).blocking_save_file()
    })
    .await
    .map_err(|e| app_error(e.to_string()))?;
    let Some(path) = target.and_then(|p| p.into_path().ok()) else { return Ok(None) };
    let store = state.engine.store.clone();
    let p2: PathBuf = path.clone();
    tauri::async_runtime::spawn_blocking(move || import::export::export_folder(&store, fid, &p2))
        .await
        .map_err(|e| app_error(e.to_string()))??;
    Ok(Some(path.to_string_lossy().into_owned()))
}

// ---- Martijn, the phone, quitting --------------------------------------------------

#[tauri::command]
fn send_wish(state: State<'_, AppState>, text: String) -> R<()> {
    if text.trim().is_empty() {
        return Ok(());
    }
    state.engine.send_wish(&text)
}

#[tauri::command]
fn help_report(state: State<'_, AppState>) -> R<String> {
    vem_core::report::help_report(&state.engine.store, &state.engine.facts)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairInfo {
    qr_svg: String,
}

#[tauri::command]
fn pair_phone(state: State<'_, AppState>) -> R<PairInfo> {
    let store = &state.engine.store;
    let account = match contacts_sync::paired_account(store)? {
        Some(a) => a,
        None => store
            .accounts()?
            .into_iter()
            .find(|a| a.kind == AccountKind::Imap)
            .map(|a| a.id)
            .ok_or_else(|| app_error("no mailbox"))?,
    };
    let qr_svg = contacts_sync::pair(store, &state.engine.secrets, &account)?;
    state.engine.wake(Some(&account));
    Ok(PairInfo { qr_svg })
}

/// Whether a phone is linked, so "Koppeling verbreken" only shows when there is one.
#[tauri::command]
fn phone_paired(state: State<'_, AppState>) -> R<bool> {
    Ok(contacts_sync::paired_account(&state.engine.store)?.is_some())
}

#[tauri::command]
fn unpair_phone(state: State<'_, AppState>) -> R<()> {
    contacts_sync::unpair(&state.engine.store, &state.engine.secrets)
}

#[tauri::command]
fn quit_app(app: AppHandle) -> R<()> {
    shell::quit(&app);
    Ok(())
}

pub fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        app_info,
        set_shell_texts,
        list_accounts,
        lookup_settings,
        add_account,
        add_account_oauth,
        oauth_available,
        remove_account,
        set_signature,
        list_folders,
        list_messages,
        get_message,
        set_read,
        set_important,
        delete_messages,
        move_messages,
        undo,
        empty_trash,
        search,
        list_tags,
        create_tag,
        delete_tag,
        set_message_tags,
        list_tag_messages,
        list_contacts,
        save_contact,
        delete_contact,
        contact_messages,
        save_draft,
        delete_draft,
        load_draft,
        send_message,
        cancel_send,
        list_outbox,
        pick_attachments,
        open_attachment,
        save_attachment,
        sync_now,
        get_settings,
        save_settings,
        find_import_sources,
        pick_import_file,
        start_import,
        phone_paired,
        import_status,
        cancel_sign_in,
        update_password,
        reauth_account,
        import_from_account,
        list_originals,
        delete_original,
        export_folder,
        send_wish,
        help_report,
        pair_phone,
        unpair_phone,
        quit_app,
    ]
}
