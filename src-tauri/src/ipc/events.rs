use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::{
    application::{EngineSnapshot, SyncStatusSnapshot},
    workspace_features::jobs::{JobEventSink, JobSnapshot},
};

use super::dto::{CloseRequestedEvent, EngineContextV1, ShellEvent};
use super::local_dto::JobSummaryDto;

pub(crate) const CLOSE_REQUESTED: &str = "testpapers://shell/close-requested";
pub(crate) const PREFERENCES_REQUESTED: &str = "testpapers://shell/preferences-requested";
pub(crate) const THEME_CHANGED: &str = "testpapers://shell/theme-changed";
pub(crate) const DIALOG_PREVIEWED: &str = "testpapers://shell/dialog-previewed";
pub(crate) const ENGINE_STATE_CHANGED: &str = "testpapers://engine/state-changed";
pub(crate) const MAINTENANCE_CHANGED: &str = "testpapers://workspace/maintenance-changed";
pub(crate) const JOB_UPDATED: &str = "testpapers://jobs/updated";
pub(crate) const SYNC_STATUS_CHANGED: &str = "testpapers://sync/status-changed";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncStatusChangedEvent {
    schema_version: u8,
    #[serde(rename = "type")]
    event_type: &'static str,
    occurred_at: u64,
    state: SyncStatusSnapshot,
}

pub(crate) struct TauriJobEventSink {
    app: AppHandle,
}

impl TauriJobEventSink {
    pub(crate) fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl JobEventSink for TauriJobEventSink {
    fn updated(&self, snapshot: JobSnapshot) {
        let _ = self
            .app
            .emit_to("main", JOB_UPDATED, JobSummaryDto::from(snapshot));
    }
}

pub(crate) fn emit_engine_state_changed(
    app: &AppHandle,
    snapshot: EngineSnapshot,
) -> tauri::Result<()> {
    app.emit_to(
        "main",
        ENGINE_STATE_CHANGED,
        EngineContextV1::from(snapshot),
    )
}

pub(crate) fn emit_maintenance_changed(
    app: &AppHandle,
    snapshot: EngineSnapshot,
) -> tauri::Result<()> {
    app.emit_to("main", MAINTENANCE_CHANGED, EngineContextV1::from(snapshot))
}

pub(crate) fn emit_sync_status_changed(
    app: &AppHandle,
    snapshot: SyncStatusSnapshot,
) -> tauri::Result<()> {
    let occurred_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_micros()).unwrap_or(u64::MAX))
        .unwrap_or(0);
    app.emit_to(
        "main",
        SYNC_STATUS_CHANGED,
        SyncStatusChangedEvent {
            schema_version: 1,
            event_type: "sync.statusChanged",
            occurred_at,
            state: snapshot,
        },
    )
}

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
