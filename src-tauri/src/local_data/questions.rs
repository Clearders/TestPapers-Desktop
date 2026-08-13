use std::{io, str::FromStr};

use rusqlite::{
    params, params_from_iter,
    types::{Type, Value as SqlValue},
    OptionalExtension, Row, Transaction, TransactionBehavior,
};
use serde_json::{json, Value};
use uuid::Uuid;

use super::{
    canonical::{canonical_json, domain_content_hash},
    error::{LocalDataError, LocalDataResult},
    migration::{now_micros, validate_canonical_uuid},
    model::{
        CreateQuestion, DeletedFilter, Difficulty, HistoryAction, MutationBase, PendingMutation,
        QuestionContent, QuestionRecord, QuestionRevision, QuestionSearch, QuestionSearchPage,
        QuestionType, ReplicationScope, SyncQueueState, UpdateQuestion, ENTITY_SCHEMA_VERSION,
    },
    LocalDataStore,
};

const QUESTION_COLUMNS: &str = "
    q.id, q.owner_id, q.replication_scope, q.schema_version, q.version,
    q.content_hash, q.created_at, q.updated_at, q.deleted_at, q.deleted_by_id,
    q.type, q.subjects_json, q.difficulty, q.tags_json, q.text, q.options_json,
    q.answer_json, q.has_latex, q.source, q.essay_blank_space_json, q.score_weight";

impl LocalDataStore {
    pub(crate) fn create_question(
        &self,
        request: CreateQuestion,
    ) -> LocalDataResult<QuestionRecord> {
        let content = request.content.normalize()?;
        let owner_id = request
            .owner_id
            .unwrap_or_else(|| self.local_principal_id.clone());
        validate_canonical_uuid(&owner_id, "ownerId")?;
        if request.replication_scope == ReplicationScope::LocalPrivate
            && owner_id != self.local_principal_id
        {
            return Err(LocalDataError::Validation(vec![
                "local-private questions must be owned by the workspace local principal".into(),
            ]));
        }

        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let record = insert_question(
            &transaction,
            &owner_id,
            request.replication_scope,
            content,
            None,
        )?;
        transaction.commit()?;
        Ok(record)
    }

    pub(crate) fn get_question(&self, question_id: &str) -> LocalDataResult<QuestionRecord> {
        validate_canonical_uuid(question_id, "questionId")?;
        get_question_from_connection(&self.connection(), question_id)
    }

    pub(crate) fn update_question(
        &self,
        question_id: &str,
        request: UpdateQuestion,
    ) -> LocalDataResult<QuestionRecord> {
        validate_canonical_uuid(question_id, "questionId")?;
        request.mutation_base.validate()?;
        let content = request.content.normalize()?;
        let candidate = serde_json::to_value(&content)?;
        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = get_question_tx(&transaction, question_id)?;

        if !request.mutation_base.matches(&current) {
            let stale = preserve_stale_candidate(
                &transaction,
                &current,
                &request.mutation_base,
                "update",
                &candidate,
            )?;
            transaction.commit()?;
            return Err(stale);
        }
        if current.deleted_at.is_some() {
            return Err(LocalDataError::EntityDeleted {
                entity: "question",
                id: question_id.into(),
            });
        }

        let next_hash = question_content_hash(&content)?;
        if next_hash == current.content_hash {
            return Ok(current);
        }
        let next = QuestionRecord {
            version: checked_next_version(current.version)?,
            content_hash: next_hash,
            updated_at: now_micros(),
            content,
            ..current.clone()
        };
        update_question_projection(&transaction, &next)?;
        append_question_history(&transaction, &next, HistoryAction::Update)?;
        append_pending_mutation(
            &transaction,
            &next,
            Some((&current.version, &current.content_hash)),
            "update",
        )?;
        transaction.commit()?;
        Ok(next)
    }

    pub(crate) fn delete_question(
        &self,
        question_id: &str,
        mutation_base: MutationBase,
        actor_id: &str,
    ) -> LocalDataResult<QuestionRecord> {
        validate_canonical_uuid(question_id, "questionId")?;
        validate_canonical_uuid(actor_id, "actorId")?;
        mutation_base.validate()?;
        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = get_question_tx(&transaction, question_id)?;
        if !mutation_base.matches(&current) {
            let stale = preserve_stale_candidate(
                &transaction,
                &current,
                &mutation_base,
                "delete",
                &json!({"requestedAction": "delete", "actorId": actor_id}),
            )?;
            transaction.commit()?;
            return Err(stale);
        }
        if current.deleted_at.is_some() {
            return Ok(current);
        }

        let deleted_at = now_micros();
        let next = QuestionRecord {
            version: checked_next_version(current.version)?,
            updated_at: deleted_at,
            deleted_at: Some(deleted_at),
            deleted_by_id: Some(actor_id.into()),
            ..current.clone()
        };
        update_question_lifecycle(&transaction, &next)?;
        super::attachments::tombstone_question_attachments(
            &transaction,
            question_id,
            actor_id,
            deleted_at,
        )?;
        append_question_history(&transaction, &next, HistoryAction::Delete)?;
        append_pending_mutation(
            &transaction,
            &next,
            Some((&current.version, &current.content_hash)),
            "delete",
        )?;
        transaction.commit()?;
        Ok(next)
    }

    pub(crate) fn restore_question(
        &self,
        question_id: &str,
        mutation_base: MutationBase,
    ) -> LocalDataResult<QuestionRecord> {
        validate_canonical_uuid(question_id, "questionId")?;
        mutation_base.validate()?;
        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = get_question_tx(&transaction, question_id)?;
        if !mutation_base.matches(&current) {
            let stale = preserve_stale_candidate(
                &transaction,
                &current,
                &mutation_base,
                "restore",
                &json!({"requestedAction": "restore"}),
            )?;
            transaction.commit()?;
            return Err(stale);
        }
        if current.deleted_at.is_none() {
            return Ok(current);
        }

        let next = QuestionRecord {
            version: checked_next_version(current.version)?,
            updated_at: now_micros(),
            deleted_at: None,
            deleted_by_id: None,
            ..current.clone()
        };
        update_question_lifecycle(&transaction, &next)?;
        append_question_history(&transaction, &next, HistoryAction::Restore)?;
        append_pending_mutation(
            &transaction,
            &next,
            Some((&current.version, &current.content_hash)),
            "restore",
        )?;
        transaction.commit()?;
        Ok(next)
    }

    pub(crate) fn revert_question(
        &self,
        question_id: &str,
        revision_version: i64,
        mutation_base: MutationBase,
    ) -> LocalDataResult<QuestionRecord> {
        validate_canonical_uuid(question_id, "questionId")?;
        mutation_base.validate()?;
        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = get_question_tx(&transaction, question_id)?;
        if !mutation_base.matches(&current) {
            let stale = preserve_stale_candidate(
                &transaction,
                &current,
                &mutation_base,
                "revert",
                &json!({"requestedAction": "revert", "revisionVersion": revision_version}),
            )?;
            transaction.commit()?;
            return Err(stale);
        }
        if current.deleted_at.is_some() {
            return Err(LocalDataError::EntityDeleted {
                entity: "question",
                id: question_id.into(),
            });
        }

        let snapshot_json: String = transaction
            .query_row(
                "SELECT snapshot_json FROM entity_history
                 WHERE entity_type = 'question' AND entity_id = ?1 AND version = ?2",
                params![question_id, revision_version],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| LocalDataError::NotFound {
                entity: "question revision",
                id: format!("{question_id}@{revision_version}"),
            })?;
        let revision: QuestionRecord = serde_json::from_str(&snapshot_json)?;
        let content = revision.content.normalize()?;
        let next_hash = question_content_hash(&content)?;
        if next_hash == current.content_hash {
            return Ok(current);
        }

        let next = QuestionRecord {
            version: checked_next_version(current.version)?,
            content_hash: next_hash,
            updated_at: now_micros(),
            content,
            ..current.clone()
        };
        update_question_projection(&transaction, &next)?;
        append_question_history(&transaction, &next, HistoryAction::Revert)?;
        append_pending_mutation(
            &transaction,
            &next,
            Some((&current.version, &current.content_hash)),
            "revert",
        )?;
        transaction.commit()?;
        Ok(next)
    }

    pub(crate) fn list_question_revisions(
        &self,
        question_id: &str,
    ) -> LocalDataResult<Vec<QuestionRevision>> {
        validate_canonical_uuid(question_id, "questionId")?;
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT version, content_hash, action, created_at, snapshot_json
             FROM entity_history
             WHERE entity_type = 'question' AND entity_id = ?1
             ORDER BY version DESC",
        )?;
        let rows = statement.query_map([question_id], |row| {
            let action: String = row.get(2)?;
            let snapshot: String = row.get(4)?;
            Ok(QuestionRevision {
                version: row.get(0)?,
                content_hash: row.get(1)?,
                action: parse_history_action(&action).map_err(sql_decode_error)?,
                created_at: row.get(3)?,
                snapshot: serde_json::from_str(&snapshot).map_err(sql_decode_error)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub(crate) fn search_questions(
        &self,
        request: QuestionSearch,
    ) -> LocalDataResult<QuestionSearchPage> {
        let page_size = request.page_size.unwrap_or(50).clamp(1, 100) as usize;
        let offset = decode_cursor(request.cursor.as_deref())?;
        let query = request
            .query
            .as_deref()
            .map(str::trim)
            .filter(|query| !query.is_empty());

        let mut sql = format!("SELECT {QUESTION_COLUMNS} FROM questions q");
        let mut values = Vec::<SqlValue>::new();
        let mut predicates = Vec::<String>::new();
        if let Some(query) = query {
            sql.push_str(" JOIN questions_fts f ON f.question_id = q.id");
            predicates.push("questions_fts MATCH ?".into());
            values.push(SqlValue::Text(fts_query(query)));
        }
        match request.deleted {
            DeletedFilter::Exclude => predicates.push("q.deleted_at IS NULL".into()),
            DeletedFilter::Include => {}
            DeletedFilter::Only => predicates.push("q.deleted_at IS NOT NULL".into()),
        }
        push_text_filter(
            &mut predicates,
            &mut values,
            "q.type",
            request.types.into_iter().map(|value| value.to_string()),
        );
        push_text_filter(
            &mut predicates,
            &mut values,
            "q.difficulty",
            request
                .difficulties
                .into_iter()
                .map(|value| value.to_string()),
        );
        push_relation_filter(
            &mut predicates,
            &mut values,
            "question_subjects",
            request.subjects,
        );
        push_relation_filter(
            &mut predicates,
            &mut values,
            "question_tags",
            request.tags.into_iter().map(|tag| tag.to_lowercase()),
        );

        if !predicates.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&predicates.join(" AND "));
        }
        if query.is_some() {
            sql.push_str(" ORDER BY bm25(questions_fts), q.updated_at DESC, q.id DESC");
        } else {
            sql.push_str(" ORDER BY q.updated_at DESC, q.id DESC");
        }
        sql.push_str(" LIMIT ? OFFSET ?");
        values.push(SqlValue::Integer((page_size + 1) as i64));
        values.push(SqlValue::Integer(i64::try_from(offset).map_err(|_| {
            LocalDataError::Validation(vec!["question search cursor is too large".into()])
        })?));

        let connection = self.connection();
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), read_question_row)?;
        let mut items = rows.collect::<Result<Vec<_>, _>>()?;
        let has_more = items.len() > page_size;
        items.truncate(page_size);
        Ok(QuestionSearchPage {
            items,
            next_cursor: if has_more {
                Some(encode_cursor(offset.checked_add(page_size).ok_or_else(
                    || {
                        LocalDataError::Validation(vec![
                            "question search cursor is too large".into()
                        ])
                    },
                )?))
            } else {
                None
            },
        })
    }

    pub(crate) fn list_pending_mutations(
        &self,
        limit: u32,
    ) -> LocalDataResult<Vec<PendingMutation>> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT operation_id, entity_type, entity_id, base_version, base_content_hash,
                    mutation_kind, candidate_json, created_at, account_id, device_id, batch_id,
                    batch_ordinal, queue_state, dependencies_json, attempt_count, next_attempt_at,
                    last_attempt_at, last_error_code, request_hash, stored_response_json, updated_at
             FROM pending_mutations
             ORDER BY created_at, operation_id LIMIT ?1",
        )?;
        let rows = statement.query_map([limit.clamp(1, 500)], |row| {
            let candidate: String = row.get(6)?;
            let queue_state: String = row.get(12)?;
            let dependencies: String = row.get(13)?;
            let stored_response: Option<String> = row.get(19)?;
            Ok(PendingMutation {
                operation_id: row.get(0)?,
                entity_type: row.get(1)?,
                entity_id: row.get(2)?,
                base_version: row.get(3)?,
                base_content_hash: row.get(4)?,
                mutation_kind: row.get(5)?,
                candidate: serde_json::from_str(&candidate).map_err(sql_decode_error)?,
                created_at: row.get(7)?,
                account_id: row.get(8)?,
                device_id: row.get(9)?,
                batch_id: row.get(10)?,
                batch_ordinal: row.get(11)?,
                queue_state: SyncQueueState::from_str(&queue_state).map_err(sql_decode_error)?,
                dependencies: serde_json::from_str(&dependencies).map_err(sql_decode_error)?,
                attempt_count: row.get(14)?,
                next_attempt_at: row.get(15)?,
                last_attempt_at: row.get(16)?,
                last_error_code: row.get(17)?,
                request_hash: row.get(18)?,
                stored_response: stored_response
                    .map(|value| serde_json::from_str(&value))
                    .transpose()
                    .map_err(sql_decode_error)?,
                updated_at: row.get(20)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

pub(super) fn insert_question(
    transaction: &Transaction<'_>,
    owner_id: &str,
    replication_scope: ReplicationScope,
    content: QuestionContent,
    id: Option<String>,
) -> LocalDataResult<QuestionRecord> {
    validate_canonical_uuid(owner_id, "ownerId")?;
    let now = now_micros();
    let record = QuestionRecord {
        id: id.unwrap_or_else(|| Uuid::now_v7().to_string()),
        owner_id: owner_id.into(),
        replication_scope,
        schema_version: ENTITY_SCHEMA_VERSION,
        version: 1,
        content_hash: question_content_hash(&content)?,
        created_at: now,
        updated_at: now,
        deleted_at: None,
        deleted_by_id: None,
        content,
    };
    let fields = question_fields(&record)?;
    transaction.execute(
        "INSERT INTO questions(
            id, owner_id, replication_scope, schema_version, version, content_hash,
            created_at, updated_at, deleted_at, deleted_by_id, type, subjects_json,
            difficulty, tags_json, text, options_json, answer_json, has_latex,
            source, essay_blank_space_json, score_weight
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, ?9, ?10, ?11, ?12,
            ?13, ?14, ?15, ?16, ?17, ?18, ?19
         )",
        params![
            record.id,
            record.owner_id,
            record.replication_scope.to_string(),
            record.schema_version,
            record.version,
            record.content_hash,
            record.created_at,
            record.updated_at,
            fields.question_type,
            fields.subjects,
            fields.difficulty,
            fields.tags,
            fields.text,
            fields.options,
            fields.answer,
            fields.has_latex,
            fields.source,
            fields.essay_blank_space,
            fields.score_weight,
        ],
    )?;
    append_question_history(transaction, &record, HistoryAction::Create)?;
    append_pending_mutation(transaction, &record, None, "create")?;
    Ok(record)
}

fn update_question_projection(
    transaction: &Transaction<'_>,
    record: &QuestionRecord,
) -> LocalDataResult<()> {
    let fields = question_fields(record)?;
    transaction.execute(
        "UPDATE questions SET
            version = ?2, content_hash = ?3, updated_at = ?4, type = ?5,
            subjects_json = ?6, difficulty = ?7, tags_json = ?8, text = ?9,
            options_json = ?10, answer_json = ?11, has_latex = ?12, source = ?13,
            essay_blank_space_json = ?14, score_weight = ?15
         WHERE id = ?1",
        params![
            record.id,
            record.version,
            record.content_hash,
            record.updated_at,
            fields.question_type,
            fields.subjects,
            fields.difficulty,
            fields.tags,
            fields.text,
            fields.options,
            fields.answer,
            fields.has_latex,
            fields.source,
            fields.essay_blank_space,
            fields.score_weight,
        ],
    )?;
    Ok(())
}

fn update_question_lifecycle(
    transaction: &Transaction<'_>,
    record: &QuestionRecord,
) -> LocalDataResult<()> {
    transaction.execute(
        "UPDATE questions SET version = ?2, updated_at = ?3, deleted_at = ?4,
                deleted_by_id = ?5
         WHERE id = ?1",
        params![
            record.id,
            record.version,
            record.updated_at,
            record.deleted_at,
            record.deleted_by_id,
        ],
    )?;
    Ok(())
}

fn get_question_from_connection(
    connection: &rusqlite::Connection,
    question_id: &str,
) -> LocalDataResult<QuestionRecord> {
    connection
        .query_row(
            &format!("SELECT {QUESTION_COLUMNS} FROM questions q WHERE q.id = ?1"),
            [question_id],
            read_question_row,
        )
        .optional()?
        .ok_or_else(|| LocalDataError::NotFound {
            entity: "question",
            id: question_id.into(),
        })
}

fn get_question_tx(
    transaction: &Transaction<'_>,
    question_id: &str,
) -> LocalDataResult<QuestionRecord> {
    transaction
        .query_row(
            &format!("SELECT {QUESTION_COLUMNS} FROM questions q WHERE q.id = ?1"),
            [question_id],
            read_question_row,
        )
        .optional()?
        .ok_or_else(|| LocalDataError::NotFound {
            entity: "question",
            id: question_id.into(),
        })
}

fn read_question_row(row: &Row<'_>) -> rusqlite::Result<QuestionRecord> {
    let scope: String = row.get(2)?;
    let question_type: String = row.get(10)?;
    let subjects: String = row.get(11)?;
    let difficulty: String = row.get(12)?;
    let tags: String = row.get(13)?;
    let options: Option<String> = row.get(15)?;
    let answer: String = row.get(16)?;
    let essay_blank_space: Option<String> = row.get(19)?;
    Ok(QuestionRecord {
        id: row.get(0)?,
        owner_id: row.get(1)?,
        replication_scope: ReplicationScope::from_str(&scope).map_err(sql_decode_error)?,
        schema_version: row.get(3)?,
        version: row.get(4)?,
        content_hash: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        deleted_at: row.get(8)?,
        deleted_by_id: row.get(9)?,
        content: QuestionContent {
            question_type: QuestionType::from_str(&question_type).map_err(sql_decode_error)?,
            subjects: serde_json::from_str(&subjects).map_err(sql_decode_error)?,
            difficulty: Difficulty::from_str(&difficulty).map_err(sql_decode_error)?,
            tags: serde_json::from_str(&tags).map_err(sql_decode_error)?,
            text: row.get(14)?,
            options: options
                .map(|value| serde_json::from_str(&value))
                .transpose()
                .map_err(sql_decode_error)?,
            answer: serde_json::from_str(&answer).map_err(sql_decode_error)?,
            has_latex: row.get::<_, i64>(17)? != 0,
            source: row.get(18)?,
            essay_blank_space: essay_blank_space
                .map(|value| serde_json::from_str(&value))
                .transpose()
                .map_err(sql_decode_error)?,
            score_weight: row.get(20)?,
        },
    })
}

struct QuestionFields {
    question_type: String,
    subjects: String,
    difficulty: String,
    tags: String,
    text: String,
    options: Option<String>,
    answer: String,
    has_latex: i64,
    source: Option<String>,
    essay_blank_space: Option<String>,
    score_weight: String,
}

fn question_fields(record: &QuestionRecord) -> LocalDataResult<QuestionFields> {
    Ok(QuestionFields {
        question_type: record.content.question_type.to_string(),
        subjects: canonical_json(&record.content.subjects)?,
        difficulty: record.content.difficulty.to_string(),
        tags: canonical_json(&record.content.tags)?,
        text: record.content.text.clone(),
        options: record
            .content
            .options
            .as_ref()
            .map(canonical_json)
            .transpose()?,
        answer: canonical_json(&record.content.answer)?,
        has_latex: i64::from(record.content.has_latex),
        source: record.content.source.clone(),
        essay_blank_space: record
            .content
            .essay_blank_space
            .as_ref()
            .map(canonical_json)
            .transpose()?,
        score_weight: record.content.score_weight.clone(),
    })
}

fn question_content_hash(content: &QuestionContent) -> LocalDataResult<String> {
    domain_content_hash("question", ENTITY_SCHEMA_VERSION, content)
}

pub(super) struct HistoryWrite<'a, T: serde::Serialize> {
    pub(super) entity_type: &'a str,
    pub(super) entity_id: &'a str,
    pub(super) version: i64,
    pub(super) hash: &'a str,
    pub(super) action: HistoryAction,
    pub(super) snapshot: &'a T,
    pub(super) created_at: i64,
}

pub(super) fn append_history<T: serde::Serialize>(
    transaction: &Transaction<'_>,
    entry: HistoryWrite<'_, T>,
) -> LocalDataResult<()> {
    transaction.execute(
        "INSERT INTO entity_history(
            entity_type, entity_id, version, content_hash, action, snapshot_json, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            entry.entity_type,
            entry.entity_id,
            entry.version,
            entry.hash,
            entry.action.to_string(),
            canonical_json(entry.snapshot)?,
            entry.created_at,
        ],
    )?;
    Ok(())
}

fn append_question_history(
    transaction: &Transaction<'_>,
    record: &QuestionRecord,
    action: HistoryAction,
) -> LocalDataResult<()> {
    append_history(
        transaction,
        HistoryWrite {
            entity_type: "question",
            entity_id: &record.id,
            version: record.version,
            hash: &record.content_hash,
            action,
            snapshot: record,
            created_at: record.updated_at,
        },
    )
}

pub(super) struct PendingWrite<'a, T: serde::Serialize> {
    pub(super) scope: ReplicationScope,
    pub(super) entity_type: &'a str,
    pub(super) entity_id: &'a str,
    pub(super) base: Option<(&'a i64, &'a String)>,
    pub(super) mutation_kind: &'a str,
    pub(super) candidate: &'a T,
    pub(super) created_at: i64,
}

pub(super) fn append_pending<T: serde::Serialize>(
    transaction: &Transaction<'_>,
    entry: PendingWrite<'_, T>,
) -> LocalDataResult<()> {
    if !entry.scope.creates_pending_mutation() {
        return Ok(());
    }
    let (base_version, base_hash) = entry
        .base
        .map(|(version, hash)| (Some(*version), Some(hash.as_str())))
        .unwrap_or((None, None));
    transaction.execute(
        "INSERT INTO pending_mutations(
            operation_id, entity_type, entity_id, base_version, base_content_hash,
            mutation_kind, candidate_json, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            Uuid::now_v7().to_string(),
            entry.entity_type,
            entry.entity_id,
            base_version,
            base_hash,
            entry.mutation_kind,
            canonical_json(entry.candidate)?,
            entry.created_at,
        ],
    )?;
    Ok(())
}

fn append_pending_mutation(
    transaction: &Transaction<'_>,
    record: &QuestionRecord,
    base: Option<(&i64, &String)>,
    mutation_kind: &str,
) -> LocalDataResult<()> {
    append_pending(
        transaction,
        PendingWrite {
            scope: record.replication_scope,
            entity_type: "question",
            entity_id: &record.id,
            base,
            mutation_kind,
            candidate: record,
            created_at: record.updated_at,
        },
    )
}

fn preserve_stale_candidate(
    transaction: &Transaction<'_>,
    current: &QuestionRecord,
    mutation_base: &MutationBase,
    action: &str,
    candidate: &Value,
) -> LocalDataResult<LocalDataError> {
    let candidate_id = Uuid::now_v7().to_string();
    transaction.execute(
        "INSERT INTO conflict_candidates(
            candidate_id, entity_type, entity_id, requested_base_version,
            requested_base_hash, current_version, current_hash, requested_action,
            candidate_json, created_at
         ) VALUES (?1, 'question', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            candidate_id,
            current.id,
            mutation_base.base_version,
            mutation_base.base_content_hash,
            current.version,
            current.content_hash,
            action,
            canonical_json(candidate)?,
            now_micros(),
        ],
    )?;
    Ok(LocalDataError::StaleBase {
        current_version: current.version,
        current_content_hash: current.content_hash.clone(),
        candidate_id,
    })
}

fn checked_next_version(version: i64) -> LocalDataResult<i64> {
    version
        .checked_add(1)
        .ok_or_else(|| LocalDataError::Corrupt("entity version overflow".into()))
}

fn parse_history_action(value: &str) -> LocalDataResult<HistoryAction> {
    match value {
        "create" => Ok(HistoryAction::Create),
        "update" => Ok(HistoryAction::Update),
        "delete" => Ok(HistoryAction::Delete),
        "restore" => Ok(HistoryAction::Restore),
        "revert" => Ok(HistoryAction::Revert),
        _ => Err(LocalDataError::Corrupt(format!(
            "unknown history action {value:?}"
        ))),
    }
}

fn push_text_filter<I>(
    predicates: &mut Vec<String>,
    values: &mut Vec<SqlValue>,
    column: &str,
    incoming: I,
) where
    I: IntoIterator<Item = String>,
{
    let incoming: Vec<_> = incoming.into_iter().collect();
    if incoming.is_empty() {
        return;
    }
    predicates.push(format!(
        "{column} IN ({})",
        vec!["?"; incoming.len()].join(", ")
    ));
    values.extend(incoming.into_iter().map(SqlValue::Text));
}

fn push_relation_filter<I>(
    predicates: &mut Vec<String>,
    values: &mut Vec<SqlValue>,
    table: &str,
    incoming: I,
) where
    I: IntoIterator<Item = String>,
{
    let incoming: Vec<_> = incoming
        .into_iter()
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
        .collect();
    if incoming.is_empty() {
        return;
    }
    predicates.push(format!(
        "EXISTS (SELECT 1 FROM {table} filter_values
         WHERE filter_values.question_id = q.id
           AND filter_values.value IN ({}))",
        vec!["?"; incoming.len()].join(", ")
    ));
    values.extend(incoming.into_iter().map(SqlValue::Text));
}

fn fts_query(query: &str) -> String {
    format!("\"{}\"", query.replace('"', "\"\""))
}

fn encode_cursor(offset: usize) -> String {
    format!("tpq1.{offset}")
}

fn decode_cursor(cursor: Option<&str>) -> LocalDataResult<usize> {
    let Some(cursor) = cursor else {
        return Ok(0);
    };
    cursor
        .strip_prefix("tpq1.")
        .and_then(|offset| offset.parse::<usize>().ok())
        .ok_or_else(|| LocalDataError::Validation(vec!["invalid question search cursor".into()]))
}

fn sql_decode_error(error: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        Type::Text,
        Box::new(io::Error::new(
            io::ErrorKind::InvalidData,
            error.to_string(),
        )),
    )
}
