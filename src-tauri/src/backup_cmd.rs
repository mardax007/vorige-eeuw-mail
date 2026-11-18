//! "Reservekopie maken" and "Reservekopie terugzetten" (C14, decision 29; vem_core::backup).
//! Windows only on screen. One job at a time, registered with the imports so an update waits.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter, Manager, State};
use vem_core::backup::{self, BackupInfo, BackupProgress};
use vem_core::error::{Code, VemError};

use crate::AppState;

type R<T> = std::result::Result<T, VemError>;

/// The key of a backup or restore in `AppState::imports`.
const JOB: &str = "backup";
/// The last event of a restore stays on screen this long before the app restarts.
const RESTART_DELAY: std::time::Duration = std::time::Duration::from_secs(3);

fn busy() -> VemError {
    VemError::new(Code::AppCmd01).with_detail("a backup, restore or import is running")
}

/// Register the job, or refuse when an import or another backup runs.
fn claim(state: &AppState) -> R<Arc<AtomicBool>> {
    let mut running = state.imports.lock().unwrap_or_else(|e| e.into_inner());
    if !running.is_empty() {
        return Err(busy());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    running.insert(JOB.to_string(), cancel.clone());
    Ok(cancel)
}

fn release(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state.imports.lock().unwrap_or_else(|e| e.into_inner()).remove(JOB);
    }
}

#[cfg(desktop)]
async fn pick_folder(app: &AppHandle) -> R<Option<std::path::PathBuf>> {
    use tauri_plugin_dialog::DialogExt;
    let app = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || app.dialog().file().blocking_pick_folder())
        .await
        .map_err(|e| VemError::new(Code::AppCmd01).with_detail(e.to_string()))?;
    Ok(picked.and_then(|p| p.into_path().ok()))
}

/// The phone keeps no backup (decision 29); the screen offers none there.
#[cfg(mobile)]
async fn pick_folder(_app: &AppHandle) -> R<Option<std::path::PathBuf>> {
    Ok(None)
}

/// Report a failure to Martijn like any other error, then return it marked as reported.
fn reported(app: &AppHandle, e: VemError, what: &str) -> VemError {
    vem_core::log::error(what, &e);
    let Some(state) = app.try_state::<AppState>() else { return e };
    let sent = tauri::async_runtime::block_on(state.engine.report(None, &e));
    e.with_reported(sent)
}

/// Ask for a folder (the USB stick) and write the backup there in the background, with
/// `backup:progress` events. False when no folder was chosen.
#[tauri::command]
pub async fn start_backup(app: AppHandle, state: State<'_, AppState>) -> R<bool> {
    let Some(parent) = pick_folder(&app).await? else { return Ok(false) };
    let cancel = claim(&state)?;
    let store = state.engine.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let emit = |p: BackupProgress| {
            let _ = app.emit("backup:progress", p);
        };
        let day = chrono::Local::now().format("%Y-%m-%d").to_string();
        let last = match backup::write(&store, &parent, &day, &emit, &cancel) {
            Ok(Some(folder)) => BackupProgress { finished: true, folder: Some(folder.to_string_lossy().into_owned()), ..Default::default() },
            Ok(None) => BackupProgress { finished: true, stopped: true, ..Default::default() },
            Err(e) => BackupProgress { finished: true, error: Some(reported(&app, e, "backup.failed")), ..Default::default() },
        };
        release(&app);
        emit(last);
    });
    Ok(true)
}

/// "Stoppen" during a backup or a restore.
#[tauri::command]
pub fn stop_backup(state: State<'_, AppState>) {
    if let Some(cancel) = state.imports.lock().unwrap_or_else(|e| e.into_inner()).get(JOB) {
        cancel.store(true, Ordering::Relaxed);
    }
}

/// Ask for a backup folder and check it. None when no folder was chosen.
#[tauri::command]
pub async fn pick_backup(app: AppHandle) -> R<Option<BackupInfo>> {
    let Some(dir) = pick_folder(&app).await? else { return Ok(None) };
    tauri::async_runtime::spawn_blocking(move || backup::inspect(&dir))
        .await
        .map_err(|e| VemError::new(Code::AppCmd01).with_detail(e.to_string()))?
        .map(Some)
}

/// Copy the checked backup into the data folder, then restart; the swap happens at the start.
#[tauri::command]
pub fn start_restore(app: AppHandle, state: State<'_, AppState>, path: String) -> R<()> {
    let cancel = claim(&state)?;
    let data = state.engine.store.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let emit = |p: BackupProgress| {
            let _ = app.emit("backup:progress", p);
        };
        let result = backup::stage_restore(&data, std::path::Path::new(&path), &emit, &cancel);
        release(&app);
        match result {
            Ok(true) => {
                emit(BackupProgress { restoring: true, finished: true, ..Default::default() });
                std::thread::sleep(RESTART_DELAY);
                app.restart();
            }
            Ok(false) => emit(BackupProgress { restoring: true, finished: true, stopped: true, ..Default::default() }),
            Err(e) => {
                let error = Some(reported(&app, e, "restore.failed"));
                emit(BackupProgress { restoring: true, finished: true, error, ..Default::default() });
            }
        }
    });
    Ok(())
}
