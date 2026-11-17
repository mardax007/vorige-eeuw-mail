//! The Tauri shell: one window, the icon next to the clock, notifications, starting with
//! Windows, silent updates, and the commands the screens call. Mail logic lives in vem-core.

#[cfg(target_os = "android")]
mod android_mail;
#[cfg(windows)]
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
    /// Android: a newer version found on GitHub, waiting for "Bijwerken".
    pub android_update: Mutex<Option<vem_core::apk_update::AndroidRelease>>,
}

/// Started by Windows at sign-in (autostart): stay next to the clock, no window.
pub const HIDDEN_ARG: &str = "--verborgen";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    // Windows only: one running copy, starting with Windows, silent signed updates.
    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| shell::show_main(app)))
        .plugin(tauri_plugin_autostart::Builder::new().args([HIDDEN_ARG]).build())
        .plugin(tauri_plugin_updater::Builder::new().build());
    // Phone only: the camera reads the pairing code the PC shows.
    #[cfg(mobile)]
    let builder = builder.plugin(tauri_plugin_barcode_scanner::init());
    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let data = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&data)?;
            vem_core::log::init(&data);
            vem_core::log::event("app.start", &[("version", vem_core::version())]);
            // Android checks certificates through its own Java code; set that up before any mail.
            #[cfg(target_os = "android")]
            vem_core::net::init_android().map_err(|e| format!("{e:?}"))?;
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
                android_update: Mutex::new(None),
            });
            // On a phone the background service may be working on the mailboxes; the app takes
            // over from it first (decision 24).
            #[cfg(target_os = "android")]
            {
                let handle = app.handle().clone();
                let reload = move || {
                    let state = handle.state::<AppState>();
                    let sink = shell::EventSink(handle.clone());
                    for a in state.engine.store.accounts().unwrap_or_default() {
                        vem_core::engine::Sink::mail_changed(&sink, &a.id, None);
                    }
                };
                android_mail::attach(engine.clone(), data.clone(), Box::new(reload));
            }
            #[cfg(not(target_os = "android"))]
            tauri::async_runtime::block_on(async { engine.start() }).map_err(|e| format!("{e:?}"))?;
            let hidden = std::env::args().any(|a| a == HIDDEN_ARG);
            shell::create_main_window(app.handle(), !hidden)?;
            #[cfg(desktop)]
            shell::create_tray(app.handle())?;
            shell::watch_notification_clicks(app.handle());
            shell::ask_notification_permission(app.handle());
            shell::update_badge(app.handle());
            updates::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(commands::handler())
        .build(tauri::generate_context!())
        .expect("the app could not start")
        .run(|app, event| {
            // Closing the window keeps the app next to the clock; only "Afsluiten" ends it. A
            // phone lets the app end with its screen: Android brings it back fresh, and new mail
            // in between is the background service's work (decision 24).
            #[cfg(desktop)]
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event
                && code.is_none()
                && !shell::quitting(app)
            {
                api.prevent_exit();
            }
            #[cfg(mobile)]
            let _ = (app, event);
        });
}
