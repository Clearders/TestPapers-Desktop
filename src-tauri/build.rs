fn main() {
    const COMMANDS: &[&str] = &[
        "get_engine_context",
        "retry_engine_start",
        "get_shell_context",
        "frontend_ready",
        "set_theme_preference",
        "set_close_behavior",
        "resolve_close_request",
        "preview_question_import_dialog",
        "preview_paper_export_dialog",
        "get_job",
        "cancel_job",
        "search_questions",
        "get_question",
        "create_question",
        "update_question",
        "delete_question",
        "restore_question",
        "list_question_revisions",
        "revert_question",
        "add_question_image",
        "select_question_import",
        "commit_question_import",
        "discard_question_import",
        "generate_paper",
        "export_paper",
        "get_backup_schedule",
        "select_backup_destination",
        "prepare_backup_encryption",
        "configure_backup_schedule",
        "create_workspace_backup",
        "select_workspace_restore",
        "commit_workspace_restore",
        "discard_workspace_restore",
        "select_data_directory",
        "migrate_workspace_data_directory",
    ];

    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to build TestPapers Desktop command manifest");
}
