//! Mail kept on the PC, also on the paired phone (decision 26, docs/pc-to-phone.md). The PC looks
//! for the phone every so often and brings it up to date; the phone listens while the app runs.

use tauri::{AppHandle, Manager};

use crate::AppState;

/// The PC: how often it looks for the phone, and the first look after the start.
#[cfg(desktop)]
const LOOK_EVERY: std::time::Duration = std::time::Duration::from_secs(90);
#[cfg(desktop)]
const FIRST_LOOK: std::time::Duration = std::time::Duration::from_secs(20);

/// Development only: the emulator sits behind its own network, so the find packet and the
/// connection go to 127.0.0.1 through `adb forward` and the emulator's UDP redirect.
#[cfg(desktop)]
fn debug_targets() -> (Option<Vec<std::net::SocketAddr>>, Option<std::net::IpAddr>) {
    if !cfg!(debug_assertions) {
        return (None, None);
    }
    let targets = std::env::var("VEM_LAN_FIND_TO")
        .ok()
        .map(|v| v.split(',').filter_map(|s| s.trim().parse().ok()).collect::<Vec<_>>());
    let peer = std::env::var("VEM_LAN_PEER").ok().and_then(|v| v.trim().parse().ok());
    (targets, peer)
}

#[cfg(desktop)]
pub fn spawn(app: AppHandle) {
    use tauri::Emitter;
    use vem_core::lan;
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_LOOK).await;
        let (fixed, peer) = debug_targets();
        loop {
            let engine = app.state::<AppState>().engine.clone();
            let targets = fixed.clone().unwrap_or_else(|| lan::broadcast_targets(lan::FIND_PORT));
            match lan::offer(&engine.store, &engine.secrets, &targets, peer).await {
                Ok(Some(copied)) => {
                    let _ = app.emit("lan:sent", copied);
                }
                Ok(None) => {}
                // Tried again at the next look; the phone may simply be away.
                Err(e) => vem_core::log::error("lan.offer_failed", &e),
            }
            tokio::time::sleep(LOOK_EVERY).await;
        }
    });
}

#[cfg(mobile)]
pub fn spawn(app: AppHandle) {
    use std::sync::Arc;
    use tauri::Emitter;
    use vem_core::engine::Sink;
    use vem_core::lan::{self, Listen, PhoneEvent};
    let engine = app.state::<AppState>().engine.clone();
    let events = app.clone();
    let on: Arc<dyn Fn(PhoneEvent) + Send + Sync> = Arc::new(move |e: PhoneEvent| {
        if let PhoneEvent::Done { received, removed } = &e
            && (*received > 0 || *removed > 0)
        {
            // New folders and messages: the screens reload their lists.
            let sink = crate::shell::EventSink(events.clone());
            for a in events.state::<AppState>().engine.store.accounts().unwrap_or_default() {
                sink.mail_changed(&a.id, None);
            }
        }
        let _ = events.emit("lan:phone", e);
    });
    tauri::async_runtime::spawn(async move {
        if let Err(e) = lan::serve(engine.store.clone(), engine.secrets.clone(), Listen::default(), on).await {
            vem_core::log::error("lan.serve_failed", &e);
        }
    });
}
