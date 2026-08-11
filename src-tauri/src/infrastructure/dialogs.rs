use std::path::{Path, PathBuf};

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::domain::{DialogPreview, DialogPreviewKind, ExportFormat};
use crate::workspace_features::export::ExportFormat as LocalExportFormat;

const QUESTION_IMPORT_EXTENSIONS: &[&str] = &["csv", "json"];
const QUESTION_IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];

pub(crate) fn preview_question_import(app: &AppHandle) -> DialogPreview {
    let selection = app
        .dialog()
        .file()
        .add_filter("Question data", QUESTION_IMPORT_EXTENSIONS)
        .blocking_pick_files();
    DialogPreview::new(
        DialogPreviewKind::QuestionImport,
        selection
            .unwrap_or_default()
            .iter()
            .map(display_name)
            .collect(),
    )
}

pub(crate) fn select_question_import(app: &AppHandle) -> Result<Option<PathBuf>, String> {
    selected_path(
        app.dialog()
            .file()
            .add_filter("Question data", QUESTION_IMPORT_EXTENSIONS)
            .blocking_pick_file(),
    )
}

pub(crate) fn select_question_image(app: &AppHandle) -> Result<Option<PathBuf>, String> {
    selected_path(
        app.dialog()
            .file()
            .add_filter("Question image", QUESTION_IMAGE_EXTENSIONS)
            .blocking_pick_file(),
    )
}

pub(crate) fn select_backup_destination(app: &AppHandle) -> Result<Option<PathBuf>, String> {
    selected_path(app.dialog().file().blocking_pick_folder())
}

pub(crate) fn select_restore_archive(app: &AppHandle) -> Result<Option<PathBuf>, String> {
    selected_path(
        app.dialog()
            .file()
            .add_filter("TestPapers backup", &["tpbackup", "age"])
            .blocking_pick_file(),
    )
}

pub(crate) fn select_data_directory_parent(app: &AppHandle) -> Result<Option<PathBuf>, String> {
    selected_path(app.dialog().file().blocking_pick_folder())
}

pub(crate) fn select_paper_export(
    app: &AppHandle,
    format: LocalExportFormat,
    suggested_name: &str,
) -> Result<Option<PathBuf>, String> {
    let extension = match format {
        LocalExportFormat::Docx => "docx",
        LocalExportFormat::Tex => "tex",
        LocalExportFormat::Pdf => "pdf",
    };
    selected_path(
        app.dialog()
            .file()
            .set_file_name(suggested_name)
            .add_filter("Paper export", &[extension])
            .blocking_save_file(),
    )
}

pub(crate) fn select_manual_backup_target(
    app: &AppHandle,
    encrypted: bool,
    suggested_name: &str,
) -> Result<Option<PathBuf>, String> {
    let extension = if encrypted { "age" } else { "tpbackup" };
    selected_path(
        app.dialog()
            .file()
            .set_file_name(suggested_name)
            .add_filter("TestPapers backup", &[extension])
            .blocking_save_file(),
    )
}

pub(crate) fn preview_paper_export(app: &AppHandle, format: ExportFormat) -> DialogPreview {
    let (kind, extension, filename) = export_dialog_config(format);
    let selection = app
        .dialog()
        .file()
        .set_file_name(filename)
        .add_filter("Paper export", &[extension])
        .blocking_save_file();
    DialogPreview::new(kind, selection.iter().map(display_name).collect())
}

fn export_dialog_config(format: ExportFormat) -> (DialogPreviewKind, &'static str, &'static str) {
    match format {
        ExportFormat::Docx => (DialogPreviewKind::PaperDocx, "docx", "Untitled Paper.docx"),
        ExportFormat::Tex => (DialogPreviewKind::PaperTex, "tex", "Untitled Paper.tex"),
    }
}

fn display_name(path: &FilePath) -> String {
    match path {
        FilePath::Path(path) => path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Selected file")
            .to_owned(),
        FilePath::Url(url) => Path::new(url.path())
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Selected file")
            .to_owned(),
    }
}

fn selected_path(selection: Option<FilePath>) -> Result<Option<PathBuf>, String> {
    selection
        .map(FilePath::into_path)
        .transpose()
        .map_err(|_| "The selected item is not a local filesystem path.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_results_expose_only_the_basename() {
        let path = FilePath::Path("/private/teacher/questions.csv".into());
        assert_eq!(display_name(&path), "questions.csv");
    }

    #[test]
    fn dialog_filters_match_the_preview_contract() {
        assert_eq!(QUESTION_IMPORT_EXTENSIONS, &["csv", "json"]);
        assert_eq!(
            QUESTION_IMAGE_EXTENSIONS,
            &["png", "jpg", "jpeg", "gif", "webp"]
        );
        assert_eq!(export_dialog_config(ExportFormat::Docx).1, "docx");
        assert_eq!(export_dialog_config(ExportFormat::Tex).1, "tex");
    }
}
