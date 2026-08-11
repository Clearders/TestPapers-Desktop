//! Paper aggregate and immutable question snapshots (CLE-27).

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

pub(crate) const PAPER_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReplicationScope {
    LocalPrivate,
    CloudSynced,
    CollaborativeShared,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
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
    pub(crate) const ALL: [Self; 6] = [
        Self::SingleChoice,
        Self::MultipleChoice,
        Self::TrueFalse,
        Self::Blank,
        Self::ShortAnswer,
        Self::Essay,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::SingleChoice => "Single Choice",
            Self::MultipleChoice => "Multiple Choice",
            Self::TrueFalse => "True or False",
            Self::Blank => "Fill in the Blank",
            Self::ShortAnswer => "Short Answer",
            Self::Essay => "Essay",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Difficulty {
    Easy,
    Medium,
    Hard,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub(crate) enum AnswerSnapshot {
    Text(String),
    Multiple(Vec<String>),
}

impl AnswerSnapshot {
    pub(crate) fn display_text(&self) -> String {
        match self {
            Self::Text(value) => value.clone(),
            Self::Multiple(values) => values.join(", "),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EssayBlankSpace {
    pub(crate) lines: u8,
}

impl Default for EssayBlankSpace {
    fn default() -> Self {
        Self { lines: 6 }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttachmentSnapshot {
    pub(crate) id: String,
    pub(crate) blob_hash: String,
    pub(crate) media_type: String,
    pub(crate) filename: String,
    pub(crate) caption: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuestionSnapshot {
    /// Provenance only. The rest of this value remains authoritative for a paper export.
    pub(crate) id: String,
    pub(crate) version: u64,
    pub(crate) content_hash: String,
    #[serde(rename = "type")]
    pub(crate) question_type: QuestionType,
    pub(crate) subjects: Vec<String>,
    pub(crate) difficulty: Difficulty,
    pub(crate) tags: Vec<String>,
    pub(crate) text: String,
    pub(crate) options: Option<Vec<String>>,
    pub(crate) answer: AnswerSnapshot,
    pub(crate) has_latex: bool,
    pub(crate) source: Option<String>,
    pub(crate) essay_blank_space: Option<EssayBlankSpace>,
    /// Canonical decimal string from the shared domain contract.
    pub(crate) score_weight: String,
    #[serde(default)]
    pub(crate) attachments: Vec<AttachmentSnapshot>,
}

impl QuestionSnapshot {
    pub(crate) fn validate(&self) -> Result<(), PaperError> {
        validate_stable_id(&self.id, "questionSnapshot.id")?;
        validate_sha256(&self.content_hash, "questionSnapshot.contentHash")?;
        if self.version == 0 {
            return Err(PaperError::Invalid(
                "questionSnapshot.version must be positive",
            ));
        }
        if self.text.trim().is_empty() {
            return Err(PaperError::Invalid(
                "questionSnapshot.text must not be empty",
            ));
        }
        validate_unique_non_empty(&self.subjects, "questionSnapshot.subjects", false)?;
        validate_unique_non_empty(&self.tags, "questionSnapshot.tags", true)?;
        validate_decimal(&self.score_weight, false, "questionSnapshot.scoreWeight")?;
        if self.question_type == QuestionType::Essay {
            let lines = self
                .essay_blank_space
                .as_ref()
                .map_or(6, |space| space.lines);
            if !(1..=20).contains(&lines) {
                return Err(PaperError::Invalid(
                    "essay blank-space lines must be 1..=20",
                ));
            }
        }
        let mut attachment_ids = BTreeSet::new();
        for attachment in &self.attachments {
            validate_stable_id(&attachment.id, "attachment.id")?;
            validate_sha256(&attachment.blob_hash, "attachment.blobHash")?;
            if attachment.media_type.trim().is_empty() || attachment.filename.trim().is_empty() {
                return Err(PaperError::Invalid("attachment metadata must not be empty"));
            }
            if !attachment_ids.insert(&attachment.id) {
                return Err(PaperError::DuplicateAttachment(attachment.id.clone()));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PaperItemSnapshot {
    pub(crate) id: String,
    pub(crate) question_id: Option<String>,
    pub(crate) order: u32,
    pub(crate) marks: Option<String>,
    pub(crate) question_snapshot: QuestionSnapshot,
}

impl PaperItemSnapshot {
    pub(crate) fn validate(&self) -> Result<(), PaperError> {
        validate_stable_id(&self.id, "paperItem.id")?;
        if let Some(question_id) = &self.question_id {
            validate_stable_id(question_id, "paperItem.questionId")?;
            if question_id != &self.question_snapshot.id {
                return Err(PaperError::QuestionProvenanceMismatch {
                    item_id: self.id.clone(),
                });
            }
        }
        if let Some(marks) = &self.marks {
            validate_decimal(marks, true, "paperItem.marks")?;
        }
        self.question_snapshot.validate()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PaperStatus {
    Draft,
    Published,
    Archived,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PaperSnapshot {
    pub(crate) id: String,
    pub(crate) owner_id: String,
    pub(crate) replication_scope: ReplicationScope,
    pub(crate) schema_version: u32,
    pub(crate) version: u64,
    pub(crate) content_hash: String,
    pub(crate) created_at_micros: i64,
    pub(crate) updated_at_micros: i64,
    pub(crate) deleted_at_micros: Option<i64>,
    pub(crate) title: String,
    pub(crate) subject: String,
    pub(crate) duration_minutes: u32,
    pub(crate) total_marks: String,
    pub(crate) status: PaperStatus,
    pub(crate) items: Vec<PaperItemSnapshot>,
}

impl PaperSnapshot {
    pub(crate) fn validate(&self) -> Result<(), PaperError> {
        validate_stable_id(&self.id, "paper.id")?;
        validate_stable_id(&self.owner_id, "paper.ownerId")?;
        validate_sha256(&self.content_hash, "paper.contentHash")?;
        if self.schema_version != PAPER_SCHEMA_VERSION {
            return Err(PaperError::UnsupportedSchema(self.schema_version));
        }
        if self.version == 0 || self.duration_minutes == 0 {
            return Err(PaperError::Invalid(
                "paper version and duration must be positive",
            ));
        }
        validate_short_text(&self.title, "paper.title")?;
        validate_short_text(&self.subject, "paper.subject")?;
        validate_decimal(&self.total_marks, false, "paper.totalMarks")?;

        let mut ids = BTreeSet::new();
        let mut orders = BTreeSet::new();
        for item in &self.items {
            item.validate()?;
            if !ids.insert(&item.id) {
                return Err(PaperError::DuplicateItem(item.id.clone()));
            }
            if !orders.insert(item.order) {
                return Err(PaperError::DuplicateOrder(item.order));
            }
        }
        if orders.iter().copied().ne(0..self.items.len() as u32) {
            return Err(PaperError::NonContiguousOrder);
        }
        Ok(())
    }

    /// Reorders existing stable item identities as one candidate aggregate mutation.
    pub(crate) fn reorder(&self, ordered_item_ids: &[String]) -> Result<Self, PaperError> {
        if ordered_item_ids.len() != self.items.len() {
            return Err(PaperError::IncompleteReorder);
        }
        let mut requested = BTreeSet::new();
        if ordered_item_ids.iter().any(|id| !requested.insert(id)) {
            return Err(PaperError::IncompleteReorder);
        }
        let existing: BTreeSet<&String> = self.items.iter().map(|item| &item.id).collect();
        if requested != existing {
            return Err(PaperError::IncompleteReorder);
        }

        let mut candidate = self.clone();
        for (order, id) in ordered_item_ids.iter().enumerate() {
            let item = candidate
                .items
                .iter_mut()
                .find(|item| &item.id == id)
                .expect("sets were compared");
            item.order = order as u32;
        }
        candidate.items.sort_by_key(|item| item.order);
        Ok(candidate)
    }

    pub(crate) fn update_marks(
        &self,
        changes: &[(String, Option<String>)],
    ) -> Result<Self, PaperError> {
        let mut candidate = self.clone();
        let mut seen = BTreeSet::new();
        for (item_id, marks) in changes {
            if !seen.insert(item_id) {
                return Err(PaperError::DuplicateItem(item_id.clone()));
            }
            if let Some(value) = marks {
                validate_decimal(value, true, "paperItem.marks")?;
            }
            let item = candidate
                .items
                .iter_mut()
                .find(|item| item.id == *item_id)
                .ok_or_else(|| PaperError::UnknownItem(item_id.clone()))?;
            item.marks = marks.clone();
        }
        Ok(candidate)
    }
}

/// Persistence adapter implemented by the CLE-25 SQLite layer. A candidate is accepted atomically
/// only when both optimistic-lock values still match.
pub(crate) trait PaperSnapshotStore: Send + Sync {
    fn load(&self, paper_id: &str) -> Result<Option<PaperSnapshot>, String>;

    fn accept_candidate(
        &self,
        base_version: u64,
        base_content_hash: &str,
        candidate: &PaperSnapshot,
    ) -> Result<PaperSnapshot, StoreCandidateError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StoreCandidateError {
    StaleBase,
    Rejected(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PaperError {
    Invalid(&'static str),
    InvalidField {
        field: &'static str,
        reason: &'static str,
    },
    UnsupportedSchema(u32),
    DuplicateItem(String),
    UnknownItem(String),
    DuplicateOrder(u32),
    NonContiguousOrder,
    IncompleteReorder,
    DuplicateAttachment(String),
    QuestionProvenanceMismatch {
        item_id: String,
    },
}

impl fmt::Display for PaperError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(reason) => formatter.write_str(reason),
            Self::InvalidField { field, reason } => write!(formatter, "{field}: {reason}"),
            Self::UnsupportedSchema(version) => {
                write!(formatter, "unsupported paper schema {version}")
            }
            Self::DuplicateItem(id) => write!(formatter, "duplicate paper item {id}"),
            Self::UnknownItem(id) => write!(formatter, "unknown paper item {id}"),
            Self::DuplicateOrder(order) => write!(formatter, "duplicate paper order {order}"),
            Self::NonContiguousOrder => {
                formatter.write_str("paper item order must be contiguous from zero")
            }
            Self::IncompleteReorder => {
                formatter.write_str("reorder must contain every item exactly once")
            }
            Self::DuplicateAttachment(id) => write!(formatter, "duplicate attachment {id}"),
            Self::QuestionProvenanceMismatch { item_id } => {
                write!(
                    formatter,
                    "question provenance does not match snapshot for item {item_id}"
                )
            }
        }
    }
}

fn validate_short_text(value: &str, field: &'static str) -> Result<(), PaperError> {
    let length = value.chars().count();
    if !(1..=255).contains(&length) || value.trim().is_empty() {
        return Err(PaperError::InvalidField {
            field,
            reason: "must contain 1..=255 characters",
        });
    }
    Ok(())
}

fn validate_unique_non_empty(
    values: &[String],
    field: &'static str,
    allow_empty: bool,
) -> Result<(), PaperError> {
    if !allow_empty && values.is_empty() {
        return Err(PaperError::InvalidField {
            field,
            reason: "must not be empty",
        });
    }
    let mut normalized = BTreeSet::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() || !normalized.insert(trimmed.to_lowercase()) {
            return Err(PaperError::InvalidField {
                field,
                reason: "must contain unique non-empty strings",
            });
        }
    }
    Ok(())
}

fn validate_sha256(value: &str, field: &'static str) -> Result<(), PaperError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(PaperError::InvalidField {
            field,
            reason: "must be a lowercase SHA-256 digest",
        });
    }
    Ok(())
}

fn validate_stable_id(value: &str, field: &'static str) -> Result<(), PaperError> {
    let bytes = value.as_bytes();
    let valid = bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(byte),
        });
    if !valid {
        return Err(PaperError::InvalidField {
            field,
            reason: "must be a canonical lowercase UUID",
        });
    }
    Ok(())
}

fn validate_decimal(value: &str, allow_zero: bool, field: &'static str) -> Result<(), PaperError> {
    let bytes = value.as_bytes();
    if bytes.is_empty() || value.starts_with('+') || value.contains('e') || value.contains('E') {
        return Err(PaperError::InvalidField {
            field,
            reason: "invalid canonical decimal",
        });
    }
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let mut parts = unsigned.split('.');
    let integer = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if parts.next().is_some()
        || integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || (integer.len() > 1 && integer.starts_with('0'))
        || fraction.is_some_and(|part| {
            part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || part.ends_with('0')
        })
        || value == "-0"
        || value.starts_with("-0.")
    {
        return Err(PaperError::InvalidField {
            field,
            reason: "invalid canonical decimal",
        });
    }
    let numeric = value.parse::<f64>().map_err(|_| PaperError::InvalidField {
        field,
        reason: "invalid canonical decimal",
    })?;
    if numeric < 0.0 || (!allow_zero && numeric <= 0.0) {
        return Err(PaperError::InvalidField {
            field,
            reason: "decimal is outside the permitted range",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAPER_ID: &str = "018f0000-0000-7000-8000-000000000001";
    const OWNER_ID: &str = "018f0000-0000-7000-8000-000000000002";
    const QUESTION_ID: &str = "018f0000-0000-7000-8000-000000000003";
    const ITEM_A: &str = "018f0000-0000-7000-8000-000000000004";
    const ITEM_B: &str = "018f0000-0000-7000-8000-000000000005";

    fn question(id: &str) -> QuestionSnapshot {
        QuestionSnapshot {
            id: id.into(),
            version: 1,
            content_hash: "a".repeat(64),
            question_type: QuestionType::ShortAnswer,
            subjects: vec!["Math".into()],
            difficulty: Difficulty::Medium,
            tags: vec![],
            text: "What is 2 + 2?".into(),
            options: None,
            answer: AnswerSnapshot::Text("4".into()),
            has_latex: false,
            source: None,
            essay_blank_space: None,
            score_weight: "1".into(),
            attachments: vec![],
        }
    }

    fn paper() -> PaperSnapshot {
        let question = question(QUESTION_ID);
        PaperSnapshot {
            id: PAPER_ID.into(),
            owner_id: OWNER_ID.into(),
            replication_scope: ReplicationScope::LocalPrivate,
            schema_version: PAPER_SCHEMA_VERSION,
            version: 1,
            content_hash: "b".repeat(64),
            created_at_micros: 1,
            updated_at_micros: 1,
            deleted_at_micros: None,
            title: "Midterm".into(),
            subject: "Math".into(),
            duration_minutes: 60,
            total_marks: "10".into(),
            status: PaperStatus::Draft,
            items: vec![
                PaperItemSnapshot {
                    id: ITEM_A.into(),
                    question_id: Some(QUESTION_ID.into()),
                    order: 0,
                    marks: Some("4".into()),
                    question_snapshot: question.clone(),
                },
                PaperItemSnapshot {
                    id: ITEM_B.into(),
                    question_id: Some(QUESTION_ID.into()),
                    order: 1,
                    marks: Some("6".into()),
                    question_snapshot: question,
                },
            ],
        }
    }

    #[test]
    fn validates_and_reorders_stable_items() {
        let paper = paper();
        paper.validate().unwrap();
        let reordered = paper.reorder(&[ITEM_B.into(), ITEM_A.into()]).unwrap();
        assert_eq!(reordered.items[0].id, ITEM_B);
        assert_eq!(reordered.items[1].id, ITEM_A);
        assert_eq!(reordered.items[0].question_snapshot.text, "What is 2 + 2?");
    }

    #[test]
    fn rejects_partial_reorder_and_noncanonical_decimal() {
        let paper = paper();
        assert_eq!(
            paper.reorder(&[ITEM_A.into()]),
            Err(PaperError::IncompleteReorder)
        );
        assert!(paper
            .update_marks(&[(ITEM_A.into(), Some("01.00".into()))])
            .is_err());
    }

    #[test]
    fn rejects_duplicate_order_and_snapshot_provenance_mismatch() {
        let mut duplicate_order = paper();
        duplicate_order.items[1].order = 0;
        assert_eq!(
            duplicate_order.validate(),
            Err(PaperError::DuplicateOrder(0))
        );

        let mut paper = paper();
        paper.items[0].question_id = Some(OWNER_ID.into());
        assert!(matches!(
            paper.validate(),
            Err(PaperError::QuestionProvenanceMismatch { .. })
        ));
    }
}
