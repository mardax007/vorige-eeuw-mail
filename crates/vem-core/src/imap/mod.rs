//! IMAP: the connection (`client`), folder names (`utf7`) and keeping the local copy in step
//! with the server (`sync`, following docs/sync.md).

pub mod client;
pub mod ops;
pub mod sync;
pub mod utf7;

pub use client::{Auth, Imap, ImapConfig, MailboxState, RemoteFolder, RemoteMessage};
