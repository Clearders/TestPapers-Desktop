use tauri::{AppHandle, Manager, State};

use crate::{
    application::ShellApplication,
    domain::{
        CloseBehavior, CloseDecision, CloseOutcome, DialogPreview, ExportFormat, ThemePreference,
    },
    infrastructure::{dialogs, native},
};

use super::dto::{CloseResolution, ShellContext};

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
