use serde_json::Value;
use tauri::{AppHandle, Manager, State};

use crate::{
    application::{EngineSupervisor, ShellApplication, SyncControlApplication, SyncStatusSnapshot},
    domain::{
        CloseBehavior, CloseDecision, CloseOutcome, DialogPreview, ExportFormat, ThemePreference,
    },
    infrastructure::{dialogs, native},
};

use super::dto::{CloseResolution, EngineContextV1, EngineErrorV1, ShellContext, SyncSessionInput};

#[tauri::command]
pub(crate) fn get_engine_context(state: State<'_, EngineSupervisor>) -> EngineContextV1 {
    state.snapshot().into()
}

#[tauri::command]
pub(crate) fn retry_engine_start(
    state: State<'_, EngineSupervisor>,
) -> Result<EngineContextV1, EngineErrorV1> {
    state
        .retry()
        .map(EngineContextV1::from)
        .map_err(EngineErrorV1::from)
}

#[tauri::command]
pub(crate) fn get_sync_status(
    state: State<'_, SyncControlApplication>,
) -> Result<SyncStatusSnapshot, String> {
    state.snapshot()
}

#[tauri::command]
pub(crate) fn pause_sync(
    state: State<'_, SyncControlApplication>,
) -> Result<SyncStatusSnapshot, String> {
    state.pause()
}

#[tauri::command]
pub(crate) fn resume_sync(
    state: State<'_, SyncControlApplication>,
) -> Result<SyncStatusSnapshot, String> {
    state.resume()
}

#[tauri::command]
pub(crate) fn sync_now(
    state: State<'_, SyncControlApplication>,
) -> Result<SyncStatusSnapshot, String> {
    state.sync_now()
}

#[tauri::command]
pub(crate) fn retry_sync(
    state: State<'_, SyncControlApplication>,
) -> Result<SyncStatusSnapshot, String> {
    state.retry_now()
}

#[tauri::command]
pub(crate) fn list_sync_conflicts(
    state: State<'_, SyncControlApplication>,
) -> Result<Vec<crate::local_data::SyncConflictRecoveryRecord>, String> {
    state.conflicts()
}

#[tauri::command]
pub(crate) fn resolve_sync_conflict(
    state: State<'_, SyncControlApplication>,
    conflict_id: String,
    request: Value,
) -> Result<SyncStatusSnapshot, String> {
    state.stage_resolution(&conflict_id, &request)
}

#[tauri::command]
pub(crate) fn configure_sync_session(
    engine: State<'_, EngineSupervisor>,
    sync: State<'_, SyncControlApplication>,
    input: SyncSessionInput,
) -> Result<SyncStatusSnapshot, String> {
    input.validate()?;
    let workspace = engine
        .workspace()
        .ok_or_else(|| "The Local Engine must be ready before Sync can start".to_owned())?;
    sync.configure_cloud_session(
        workspace.store,
        input.base_url,
        input.access_token,
        input.account_id,
        input.device_id,
    )
}

fn context(app: &AppHandle, state: &ShellApplication) -> Result<ShellContext, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "The main window is unavailable".to_owned())?;
    Ok(ShellContext::new(
        app.package_info().version.to_string(),
        state.snapshot(),
        native::effective_theme(&window),
    ))
}

#[tauri::command]
pub(crate) fn get_shell_context(
    app: AppHandle,
    state: State<'_, ShellApplication>,
) -> Result<ShellContext, String> {
    context(&app, &state)
}

#[tauri::command]
pub(crate) fn frontend_ready(app: AppHandle) -> Result<(), String> {
    native::show_main_window(&app);
    println!("[desktop-smoke] ready");
    if std::env::var("TESTPAPERS_DESKTOP_SMOKE").as_deref() == Ok("1") {
        app.state::<ShellApplication>().request_explicit_quit();
        app.exit(0);
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn set_theme_preference(
    app: AppHandle,
    state: State<'_, ShellApplication>,
    preference: ThemePreference,
) -> Result<ShellContext, String> {
    state.set_theme_preference(preference)?;
    native::apply_theme(&app, preference)?;
    context(&app, &state)
}

#[tauri::command]
pub(crate) fn set_close_behavior(
    app: AppHandle,
    state: State<'_, ShellApplication>,
    behavior: CloseBehavior,
) -> Result<ShellContext, String> {
    state.set_close_behavior(behavior)?;
    context(&app, &state)
}

#[tauri::command]
pub(crate) fn resolve_close_request(
    app: AppHandle,
    state: State<'_, ShellApplication>,
    request_id: u32,
    decision: CloseDecision,
) -> Result<CloseResolution, String> {
    let outcome = state.resolve_close(request_id, decision)?;
    match outcome {
        CloseOutcome::Hiding => native::hide_main_window(&app),
        CloseOutcome::Exiting => app.exit(0),
        CloseOutcome::Cancelled => {}
    }
    Ok(CloseResolution::new(outcome))
}

#[tauri::command]
pub(crate) async fn preview_question_import_dialog(
    app: AppHandle,
) -> Result<DialogPreview, String> {
    tauri::async_runtime::spawn_blocking(move || dialogs::preview_question_import(&app))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn preview_paper_export_dialog(
    app: AppHandle,
    format: ExportFormat,
) -> Result<DialogPreview, String> {
    tauri::async_runtime::spawn_blocking(move || dialogs::preview_paper_export(&app, format))
        .await
        .map_err(|error| error.to_string())
}
