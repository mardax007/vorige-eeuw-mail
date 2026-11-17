//! Who works on the mailboxes on Android (decision 24): the open app, or the background service.
//!
//! The two run as separate processes on one index, and only one may sync at a time, or messages
//! would be fetched and sent twice. Two lock files in the data folder settle it:
//!
//! - `ui.lock` is held by the app while it is on screen;
//! - `sync.lock` is held by whichever process runs the engine.
//!
//! The app takes both when it comes on screen. The service waits until the app is gone, takes
//! `sync.lock`, and gives it back as soon as it sees the app on screen again. A third lock,
//! `service.lock`, is held by the service for as long as its process lives, so the app lets go
//! only when a service is there to take over. A lock ends with its process, so an app that
//! Android kills never blocks the service.

use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::Path;

const UI_LOCK: &str = "ui.lock";
const SYNC_LOCK: &str = "sync.lock";
const SERVICE_LOCK: &str = "service.lock";

/// The right to work on the mailboxes. Dropping it gives the turn away.
#[derive(Debug)]
pub struct Turn {
    _sync: File,
    _ui: Option<File>,
}

fn open(dir: &Path, name: &str) -> io::Result<File> {
    OpenOptions::new().create(true).truncate(false).write(true).open(dir.join(name))
}

/// The app comes on screen: waits until the service has stopped working, which it does within a
/// few seconds of seeing the app (see `app_is_on_screen`). Blocks; call off the main thread.
pub fn app_take(dir: &Path) -> io::Result<Turn> {
    let ui = open(dir, UI_LOCK)?;
    ui.lock()?;
    let sync = open(dir, SYNC_LOCK)?;
    sync.lock()?;
    Ok(Turn { _sync: sync, _ui: Some(ui) })
}

/// The service's turn: waits while the app is on screen, then takes `sync.lock`. Blocks.
pub fn service_take(dir: &Path) -> io::Result<Turn> {
    // Holding ui.lock while waiting for sync.lock keeps an app that comes back in this moment
    // from being overtaken; the app releases sync.lock without waiting for anything.
    let ui = open(dir, UI_LOCK)?;
    ui.lock()?;
    let sync = open(dir, SYNC_LOCK)?;
    sync.lock()?;
    drop(ui);
    Ok(Turn { _sync: sync, _ui: None })
}

/// The service's presence: held for the life of the service's process. None when another
/// process already holds it.
pub fn service_present(dir: &Path) -> io::Result<Option<File>> {
    let f = open(dir, SERVICE_LOCK)?;
    match f.try_lock() {
        Ok(()) => Ok(Some(f)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(e)) => Err(e),
    }
}

/// Whether a service is running to take over the mailboxes when the app leaves the screen.
pub fn service_is_running(dir: &Path) -> io::Result<bool> {
    Ok(service_present(dir)?.is_none())
}

/// Whether the app is on screen now (it holds `ui.lock`). The service asks this every few
/// seconds while it works, and stops when the answer is yes.
pub fn app_is_on_screen(dir: &Path) -> io::Result<bool> {
    let ui = open(dir, UI_LOCK)?;
    match ui.try_lock() {
        Ok(()) => Ok(false),
        Err(TryLockError::WouldBlock) => Ok(true),
        Err(TryLockError::Error(e)) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    const WAIT: Duration = Duration::from_millis(300);

    /// Runs `f` on a thread; the receiver gets its turn once it has one.
    fn in_thread(dir: &Path, f: fn(&Path) -> io::Result<Turn>) -> mpsc::Receiver<Turn> {
        let (tx, rx) = mpsc::channel();
        let dir = dir.to_path_buf();
        std::thread::spawn(move || {
            let turn = f(&dir).unwrap();
            let _ = tx.send(turn);
        });
        rx
    }

    #[test]
    fn the_service_sees_the_app_on_screen_until_it_leaves() {
        let d = tempfile::tempdir().unwrap();
        assert!(!app_is_on_screen(d.path()).unwrap());
        let app = app_take(d.path()).unwrap();
        assert!(app_is_on_screen(d.path()).unwrap());
        drop(app);
        assert!(!app_is_on_screen(d.path()).unwrap());
    }

    #[test]
    fn the_service_waits_while_the_app_is_on_screen() {
        let d = tempfile::tempdir().unwrap();
        let app = app_take(d.path()).unwrap();
        let service = in_thread(d.path(), service_take);
        assert!(service.recv_timeout(WAIT).is_err(), "no turn while the app is on screen");
        drop(app);
        let turn = service.recv_timeout(Duration::from_secs(5)).expect("the turn once the app has gone");
        assert!(!app_is_on_screen(d.path()).unwrap(), "the waiting service does not look like the app");
        drop(turn);
    }

    #[test]
    fn the_app_sees_whether_a_service_is_there_to_take_over() {
        let d = tempfile::tempdir().unwrap();
        assert!(!service_is_running(d.path()).unwrap());
        let presence = service_present(d.path()).unwrap().expect("the first service is present");
        assert!(service_is_running(d.path()).unwrap());
        assert!(service_present(d.path()).unwrap().is_none(), "only one service at a time");
        drop(presence);
        assert!(!service_is_running(d.path()).unwrap(), "gone with the service");
    }

    #[test]
    fn the_app_waits_until_the_service_gives_the_turn_back() {
        let d = tempfile::tempdir().unwrap();
        let service = service_take(d.path()).unwrap();
        let app = in_thread(d.path(), app_take);
        assert!(app.recv_timeout(WAIT).is_err(), "the app waits for the working service");
        // The service sees the app (it already holds ui.lock) and stops.
        assert!(app_is_on_screen(d.path()).unwrap());
        drop(service);
        let turn = app.recv_timeout(Duration::from_secs(5)).expect("the app's turn");
        drop(turn);
    }
}
