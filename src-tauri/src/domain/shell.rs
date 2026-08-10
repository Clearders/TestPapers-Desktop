use serde::{Deserialize, Serialize};

pub(crate) const SHELL_SCHEMA_VERSION: u8 = 1;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EffectiveTheme {
    #[default]
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CloseBehavior {
    #[default]
    Ask,
    Quit,
    Tray,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CloseDecision {
    Quit,
    Tray,
    Cancel,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ExportFormat {
    Docx,
    Tex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DialogPreviewKind {
    QuestionImport,
    PaperDocx,
    PaperTex,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppPreferences {
    pub(crate) schema_version: u8,
    pub(crate) theme: ThemePreference,
    pub(crate) close_behavior: CloseBehavior,
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            schema_version: SHELL_SCHEMA_VERSION,
            theme: ThemePreference::System,
            close_behavior: CloseBehavior::Ask,
        }
    }
}

impl AppPreferences {
    pub(crate) fn validate(self) -> Result<Self, String> {
        if self.schema_version != SHELL_SCHEMA_VERSION {
            return Err(format!(
                "unsupported settings schema version {}",
                self.schema_version
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CloseAction {
    Exit,
    Hide,
    Prompt(u32),
    Ignore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CloseOutcome {
    Cancelled,
    Hiding,
    Exiting,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DialogPreview {
    pub(crate) schema_version: u8,
    pub(crate) kind: DialogPreviewKind,
    pub(crate) cancelled: bool,
    pub(crate) selection_count: usize,
    pub(crate) display_names: Vec<String>,
}

impl DialogPreview {
    pub(crate) fn new(kind: DialogPreviewKind, display_names: Vec<String>) -> Self {
        Self {
            schema_version: SHELL_SCHEMA_VERSION,
            kind,
            cancelled: display_names.is_empty(),
            selection_count: display_names.len(),
            display_names,
        }
    }
}
