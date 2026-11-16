//! Mail logic for Vorige Eeuw Mail. No Tauri dependency: testable headless, reused on Android.

pub mod error;
pub mod imap;
pub mod lookup;
pub mod net;
pub mod model;
pub mod parse;
pub mod smtp;
pub mod store;

pub use error::{Code, Result, VemError};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
