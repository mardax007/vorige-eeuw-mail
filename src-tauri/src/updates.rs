//! Silent updates from GitHub Releases and Martijn's log requests (docs/PLAN.md sections 5, 6).
//!
//! An update downloads in the background and installs only when the app is idle: nothing
//! being written, sent or imported. The release URL, the update key and the log-request URL
//! come from build-time settings; a local build without them never updates.

use std::sync::atomic::Ordering;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;
use vem_core::model::now;
use vem_core::store::Store;

use crate::AppState;

const FIRST_CHECK: Duration = Duration::from_secs(120);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
const IDLE_POLL: Duration = Duration::from_secs(60);
/// Someone who saved a draft this recently may still be writing.
const WRITING_SECS: i64 = 10 * 60;
/// "Bijgewerkt naar …" shows for one day.
const SHOW_UPDATE_SECS: i64 = 24 * 60 * 60;
const UPDATED_KEY: &str = "updated";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Updated {
    pub version: String,
    pub notes: Vec<String>,
    pub at: i64,
}

pub fn recent_update(store: &Store) -> Option<Updated> {
    let u: Updated = store.kv_get(UPDATED_KEY).ok().flatten()?;
    // Shown only once the new version runs, for a day.
    (u.version == vem_core::version() && now() - u.at < SHOW_UPDATE_SECS).then_some(u)
}

fn idle(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let writing = now() - state.last_draft_save.load(Ordering::Relaxed) < WRITING_SECS;
    let importing = !state.imports.lock().unwrap_or_else(|e| e.into_inner()).is_empty();
    !writing && !importing && state.engine.idle_for_update()
}

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            check_log_requests(&app).await;
            if let Err(e) = check_update(&app).await {
                vem_core::log::event("update.failed", &[("detail", &e)]);
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

async fn check_log_requests(app: &AppHandle) {
    let Some(url) = option_env!("VEM_LOG_REQUESTS_URL") else { return };
    let Ok(client) = vem_core::lookup::http() else { return };
    let Ok(resp) = client.get(url).send().await else { return };
    let Ok(list) = resp.json::<vem_core::report::LogRequests>().await else { return };
    let state = app.state::<AppState>();
    if let Err(e) = state.engine.answer_log_request(&list) {
        vem_core::log::error("logrequest.failed", &e);
    }
}

async fn check_update(app: &AppHandle) -> Result<(), String> {
    let (Some(url), Some(key)) = (option_env!("VEM_UPDATE_URL"), option_env!("VEM_UPDATER_PUBKEY")) else {
        return Ok(());
    };
    let endpoint = url.parse().map_err(|_| "update url".to_string())?;
    let updater = app
        .updater_builder()
        .pubkey(key)
        .endpoints(vec![endpoint])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else { return Ok(()) };
    vem_core::log::event("update.found", &[("version", &update.version)]);
    let bytes = update.download(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    while !idle(app) {
        tokio::time::sleep(IDLE_POLL).await;
    }
    let notes: Vec<String> = update
        .body
        .clone()
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().trim_start_matches(['-', '*']).trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let state = app.state::<AppState>();
    let _ = state.engine.store.kv_set(UPDATED_KEY, &Updated { version: update.version.clone(), notes, at: now() });
    vem_core::log::event("update.install", &[("version", &update.version)]);
    // On Windows the installer closes the app, installs quietly and starts it again.
    update.install(bytes).map_err(|e| e.to_string())?;
    Ok(())
}
