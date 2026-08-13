use std::{fmt, str::FromStr};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    canonical::canonical_json,
    error::{LocalDataError, LocalDataResult},
    migration::{now_micros, validate_canonical_uuid},
    LocalDataStore,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct StartupRecoveryReport {
    pub(crate) retryable_operations: u32,
    pub(crate) reset_runtime_states: u32,
    pub(crate) retryable_snapshot_rebuilds: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SyncRuntimePhase {
    Idle,
    Pull,
    Apply,
    Ack,
    Push,
    Settle,
}

impl fmt::Display for SyncRuntimePhase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Idle => "idle",
            Self::Pull => "pull",
            Self::Apply => "apply",
            Self::Ack => "ack",
            Self::Push => "push",
            Self::Settle => "settle",
        })
    }
}

impl FromStr for SyncRuntimePhase {
    type Err = LocalDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "idle" => Ok(Self::Idle),
            "pull" => Ok(Self::Pull),
            "apply" => Ok(Self::Apply),
            "ack" => Ok(Self::Ack),
            "push" => Ok(Self::Push),
            "settle" => Ok(Self::Settle),
            _ => Err(LocalDataError::Corrupt(format!(
                "unknown sync runtime phase {value:?}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncDeviceState {
    pub(crate) account_id: String,
    pub(crate) device_id: String,
    pub(crate) acknowledged_cursor: Option<String>,
    pub(crate) pulled_cursor: Option<String>,
    pub(crate) authentication_state: String,
    pub(crate) runtime_phase: SyncRuntimePhase,
    pub(crate) active_batch_id: Option<String>,
    pub(crate) updated_at: i64,
}

impl LocalDataStore {
    pub(crate) fn register_sync_device(
        &self,
        account_id: &str,
        device_id: &str,
    ) -> LocalDataResult<()> {
        validate_sync_identity(account_id, device_id)?;
        let now = now_micros();
        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO sync_devices(
                account_id, device_id, protocol_version, authentication_state, created_at, updated_at
             ) VALUES (?1, ?2, 'v1', 'ready', ?3, ?3)
             ON CONFLICT(account_id, device_id) DO UPDATE SET updated_at = excluded.updated_at",
            params![account_id, device_id, now],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO sync_runtime_state(
                account_id, device_id, phase, updated_at
             ) VALUES (?1, ?2, 'idle', ?3)",
            params![account_id, device_id, now],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn sync_device_state(
        &self,
        account_id: &str,
        device_id: &str,
    ) -> LocalDataResult<Option<SyncDeviceState>> {
        validate_sync_identity(account_id, device_id)?;
        self.connection()
            .query_row(
                "SELECT d.account_id, d.device_id, d.acknowledged_cursor, d.pulled_cursor,
                        d.authentication_state, r.phase, r.active_batch_id,
                        max(d.updated_at, r.updated_at)
                 FROM sync_devices d
                 JOIN sync_runtime_state r
                   ON r.account_id = d.account_id AND r.device_id = d.device_id
                 WHERE d.account_id = ?1 AND d.device_id = ?2",
                params![account_id, device_id],
                |row| {
                    let phase: String = row.get(5)?;
                    Ok(SyncDeviceState {
                        account_id: row.get(0)?,
                        device_id: row.get(1)?,
                        acknowledged_cursor: row.get(2)?,
                        pulled_cursor: row.get(3)?,
                        authentication_state: row.get(4)?,
                        runtime_phase: SyncRuntimePhase::from_str(&phase)
                            .map_err(sql_decode_error)?,
                        active_batch_id: row.get(6)?,
                        updated_at: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Persists the cursor returned with a pulled page without advancing the acknowledged cursor.
    pub(crate) fn stage_pulled_cursor(
        &self,
        account_id: &str,
        device_id: &str,
        pulled_cursor: &str,
    ) -> LocalDataResult<()> {
        validate_sync_identity(account_id, device_id)?;
        if pulled_cursor.is_empty() {
            return Err(LocalDataError::Validation(vec![
                "pulledCursor must not be empty".into(),
            ]));
        }
        let changed = self.connection().execute(
            "UPDATE sync_devices SET pulled_cursor = ?3, updated_at = ?4
             WHERE account_id = ?1 AND device_id = ?2",
            params![account_id, device_id, pulled_cursor, now_micros()],
        )?;
        require_registered_device(changed)
    }

    /// Advances the durable acknowledgement only after the caller commits the pulled page locally.
    pub(crate) fn commit_pulled_cursor(
        &self,
        account_id: &str,
        device_id: &str,
    ) -> LocalDataResult<String> {
        validate_sync_identity(account_id, device_id)?;
        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let cursor = transaction
            .query_row(
                "SELECT pulled_cursor FROM sync_devices
                 WHERE account_id = ?1 AND device_id = ?2",
                params![account_id, device_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten()
            .ok_or_else(|| {
                LocalDataError::Validation(vec![
                    "no pulled cursor is staged for acknowledgement".into()
                ])
            })?;
        transaction.execute(
            "UPDATE sync_devices
             SET acknowledged_cursor = ?3, pulled_cursor = NULL, updated_at = ?4
             WHERE account_id = ?1 AND device_id = ?2",
            params![account_id, device_id, cursor, now_micros()],
        )?;
        transaction.commit()?;
        Ok(cursor)
    }

    pub(crate) fn set_sync_runtime_phase(
        &self,
        account_id: &str,
        device_id: &str,
        phase: SyncRuntimePhase,
        active_batch_id: Option<&str>,
    ) -> LocalDataResult<()> {
        validate_sync_identity(account_id, device_id)?;
        if let Some(batch_id) = active_batch_id {
            validate_canonical_uuid(batch_id, "activeBatchId")?;
        }
        let now = now_micros();
        let changed = self.connection().execute(
            "UPDATE sync_runtime_state
             SET phase = ?3, active_batch_id = ?4,
                 phase_started_at = CASE WHEN ?3 = 'idle' THEN NULL ELSE ?5 END,
                 last_completed_at = CASE WHEN ?3 = 'idle' THEN ?5 ELSE last_completed_at END,
                 last_error_code = NULL, updated_at = ?5
             WHERE account_id = ?1 AND device_id = ?2",
            params![
                account_id,
                device_id,
                phase.to_string(),
                active_batch_id,
                now
            ],
        )?;
        require_registered_device(changed)
    }

    /// Attaches otherwise protocol-neutral local edits to a device queue without changing payloads.
    pub(crate) fn bind_pending_mutation(
        &self,
        operation_id: &str,
        account_id: &str,
        device_id: &str,
        dependencies: &[String],
    ) -> LocalDataResult<()> {
        validate_canonical_uuid(operation_id, "operationId")?;
        validate_sync_identity(account_id, device_id)?;
        for dependency in dependencies {
            validate_canonical_uuid(dependency, "dependency operationId")?;
            if dependency == operation_id {
                return Err(LocalDataError::Validation(vec![
                    "an operation cannot depend on itself".into(),
                ]));
            }
        }
        let dependencies_json = canonical_json(&dependencies)?;
        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let registered: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sync_devices WHERE account_id = ?1 AND device_id = ?2
             )",
            params![account_id, device_id],
            |row| row.get(0),
        )?;
        if !registered {
            return Err(LocalDataError::Validation(vec![
                "sync device must be registered before queue binding".into(),
            ]));
        }
        let changed = transaction.execute(
            "UPDATE pending_mutations
             SET account_id = ?2, device_id = ?3, dependencies_json = ?4, updated_at = ?5
             WHERE operation_id = ?1 AND queue_state IN ('pending', 'retrying')",
            params![
                operation_id,
                account_id,
                device_id,
                dependencies_json,
                now_micros()
            ],
        )?;
        if changed == 0 {
            return Err(LocalDataError::NotFound {
                entity: "retryable sync operation",
                id: operation_id.into(),
            });
        }
        transaction.execute(
            "DELETE FROM sync_operation_dependencies WHERE operation_id = ?1",
            [operation_id],
        )?;
        for dependency in dependencies {
            transaction.execute(
                "INSERT INTO sync_operation_dependencies(operation_id, depends_on_operation_id)
                 VALUES (?1, ?2)",
                params![operation_id, dependency],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Stores a server response independently of queue cleanup for exact restart-safe replay.
    pub(crate) fn store_sync_operation_response(
        &self,
        operation_id: &str,
        request_hash: &str,
        response: &Value,
    ) -> LocalDataResult<()> {
        validate_canonical_uuid(operation_id, "operationId")?;
        validate_sha256(request_hash, "requestHash")?;
        let response_json = canonical_json(response)?;
        let now = now_micros();
        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = transaction
            .query_row(
                "SELECT request_hash, response_json FROM sync_operation_results
                 WHERE operation_id = ?1",
                [operation_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        match existing {
            Some((stored_hash, stored_response))
                if stored_hash != request_hash || stored_response != response_json =>
            {
                return Err(LocalDataError::Validation(vec![
                    "operationId is already associated with different request or response content"
                        .into(),
                ]));
            }
            Some(_) => {}
            None => {
                transaction.execute(
                    "INSERT INTO sync_operation_results(
                        operation_id, request_hash, response_json, recorded_at
                     ) VALUES (?1, ?2, ?3, ?4)",
                    params![operation_id, request_hash, response_json, now],
                )?;
            }
        }
        let changed = transaction.execute(
            "UPDATE pending_mutations
             SET request_hash = ?2, stored_response_json = ?3, queue_state = 'settled', updated_at = ?4
             WHERE operation_id = ?1",
            params![operation_id, request_hash, response_json, now],
        )?;
        if changed == 0 {
            return Err(LocalDataError::NotFound {
                entity: "sync operation",
                id: operation_id.into(),
            });
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn sync_operation_response(
        &self,
        operation_id: &str,
        request_hash: &str,
    ) -> LocalDataResult<Option<Value>> {
        validate_canonical_uuid(operation_id, "operationId")?;
        validate_sha256(request_hash, "requestHash")?;
        let stored = self
            .connection()
            .query_row(
                "SELECT request_hash, response_json FROM sync_operation_results
                 WHERE operation_id = ?1",
                [operation_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        match stored {
            None => Ok(None),
            Some((stored_hash, _)) if stored_hash != request_hash => {
                Err(LocalDataError::Validation(vec![
                    "operationId is already associated with different request content".into(),
                ]))
            }
            Some((_, response_json)) => Ok(Some(serde_json::from_str(&response_json)?)),
        }
    }
}

pub(super) fn recover_startup(
    connection: &mut Connection,
) -> LocalDataResult<StartupRecoveryReport> {
    let now = now_micros();
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let retryable_operations = transaction.execute(
        "UPDATE pending_mutations
         SET queue_state = 'retrying', next_attempt_at = ?1,
             last_error_code = 'desktop_restart', updated_at = ?1
         WHERE queue_state = 'in_flight'",
        [now],
    )?;
    let reset_runtime_states = transaction.execute(
        "UPDATE sync_runtime_state
         SET phase = 'idle', active_batch_id = NULL, phase_started_at = NULL,
             last_error_code = 'desktop_restart', updated_at = ?1
         WHERE phase <> 'idle'",
        [now],
    )?;
    let retryable_snapshot_rebuilds = transaction.execute(
        "UPDATE sync_snapshot_rebuilds
         SET state = 'ready', last_error_code = 'desktop_restart', updated_at = ?1
         WHERE state IN ('applying', 'swapping')",
        [now],
    )?;
    transaction.commit()?;

    Ok(StartupRecoveryReport {
        retryable_operations: u32::try_from(retryable_operations).unwrap_or(u32::MAX),
        reset_runtime_states: u32::try_from(reset_runtime_states).unwrap_or(u32::MAX),
        retryable_snapshot_rebuilds: u32::try_from(retryable_snapshot_rebuilds).unwrap_or(u32::MAX),
    })
}

fn validate_sync_identity(account_id: &str, device_id: &str) -> LocalDataResult<()> {
    validate_canonical_uuid(account_id, "accountId")?;
    validate_canonical_uuid(device_id, "deviceId")
}

fn validate_sha256(value: &str, field: &str) -> LocalDataResult<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(LocalDataError::Validation(vec![format!(
            "{field} must be a lowercase SHA-256 hex digest"
        )]))
    }
}

fn require_registered_device(changed: usize) -> LocalDataResult<()> {
    if changed == 1 {
        Ok(())
    } else {
        Err(LocalDataError::Validation(vec![
            "sync device is not registered".into(),
        ]))
    }
}

fn sql_decode_error(error: LocalDataError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}
