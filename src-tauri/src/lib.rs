mod application;
mod domain;
mod infrastructure;
mod ipc;
mod sync;

use application::{PreferencesRepository, ShellApplication};
use domain::{CloseAction, ThemePreference};
use infrastructure::settings::{FilePreferencesRepository, SessionPreferencesRepository};
use tauri::{Manager, RunEvent, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            ipc::commands::get_shell_context,
            ipc::commands::frontend_ready,
            ipc::commands::set_theme_preference,
            ipc::commands::set_close_behavior,
            ipc::commands::resolve_close_request,
            ipc::commands::preview_question_import_dialog,
            ipc::commands::preview_paper_export_dialog,
        ])
        .setup(|app| {
            let repository: Box<dyn PreferencesRepository> = match app.path().app_data_dir() {
                Ok(directory) => Box::new(FilePreferencesRepository::new(
                    directory.join("settings.v1.json"),
                )),
                Err(error) => Box::new(SessionPreferencesRepository::new(format!(
                    "Settings are available for this session only: {error}"
                ))),
            };
            app.manage(ShellApplication::new(repository));
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
    app.run(|app, event| {
        if let RunEvent::Exit = event {
            if app.state::<ShellApplication>().cleanup() {
                println!("[desktop-smoke] cleanup");
            }
        }
    });
}
