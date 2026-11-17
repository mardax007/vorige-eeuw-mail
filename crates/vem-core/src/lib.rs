//! Mail logic for Vorige Eeuw Mail. No Tauri dependency: testable headless, reused on Android.

pub mod actions;
pub mod apk_update;
pub mod compose;
pub mod contacts_sync;
pub mod engine;
pub mod error;
pub mod imap;
pub mod import;
pub mod log;
pub mod lookup;
pub mod net;
pub mod model;
pub mod oauth;
pub mod parse;
pub mod report;
pub mod sanitize;
pub mod secrets;
pub mod smtp;
pub mod store;

pub use error::{Code, Result, VemError};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
