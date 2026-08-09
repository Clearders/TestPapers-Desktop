use tauri::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Emitter, Manager, Theme,
};

use crate::{
    application::ShellApplication,
    domain::{ExportFormat, ThemePreference},
    infrastructure::dialogs,
    ipc::{self, dto::ThemeState},
};

const MENU_PREFERENCES: &str = "app.preferences";
const MENU_QUIT: &str = "app.quit";
const MENU_IMPORT: &str = "file.import-preview";
const MENU_EXPORT_DOCX: &str = "file.export-docx-preview";
const MENU_EXPORT_TEX: &str = "file.export-tex-preview";
const MENU_THEME_SYSTEM: &str = "view.theme-system";
const MENU_THEME_LIGHT: &str = "view.theme-light";
const MENU_THEME_DARK: &str = "view.theme-dark";
const TRAY_SHOW: &str = "tray.show";
const TRAY_HIDE: &str = "tray.hide";
const TRAY_PREFERENCES: &str = "tray.preferences";
const TRAY_QUIT: &str = "tray.quit";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeAction {
    Preferences,
    Quit,
    Show,
    Hide,
    Theme(ThemePreference),
    Import,
    Export(ExportFormat),
}

pub(crate) struct NativeMenuState {
    system: CheckMenuItem<tauri::Wry>,
    light: CheckMenuItem<tauri::Wry>,
    dark: CheckMenuItem<tauri::Wry>,
}

pub(crate) fn build_menu(app: &App) -> tauri::Result<()> {
    let preferences = MenuItem::with_id(app, MENU_PREFERENCES, "Preferences…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Quit TestPapers", true, Some("CmdOrCtrl+Q"))?;
    let application = Submenu::with_items(app, "TestPapers Desktop", true, &[&preferences, &quit])?;

    let import = MenuItem::with_id(
        app,
        MENU_IMPORT,
        "Preview question import…",
        true,
        Some("CmdOrCtrl+O"),
    )?;
    let export_docx = MenuItem::with_id(
        app,
        MENU_EXPORT_DOCX,
        "Preview DOCX target…",
        true,
        None::<&str>,
    )?;
    let export_tex = MenuItem::with_id(
        app,
        MENU_EXPORT_TEX,
        "Preview TeX target…",
        true,
        None::<&str>,
    )?;
    let file = Submenu::with_items(app, "File", true, &[&import, &export_docx, &export_tex])?;

    let preference = app.state::<ShellApplication>().snapshot().preferences.theme;
    let system = CheckMenuItem::with_id(
        app,
        MENU_THEME_SYSTEM,
        "System theme",
        true,
        preference == ThemePreference::System,
        None::<&str>,
    )?;
    let light = CheckMenuItem::with_id(
        app,
        MENU_THEME_LIGHT,
        "Light theme",
        true,
        preference == ThemePreference::Light,
        None::<&str>,
    )?;
    let dark = CheckMenuItem::with_id(
        app,
        MENU_THEME_DARK,
        "Dark theme",
        true,
        preference == ThemePreference::Dark,
        None::<&str>,
    )?;
    let view = Submenu::with_items(app, "View", true, &[&system, &light, &dark])?;

    app.set_menu(Menu::with_items(app, &[&application, &file, &view])?)?;
    app.manage(NativeMenuState {
        system,
        light,
        dark,
    });
    Ok(())
}

pub(crate) fn build_tray(app: &App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, TRAY_SHOW, "Show TestPapers", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, TRAY_HIDE, "Hide window", true, None::<&str>)?;
    let preferences = MenuItem::with_id(app, TRAY_PREFERENCES, "Close behavior…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, TRAY_QUIT, "Quit TestPapers", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &hide, &preferences, &quit])?;
    let mut builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("TestPapers Desktop")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

pub(crate) fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    let Some(action) = action_for_id(event.id().as_ref()) else {
        return;
    };
    match action {
        NativeAction::Preferences => {
            show_main_window(app);
            let _ = ipc::events::emit_preferences_requested(app);
        }
        NativeAction::Quit => {
            app.state::<ShellApplication>().request_explicit_quit();
            app.exit(0);
        }
        NativeAction::Show => show_main_window(app),
        NativeAction::Hide => hide_main_window(app),
        NativeAction::Theme(preference) => set_theme_from_menu(app, preference),
        NativeAction::Import => {
            let preview = dialogs::preview_question_import(app);
            let _ = app.emit_to("main", ipc::events::DIALOG_PREVIEWED, preview);
        }
        NativeAction::Export(format) => emit_export_preview(app, format),
    }
}

fn action_for_id(id: &str) -> Option<NativeAction> {
    match id {
        MENU_PREFERENCES | TRAY_PREFERENCES => Some(NativeAction::Preferences),
        MENU_QUIT | TRAY_QUIT => Some(NativeAction::Quit),
        TRAY_SHOW => Some(NativeAction::Show),
        TRAY_HIDE => Some(NativeAction::Hide),
        MENU_THEME_SYSTEM => Some(NativeAction::Theme(ThemePreference::System)),
        MENU_THEME_LIGHT => Some(NativeAction::Theme(ThemePreference::Light)),
        MENU_THEME_DARK => Some(NativeAction::Theme(ThemePreference::Dark)),
        MENU_IMPORT => Some(NativeAction::Import),
        MENU_EXPORT_DOCX => Some(NativeAction::Export(ExportFormat::Docx)),
        MENU_EXPORT_TEX => Some(NativeAction::Export(ExportFormat::Tex)),
        _ => None,
    }
}

fn emit_export_preview(app: &AppHandle, format: ExportFormat) {
    let preview = dialogs::preview_paper_export(app, format);
    let _ = app.emit_to("main", ipc::events::DIALOG_PREVIEWED, preview);
}

fn set_theme_from_menu(app: &AppHandle, preference: ThemePreference) {
    let state = app.state::<ShellApplication>();
    if let Err(error) = state.set_theme_preference(preference) {
        state.add_warning(error);
    }
    let _ = apply_theme(app, preference);
}

pub(crate) fn apply_theme(
    app: &AppHandle,
    preference: ThemePreference,
) -> Result<ThemeState, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "The main window is unavailable".to_owned())?;
    let native_theme = match preference {
        ThemePreference::System => None,
        ThemePreference::Light => Some(Theme::Light),
        ThemePreference::Dark => Some(Theme::Dark),
    };
    window
        .set_theme(native_theme)
        .map_err(|error| error.to_string())?;
    sync_theme_menu(app, preference);
    let payload = ThemeState::new(preference, effective_theme(&window));
    let _ = app.emit_to("main", ipc::events::THEME_CHANGED, &payload);
    Ok(payload)
}

pub(crate) fn emit_current_theme(app: &AppHandle) {
    let state = app.state::<ShellApplication>();
    let preference = state.snapshot().preferences.theme;
    if let Some(window) = app.get_webview_window("main") {
        let payload = ThemeState::new(preference, effective_theme(&window));
        let _ = app.emit_to("main", ipc::events::THEME_CHANGED, payload);
    }
}

pub(crate) fn effective_theme(window: &tauri::WebviewWindow) -> crate::domain::EffectiveTheme {
    match window.theme().unwrap_or(Theme::Light) {
        Theme::Dark => crate::domain::EffectiveTheme::Dark,
        Theme::Light => crate::domain::EffectiveTheme::Light,
        _ => crate::domain::EffectiveTheme::Light,
    }
}

fn sync_theme_menu(app: &AppHandle, preference: ThemePreference) {
    let menu = app.state::<NativeMenuState>();
    let _ = menu
        .system
        .set_checked(preference == ThemePreference::System);
    let _ = menu.light.set_checked(preference == ThemePreference::Light);
    let _ = menu.dark.set_checked(preference == ThemePreference::Dark);
}

pub(crate) fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub(crate) fn hide_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_menu_and_tray_ids_map_to_owned_actions() {
        assert_eq!(
            action_for_id(MENU_PREFERENCES),
            Some(NativeAction::Preferences)
        );
        assert_eq!(action_for_id(TRAY_SHOW), Some(NativeAction::Show));
        assert_eq!(action_for_id(TRAY_HIDE), Some(NativeAction::Hide));
        assert_eq!(action_for_id(TRAY_QUIT), Some(NativeAction::Quit));
        assert_eq!(
            action_for_id(MENU_THEME_DARK),
            Some(NativeAction::Theme(ThemePreference::Dark))
        );
        assert_eq!(
            action_for_id(MENU_EXPORT_TEX),
            Some(NativeAction::Export(ExportFormat::Tex))
        );
        assert_eq!(action_for_id("unknown"), None);
    }
}
