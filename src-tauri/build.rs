fn main() {
    const COMMANDS: &[&str] = &[
        "get_shell_context",
        "frontend_ready",
        "set_theme_preference",
        "set_close_behavior",
        "resolve_close_request",
        "preview_question_import_dialog",
        "preview_paper_export_dialog",
    ];

    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to build TestPapers Desktop command manifest");
}
