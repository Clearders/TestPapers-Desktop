use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::error::{LocalDataError, LocalDataResult};

pub(crate) const ENTITY_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReplicationScope {
    LocalPrivate,
    CloudSynced,
    CollaborativeShared,
}

impl ReplicationScope {
    pub(crate) fn creates_pending_mutation(self) -> bool {
        self != Self::LocalPrivate
    }
}

impl fmt::Display for ReplicationScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::LocalPrivate => "local_private",
            Self::CloudSynced => "cloud_synced",
            Self::CollaborativeShared => "collaborative_shared",
        })
    }
}

impl FromStr for ReplicationScope {
    type Err = LocalDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "local_private" => Ok(Self::LocalPrivate),
            "cloud_synced" => Ok(Self::CloudSynced),
            "collaborative_shared" => Ok(Self::CollaborativeShared),
            _ => Err(LocalDataError::Corrupt(format!(
                "unknown replication scope {value:?}"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum QuestionType {
    SingleChoice,
    MultipleChoice,
    TrueFalse,
    Blank,
    ShortAnswer,
    Essay,
}

impl QuestionType {
    pub(crate) fn has_options(self) -> bool {
        matches!(
            self,
            Self::SingleChoice | Self::MultipleChoice | Self::TrueFalse
        )
    }
}

impl fmt::Display for QuestionType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SingleChoice => "single_choice",
            Self::MultipleChoice => "multiple_choice",
            Self::TrueFalse => "true_false",
            Self::Blank => "blank",
            Self::ShortAnswer => "short_answer",
            Self::Essay => "essay",
        })
    }
}

impl FromStr for QuestionType {
    type Err = LocalDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "single_choice" => Ok(Self::SingleChoice),
            "multiple_choice" => Ok(Self::MultipleChoice),
            "true_false" => Ok(Self::TrueFalse),
            "blank" => Ok(Self::Blank),
            "short_answer" => Ok(Self::ShortAnswer),
            "essay" => Ok(Self::Essay),
            _ => Err(LocalDataError::Corrupt(format!(
                "unknown question type {value:?}"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Difficulty {
    Easy,
    Medium,
    Hard,
}

impl fmt::Display for Difficulty {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Easy => "easy",
            Self::Medium => "medium",
            Self::Hard => "hard",
        })
    }
}

impl FromStr for Difficulty {
    type Err = LocalDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "easy" => Ok(Self::Easy),
            "medium" => Ok(Self::Medium),
            "hard" => Ok(Self::Hard),
            _ => Err(LocalDataError::Corrupt(format!(
                "unknown question difficulty {value:?}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EssayBlankSpace {
    pub(crate) lines: u32,
    pub(crate) line_height: u32,
}

impl Default for EssayBlankSpace {
    fn default() -> Self {
        Self {
            lines: 6,
            line_height: 28,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionContent {
    #[serde(rename = "type")]
    pub(crate) question_type: QuestionType,
    pub(crate) subjects: Vec<String>,
    pub(crate) difficulty: Difficulty,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
    pub(crate) text: String,
    #[serde(default)]
    pub(crate) options: Option<Vec<String>>,
    pub(crate) answer: Value,
    #[serde(default)]
    pub(crate) has_latex: bool,
    #[serde(default)]
    pub(crate) source: Option<String>,
    #[serde(default)]
    pub(crate) essay_blank_space: Option<EssayBlankSpace>,
    #[serde(default = "default_score_weight")]
    pub(crate) score_weight: String,
}

fn default_score_weight() -> String {
    "1".into()
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionRecord {
    pub(crate) id: String,
    pub(crate) owner_id: String,
    pub(crate) replication_scope: ReplicationScope,
    pub(crate) schema_version: u32,
    pub(crate) version: i64,
    pub(crate) content_hash: String,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
    pub(crate) deleted_at: Option<i64>,
    pub(crate) deleted_by_id: Option<String>,
    pub(crate) content: QuestionContent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MutationBase {
    pub(crate) base_version: i64,
    pub(crate) base_content_hash: String,
}

impl MutationBase {
    pub(crate) fn validate(&self) -> LocalDataResult<()> {
        if self.base_version < 1
            || self.base_content_hash.len() != 64
            || !self
                .base_content_hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(LocalDataError::Validation(vec![
                "mutation base must contain a positive version and lowercase SHA-256 hash".into(),
            ]));
        }
        Ok(())
    }

    pub(crate) fn matches(&self, question: &QuestionRecord) -> bool {
        self.base_version == question.version && self.base_content_hash == question.content_hash
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateQuestion {
    #[serde(default)]
    pub(crate) owner_id: Option<String>,
    #[serde(default = "default_scope")]
    pub(crate) replication_scope: ReplicationScope,
    pub(crate) content: QuestionContent,
}

fn default_scope() -> ReplicationScope {
    ReplicationScope::LocalPrivate
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateQuestion {
    pub(crate) mutation_base: MutationBase,
    pub(crate) content: QuestionContent,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HistoryAction {
    Create,
    Update,
    Delete,
    Restore,
    Revert,
}

impl fmt::Display for HistoryAction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Restore => "restore",
            Self::Revert => "revert",
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionRevision {
    pub(crate) version: i64,
    pub(crate) content_hash: String,
    pub(crate) action: HistoryAction,
    pub(crate) created_at: i64,
    pub(crate) snapshot: QuestionRecord,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeletedFilter {
    #[default]
    Exclude,
    Include,
    Only,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionSearch {
    pub(crate) query: Option<String>,
    #[serde(default)]
    pub(crate) subjects: Vec<String>,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
    #[serde(default)]
    pub(crate) types: Vec<QuestionType>,
    #[serde(default)]
    pub(crate) difficulties: Vec<Difficulty>,
    #[serde(default)]
    pub(crate) deleted: DeletedFilter,
    pub(crate) cursor: Option<String>,
    pub(crate) page_size: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionSearchPage {
    pub(crate) items: Vec<QuestionRecord>,
    pub(crate) next_cursor: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PendingMutation {
    pub(crate) operation_id: String,
    pub(crate) entity_type: String,
    pub(crate) entity_id: String,
    pub(crate) base_version: Option<i64>,
    pub(crate) base_content_hash: Option<String>,
    pub(crate) mutation_kind: String,
    pub(crate) candidate: Value,
    pub(crate) created_at: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttachmentRecord {
    pub(crate) id: String,
    pub(crate) owner_id: String,
    pub(crate) replication_scope: ReplicationScope,
    pub(crate) schema_version: u32,
    pub(crate) version: i64,
    pub(crate) content_hash: String,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
    pub(crate) deleted_at: Option<i64>,
    pub(crate) deleted_by_id: Option<String>,
    pub(crate) target_type: String,
    pub(crate) target_id: String,
    pub(crate) file_name: String,
    pub(crate) media_type: String,
    pub(crate) byte_size: i64,
    pub(crate) blob_hash: String,
    pub(crate) caption: Option<String>,
    pub(crate) position: u32,
    pub(crate) uploaded_by_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NewQuestionAttachment {
    pub(crate) question_id: String,
    pub(crate) file_name: String,
    pub(crate) media_type: String,
    pub(crate) caption: Option<String>,
    pub(crate) position: u32,
    pub(crate) uploaded_by_id: Option<String>,
}

impl QuestionContent {
    pub(crate) fn normalize(mut self) -> LocalDataResult<Self> {
        let mut errors = Vec::new();
        self.subjects = normalize_unique_strings(self.subjects, "subjects", &mut errors);
        self.tags = normalize_unique_strings(
            self.tags
                .into_iter()
                .map(|tag| tag.to_lowercase())
                .collect(),
            "tags",
            &mut errors,
        );
        self.text = self.text.trim().to_owned();
        if self.text.is_empty() {
            errors.push("text is required".into());
        }

        self.source = self
            .source
            .map(|source| source.trim().to_owned())
            .filter(|source| !source.is_empty());

        if self.question_type.has_options() {
            let options = normalize_unique_strings(
                self.options.take().unwrap_or_default(),
                "options",
                &mut errors,
            );
            if options.is_empty() {
                errors.push("options is required for choice and true/false questions".into());
            }
            self.options = Some(options);
        } else {
            self.options = None;
        }

        match &mut self.answer {
            Value::String(answer) => {
                *answer = answer.trim().to_owned();
                if answer.is_empty() {
                    errors.push("answer is required".into());
                }
            }
            Value::Array(answers) => {
                answers.retain_mut(|answer| match answer {
                    Value::String(value) => {
                        *value = value.trim().to_owned();
                        !value.is_empty()
                    }
                    _ => false,
                });
                if answers.is_empty() {
                    errors.push("answer is required".into());
                }
            }
            _ => errors.push("answer must be a string or string array".into()),
        }

        self.essay_blank_space = if self.question_type == QuestionType::Essay {
            let blank_space = self.essay_blank_space.unwrap_or_default();
            Some(EssayBlankSpace {
                lines: blank_space.lines.clamp(1, 20),
                line_height: blank_space.line_height.clamp(20, 48),
            })
        } else {
            None
        };
        self.score_weight = normalize_score_weight(&self.score_weight, &mut errors);

        if errors.is_empty() {
            Ok(self)
        } else {
            Err(LocalDataError::Validation(errors))
        }
    }
}

fn normalize_unique_strings(
    items: Vec<String>,
    field: &str,
    errors: &mut Vec<String>,
) -> Vec<String> {
    let mut normalized = Vec::new();
    for item in items {
        let item = item.trim();
        if item.is_empty() || normalized.iter().any(|existing| existing == item) {
            continue;
        }
        normalized.push(item.to_owned());
    }
    if field == "subjects" && normalized.is_empty() {
        errors.push("subjects is required".into());
    }
    normalized
}

fn normalize_score_weight(value: &str, errors: &mut Vec<String>) -> String {
    let value = value.trim();
    let (integer, fraction) = match value.split_once('.') {
        Some((integer, fraction)) => (integer, fraction),
        None => (value, ""),
    };
    if integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > 4
        || value.matches('.').count() > 1
    {
        errors.push("scoreWeight must be a plain positive decimal with at most four places".into());
        return "1".into();
    }
    let whole = integer.parse::<u64>().unwrap_or(u64::MAX);
    let mut fractional = fraction.to_owned();
    while fractional.len() < 4 {
        fractional.push('0');
    }
    let fractional = fractional.parse::<u64>().unwrap_or(0);
    let scaled = whole
        .checked_mul(10_000)
        .and_then(|value| value.checked_add(fractional));
    let Some(scaled) = scaled else {
        errors.push("scoreWeight is too large".into());
        return "1".into();
    };
    if !(1..=99_999_999).contains(&scaled) {
        errors.push("scoreWeight must be between 0.0001 and 9999.9999".into());
        return "1".into();
    }
    let whole = scaled / 10_000;
    let remainder = scaled % 10_000;
    if remainder == 0 {
        return whole.to_string();
    }
    let fraction = format!("{remainder:04}").trim_end_matches('0').to_owned();
    format!("{whole}.{fraction}")
}
