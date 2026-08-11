//! Semantic local paper export.

mod docx;
mod tectonic;
mod tex;

pub(crate) use docx::build_docx;
pub(crate) use tectonic::{
    BundledTectonic, CompileControl, NoopCompileControl, TectonicError, TectonicRunner,
};
pub(crate) use tex::build_tex;

use super::paper::{PaperError, PaperItemSnapshot, PaperSnapshot, QuestionType};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExportFormat {
    Docx,
    Tex,
    Pdf,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum QuestionOrder {
    Paper,
    Categorized,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LayoutDensity {
    Auto,
    Normal,
    Compact,
    Dense,
}

impl LayoutDensity {
    pub(super) fn resolved(self, question_count: usize) -> Self {
        match self {
            Self::Auto if question_count >= 24 => Self::Dense,
            Self::Auto if question_count >= 14 => Self::Compact,
            Self::Auto => Self::Normal,
            value => value,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportOptions {
    pub(crate) include_answers: bool,
    pub(crate) question_order: QuestionOrder,
    pub(crate) layout_density: LayoutDensity,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            include_answers: false,
            question_order: QuestionOrder::Paper,
            layout_density: LayoutDensity::Auto,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CompanionFile {
    pub(crate) relative_path: String,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExportArtifact {
    pub(crate) filename: String,
    pub(crate) media_type: &'static str,
    pub(crate) bytes: Vec<u8>,
    /// TeX image companions. DOCX embeds its assets and leaves this empty.
    pub(crate) companions: Vec<CompanionFile>,
}

pub(crate) trait AttachmentSource: Send + Sync {
    fn load_blob(&self, sha256: &str) -> Result<Vec<u8>, String>;
}

pub(crate) struct NoAttachments;

impl AttachmentSource for NoAttachments {
    fn load_blob(&self, sha256: &str) -> Result<Vec<u8>, String> {
        Err(format!("attachment blob is unavailable: {sha256}"))
    }
}

#[derive(Debug)]
pub(crate) enum ExportError {
    InvalidPaper(PaperError),
    MissingAttachment { hash: String, detail: String },
    InvalidAttachment(&'static str),
    ArtifactTooLarge,
    Archive(String),
}

impl fmt::Display for ExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPaper(error) => write!(formatter, "paper is not exportable: {error}"),
            Self::MissingAttachment { hash, detail } => {
                write!(formatter, "attachment {hash} is unavailable: {detail}")
            }
            Self::InvalidAttachment(reason) => write!(formatter, "invalid attachment: {reason}"),
            Self::ArtifactTooLarge => {
                formatter.write_str("export artifact exceeds the supported size")
            }
            Self::Archive(reason) => write!(formatter, "could not build export archive: {reason}"),
        }
    }
}

pub(super) fn ordered_items(
    paper: &PaperSnapshot,
    order: QuestionOrder,
) -> Vec<&PaperItemSnapshot> {
    let mut items = paper.items.iter().collect::<Vec<_>>();
    match order {
        QuestionOrder::Paper => items.sort_by_key(|item| item.order),
        QuestionOrder::Categorized => {
            items.sort_by_key(|item| (type_rank(item.question_snapshot.question_type), item.order))
        }
    }
    items
}

fn type_rank(question_type: QuestionType) -> usize {
    QuestionType::ALL
        .iter()
        .position(|candidate| *candidate == question_type)
        .expect("all question types are ranked")
}

pub(super) fn safe_filename(title: &str, extension: &str) -> String {
    let mut cleaned = String::with_capacity(title.len());
    for character in title.chars() {
        if matches!(
            character,
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
        ) || character.is_control()
        {
            cleaned.push(' ');
        } else {
            cleaned.push(character);
        }
    }
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let basename: String = if collapsed.is_empty() {
        "examination-paper".into()
    } else {
        collapsed.chars().take(80).collect()
    };
    format!("{basename}.{extension}")
}

pub(super) fn attachment_extension(media_type: &str) -> Result<&'static str, ExportError> {
    match media_type.to_ascii_lowercase().as_str() {
        "image/png" => Ok("png"),
        "image/jpeg" => Ok("jpg"),
        "image/gif" => Ok("gif"),
        "image/bmp" => Ok("bmp"),
        "image/tiff" => Ok("tiff"),
        _ => Err(ExportError::InvalidAttachment(
            "only supported raster image media types can be exported",
        )),
    }
}
