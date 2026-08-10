use std::path::Path;

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::domain::{DialogPreview, DialogPreviewKind, ExportFormat};

const QUESTION_IMPORT_EXTENSIONS: &[&str] = &["csv", "json"];

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
        assert_eq!(export_dialog_config(ExportFormat::Docx).1, "docx");
        assert_eq!(export_dialog_config(ExportFormat::Tex).1, "tex");
    }
}
