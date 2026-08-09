use tauri::{AppHandle, Emitter};

use super::dto::{CloseRequestedEvent, ShellEvent};

pub(crate) const CLOSE_REQUESTED: &str = "testpapers://shell/close-requested";
pub(crate) const PREFERENCES_REQUESTED: &str = "testpapers://shell/preferences-requested";
pub(crate) const THEME_CHANGED: &str = "testpapers://shell/theme-changed";
pub(crate) const DIALOG_PREVIEWED: &str = "testpapers://shell/dialog-previewed";

pub(crate) fn emit_close_requested(app: &AppHandle, request_id: u32) -> tauri::Result<()> {
    app.emit_to(
        "main",
        CLOSE_REQUESTED,
        CloseRequestedEvent::new(request_id),
    )
}

pub(crate) fn emit_preferences_requested(app: &AppHandle) -> tauri::Result<()> {
    app.emit_to("main", PREFERENCES_REQUESTED, ShellEvent::default())
}
