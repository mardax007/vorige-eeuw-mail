//! Window, tray icon, notifications and how links leave the app.

use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
#[cfg(desktop)]
use tauri::menu::{Menu, MenuItem};
#[cfg(desktop)]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::webview::NewWindowResponse;
#[cfg(windows)]
use tauri::image::Image;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
use vem_core::VemError;
use vem_core::engine::Sink;
use vem_core::model::{MessageSummary, OutboxItem};

use crate::AppState;

pub const MAIN: &str = "main";
#[cfg(desktop)]
const TRAY_ID: &str = "vem-tray";

static QUITTING: AtomicBool = AtomicBool::new(false);

/// The few words the shell shows itself (tray menu, notifications). The screens send them from
/// the Dutch catalogue at start (`set_shell_texts`), so no screen text lives in Rust.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellTexts {
    pub open: String,
    pub quit: String,
    pub new_mail: String,
    pub new_mail_many: String,
    pub still_running: String,
}

pub struct EventSink(pub AppHandle);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MailChanged<'a> {
    account_id: &'a str,
    folder_id: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncState<'a> {
    account_id: &'a str,
    busy: bool,
    error: Option<VemError>,
}

impl Sink for EventSink {
    fn mail_changed(&self, account_id: &str, folder_id: Option<String>) {
        let _ = self.0.emit("mail:changed", MailChanged { account_id, folder_id });
        update_badge(&self.0);
    }
    fn sync_state(&self, account_id: &str, busy: bool, error: Option<VemError>) {
        let _ = self.0.emit("sync:state", SyncState { account_id, busy, error });
    }
    fn outbox_changed(&self, items: Vec<OutboxItem>) {
        let _ = self.0.emit("outbox:changed", items);
    }
    fn new_mail(&self, _account_id: &str, messages: Vec<MessageSummary>) {
        notify_new_mail(&self.0, &messages);
    }
}

fn texts(app: &AppHandle) -> ShellTexts {
    app.state::<AppState>().texts.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn notify_new_mail(app: &AppHandle, messages: &[MessageSummary]) {
    let Ok(settings) = app.state::<AppState>().engine.store.settings() else { return };
    if !settings.notifications || messages.is_empty() {
        return;
    }
    let t = texts(app);
    // A single message opens itself when the notification is clicked; several open Postvak IN.
    let target = (messages.len() == 1).then(|| messages[0].id.clone());
    let (title, body) = if messages.len() == 1 {
        let m = &messages[0];
        let who = m.from.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| m.from.email.clone());
        (if t.new_mail.is_empty() { who.clone() } else { t.new_mail.clone() }, format!("{who}: {}", m.subject))
    } else {
        (t.new_mail_many.replace("{count}", &messages.len().to_string()), String::new())
    };
    let mut b = app.notification().builder().title(title).body(body);
    if let Some(id) = target {
        b = b.extra(OPEN_MESSAGE, id);
    }
    let _ = b.show();
}

/// Key in a notification's extra data: the message a click opens.
const OPEN_MESSAGE: &str = "openMessage";

/// A click on a notification opens the window, and the message when the notification was about
/// one message. Windows reports the click only while the notification is on screen.
pub fn watch_notification_clicks(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.notification().on_action(move |performed| {
        if performed.action_id() != "tap" {
            return;
        }
        show_main(&handle);
        let id = performed.notification().and_then(|n| n.extra().get(OPEN_MESSAGE)).and_then(|v| v.as_str());
        if let Some(id) = id {
            let _ = handle.emit("open:message", serde_json::json!({ "messageId": id }));
        }
    });
}

/// The unread count of every Postvak IN as a small number on the taskbar icon.
pub fn update_badge(app: &AppHandle) {
    let Some(w) = app.get_webview_window(MAIN) else { return };
    let count = app.state::<AppState>().engine.store.inbox_unread().unwrap_or(0);
    #[cfg(windows)]
    {
        let icon = crate::badge::pixels(count).map(|px| Image::new_owned(px, crate::badge::SIZE, crate::badge::SIZE));
        let _ = w.set_overlay_icon(icon);
    }
    #[cfg(all(desktop, not(windows)))]
    let _ = w.set_badge_count((count > 0).then_some(i64::from(count)));
    // Android draws its own dot on the app icon from the notifications.
    #[cfg(mobile)]
    let _ = (w, count);
}

pub fn quitting(_app: &AppHandle) -> bool {
    QUITTING.load(Ordering::SeqCst)
}

pub fn quit(app: &AppHandle) {
    QUITTING.store(true, Ordering::SeqCst);
    vem_core::log::event("app.quit", &[]);
    app.exit(0);
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN) {
        #[cfg(desktop)]
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn is_app_url(url: &tauri::Url) -> bool {
    match url.scheme() {
        "tauri" => true,
        "about" => true,
        "http" | "https" => matches!(url.host_str(), Some("tauri.localhost") | Some("localhost") | Some("127.0.0.1"))
            && (url.host_str() == Some("tauri.localhost") || url.port() == Some(1420)),
        _ => false,
    }
}

/// A link from a mail: the web in the system browser, `mailto:` in our own Write screen.
/// Anything else (file:, javascript:, other apps' schemes) is not followed.
fn open_outside(app: &AppHandle, url: &tauri::Url) {
    match url.scheme() {
        "http" | "https" => {
            vem_core::log::event("link.opened", &[("host", url.host_str().unwrap_or(""))]);
            let _ = app.opener().open_url(url.as_str(), None::<&str>);
        }
        "mailto" => {
            show_main(app);
            let to = url.path().to_string();
            let _ = app.emit("open:compose", serde_json::json!({ "to": to }));
        }
        _ => vem_core::log::event("link.refused", &[("scheme", url.scheme())]),
    }
}

pub fn create_main_window(app: &AppHandle, visible: bool) -> tauri::Result<()> {
    let nav_app = app.clone();
    let new_app = app.clone();
    let w = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::default())
        .title("Vorige Eeuw Mail")
        .inner_size(1280.0, 820.0)
        .min_inner_size(640.0, 520.0)
        .visible(visible)
        .on_navigation(move |url| {
            if is_app_url(url) {
                return true;
            }
            open_outside(&nav_app, url);
            false
        })
        .on_new_window(move |url, _features| {
            open_outside(&new_app, &url);
            NewWindowResponse::Deny
        })
        .build()?;
    let app2 = app.clone();
    w.on_window_event(move |e| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = e {
            if QUITTING.load(Ordering::SeqCst) {
                return;
            }
            api.prevent_close();
            if let Some(w) = app2.get_webview_window(MAIN) {
                let _ = w.hide();
            }
            tell_once_still_running(&app2);
        }
    });
    Ok(())
}

/// The first time the window closes, say once that mail keeps coming in next to the clock.
fn tell_once_still_running(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Ok(mut s) = state.engine.store.settings() else { return };
    if s.tray_notice_shown {
        return;
    }
    let t = texts(app);
    if !t.still_running.is_empty() {
        let _ = app.notification().builder().title("Vorige Eeuw Mail").body(t.still_running).show();
    }
    s.tray_notice_shown = true;
    let _ = state.engine.store.save_settings(&s);
}

#[cfg(desktop)]
pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = tray_menu(app, &ShellTexts::default())?;
    let mut b = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Vorige Eeuw Mail")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id().as_ref() {
            "open" => show_main(app),
            "quit" => quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        b = b.icon(icon.clone());
    }
    b.build(app)?;
    Ok(())
}

#[cfg(desktop)]
fn tray_menu(app: &AppHandle, t: &ShellTexts) -> tauri::Result<Menu<tauri::Wry>> {
    let open = MenuItem::with_id(app, "open", if t.open.is_empty() { "Vorige Eeuw Mail" } else { &t.open }, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", if t.quit.is_empty() { "X" } else { &t.quit }, true, None::<&str>)?;
    Menu::with_items(app, &[&open, &quit])
}

/// The screens sent their words: rebuild the tray menu with them.
pub fn apply_texts(app: &AppHandle, t: ShellTexts) -> tauri::Result<()> {
    #[cfg(desktop)]
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(tray_menu(app, &t)?))?;
    }
    *app.state::<AppState>().texts.lock().unwrap_or_else(|e| e.into_inner()) = t;
    Ok(())
}

/// "Windows 11 (10.0.26100)" style text for reports, from the `ver` command.
pub fn windows_version() -> String {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        if let Ok(out) = std::process::Command::new("cmd").args(["/C", "ver"]).creation_flags(CREATE_NO_WINDOW).output() {
            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !text.is_empty() {
                return text;
            }
        }
    }
    std::env::consts::OS.to_string()
}
