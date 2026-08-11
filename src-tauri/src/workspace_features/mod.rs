//! Local-first workspace features implemented independently from Tauri IPC.
//!
//! The module deliberately exposes application-facing values and traits only.  Central runtime
//! wiring is expected to adapt SQLite repositories, native dialogs, the OS credential store and
//! Tauri events to these interfaces.

pub(crate) mod backup;
pub(crate) mod export;
pub(crate) mod generation;
pub(crate) mod hash;
pub(crate) mod jobs;
pub(crate) mod paper;
pub(crate) mod zip_store;
