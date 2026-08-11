use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    application::{BackupRuntime, DirectoryGrant},
    domain::SuggestedAction,
    local_data::{
        AttachmentRecord, DeletedFilter, Difficulty, EssayBlankSpace, MutationBase,
        QuestionContent, QuestionRecord, QuestionRevision, QuestionSearch, QuestionType,
    },
    workspace_features::{
        export::{ExportFormat, LayoutDensity, QuestionOrder},
        jobs::{JobSnapshot, JobState},
        paper::QuestionType as PaperQuestionType,
    },
};

pub(crate) const LOCAL_SCHEMA_VERSION: u8 = 1;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionInputDto {
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

impl From<QuestionInputDto> for QuestionContent {
    fn from(value: QuestionInputDto) -> Self {
        Self {
            question_type: value.question_type,
            subjects: value.subjects,
            difficulty: value.difficulty,
            tags: value.tags,
            text: value.text,
            options: value.options,
            answer: value.answer,
            has_latex: value.has_latex,
            source: value.source,
            essay_blank_space: value.essay_blank_space,
            score_weight: value.score_weight,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MutationBaseDto {
    pub(crate) base_version: i64,
    pub(crate) base_content_hash: String,
}

impl From<MutationBaseDto> for MutationBase {
    fn from(value: MutationBaseDto) -> Self {
        Self {
            base_version: value.base_version,
            base_content_hash: value.base_content_hash,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionImageDto {
    attachment_id: String,
    file_name: String,
    media_type: String,
    byte_size: i64,
    caption: Option<String>,
}

impl From<AttachmentRecord> for QuestionImageDto {
    fn from(value: AttachmentRecord) -> Self {
        Self {
            attachment_id: value.id,
            file_name: value.file_name,
            media_type: value.media_type,
            byte_size: value.byte_size,
            caption: value.caption,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionDto {
    schema_version: u8,
    id: String,
    owner_id: String,
    replication_scope: crate::local_data::ReplicationScope,
    version: i64,
    content_hash: String,
    created_at: i64,
    updated_at: i64,
    deleted_at: Option<i64>,
    #[serde(rename = "type")]
    question_type: QuestionType,
    subjects: Vec<String>,
    difficulty: Difficulty,
    tags: Vec<String>,
    text: String,
    options: Vec<String>,
    answer: Value,
    has_latex: bool,
    source: Option<String>,
    essay_blank_space: Option<EssayBlankSpace>,
    score_weight: String,
    images: Vec<QuestionImageDto>,
}

impl QuestionDto {
    pub(crate) fn new(record: QuestionRecord, attachments: Vec<AttachmentRecord>) -> Self {
        Self {
            schema_version: LOCAL_SCHEMA_VERSION,
            id: record.id,
            owner_id: record.owner_id,
            replication_scope: record.replication_scope,
            version: record.version,
            content_hash: record.content_hash,
            created_at: record.created_at,
            updated_at: record.updated_at,
            deleted_at: record.deleted_at,
            question_type: record.content.question_type,
            subjects: record.content.subjects,
            difficulty: record.content.difficulty,
            tags: record.content.tags,
            text: record.content.text,
            options: record.content.options.unwrap_or_default(),
            answer: record.content.answer,
            has_latex: record.content.has_latex,
            source: record.content.source,
            essay_blank_space: record.content.essay_blank_space,
            score_weight: record.content.score_weight,
            images: attachments.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionSearchRequestDto {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    subjects: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    types: Vec<QuestionType>,
    #[serde(default)]
    difficulties: Vec<Difficulty>,
    #[serde(default)]
    include_deleted: bool,
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default = "default_search_limit")]
    limit: u32,
}

fn default_search_limit() -> u32 {
    50
}

impl From<QuestionSearchRequestDto> for QuestionSearch {
    fn from(value: QuestionSearchRequestDto) -> Self {
        Self {
            query: value.query,
            subjects: value.subjects,
            tags: value.tags,
            types: value.types,
            difficulties: value.difficulties,
            deleted: if value.include_deleted {
                DeletedFilter::Include
            } else {
                DeletedFilter::Exclude
            },
            cursor: value.cursor,
            page_size: Some(value.limit.clamp(1, 100)),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionSearchPageDto {
    schema_version: u8,
    items: Vec<QuestionDto>,
    next_cursor: Option<String>,
}

impl QuestionSearchPageDto {
    pub(crate) fn new(items: Vec<QuestionDto>, next_cursor: Option<String>) -> Self {
        Self {
            schema_version: LOCAL_SCHEMA_VERSION,
            items,
            next_cursor,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionRevisionDto {
    schema_version: u8,
    entity_id: String,
    version: i64,
    content_hash: String,
    action: crate::local_data::HistoryAction,
    accepted_at: i64,
    snapshot: QuestionDto,
}

impl QuestionRevisionDto {
    pub(crate) fn new(entity_id: &str, revision: QuestionRevision) -> Self {
        Self {
            schema_version: LOCAL_SCHEMA_VERSION,
            entity_id: entity_id.into(),
            version: revision.version,
            content_hash: revision.content_hash,
            action: revision.action,
            accepted_at: revision.created_at,
            snapshot: QuestionDto::new(revision.snapshot, Vec::new()),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportErrorDto {
    row_number: usize,
    messages: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportInspectionDto {
    schema_version: u8,
    import_id: String,
    display_name: String,
    valid_rows: usize,
    invalid_rows: usize,
    errors: Vec<ImportErrorDto>,
}

impl ImportInspectionDto {
    pub(crate) fn new(
        import_id: String,
        display_name: String,
        inspection: &crate::local_data::ImportInspection,
    ) -> Self {
        let mut errors = inspection
            .rows
            .iter()
            .filter(|row| !row.errors.is_empty())
            .take(200)
            .map(|row| ImportErrorDto {
                row_number: row.row_number,
                messages: row.errors.clone(),
            })
            .collect::<Vec<_>>();
        if let Some(error) = &inspection.fatal_error {
            errors.insert(
                0,
                ImportErrorDto {
                    row_number: 0,
                    messages: vec![error.clone()],
                },
            );
        }
        Self {
            schema_version: LOCAL_SCHEMA_VERSION,
            import_id,
            display_name,
            valid_rows: inspection.valid_count(),
            invalid_rows: inspection.invalid_count(),
            errors,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommandErrorDto {
    schema_version: u8,
    code: String,
    message: String,
    recoverable: bool,
    suggested_action: SuggestedAction,
}

impl CommandErrorDto {
    pub(crate) fn recoverable(
        code: impl Into<String>,
        message: impl Into<String>,
        suggested_action: SuggestedAction,
    ) -> Self {
        Self {
            schema_version: LOCAL_SCHEMA_VERSION,
            code: code.into(),
            message: message.into(),
            recoverable: true,
            suggested_action,
        }
    }

    pub(crate) fn fatal(
        code: impl Into<String>,
        message: impl Into<String>,
        suggested_action: SuggestedAction,
    ) -> Self {
        Self {
            schema_version: LOCAL_SCHEMA_VERSION,
            code: code.into(),
            message: message.into(),
            recoverable: false,
            suggested_action,
        }
    }

    pub(crate) fn safe_message(&self) -> String {
        self.message.clone()
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JobSummaryDto {
    schema_version: u8,
    id: String,
    kind: crate::workspace_features::jobs::JobKind,
    state: JobState,
    completed_units: u64,
    total_units: Option<u64>,
    phase: String,
    cancellable: bool,
    result: Option<Value>,
    error: Option<CommandErrorDto>,
}

impl From<JobSnapshot> for JobSummaryDto {
    fn from(snapshot: JobSnapshot) -> Self {
        Self {
            schema_version: LOCAL_SCHEMA_VERSION,
            id: snapshot.id.0.to_string(),
            kind: snapshot.kind,
            state: snapshot.state,
            completed_units: snapshot.completed_units,
            total_units: snapshot.total_units,
            phase: snapshot.phase,
            cancellable: snapshot.cancellable,
            result: snapshot.result,
            error: snapshot.error.map(|error| {
                if error.recoverable {
                    CommandErrorDto::recoverable(error.code, error.message, SuggestedAction::Retry)
                } else {
                    CommandErrorDto::fatal(
                        error.code,
                        error.message,
                        SuggestedAction::ContactSupport,
                    )
                }
            }),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GenerationQuestionTypeDto {
    pub(crate) question_type: PaperQuestionType,
    pub(crate) count: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeneratePaperInputDto {
    pub(crate) title: String,
    pub(crate) subjects: Vec<String>,
    pub(crate) duration_minutes: u32,
    pub(crate) total_marks: String,
    pub(crate) difficulty_coefficient: f64,
    pub(crate) question_types: Vec<GenerationQuestionTypeDto>,
    #[serde(default)]
    pub(crate) required_tags: Vec<String>,
    #[serde(default)]
    pub(crate) preferred_tags: Vec<String>,
    pub(crate) seed: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportPaperInputDto {
    pub(crate) paper_id: String,
    pub(crate) format: ExportFormat,
    pub(crate) include_answers: bool,
    pub(crate) question_order: QuestionOrder,
    pub(crate) layout_density: LayoutDensity,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DirectorySelectionDto {
    schema_version: u8,
    selection_id: String,
    display_name: String,
    writable: bool,
    available_bytes: Option<u64>,
}

impl From<DirectoryGrant> for DirectorySelectionDto {
    fn from(grant: DirectoryGrant) -> Self {
        Self {
            schema_version: LOCAL_SCHEMA_VERSION,
            selection_id: grant.id,
            display_name: grant.display_name,
            writable: grant.writable,
            available_bytes: grant.available_bytes,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupScheduleInputDto {
    pub(crate) enabled: bool,
    pub(crate) destination_selection_id: Option<String>,
    pub(crate) interval_minutes: u32,
    pub(crate) retention_days: u32,
    pub(crate) encryption_mode: BackupScheduleEncryptionDto,
    #[serde(default)]
    pub(crate) recovery_key_confirmed: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BackupScheduleEncryptionDto {
    Keychain,
    None,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupScheduleDto {
    schema_version: u8,
    enabled: bool,
    destination_display_name: Option<String>,
    interval_minutes: u32,
    retention_days: u32,
    encryption_mode: BackupScheduleEncryptionDto,
    last_successful_at: Option<i64>,
    next_due_at: Option<i64>,
}

impl BackupScheduleDto {
    pub(crate) fn new(runtime: BackupRuntime, now_micros: i64) -> Self {
        let next_due_at = runtime
            .automatic_state
            .next_due_micros(now_micros, &runtime.config)
            .ok()
            .flatten();
        Self {
            schema_version: LOCAL_SCHEMA_VERSION,
            enabled: runtime.config.enabled,
            destination_display_name: runtime.destination_display_name,
            interval_minutes: runtime.config.interval_minutes,
            retention_days: runtime.config.retention_days,
            encryption_mode: if runtime.config.encrypted {
                BackupScheduleEncryptionDto::Keychain
            } else {
                BackupScheduleEncryptionDto::None
            },
            last_successful_at: runtime.automatic_state.last_success_micros,
            next_due_at,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateWorkspaceBackupInputDto {
    pub(crate) encryption_mode: ManualBackupEncryptionDto,
    pub(crate) passphrase: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ManualBackupEncryptionDto {
    Passphrase,
    None,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestoreUnlockDto {
    pub(crate) passphrase: Option<String>,
    pub(crate) recovery_key: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupPreflightDto {
    schema_version: u8,
    restore_id: String,
    display_name: String,
    workspace_id: String,
    app_version: String,
    schema_version_found: u32,
    created_at: i64,
    encrypted: bool,
    requires_recovery_key: bool,
    compatible: bool,
    warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecoveryKeyDto {
    schema_version: u8,
    key_id: String,
    recovery_key: String,
}

impl RecoveryKeyDto {
    pub(crate) fn new(key_id: String, recovery_key: String) -> Self {
        Self {
            schema_version: LOCAL_SCHEMA_VERSION,
            key_id,
            recovery_key,
        }
    }
}

impl BackupPreflightDto {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        restore_id: String,
        display_name: String,
        workspace_id: String,
        app_version: String,
        schema_version_found: u32,
        created_at: i64,
        encrypted: bool,
        requires_recovery_key: bool,
        compatible: bool,
        warnings: Vec<String>,
    ) -> Self {
        Self {
            schema_version: LOCAL_SCHEMA_VERSION,
            restore_id,
            display_name,
            workspace_id,
            app_version,
            schema_version_found,
            created_at,
            encrypted,
            requires_recovery_key,
            compatible,
            warnings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_data::ReplicationScope;

    #[test]
    fn question_dto_flattens_content_and_never_contains_a_path() {
        let record = QuestionRecord {
            id: "018f8f2a-7c20-7abc-8def-1234567890ab".into(),
            owner_id: "018f8f2a-7c20-7abc-8def-1234567890ac".into(),
            replication_scope: ReplicationScope::LocalPrivate,
            schema_version: 1,
            version: 1,
            content_hash: "a".repeat(64),
            created_at: 1,
            updated_at: 1,
            deleted_at: None,
            deleted_by_id: None,
            content: QuestionContent {
                question_type: QuestionType::ShortAnswer,
                subjects: vec!["Mathematics".into()],
                difficulty: Difficulty::Easy,
                tags: vec![],
                text: "What is 2 + 2?".into(),
                options: None,
                answer: Value::String("4".into()),
                has_latex: false,
                source: None,
                essay_blank_space: None,
                score_weight: "1".into(),
            },
        };
        let value = serde_json::to_value(QuestionDto::new(record, Vec::new())).unwrap();
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["type"], "short_answer");
        assert_eq!(value["options"], serde_json::json!([]));
        assert!(value.get("content").is_none());
        assert!(value.get("path").is_none());
    }
}
