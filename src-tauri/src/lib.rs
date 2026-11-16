//! The Tauri shell: one window, the icon next to the clock, notifications, starting with
//! Windows, silent updates, and the commands the screens call. Mail logic lives in vem-core.

mod badge;
mod commands;
mod shell;
mod updates;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64};
use std::sync::{Arc, Mutex};

use tauri::Manager;
use vem_core::engine::Engine;
use vem_core::net::Trust;
use vem_core::report::AppFacts;
use vem_core::secrets::Secrets;
use vem_core::store::Store;

/// Everything the commands share.
pub struct AppState {
    pub engine: Arc<Engine>,
    /// Running imports, by job id, with their stop switch.
    pub imports: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// When the Write screen last saved a draft: an update waits while someone is writing.
    pub last_draft_save: AtomicI64,
    pub texts: Mutex<shell::ShellTexts>,
    /// Stops a browser sign-in that is waiting ("Stoppen").
    pub sign_in_cancel: Mutex<Option<Arc<tokio::sync::Notify>>>,
    /// The latest import progress, so the Import screen finds a running import again.
    pub last_import: Mutex<Option<vem_core::model::ImportProgress>>,
}

/// Started by Windows at sign-in (autostart): stay next to the clock, no window.
pub const HIDDEN_ARG: &str = "--verborgen";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| shell::show_main(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::Builder::new().args([HIDDEN_ARG]).build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let data = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&data)?;
            vem_core::log::init(&data);
            vem_core::log::event("app.start", &[("version", vem_core::version())]);
            let store = Arc::new(Store::open(&data).map_err(|e| format!("{e:?}"))?);
            let secrets = Arc::new(Secrets::new(&data));
            let sink = Arc::new(shell::EventSink(app.handle().clone()));
            let facts = AppFacts { version: vem_core::version().to_string(), os: shell::windows_version() };
            let engine = Engine::new(store, secrets, sink, Trust::Platform, facts);
            app.manage(AppState {
                engine: engine.clone(),
                imports: Mutex::new(HashMap::new()),
                last_draft_save: AtomicI64::new(0),
                texts: Mutex::new(shell::ShellTexts::default()),
                sign_in_cancel: Mutex::new(None),
                last_import: Mutex::new(None),
            });
            tauri::async_runtime::block_on(async { engine.start() }).map_err(|e| format!("{e:?}"))?;
            let hidden = std::env::args().any(|a| a == HIDDEN_ARG);
            shell::create_main_window(app.handle(), !hidden)?;
            shell::create_tray(app.handle())?;
            shell::watch_notification_clicks(app.handle());
            shell::update_badge(app.handle());
            updates::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(commands::handler())
        .build(tauri::generate_context!())
        .expect("the app could not start")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                // Closing the window keeps the app next to the clock; only "Afsluiten" ends it.
                if code.is_none() && !shell::quitting(app) {
                    api.prevent_exit();
                }
            }
        });
}
