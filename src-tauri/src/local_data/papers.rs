use std::{io, str::FromStr};

use rusqlite::{params, OptionalExtension, Row, Transaction, TransactionBehavior};
use serde::Serialize;
use uuid::Uuid;

use crate::workspace_features::paper::{
    PaperItemSnapshot, PaperSnapshot, PaperSnapshotStore, PaperStatus,
    ReplicationScope as PaperReplicationScope, StoreCandidateError, PAPER_SCHEMA_VERSION,
};

use super::{
    canonical::{canonical_json, domain_content_hash},
    error::{LocalDataError, LocalDataResult},
    migration::{now_micros, validate_canonical_uuid},
    model::{HistoryAction, ReplicationScope},
    questions::{append_history, append_pending, HistoryWrite, PendingWrite},
    LocalDataStore,
};

const PAPER_COLUMNS: &str = "
    p.id, p.owner_id, p.replication_scope, p.schema_version, p.version,
    p.content_hash, p.created_at, p.updated_at, p.deleted_at, p.title, p.subject,
    p.duration_minutes, p.total_marks, p.status, p.items_json";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PaperContent<'a> {
    title: &'a str,
    subject: &'a str,
    duration_minutes: u32,
    total_marks: &'a str,
    status: PaperStatus,
    items: &'a [PaperItemSnapshot],
}

impl LocalDataStore {
    pub(crate) fn create_paper_snapshot(
        &self,
        candidate: &PaperSnapshot,
    ) -> LocalDataResult<PaperSnapshot> {
        validate_paper_candidate(candidate)?;
        if candidate.replication_scope == PaperReplicationScope::LocalPrivate
            && candidate.owner_id != self.local_principal_id
        {
            return Err(LocalDataError::Validation(vec![
                "local-private papers must be owned by the workspace local principal".into(),
            ]));
        }
        if candidate.deleted_at_micros.is_some() {
            return Err(LocalDataError::Validation(vec![
                "a new paper cannot be created as a tombstone".into(),
            ]));
        }
        let now = now_micros();
        let mut accepted = candidate.clone();
        accepted.items.sort_by_key(|item| item.order);
        accepted.version = 1;
        accepted.created_at_micros = now;
        accepted.updated_at_micros = now;
        accepted.deleted_at_micros = None;
        accepted.content_hash = paper_content_hash(&accepted)?;
        validate_paper_candidate(&accepted)?;

        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if paper_exists(&transaction, &accepted.id)? {
            return Err(LocalDataError::Validation(vec![format!(
                "paper {} already exists",
                accepted.id
            )]));
        }
        insert_paper_projection(&transaction, &accepted)?;
        append_paper_history(&transaction, &accepted, HistoryAction::Create)?;
        append_paper_pending(&transaction, &accepted, None, "create")?;
        transaction.commit()?;
        Ok(accepted)
    }

    pub(crate) fn load_paper_snapshot(
        &self,
        paper_id: &str,
    ) -> LocalDataResult<Option<PaperSnapshot>> {
        validate_canonical_uuid(paper_id, "paperId")?;
        load_paper_from_connection(&self.connection(), paper_id)
    }

    pub(crate) fn accept_paper_candidate(
        &self,
        base_version: u64,
        base_content_hash: &str,
        candidate: &PaperSnapshot,
    ) -> LocalDataResult<PaperSnapshot> {
        validate_paper_candidate(candidate)?;
        validate_paper_base(base_version, base_content_hash)?;
        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = load_paper_tx(&transaction, &candidate.id)?.ok_or_else(|| {
            LocalDataError::NotFound {
                entity: "paper",
                id: candidate.id.clone(),
            }
        })?;
        if base_version != current.version || base_content_hash != current.content_hash {
            let stale = preserve_paper_candidate(
                &transaction,
                &current,
                base_version,
                base_content_hash,
                candidate,
            )?;
            transaction.commit()?;
            return Err(stale);
        }
        if current.deleted_at_micros.is_some() {
            return Err(LocalDataError::EntityDeleted {
                entity: "paper",
                id: current.id,
            });
        }
        if candidate.owner_id != current.owner_id
            || candidate.replication_scope != current.replication_scope
            || candidate.schema_version != current.schema_version
            || candidate.created_at_micros != current.created_at_micros
            || candidate.deleted_at_micros != current.deleted_at_micros
        {
            return Err(LocalDataError::Validation(vec![
                "paper candidate cannot change identity, ownership, scope, schema, creation time, or lifecycle"
                    .into(),
            ]));
        }

        let mut accepted = candidate.clone();
        accepted.items.sort_by_key(|item| item.order);
        accepted.content_hash = paper_content_hash(&accepted)?;
        if accepted.content_hash == current.content_hash {
            return Ok(current);
        }
        accepted.version = current
            .version
            .checked_add(1)
            .ok_or_else(|| LocalDataError::Corrupt("paper version overflow".into()))?;
        accepted.updated_at_micros = now_micros();
        accepted.created_at_micros = current.created_at_micros;
        update_paper_projection(&transaction, &accepted)?;
        append_paper_history(&transaction, &accepted, HistoryAction::Update)?;
        let base_version = i64::try_from(current.version)
            .map_err(|_| LocalDataError::Corrupt("paper version exceeds SQLite i64".into()))?;
        append_paper_pending(
            &transaction,
            &accepted,
            Some((&base_version, &current.content_hash)),
            "update",
        )?;
        transaction.commit()?;
        Ok(accepted)
    }
}

impl PaperSnapshotStore for LocalDataStore {
    fn load(&self, paper_id: &str) -> Result<Option<PaperSnapshot>, String> {
        self.load_paper_snapshot(paper_id)
            .map_err(|error| error.to_string())
    }

    fn accept_candidate(
        &self,
        base_version: u64,
        base_content_hash: &str,
        candidate: &PaperSnapshot,
    ) -> Result<PaperSnapshot, StoreCandidateError> {
        match self.accept_paper_candidate(base_version, base_content_hash, candidate) {
            Ok(accepted) => Ok(accepted),
            Err(LocalDataError::StaleBase { .. }) => Err(StoreCandidateError::StaleBase),
            Err(error) => Err(StoreCandidateError::Rejected(error.to_string())),
        }
    }
}

fn insert_paper_projection(
    transaction: &Transaction<'_>,
    paper: &PaperSnapshot,
) -> LocalDataResult<()> {
    transaction.execute(
        "INSERT INTO papers(
            id, owner_id, replication_scope, schema_version, version, content_hash,
            created_at, updated_at, deleted_at, deleted_by_id, title, subject,
            duration_minutes, total_marks, status, items_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, ?9, ?10,
                   ?11, ?12, ?13, ?14)",
        params![
            paper.id,
            paper.owner_id,
            paper_scope_text(paper.replication_scope),
            paper.schema_version,
            i64::try_from(paper.version).map_err(|_| LocalDataError::Validation(vec![
                "paper version is too large".into()
            ]))?,
            paper.content_hash,
            paper.created_at_micros,
            paper.updated_at_micros,
            paper.title,
            paper.subject,
            paper.duration_minutes,
            paper.total_marks,
            paper_status_text(paper.status),
            canonical_json(&paper.items)?,
        ],
    )?;
    insert_paper_items(transaction, paper)
}

fn update_paper_projection(
    transaction: &Transaction<'_>,
    paper: &PaperSnapshot,
) -> LocalDataResult<()> {
    transaction.execute(
        "UPDATE papers SET version = ?2, content_hash = ?3, updated_at = ?4,
                title = ?5, subject = ?6, duration_minutes = ?7, total_marks = ?8,
                status = ?9, items_json = ?10
         WHERE id = ?1",
        params![
            paper.id,
            i64::try_from(paper.version).map_err(|_| LocalDataError::Validation(vec![
                "paper version is too large".into()
            ]))?,
            paper.content_hash,
            paper.updated_at_micros,
            paper.title,
            paper.subject,
            paper.duration_minutes,
            paper.total_marks,
            paper_status_text(paper.status),
            canonical_json(&paper.items)?,
        ],
    )?;
    transaction.execute("DELETE FROM paper_items WHERE paper_id = ?1", [&paper.id])?;
    insert_paper_items(transaction, paper)
}

fn insert_paper_items(transaction: &Transaction<'_>, paper: &PaperSnapshot) -> LocalDataResult<()> {
    let mut statement = transaction.prepare(
        "INSERT INTO paper_items(
            id, paper_id, question_id, item_order, marks, question_snapshot_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for item in &paper.items {
        statement.execute(params![
            item.id,
            paper.id,
            item.question_id,
            item.order,
            item.marks,
            canonical_json(&item.question_snapshot)?,
        ])?;
    }
    Ok(())
}

fn load_paper_from_connection(
    connection: &rusqlite::Connection,
    paper_id: &str,
) -> LocalDataResult<Option<PaperSnapshot>> {
    let row = connection
        .query_row(
            &format!("SELECT {PAPER_COLUMNS} FROM papers p WHERE p.id = ?1"),
            [paper_id],
            read_paper_row_without_items,
        )
        .optional()?;
    row.map(|(paper, items_json)| hydrate_paper(connection, paper, items_json))
        .transpose()
}

fn load_paper_tx(
    transaction: &Transaction<'_>,
    paper_id: &str,
) -> LocalDataResult<Option<PaperSnapshot>> {
    let row = transaction
        .query_row(
            &format!("SELECT {PAPER_COLUMNS} FROM papers p WHERE p.id = ?1"),
            [paper_id],
            read_paper_row_without_items,
        )
        .optional()?;
    row.map(|(paper, items_json)| hydrate_paper(transaction, paper, items_json))
        .transpose()
}

fn hydrate_paper(
    connection: &rusqlite::Connection,
    mut paper: PaperSnapshot,
    items_json: String,
) -> LocalDataResult<PaperSnapshot> {
    let canonical_items: Vec<PaperItemSnapshot> = serde_json::from_str(&items_json)?;
    let mut statement = connection.prepare(
        "SELECT id, question_id, item_order, marks, question_snapshot_json
         FROM paper_items WHERE paper_id = ?1 ORDER BY item_order, id",
    )?;
    let rows = statement.query_map([&paper.id], |row| {
        let snapshot: String = row.get(4)?;
        Ok(PaperItemSnapshot {
            id: row.get(0)?,
            question_id: row.get(1)?,
            order: row.get(2)?,
            marks: row.get(3)?,
            question_snapshot: serde_json::from_str(&snapshot).map_err(sql_decode_error)?,
        })
    })?;
    paper.items = rows.collect::<Result<Vec<_>, _>>()?;
    if paper.items != canonical_items {
        return Err(LocalDataError::Corrupt(format!(
            "paper {} item projection differs from canonical items JSON",
            paper.id
        )));
    }
    validate_paper_candidate(&paper)?;
    if paper_content_hash(&paper)? != paper.content_hash {
        return Err(LocalDataError::Corrupt(format!(
            "paper {} content hash does not match its canonical content",
            paper.id
        )));
    }
    Ok(paper)
}

fn read_paper_row_without_items(row: &Row<'_>) -> rusqlite::Result<(PaperSnapshot, String)> {
    let scope: String = row.get(2)?;
    let status: String = row.get(13)?;
    let version: i64 = row.get(4)?;
    Ok((
        PaperSnapshot {
            id: row.get(0)?,
            owner_id: row.get(1)?,
            replication_scope: parse_paper_scope(&scope).map_err(sql_decode_error)?,
            schema_version: row.get(3)?,
            version: u64::try_from(version).map_err(sql_decode_error)?,
            content_hash: row.get(5)?,
            created_at_micros: row.get(6)?,
            updated_at_micros: row.get(7)?,
            deleted_at_micros: row.get(8)?,
            title: row.get(9)?,
            subject: row.get(10)?,
            duration_minutes: row.get(11)?,
            total_marks: row.get(12)?,
            status: parse_paper_status(&status).map_err(sql_decode_error)?,
            items: Vec::new(),
        },
        row.get(14)?,
    ))
}

fn paper_content_hash(paper: &PaperSnapshot) -> LocalDataResult<String> {
    domain_content_hash(
        "paper",
        PAPER_SCHEMA_VERSION,
        &PaperContent {
            title: &paper.title,
            subject: &paper.subject,
            duration_minutes: paper.duration_minutes,
            total_marks: &paper.total_marks,
            status: paper.status,
            items: &paper.items,
        },
    )
}

fn append_paper_history(
    transaction: &Transaction<'_>,
    paper: &PaperSnapshot,
    action: HistoryAction,
) -> LocalDataResult<()> {
    append_history(
        transaction,
        HistoryWrite {
            entity_type: "paper",
            entity_id: &paper.id,
            version: i64::try_from(paper.version)
                .map_err(|_| LocalDataError::Corrupt("paper version exceeds SQLite i64".into()))?,
            hash: &paper.content_hash,
            action,
            snapshot: paper,
            created_at: paper.updated_at_micros,
        },
    )
}

fn append_paper_pending(
    transaction: &Transaction<'_>,
    paper: &PaperSnapshot,
    base: Option<(&i64, &String)>,
    mutation_kind: &str,
) -> LocalDataResult<()> {
    append_pending(
        transaction,
        PendingWrite {
            scope: local_scope(paper.replication_scope),
            entity_type: "paper",
            entity_id: &paper.id,
            base,
            mutation_kind,
            candidate: paper,
            created_at: paper.updated_at_micros,
        },
    )
}

fn preserve_paper_candidate(
    transaction: &Transaction<'_>,
    current: &PaperSnapshot,
    base_version: u64,
    base_hash: &str,
    candidate: &PaperSnapshot,
) -> LocalDataResult<LocalDataError> {
    let candidate_id = Uuid::now_v7().to_string();
    transaction.execute(
        "INSERT INTO conflict_candidates(
            candidate_id, entity_type, entity_id, requested_base_version,
            requested_base_hash, current_version, current_hash, requested_action,
            candidate_json, created_at
         ) VALUES (?1, 'paper', ?2, ?3, ?4, ?5, ?6, 'update', ?7, ?8)",
        params![
            candidate_id,
            current.id,
            i64::try_from(base_version)
                .map_err(|_| LocalDataError::Validation(vec!["baseVersion is too large".into()]))?,
            base_hash,
            i64::try_from(current.version)
                .map_err(|_| LocalDataError::Corrupt("paper version exceeds SQLite i64".into()))?,
            current.content_hash,
            canonical_json(candidate)?,
            now_micros(),
        ],
    )?;
    Ok(LocalDataError::StaleBase {
        current_version: i64::try_from(current.version)
            .map_err(|_| LocalDataError::Corrupt("paper version exceeds SQLite i64".into()))?,
        current_content_hash: current.content_hash.clone(),
        candidate_id,
    })
}

fn validate_paper_candidate(paper: &PaperSnapshot) -> LocalDataResult<()> {
    paper
        .validate()
        .map_err(|error| LocalDataError::Validation(vec![error.to_string()]))?;
    if paper.duration_minutes > i32::MAX as u32
        || paper.items.iter().any(|item| item.order > i32::MAX as u32)
    {
        return Err(LocalDataError::Validation(vec![
            "paper duration or item order exceeds the supported range".into(),
        ]));
    }
    validate_bounded_decimal(&paper.total_marks, 12, 2, 1, "paper.totalMarks")?;
    for item in &paper.items {
        if let Some(marks) = &item.marks {
            validate_bounded_decimal(marks, 10, 2, 0, "paperItem.marks")?;
        }
        validate_bounded_decimal(
            &item.question_snapshot.score_weight,
            8,
            4,
            1,
            "questionSnapshot.scoreWeight",
        )?;
    }
    Ok(())
}

fn validate_paper_base(version: u64, content_hash: &str) -> LocalDataResult<()> {
    if version == 0
        || version > i64::MAX as u64
        || content_hash.len() != 64
        || !content_hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(LocalDataError::Validation(vec![
            "paper mutation base must contain a supported version and lowercase SHA-256 hash"
                .into(),
        ]));
    }
    Ok(())
}

fn validate_bounded_decimal(
    value: &str,
    precision: usize,
    scale: usize,
    minimum_scaled: u128,
    field: &str,
) -> LocalDataResult<()> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.starts_with('-')
        || whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > scale
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(LocalDataError::Validation(vec![format!(
            "{field} exceeds its decimal precision or scale"
        )]));
    }
    let multiplier = 10_u128.pow(scale as u32);
    let mut padded_fraction = fraction.to_owned();
    while padded_fraction.len() < scale {
        padded_fraction.push('0');
    }
    let scaled = whole
        .parse::<u128>()
        .ok()
        .and_then(|whole| whole.checked_mul(multiplier))
        .and_then(|whole| {
            padded_fraction
                .parse::<u128>()
                .ok()
                .and_then(|fraction| whole.checked_add(fraction))
        });
    let maximum_scaled = 10_u128.pow(precision as u32).saturating_sub(1);
    if !scaled.is_some_and(|value| (minimum_scaled..=maximum_scaled).contains(&value)) {
        return Err(LocalDataError::Validation(vec![format!(
            "{field} is outside its permitted range"
        )]));
    }
    Ok(())
}

fn paper_exists(transaction: &Transaction<'_>, paper_id: &str) -> LocalDataResult<bool> {
    Ok(transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM papers WHERE id = ?1)",
        [paper_id],
        |row| row.get(0),
    )?)
}

fn paper_scope_text(scope: PaperReplicationScope) -> &'static str {
    match scope {
        PaperReplicationScope::LocalPrivate => "local_private",
        PaperReplicationScope::CloudSynced => "cloud_synced",
        PaperReplicationScope::CollaborativeShared => "collaborative_shared",
    }
}

fn parse_paper_scope(value: &str) -> LocalDataResult<PaperReplicationScope> {
    match value {
        "local_private" => Ok(PaperReplicationScope::LocalPrivate),
        "cloud_synced" => Ok(PaperReplicationScope::CloudSynced),
        "collaborative_shared" => Ok(PaperReplicationScope::CollaborativeShared),
        _ => Err(LocalDataError::Corrupt(format!(
            "unknown paper replication scope {value:?}"
        ))),
    }
}

fn local_scope(scope: PaperReplicationScope) -> ReplicationScope {
    ReplicationScope::from_str(paper_scope_text(scope)).expect("paper scopes match local scopes")
}

fn paper_status_text(status: PaperStatus) -> &'static str {
    match status {
        PaperStatus::Draft => "draft",
        PaperStatus::Published => "published",
        PaperStatus::Archived => "archived",
    }
}

fn parse_paper_status(value: &str) -> LocalDataResult<PaperStatus> {
    match value {
        "draft" => Ok(PaperStatus::Draft),
        "published" => Ok(PaperStatus::Published),
        "archived" => Ok(PaperStatus::Archived),
        _ => Err(LocalDataError::Corrupt(format!(
            "unknown paper status {value:?}"
        ))),
    }
}

fn sql_decode_error(error: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(io::Error::new(
            io::ErrorKind::InvalidData,
            error.to_string(),
        )),
    )
}
