use std::collections::{HashMap, HashSet};

use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    canonical::canonical_json,
    error::{LocalDataError, LocalDataResult},
    migration::{now_micros, validate_canonical_uuid},
    LocalDataStore,
};

const MAX_PUSH_BATCH: u32 = 100;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PreparedSyncOperation {
    pub(crate) operation_id: String,
    pub(crate) entity_type: String,
    pub(crate) entity_id: String,
    pub(crate) kind: String,
    pub(crate) base_version: Option<i64>,
    pub(crate) base_content_hash: Option<String>,
    pub(crate) payload: Option<Value>,
    pub(crate) dependencies: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PreparedSyncBatch {
    pub(crate) batch_id: String,
    pub(crate) operations: Vec<PreparedSyncOperation>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RemoteSyncChange {
    pub(crate) sequence: String,
    pub(crate) entity_type: String,
    pub(crate) entity_id: String,
    pub(crate) kind: String,
    pub(crate) version: i64,
    pub(crate) content_hash: String,
    pub(crate) snapshot: Option<Value>,
    pub(crate) updated_at: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SyncOperationOutcome {
    pub(crate) operation_id: String,
    pub(crate) status: String,
    pub(crate) entity_version: Option<i64>,
    pub(crate) content_hash: Option<String>,
    pub(crate) conflict_id: Option<String>,
    pub(crate) response: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RemoteEntityBaseline {
    pub(crate) version: i64,
    pub(crate) content_hash: String,
    pub(crate) tombstone: bool,
    pub(crate) snapshot: Option<Value>,
}

impl LocalDataStore {
    #[cfg(test)]
    pub(crate) fn make_sync_retries_due(&self) -> LocalDataResult<()> {
        self.connection()
            .execute(
                "UPDATE pending_mutations SET next_attempt_at = 0
                 WHERE queue_state = 'retrying'",
                [],
            )
            .map(|_| ())
            .map_err(Into::into)
    }

    pub(crate) fn prepare_sync_batch(
        &self,
        account_id: &str,
        device_id: &str,
        limit: u32,
    ) -> LocalDataResult<Option<PreparedSyncBatch>> {
        validate_canonical_uuid(account_id, "accountId")?;
        validate_canonical_uuid(device_id, "deviceId")?;
        let now = now_micros();
        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let replay_batch = transaction
            .query_row(
                "SELECT batch_id FROM pending_mutations
                 WHERE account_id = ?1 AND device_id = ?2 AND queue_state = 'retrying'
                   AND batch_id IS NOT NULL AND (next_attempt_at IS NULL OR next_attempt_at <= ?3)
                 ORDER BY created_at, operation_id LIMIT 1",
                params![account_id, device_id, now],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        let batch_id = replay_batch.unwrap_or_else(|| Uuid::now_v7().to_string());
        let mut operations = if transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM pending_mutations WHERE batch_id = ?1)",
            [&batch_id],
            |row| row.get::<_, bool>(0),
        )? {
            read_batch_operations(&transaction, &batch_id)?
        } else {
            read_new_operations(
                &transaction,
                account_id,
                device_id,
                now,
                limit.clamp(1, MAX_PUSH_BATCH),
            )?
        };
        if operations.is_empty() {
            transaction.commit()?;
            return Ok(None);
        }

        if operations
            .iter()
            .all(|operation| operation.batch_id.is_none())
        {
            normalize_new_batch(&transaction, account_id, &mut operations)?;
            for (ordinal, operation) in operations.iter().enumerate() {
                transaction.execute(
                    "UPDATE pending_mutations
                     SET account_id = ?2, device_id = ?3, batch_id = ?4, batch_ordinal = ?5,
                         base_version = ?6, base_content_hash = ?7, dependencies_json = ?8,
                         queue_state = 'in_flight', attempt_count = attempt_count + 1,
                         last_attempt_at = ?9, next_attempt_at = NULL, last_error_code = NULL,
                         updated_at = ?9
                     WHERE operation_id = ?1",
                    params![
                        operation.operation_id,
                        account_id,
                        device_id,
                        batch_id,
                        ordinal,
                        operation.base_version,
                        operation.base_content_hash,
                        canonical_json(&operation.dependencies)?,
                        now,
                    ],
                )?;
                transaction.execute(
                    "DELETE FROM sync_operation_dependencies WHERE operation_id = ?1",
                    [&operation.operation_id],
                )?;
                for dependency in &operation.dependencies {
                    transaction.execute(
                        "INSERT INTO sync_operation_dependencies(operation_id, depends_on_operation_id)
                         VALUES (?1, ?2)",
                        params![operation.operation_id, dependency],
                    )?;
                }
            }
        } else {
            transaction.execute(
                "UPDATE pending_mutations
                 SET queue_state = 'in_flight', attempt_count = attempt_count + 1,
                     last_attempt_at = ?2, next_attempt_at = NULL, last_error_code = NULL,
                     updated_at = ?2
                 WHERE batch_id = ?1 AND queue_state = 'retrying'",
                params![batch_id, now],
            )?;
        }
        transaction.commit()?;

        Ok(Some(PreparedSyncBatch {
            batch_id,
            operations: operations.into_iter().map(Into::into).collect(),
        }))
    }

    pub(crate) fn retry_sync_batch(
        &self,
        batch_id: &str,
        error_code: &str,
    ) -> LocalDataResult<u32> {
        validate_canonical_uuid(batch_id, "batchId")?;
        let now = now_micros();
        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let attempt: i64 = transaction.query_row(
            "SELECT coalesce(max(attempt_count), 1) FROM pending_mutations WHERE batch_id = ?1",
            [batch_id],
            |row| row.get(0),
        )?;
        let delay_seconds = 1_i64 << u32::try_from(attempt.saturating_sub(1).min(8)).unwrap_or(8);
        let next_attempt = now.saturating_add(delay_seconds.saturating_mul(1_000_000));
        let changed = transaction.execute(
            "UPDATE pending_mutations
             SET queue_state = 'retrying', next_attempt_at = ?3, last_error_code = ?2,
                 updated_at = ?4
             WHERE batch_id = ?1 AND queue_state = 'in_flight'",
            params![batch_id, error_code, next_attempt, now],
        )?;
        transaction.commit()?;
        Ok(u32::try_from(changed).unwrap_or(u32::MAX))
    }

    pub(crate) fn settle_sync_batch(
        &self,
        account_id: &str,
        batch_id: &str,
        outcomes: &[SyncOperationOutcome],
    ) -> LocalDataResult<()> {
        validate_canonical_uuid(account_id, "accountId")?;
        validate_canonical_uuid(batch_id, "batchId")?;
        let now = now_micros();
        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let expected_operation_ids = {
            let mut statement = transaction.prepare(
                "SELECT operation_id FROM pending_mutations
                 WHERE batch_id = ?1 AND queue_state = 'in_flight'",
            )?;
            let ids = statement
                .query_map([batch_id], |row| row.get::<_, String>(0))?
                .collect::<Result<HashSet<_>, _>>()?;
            ids
        };
        let outcome_ids = outcomes
            .iter()
            .map(|outcome| outcome.operation_id.as_str())
            .collect::<HashSet<_>>();
        if expected_operation_ids.len() != outcomes.len()
            || outcome_ids.len() != outcomes.len()
            || !expected_operation_ids
                .iter()
                .all(|operation_id| outcome_ids.contains(operation_id.as_str()))
        {
            return Err(LocalDataError::Corrupt(
                "sync response does not contain exactly one result for every batch operation"
                    .into(),
            ));
        }
        for outcome in outcomes {
            let operation =
                read_operation_by_id(&transaction, &outcome.operation_id)?.ok_or_else(|| {
                    LocalDataError::NotFound {
                        entity: "sync operation",
                        id: outcome.operation_id.clone(),
                    }
                })?;
            if operation.batch_id.as_deref() != Some(batch_id) {
                return Err(LocalDataError::Validation(vec![
                    "sync response operation does not belong to the active batch".into(),
                ]));
            }
            let response_json = canonical_json(&outcome.response)?;
            let request_hash = operation_request_hash(&operation)?;
            store_exact_result(
                &transaction,
                &operation.operation_id,
                &request_hash,
                &response_json,
                now,
            )?;
            let queue_state = match outcome.status.as_str() {
                "applied" | "noop" => "settled",
                "conflict" => "conflict",
                "rejected" | "dependencyFailed" => "failed",
                unknown => {
                    return Err(LocalDataError::Corrupt(format!(
                        "unknown sync operation status {unknown:?}"
                    )))
                }
            };
            transaction.execute(
                "UPDATE pending_mutations
                 SET request_hash = ?2, stored_response_json = ?3, queue_state = ?4,
                     last_error_code = CASE WHEN ?4 = 'settled' THEN NULL ELSE ?4 END,
                     updated_at = ?5
                 WHERE operation_id = ?1",
                params![
                    operation.operation_id,
                    request_hash,
                    response_json,
                    queue_state,
                    now
                ],
            )?;
            if matches!(queue_state, "settled") {
                if let (Some(version), Some(content_hash)) =
                    (outcome.entity_version, outcome.content_hash.as_deref())
                {
                    upsert_remote_entity(
                        &transaction,
                        account_id,
                        &RemoteSyncChange {
                            sequence: outcome
                                .response
                                .get("changeCursor")
                                .and_then(Value::as_str)
                                .unwrap_or("push")
                                .to_owned(),
                            entity_type: operation.entity_type.clone(),
                            entity_id: operation.entity_id.clone(),
                            kind: operation.kind.clone(),
                            version,
                            content_hash: content_hash.to_owned(),
                            snapshot: operation.payload.clone(),
                            updated_at: now,
                        },
                    )?;
                }
            }
        }
        transaction.commit()?;
        Ok(())
    }

    /// Applies one complete pull page and stages its cursor in the same transaction.
    pub(crate) fn apply_remote_page(
        &self,
        account_id: &str,
        device_id: &str,
        changes: &[RemoteSyncChange],
        next_cursor: &str,
    ) -> LocalDataResult<()> {
        validate_canonical_uuid(account_id, "accountId")?;
        validate_canonical_uuid(device_id, "deviceId")?;
        if next_cursor.is_empty() {
            return Err(LocalDataError::Validation(vec![
                "nextCursor must not be empty".into(),
            ]));
        }
        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for change in changes {
            apply_remote_change(&transaction, account_id, change)?;
        }
        let changed = transaction.execute(
            "UPDATE sync_devices SET pulled_cursor = ?3, updated_at = ?4
             WHERE account_id = ?1 AND device_id = ?2",
            params![account_id, device_id, next_cursor, now_micros()],
        )?;
        if changed != 1 {
            return Err(LocalDataError::Validation(vec![
                "sync device is not registered".into(),
            ]));
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn begin_snapshot_rebuild(
        &self,
        account_id: &str,
        device_id: &str,
        snapshot_id: &str,
    ) -> LocalDataResult<String> {
        validate_canonical_uuid(account_id, "accountId")?;
        validate_canonical_uuid(device_id, "deviceId")?;
        validate_canonical_uuid(snapshot_id, "snapshotId")?;
        let rebuild_id = Uuid::now_v7().to_string();
        let now = now_micros();
        self.connection().execute(
            "INSERT INTO sync_snapshot_rebuilds(
                rebuild_id, account_id, device_id, snapshot_id, state,
                pages_received, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 'downloading', 0, ?5, ?5)",
            params![rebuild_id, account_id, device_id, snapshot_id, now],
        )?;
        Ok(rebuild_id)
    }

    pub(crate) fn append_snapshot_page(
        &self,
        rebuild_id: &str,
        entries: &[RemoteSyncChange],
        resume_cursor: &str,
    ) -> LocalDataResult<()> {
        validate_canonical_uuid(rebuild_id, "rebuildId")?;
        let now = now_micros();
        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for entry in entries {
            validate_remote_change(entry)?;
            transaction.execute(
                "INSERT INTO sync_snapshot_entries(
                    rebuild_id, entity_type, entity_id, version, content_hash, tombstone,
                    snapshot_json, sequence, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(rebuild_id, entity_type, entity_id) DO UPDATE SET
                    version = excluded.version, content_hash = excluded.content_hash,
                    tombstone = excluded.tombstone, snapshot_json = excluded.snapshot_json,
                    sequence = excluded.sequence, updated_at = excluded.updated_at",
                params![
                    rebuild_id,
                    entry.entity_type,
                    entry.entity_id,
                    entry.version,
                    entry.content_hash,
                    entry.snapshot.is_none(),
                    canonical_json(&entry.snapshot)?,
                    entry.sequence,
                    entry.updated_at,
                ],
            )?;
        }
        let changed = transaction.execute(
            "UPDATE sync_snapshot_rebuilds
             SET resume_cursor = ?2, pages_received = pages_received + 1, updated_at = ?3
             WHERE rebuild_id = ?1 AND state = 'downloading'",
            params![rebuild_id, resume_cursor, now],
        )?;
        if changed != 1 {
            return Err(LocalDataError::Validation(vec![
                "snapshot rebuild is not accepting pages".into(),
            ]));
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn complete_snapshot_rebuild(&self, rebuild_id: &str) -> LocalDataResult<String> {
        validate_canonical_uuid(rebuild_id, "rebuildId")?;
        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (account_id, device_id, resume_cursor): (String, String, String) = transaction
            .query_row(
                "SELECT account_id, device_id, resume_cursor FROM sync_snapshot_rebuilds
                 WHERE rebuild_id = ?1 AND state = 'downloading'",
                [rebuild_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
        transaction.execute(
            "UPDATE sync_snapshot_rebuilds SET state = 'applying', updated_at = ?2
             WHERE rebuild_id = ?1",
            params![rebuild_id, now_micros()],
        )?;
        // A snapshot is a complete, consistent Cloud projection for this account. Replace only
        // the accepted remote baseline; local projections and pending candidates remain intact
        // and will be compared with the incoming entries below.
        transaction.execute(
            "DELETE FROM sync_remote_entities WHERE account_id = ?1",
            [&account_id],
        )?;
        {
            let mut statement = transaction.prepare(
                "SELECT sequence, entity_type, entity_id,
                        CASE WHEN tombstone = 1 THEN 'delete' ELSE 'update' END,
                        version, content_hash, snapshot_json, updated_at
                 FROM sync_snapshot_entries WHERE rebuild_id = ?1
                 ORDER BY entity_type, entity_id",
            )?;
            let entries = statement
                .query_map([rebuild_id], |row| {
                    let snapshot_json: String = row.get(6)?;
                    Ok(RemoteSyncChange {
                        sequence: row
                            .get::<_, Option<String>>(0)?
                            .unwrap_or_else(|| "snapshot".into()),
                        entity_type: row.get(1)?,
                        entity_id: row.get(2)?,
                        kind: row.get(3)?,
                        version: row.get(4)?,
                        content_hash: row.get(5)?,
                        snapshot: serde_json::from_str(&snapshot_json).map_err(sql_json_error)?,
                        updated_at: row.get::<_, Option<i64>>(7)?.unwrap_or(0),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            drop(statement);
            for entry in entries {
                apply_remote_change(&transaction, &account_id, &entry)?;
            }
        }
        transaction.execute(
            "UPDATE sync_devices SET pulled_cursor = ?3, updated_at = ?4
             WHERE account_id = ?1 AND device_id = ?2",
            params![account_id, device_id, resume_cursor, now_micros()],
        )?;
        transaction.execute(
            "UPDATE sync_snapshot_rebuilds SET state = 'complete', updated_at = ?2
             WHERE rebuild_id = ?1",
            params![rebuild_id, now_micros()],
        )?;
        transaction.commit()?;
        Ok(resume_cursor)
    }

    pub(crate) fn remote_entity_baseline(
        &self,
        account_id: &str,
        entity_type: &str,
        entity_id: &str,
    ) -> LocalDataResult<Option<RemoteEntityBaseline>> {
        self.connection()
            .query_row(
                "SELECT version, content_hash, tombstone, snapshot_json
                 FROM sync_remote_entities
                 WHERE account_id = ?1 AND entity_type = ?2 AND entity_id = ?3",
                params![account_id, entity_type, entity_id],
                |row| {
                    let snapshot_json: String = row.get(3)?;
                    Ok(RemoteEntityBaseline {
                        version: row.get(0)?,
                        content_hash: row.get(1)?,
                        tombstone: row.get(2)?,
                        snapshot: serde_json::from_str(&snapshot_json).map_err(sql_json_error)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub(crate) fn set_sync_authentication_state(
        &self,
        account_id: &str,
        device_id: &str,
        state: &str,
    ) -> LocalDataResult<()> {
        if !matches!(state, "ready" | "required" | "revoked") {
            return Err(LocalDataError::Validation(vec![
                "invalid sync authentication state".into(),
            ]));
        }
        let changed = self.connection().execute(
            "UPDATE sync_devices SET authentication_state = ?3, updated_at = ?4
             WHERE account_id = ?1 AND device_id = ?2",
            params![account_id, device_id, state, now_micros()],
        )?;
        if changed == 1 {
            Ok(())
        } else {
            Err(LocalDataError::Validation(vec![
                "sync device is not registered".into(),
            ]))
        }
    }
}

#[derive(Clone, Debug)]
struct StoredOperation {
    operation_id: String,
    entity_type: String,
    entity_id: String,
    kind: String,
    base_version: Option<i64>,
    base_content_hash: Option<String>,
    payload: Option<Value>,
    dependencies: Vec<String>,
    batch_id: Option<String>,
}

impl From<StoredOperation> for PreparedSyncOperation {
    fn from(value: StoredOperation) -> Self {
        Self {
            operation_id: value.operation_id,
            entity_type: value.entity_type,
            entity_id: value.entity_id,
            kind: value.kind,
            base_version: value.base_version,
            base_content_hash: value.base_content_hash,
            payload: value.payload,
            dependencies: value.dependencies,
        }
    }
}

fn read_new_operations(
    transaction: &Transaction<'_>,
    account_id: &str,
    device_id: &str,
    now: i64,
    limit: u32,
) -> LocalDataResult<Vec<StoredOperation>> {
    let mut statement = transaction.prepare(
        "SELECT operation_id, entity_type, entity_id, mutation_kind, base_version,
                base_content_hash, candidate_json, dependencies_json, batch_id
         FROM pending_mutations
         WHERE queue_state IN ('pending', 'retrying') AND batch_id IS NULL
           AND (account_id IS NULL OR (account_id = ?1 AND device_id = ?2))
           AND (next_attempt_at IS NULL OR next_attempt_at <= ?3)
         ORDER BY created_at, operation_id LIMIT ?4",
    )?;
    let rows = statement.query(params![account_id, device_id, now, limit])?;
    let operations = read_operations(rows)?;
    Ok(operations)
}

fn read_batch_operations(
    transaction: &Transaction<'_>,
    batch_id: &str,
) -> LocalDataResult<Vec<StoredOperation>> {
    let mut statement = transaction.prepare(
        "SELECT operation_id, entity_type, entity_id, mutation_kind, base_version,
                base_content_hash, candidate_json, dependencies_json, batch_id
         FROM pending_mutations WHERE batch_id = ?1 AND queue_state = 'retrying'
         ORDER BY batch_ordinal, operation_id",
    )?;
    let rows = statement.query([batch_id])?;
    let operations = read_operations(rows)?;
    Ok(operations)
}

fn read_operations(mut rows: rusqlite::Rows<'_>) -> LocalDataResult<Vec<StoredOperation>> {
    let mut operations = Vec::new();
    while let Some(row) = rows.next()? {
        let candidate_json: String = row.get(6)?;
        let dependencies_json: String = row.get(7)?;
        let kind: String = row.get(3)?;
        operations.push(StoredOperation {
            operation_id: row.get(0)?,
            entity_type: row.get(1)?,
            entity_id: row.get(2)?,
            kind: kind.clone(),
            base_version: row.get(4)?,
            base_content_hash: row.get(5)?,
            payload: if kind == "delete" {
                None
            } else {
                Some(serde_json::from_str(&candidate_json)?)
            },
            dependencies: serde_json::from_str(&dependencies_json)?,
            batch_id: row.get(8)?,
        });
    }
    Ok(operations)
}

fn read_operation_by_id(
    transaction: &Transaction<'_>,
    operation_id: &str,
) -> LocalDataResult<Option<StoredOperation>> {
    let mut statement = transaction.prepare(
        "SELECT operation_id, entity_type, entity_id, mutation_kind, base_version,
                base_content_hash, candidate_json, dependencies_json, batch_id
         FROM pending_mutations WHERE operation_id = ?1",
    )?;
    let rows = statement.query([operation_id])?;
    let operation = read_operations(rows)?.into_iter().next();
    Ok(operation)
}

fn normalize_new_batch(
    transaction: &Transaction<'_>,
    account_id: &str,
    operations: &mut [StoredOperation],
) -> LocalDataResult<()> {
    let mut predicted: HashMap<(String, String), (i64, String, String)> = HashMap::new();
    for operation in operations {
        let key = (operation.entity_type.clone(), operation.entity_id.clone());
        if let Some((version, hash, previous_id)) = predicted.get(&key) {
            operation.base_version = Some(*version);
            operation.base_content_hash = Some(hash.clone());
            if !operation.dependencies.contains(previous_id) {
                operation.dependencies.push(previous_id.clone());
            }
        } else if operation.kind != "create" {
            if let Some((version, hash)) = transaction
                .query_row(
                    "SELECT version, content_hash FROM sync_remote_entities
                     WHERE account_id = ?1 AND entity_type = ?2 AND entity_id = ?3",
                    params![account_id, operation.entity_type, operation.entity_id],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?
            {
                operation.base_version = Some(version);
                operation.base_content_hash = Some(hash);
            }
        } else {
            operation.base_version = None;
            operation.base_content_hash = None;
        }
        let next_version = operation.base_version.unwrap_or(0).saturating_add(1);
        let desired_hash = sync_payload_hash(operation.payload.as_ref())?;
        predicted.insert(
            key,
            (next_version, desired_hash, operation.operation_id.clone()),
        );
    }
    Ok(())
}

fn operation_request_hash(operation: &StoredOperation) -> LocalDataResult<String> {
    let request = serde_json::json!({
        "operationId": operation.operation_id,
        "entityType": operation.entity_type,
        "entityId": operation.entity_id,
        "kind": operation.kind,
        "baseVersion": operation.base_version,
        "baseContentHash": operation.base_content_hash,
        "payload": operation.payload,
        "dependsOn": operation.dependencies,
    });
    Ok(hex_sha256(canonical_json(&request)?.as_bytes()))
}

fn sync_payload_hash(payload: Option<&Value>) -> LocalDataResult<String> {
    Ok(hex_sha256(canonical_json(&payload)?.as_bytes()))
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn store_exact_result(
    transaction: &Transaction<'_>,
    operation_id: &str,
    request_hash: &str,
    response_json: &str,
    now: i64,
) -> LocalDataResult<()> {
    let existing = transaction
        .query_row(
            "SELECT request_hash, response_json FROM sync_operation_results
             WHERE operation_id = ?1",
            [operation_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    match existing {
        Some((hash, response)) if hash != request_hash || response != response_json => Err(
            LocalDataError::Corrupt("a replayed operation produced a different response".into()),
        ),
        Some(_) => Ok(()),
        None => {
            transaction.execute(
                "INSERT INTO sync_operation_results(
                    operation_id, request_hash, response_json, recorded_at
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![operation_id, request_hash, response_json, now],
            )?;
            Ok(())
        }
    }
}

fn apply_remote_change(
    transaction: &Transaction<'_>,
    account_id: &str,
    change: &RemoteSyncChange,
) -> LocalDataResult<()> {
    validate_remote_change(change)?;
    let existing = transaction
        .query_row(
            "SELECT version, content_hash FROM sync_remote_entities
             WHERE account_id = ?1 AND entity_type = ?2 AND entity_id = ?3",
            params![account_id, change.entity_type, change.entity_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    if let Some((version, hash)) = existing {
        if version > change.version || (version == change.version && hash == change.content_hash) {
            return Ok(());
        }
        if version == change.version {
            return Err(LocalDataError::Corrupt(
                "the same remote entity version has divergent content hashes".into(),
            ));
        }
    }

    let local_candidate = transaction
        .query_row(
            "SELECT operation_id, base_version, base_content_hash, candidate_json
             FROM pending_mutations
             WHERE entity_type = ?1 AND entity_id = ?2 AND queue_state <> 'settled'
               AND (account_id IS NULL OR account_id = ?3)
             ORDER BY created_at DESC, operation_id DESC LIMIT 1",
            params![change.entity_type, change.entity_id, account_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    if let Some((operation_id, base_version, base_hash, candidate_json)) = local_candidate {
        if sync_payload_hash(Some(&serde_json::from_str(&candidate_json)?))? != change.content_hash
        {
            let conflict_id = Uuid::now_v7().to_string();
            transaction.execute(
                "INSERT INTO sync_conflict_baselines(
                    conflict_id, entity_type, entity_id, operation_id,
                    base_version, base_content_hash, base_snapshot_json,
                    local_snapshot_json, cloud_version, cloud_content_hash,
                    cloud_snapshot_json, resolution_state, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, ?9, ?10,
                           'unresolved', ?11, ?11)",
                params![
                    conflict_id,
                    change.entity_type,
                    change.entity_id,
                    operation_id,
                    base_version,
                    base_hash,
                    candidate_json,
                    change.version,
                    change.content_hash,
                    canonical_json(&change.snapshot)?,
                    now_micros(),
                ],
            )?;
            transaction.execute(
                "UPDATE pending_mutations SET queue_state = 'conflict',
                        last_error_code = 'SYNC_CONFLICT', updated_at = ?2
                 WHERE operation_id = ?1",
                params![operation_id, now_micros()],
            )?;
            return upsert_remote_entity(transaction, account_id, change);
        }
        transaction.execute(
            "UPDATE pending_mutations SET queue_state = 'settled', last_error_code = NULL,
                    updated_at = ?2 WHERE operation_id = ?1",
            params![operation_id, now_micros()],
        )?;
    }
    upsert_remote_entity(transaction, account_id, change)
}

fn upsert_remote_entity(
    transaction: &Transaction<'_>,
    account_id: &str,
    change: &RemoteSyncChange,
) -> LocalDataResult<()> {
    transaction.execute(
        "INSERT INTO sync_remote_entities(
            account_id, entity_type, entity_id, version, content_hash, tombstone,
            snapshot_json, last_sequence, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(account_id, entity_type, entity_id) DO UPDATE SET
            version = excluded.version, content_hash = excluded.content_hash,
            tombstone = excluded.tombstone, snapshot_json = excluded.snapshot_json,
            last_sequence = excluded.last_sequence, updated_at = excluded.updated_at",
        params![
            account_id,
            change.entity_type,
            change.entity_id,
            change.version,
            change.content_hash,
            change.snapshot.is_none(),
            canonical_json(&change.snapshot)?,
            change.sequence,
            change.updated_at,
        ],
    )?;
    Ok(())
}

fn validate_remote_change(change: &RemoteSyncChange) -> LocalDataResult<()> {
    validate_canonical_uuid(&change.entity_id, "remote entityId")?;
    if !matches!(
        change.entity_type.as_str(),
        "question" | "paper" | "draft" | "attachment" | "comment" | "favorite" | "setting"
    ) {
        return Err(LocalDataError::Validation(vec![
            "remote entityType is unsupported".into(),
        ]));
    }
    if change.version < 1
        || change.content_hash.len() != 64
        || !change
            .content_hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || change.sequence.is_empty()
    {
        return Err(LocalDataError::Validation(vec![
            "remote change envelope is invalid".into(),
        ]));
    }
    Ok(())
}

fn sql_json_error(error: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}
