use std::{
    collections::VecDeque,
    sync::{Arc, Barrier, Mutex},
};

use serde_json::json;
use tempfile::TempDir;
use uuid::Uuid;

use super::*;
use crate::local_data::{
    CreateQuestion, Difficulty, QuestionContent, QuestionType, ReplicationScope, StoreConfig,
    SyncQueueState,
};

enum PushAction {
    Succeed,
    Fail(TransportError),
}

#[derive(Default)]
struct MockTransport {
    pulls: Mutex<VecDeque<Result<PullPage, TransportError>>>,
    snapshots: Mutex<VecDeque<Result<SnapshotPage, TransportError>>>,
    acknowledgements: Mutex<VecDeque<Result<(), TransportError>>>,
    push_actions: Mutex<VecDeque<PushAction>>,
    pushed_batches: Mutex<Vec<PreparedSyncBatch>>,
    resolutions: Mutex<VecDeque<Result<Value, TransportError>>>,
    resolved_conflicts: Mutex<Vec<PreparedConflictResolution>>,
    acknowledged_cursors: Mutex<Vec<String>>,
    pull_gate: Option<(Arc<Barrier>, Arc<Barrier>)>,
}

impl MockTransport {
    fn default_pull() -> PullPage {
        PullPage {
            changes: Vec::new(),
            next_cursor: "cursor-empty".into(),
            has_more: false,
        }
    }

    fn offline() -> TransportError {
        TransportError {
            kind: TransportErrorKind::Offline,
            code: "SYNC_TRANSPORT_UNAVAILABLE".into(),
        }
    }
}

impl SyncTransport for MockTransport {
    fn resolve_conflict(
        &self,
        resolution: &PreparedConflictResolution,
    ) -> Result<Value, TransportError> {
        self.resolved_conflicts
            .lock()
            .unwrap()
            .push(resolution.clone());
        self.resolutions
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Err(fatal("UNEXPECTED_CONFLICT_RESOLUTION")))
    }

    fn pull(&self, _cursor: Option<&str>, _page_size: i32) -> Result<PullPage, TransportError> {
        if let Some((entered, release)) = &self.pull_gate {
            entered.wait();
            release.wait();
        }
        self.pulls
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Ok(Self::default_pull()))
    }

    fn acknowledge(&self, _device_id: &str, cursor: &str) -> Result<(), TransportError> {
        self.acknowledged_cursors
            .lock()
            .unwrap()
            .push(cursor.into());
        self.acknowledgements
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Ok(()))
    }

    fn snapshot(
        &self,
        _cursor: Option<&str>,
        _page_size: i32,
    ) -> Result<SnapshotPage, TransportError> {
        self.snapshots
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Err(fatal("UNEXPECTED_SNAPSHOT")))
    }

    fn push(
        &self,
        _device_id: &str,
        batch: &PreparedSyncBatch,
    ) -> Result<Vec<SyncOperationOutcome>, TransportError> {
        self.pushed_batches.lock().unwrap().push(batch.clone());
        match self
            .push_actions
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(PushAction::Succeed)
        {
            PushAction::Fail(error) => Err(error),
            PushAction::Succeed => Ok(batch
                .operations
                .iter()
                .map(|operation| SyncOperationOutcome {
                    operation_id: operation.operation_id.clone(),
                    status: "applied".into(),
                    entity_version: Some(operation.base_version.unwrap_or(0) + 1),
                    content_hash: Some("a".repeat(64)),
                    conflict_id: None,
                    response: json!({
                        "operationId": operation.operation_id,
                        "status": "applied",
                        "entityVersion": operation.base_version.unwrap_or(0) + 1,
                        "contentHash": "a".repeat(64),
                        "changeCursor": "push-cursor"
                    }),
                })
                .collect()),
        }
    }
}

#[test]
fn worker_delivers_a_persisted_conflict_resolution_before_normal_push() {
    let (_directory, store, account_id, device_id) = test_store();
    let question = store
        .create_question(CreateQuestion {
            owner_id: None,
            replication_scope: ReplicationScope::CloudSynced,
            content: content("Local conflict candidate"),
        })
        .unwrap();
    store.register_sync_device(&account_id, &device_id).unwrap();
    store
        .apply_remote_page(
            &account_id,
            &device_id,
            &[RemoteSyncChange {
                sequence: "cloud-2".into(),
                entity_type: "question".into(),
                entity_id: question.id,
                kind: "update".into(),
                version: 2,
                content_hash: "c".repeat(64),
                snapshot: Some(json!({"text": "Cloud candidate"})),
                updated_at: 200,
            }],
            "cursor-2",
        )
        .unwrap();
    let conflict_id = store.list_sync_conflict_recovery(&account_id).unwrap()[0]
        .conflict_id
        .clone();
    let operation_id = Uuid::now_v7().to_string();
    store
        .stage_conflict_resolution(
            &account_id,
            &conflict_id,
            &json!({
                "protocolVersion": 1,
                "operationId": operation_id,
                "action": "useCloud",
                "currentVersion": 2,
                "currentContentHash": "c".repeat(64)
            }),
        )
        .unwrap();
    let response = json!({
        "protocolVersion": 1,
        "resolutionId": Uuid::now_v7().to_string(),
        "conflictId": conflict_id,
        "operationId": operation_id,
        "action": "useCloud",
        "actorDeviceId": device_id,
        "acceptedVersion": 2,
        "acceptedContentHash": "c".repeat(64),
        "result": {
            "schemaVersion": 1,
            "version": 2,
            "contentHash": "c".repeat(64),
            "mutationKind": "update",
            "tombstone": false,
            "payload": {"text": "Cloud candidate"},
            "deviceId": "web",
            "modifiedAt": "2026-08-13T00:00:00Z"
        },
        "resolvedAt": "2026-08-13T00:00:00Z"
    });
    let transport = Arc::new(MockTransport {
        resolutions: Mutex::new(VecDeque::from([Ok(response)])),
        ..Default::default()
    });
    let worker = SyncWorker::new(
        store.clone(),
        transport.clone(),
        account_id.clone(),
        device_id,
    )
    .unwrap();

    let report = worker.run_once().unwrap();

    assert_eq!(report.pushed_operations, 1);
    assert_eq!(transport.resolved_conflicts.lock().unwrap().len(), 1);
    let recovered = store.list_sync_conflict_recovery(&account_id).unwrap();
    assert_eq!(recovered[0].state, "resolved");
    assert_eq!(recovered[0].resolutions.len(), 1);
}

fn test_store() -> (TempDir, Arc<LocalDataStore>, String, String) {
    let directory = tempfile::tempdir().unwrap();
    let account_id = Uuid::now_v7().to_string();
    let device_id = Uuid::now_v7().to_string();
    let store = LocalDataStore::open(StoreConfig {
        database_path: directory.path().join("workspace.sqlite3"),
        blob_root: directory.path().join("blobs"),
        workspace_id: Uuid::now_v7().to_string(),
        local_principal_id: account_id.clone(),
    })
    .unwrap();
    (directory, Arc::new(store), account_id, device_id)
}

fn content(text: &str) -> QuestionContent {
    QuestionContent {
        question_type: QuestionType::ShortAnswer,
        subjects: vec!["Mathematics".into()],
        difficulty: Difficulty::Medium,
        tags: vec!["sync".into()],
        text: text.into(),
        options: None,
        answer: json!("4"),
        has_latex: false,
        source: None,
        essay_blank_space: None,
        score_weight: "1".into(),
    }
}

fn change(entity_id: &str, sequence: &str, text: &str) -> RemoteSyncChange {
    RemoteSyncChange {
        sequence: sequence.into(),
        entity_type: "question".into(),
        entity_id: entity_id.into(),
        kind: "update".into(),
        version: 1,
        content_hash: "b".repeat(64),
        snapshot: Some(json!({"schemaVersion": 1, "text": text})),
        updated_at: 123,
    }
}

#[test]
fn slow_transport_does_not_block_local_editing() {
    let (_directory, store, account_id, device_id) = test_store();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let transport = Arc::new(MockTransport {
        pull_gate: Some((entered.clone(), release.clone())),
        ..Default::default()
    });
    let worker =
        Arc::new(SyncWorker::new(store.clone(), transport.clone(), account_id, device_id).unwrap());
    let handle = worker.run_once_in_background();
    entered.wait();

    let local = store
        .create_question(CreateQuestion {
            owner_id: None,
            replication_scope: ReplicationScope::CloudSynced,
            content: content("Edited while pull was blocked"),
        })
        .unwrap();
    assert_eq!(
        store.get_question(&local.id).unwrap().content.text,
        "Edited while pull was blocked"
    );
    release.wait();

    let report = handle.join().unwrap().unwrap();
    assert_eq!(report.pushed_operations, 1);
}

#[test]
fn dropped_push_response_retries_the_exact_batch_without_duplicate_semantics() {
    let (_directory, store, account_id, device_id) = test_store();
    store
        .create_question(CreateQuestion {
            owner_id: None,
            replication_scope: ReplicationScope::CloudSynced,
            content: content("Retry exactly once"),
        })
        .unwrap();
    let transport = Arc::new(MockTransport::default());
    transport
        .push_actions
        .lock()
        .unwrap()
        .push_back(PushAction::Fail(MockTransport::offline()));
    transport
        .push_actions
        .lock()
        .unwrap()
        .push_back(PushAction::Succeed);
    let worker = SyncWorker::new(store.clone(), transport.clone(), account_id, device_id).unwrap();

    let first = worker.run_once().unwrap();
    assert!(first.deferred);
    let queued = store.list_pending_mutations(10).unwrap();
    assert_eq!(queued[0].queue_state, SyncQueueState::Retrying);
    assert!(queued[0].batch_id.is_some());

    store.make_sync_retries_due().unwrap();
    let second = worker.run_once().unwrap();
    assert_eq!(second.pushed_operations, 1);
    let batches = transport.pushed_batches.lock().unwrap();
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0], batches[1]);
    drop(batches);
    let settled = store.list_pending_mutations(10).unwrap();
    assert_eq!(settled[0].queue_state, SyncQueueState::Settled);
    assert_eq!(settled[0].attempt_count, 2);
    assert!(settled[0].stored_response.is_some());
}

#[test]
fn remote_change_preserves_and_conflicts_with_unmerged_local_candidate() {
    let (_directory, store, account_id, device_id) = test_store();
    let local = store
        .create_question(CreateQuestion {
            owner_id: None,
            replication_scope: ReplicationScope::CloudSynced,
            content: content("Keep this local candidate"),
        })
        .unwrap();
    let transport = Arc::new(MockTransport::default());
    transport.pulls.lock().unwrap().push_back(Ok(PullPage {
        changes: vec![change(&local.id, "1", "Cloud copy")],
        next_cursor: "cursor-1".into(),
        has_more: false,
    }));
    let worker = SyncWorker::new(store.clone(), transport, account_id.clone(), device_id).unwrap();

    worker.run_once().unwrap();
    assert_eq!(
        store.get_question(&local.id).unwrap().content.text,
        "Keep this local candidate"
    );
    assert_eq!(
        store.list_pending_mutations(10).unwrap()[0].queue_state,
        SyncQueueState::Conflict
    );
    let cloud = store
        .remote_entity_baseline(&account_id, "question", &local.id)
        .unwrap()
        .unwrap();
    assert_eq!(cloud.version, 1);
    assert_eq!(cloud.snapshot.unwrap()["text"], "Cloud copy");
}

#[test]
fn failed_ack_replays_the_page_idempotently_then_advances_local_cursor() {
    let (_directory, store, account_id, device_id) = test_store();
    let entity_id = Uuid::now_v7().to_string();
    let page = PullPage {
        changes: vec![change(&entity_id, "2", "Remote")],
        next_cursor: "cursor-2".into(),
        has_more: false,
    };
    let transport = Arc::new(MockTransport::default());
    transport.pulls.lock().unwrap().push_back(Ok(page.clone()));
    transport.pulls.lock().unwrap().push_back(Ok(page));
    transport
        .acknowledgements
        .lock()
        .unwrap()
        .push_back(Err(MockTransport::offline()));
    transport.acknowledgements.lock().unwrap().push_back(Ok(()));
    let worker = SyncWorker::new(
        store.clone(),
        transport,
        account_id.clone(),
        device_id.clone(),
    )
    .unwrap();

    assert!(worker.run_once().unwrap().deferred);
    let staged = store
        .sync_device_state(&account_id, &device_id)
        .unwrap()
        .unwrap();
    assert_eq!(staged.acknowledged_cursor, None);
    assert_eq!(staged.pulled_cursor.as_deref(), Some("cursor-2"));
    worker.run_once().unwrap();
    let committed = store
        .sync_device_state(&account_id, &device_id)
        .unwrap()
        .unwrap();
    assert_eq!(committed.acknowledged_cursor.as_deref(), Some("cursor-2"));
    assert_eq!(
        store
            .remote_entity_baseline(&account_id, "question", &entity_id)
            .unwrap()
            .unwrap()
            .version,
        1
    );
}

#[test]
fn expired_cursor_rebuilds_snapshot_and_acknowledges_resume_cursor() {
    let (_directory, store, account_id, device_id) = test_store();
    let entity_id = Uuid::now_v7().to_string();
    let stale_entity_id = Uuid::now_v7().to_string();
    let snapshot_id = Uuid::now_v7().to_string();
    let transport = Arc::new(MockTransport::default());
    transport
        .pulls
        .lock()
        .unwrap()
        .push_back(Err(TransportError {
            kind: TransportErrorKind::CursorExpired,
            code: "SYNC_CURSOR_EXPIRED".into(),
        }));
    transport
        .snapshots
        .lock()
        .unwrap()
        .push_back(Ok(SnapshotPage {
            snapshot_id,
            entries: vec![change(&entity_id, "3", "Snapshot")],
            next_cursor: "snapshot-end".into(),
            has_more: false,
            resume_cursor: "resume-3".into(),
        }));
    let worker = SyncWorker::new(
        store.clone(),
        transport.clone(),
        account_id.clone(),
        device_id.clone(),
    )
    .unwrap();
    store
        .apply_remote_page(
            &account_id,
            &device_id,
            &[change(&stale_entity_id, "stale", "Removed before snapshot")],
            "stale-cursor",
        )
        .unwrap();
    store.commit_pulled_cursor(&account_id, &device_id).unwrap();

    let report = worker.run_once().unwrap();
    assert!(report.rebuilt_snapshot);
    assert_eq!(
        store
            .sync_device_state(&account_id, &device_id)
            .unwrap()
            .unwrap()
            .acknowledged_cursor
            .as_deref(),
        Some("resume-3")
    );
    assert_eq!(
        transport.acknowledged_cursors.lock().unwrap().as_slice(),
        ["resume-3"]
    );
    assert!(store
        .remote_entity_baseline(&account_id, "question", &stale_entity_id)
        .unwrap()
        .is_none());
}

#[test]
fn authentication_failure_stops_sync_and_marks_credentials_required() {
    let (_directory, store, account_id, device_id) = test_store();
    let transport = Arc::new(MockTransport::default());
    transport
        .pulls
        .lock()
        .unwrap()
        .push_back(Err(TransportError {
            kind: TransportErrorKind::AuthenticationRequired,
            code: "AUTH_UNAUTHORIZED".into(),
        }));
    let worker = SyncWorker::new(
        store.clone(),
        transport,
        account_id.clone(),
        device_id.clone(),
    )
    .unwrap();

    assert!(matches!(
        worker.run_once(),
        Err(SyncWorkerError::Transport(TransportError {
            kind: TransportErrorKind::AuthenticationRequired,
            ..
        }))
    ));
    let state = store
        .sync_device_state(&account_id, &device_id)
        .unwrap()
        .unwrap();
    assert_eq!(state.authentication_state, "required");
    assert_eq!(state.runtime_phase, SyncRuntimePhase::Idle);
}

#[test]
fn persisted_pause_prevents_network_access() {
    let (_directory, store, account_id, device_id) = test_store();
    let transport = Arc::new(MockTransport::default());
    transport
        .pulls
        .lock()
        .unwrap()
        .push_back(Err(fatal("NETWORK_MUST_NOT_BE_CALLED")));
    let worker = SyncWorker::new(
        store.clone(),
        transport,
        account_id.clone(),
        device_id.clone(),
    )
    .unwrap();
    store
        .set_sync_paused(&account_id, &device_id, true)
        .unwrap();

    let report = worker.run_once().unwrap();
    assert!(report.deferred);
    assert_eq!(report.deferred_reason, None);
}
