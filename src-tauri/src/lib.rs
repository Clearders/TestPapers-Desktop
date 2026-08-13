mod application;
mod domain;
mod infrastructure;
mod ipc;
#[allow(dead_code, unused_imports)]
mod local_data;
#[allow(dead_code)]
mod sync;
#[allow(dead_code, unused_imports)]
mod workspace_features;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use application::{
    EngineSupervisor, LocalWorkspaceApplication, PreferencesRepository, ShellApplication,
    SyncControlApplication,
};
use domain::{CloseAction, ThemePreference};
use infrastructure::{
    settings::{FilePreferencesRepository, SessionPreferencesRepository},
    workspace::WorkspaceBootstrap,
};
use tauri::{Manager, RunEvent, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _arguments, _working_directory| {
            infrastructure::native::show_main_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            ipc::commands::get_engine_context,
            ipc::commands::retry_engine_start,
            ipc::commands::get_sync_status,
            ipc::commands::pause_sync,
            ipc::commands::resume_sync,
            ipc::commands::sync_now,
            ipc::commands::retry_sync,
            ipc::commands::configure_sync_session,
            ipc::commands::get_shell_context,
            ipc::commands::frontend_ready,
            ipc::commands::set_theme_preference,
            ipc::commands::set_close_behavior,
            ipc::commands::resolve_close_request,
            ipc::commands::preview_question_import_dialog,
            ipc::commands::preview_paper_export_dialog,
            ipc::local_commands::get_job,
            ipc::local_commands::cancel_job,
            ipc::local_commands::search_questions,
            ipc::local_commands::get_question,
            ipc::local_commands::add_question_image,
            ipc::local_commands::create_question,
            ipc::local_commands::update_question,
            ipc::local_commands::delete_question,
            ipc::local_commands::restore_question,
            ipc::local_commands::list_question_revisions,
            ipc::local_commands::revert_question,
            ipc::local_commands::select_question_import,
            ipc::local_commands::commit_question_import,
            ipc::local_commands::discard_question_import,
            ipc::local_commands::generate_paper,
            ipc::local_commands::export_paper,
            ipc::local_commands::get_backup_schedule,
            ipc::local_commands::select_backup_destination,
            ipc::local_commands::prepare_backup_encryption,
            ipc::local_commands::configure_backup_schedule,
            ipc::local_commands::create_workspace_backup,
            ipc::local_commands::select_workspace_restore,
            ipc::local_commands::commit_workspace_restore,
            ipc::local_commands::discard_workspace_restore,
            ipc::local_commands::select_data_directory,
            ipc::local_commands::migrate_workspace_data_directory,
        ])
        .setup(|app| {
            let (repository, workspace, backup_settings_path, workspace_pointer_path): (
                Box<dyn PreferencesRepository>,
                WorkspaceBootstrap,
                Option<std::path::PathBuf>,
                Option<std::path::PathBuf>,
            ) = match app.path().app_data_dir() {
                Ok(directory) => {
                    let (workspace, pointer_path) =
                        WorkspaceBootstrap::configured(directory.clone());
                    (
                        Box::new(FilePreferencesRepository::new(
                            directory.join("settings.v1.json"),
                        )),
                        workspace,
                        Some(directory.join("backup-schedule.v1.json")),
                        Some(pointer_path),
                    )
                }
                Err(error) => (
                    Box::new(SessionPreferencesRepository::new(format!(
                        "Settings are available for this session only: {error}"
                    ))),
                    WorkspaceBootstrap::unavailable(
                        "The Local Engine cannot locate its workspace directory",
                    ),
                    None,
                    None,
                ),
            };
            app.manage(ShellApplication::new(repository));
            app.manage(LocalWorkspaceApplication::new(
                Arc::new(ipc::events::TauriJobEventSink::new(app.handle().clone())),
                backup_settings_path,
                workspace_pointer_path,
                workspace.clone(),
            ));
            let sync_event_app = app.handle().clone();
            app.manage(SyncControlApplication::new(Arc::new(move |snapshot| {
                let _ = ipc::events::emit_sync_status_changed(&sync_event_app, snapshot);
            })));

            let event_app = app.handle().clone();
            let previous_maintenance = Arc::new(AtomicBool::new(false));
            let engine = EngineSupervisor::new(
                workspace,
                Arc::new(move |snapshot| {
                    let maintenance_changed = previous_maintenance
                        .swap(snapshot.maintenance_mode, Ordering::SeqCst)
                        != snapshot.maintenance_mode;
                    let _ = ipc::events::emit_engine_state_changed(&event_app, snapshot.clone());
                    if maintenance_changed {
                        let _ = ipc::events::emit_maintenance_changed(&event_app, snapshot);
                    }
                }),
            );
            app.manage(engine);
            app.state::<EngineSupervisor>().start();
            app.manage(ipc::local_commands::AutomaticBackupScheduler::start(
                app.handle().clone(),
            )?);

            infrastructure::native::build_menu(app)?;

            let tray_result = infrastructure::native::build_tray(app);
            let state = app.state::<ShellApplication>();
            match tray_result {
                Ok(()) => state.set_tray_available(true, None),
                Err(error) => state.set_tray_available(
                    false,
                    Some(format!(
                        "The system tray is unavailable; closing the window will ask before exiting: {error}"
                    )),
                ),
            }
            let preference = state.snapshot().preferences.theme;
            if let Err(error) = infrastructure::native::apply_theme(app.handle(), preference) {
                state.add_warning(format!("The native theme could not be applied: {error}"));
                let _ = infrastructure::native::apply_theme(app.handle(), ThemePreference::System);
            }
            Ok(())
        })
        .on_menu_event(infrastructure::native::handle_menu_event)
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            let app = window.app_handle();
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    let state = app.state::<ShellApplication>();
                    match state.begin_close() {
                        CloseAction::Exit => app.exit(0),
                        CloseAction::Hide => {
                            api.prevent_close();
                            infrastructure::native::hide_main_window(app);
                        }
                        CloseAction::Prompt(request_id) => {
                            api.prevent_close();
                            if let Err(error) = ipc::events::emit_close_requested(app, request_id) {
                                state.add_warning(format!(
                                    "The close preference prompt could not be displayed: {error}"
                                ));
                            }
                        }
                        CloseAction::Ignore => api.prevent_close(),
                    }
                }
                WindowEvent::ThemeChanged(_) => infrastructure::native::emit_current_theme(app),
                _ => {}
            }
        });

    let app = builder
        .build(tauri::generate_context!())
        .expect("failed to build TestPapers Desktop");
    app.run(|app, event| match event {
        RunEvent::ExitRequested { .. } => {
            app.state::<ipc::local_commands::AutomaticBackupScheduler>()
                .shutdown();
            app.state::<EngineSupervisor>()
                .shutdown(Duration::from_secs(5));
        }
        RunEvent::Exit => {
            if app.state::<ShellApplication>().cleanup() {
                println!("[desktop-smoke] cleanup");
            }
        }
        _ => {}
    });
}
