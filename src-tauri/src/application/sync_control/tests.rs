use std::{
    sync::{Arc, Barrier},
    time::{Duration, Instant},
};

use serde_json::json;
use tempfile::TempDir;
use uuid::Uuid;

use super::*;
use crate::local_data::{
    CreateQuestion, Difficulty, QuestionContent, QuestionType, ReplicationScope, StoreConfig,
};

fn store() -> (TempDir, Arc<LocalDataStore>, String, String) {
    let directory = tempfile::tempdir().unwrap();
    let account_id = Uuid::now_v7().to_string();
    let device_id = Uuid::now_v7().to_string();
    let store = Arc::new(
        LocalDataStore::open(StoreConfig {
            database_path: directory.path().join("workspace.sqlite3"),
            blob_root: directory.path().join("blobs"),
            workspace_id: Uuid::now_v7().to_string(),
            local_principal_id: account_id.clone(),
        })
        .unwrap(),
    );
    (directory, store, account_id, device_id)
}

fn question(text: &str, replication_scope: ReplicationScope) -> CreateQuestion {
    CreateQuestion {
        owner_id: None,
        replication_scope,
        content: QuestionContent {
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
        },
    }
}

#[test]
fn unauthenticated_snapshot_uses_the_portable_contract() {
    let application = SyncControlApplication::new(Arc::new(|_| {}));
    let snapshot = application.snapshot().unwrap();
    assert_eq!(snapshot.status, SyncStatus::AuthRequired);
    assert_eq!(snapshot.recommended_action, SyncRecommendedAction::SignIn);
    assert!(!snapshot.can_sync_now);
    assert_eq!(
        serde_json::to_value(snapshot).unwrap()["status"],
        "authRequired"
    );
}

#[test]
fn pause_is_durable_and_never_blocks_local_editing() {
    let (_directory, store, account_id, device_id) = store();
    let application = SyncControlApplication::new(Arc::new(|_| {}));
    application
        .install_session(
            Arc::clone(&store),
            account_id.clone(),
            device_id.clone(),
            Arc::new(|| Ok(SyncRunReport::default())),
        )
        .unwrap();

    let paused = application.pause().unwrap();
    assert!(paused.paused);
    assert_eq!(paused.recommended_action, SyncRecommendedAction::Resume);
    assert!(
        store
            .sync_device_state(&account_id, &device_id)
            .unwrap()
            .unwrap()
            .paused
    );

    let saved = store
        .create_question(question(
            "Created while sync is paused",
            ReplicationScope::LocalPrivate,
        ))
        .unwrap();
    assert_eq!(saved.content.text, "Created while sync is paused");
    assert!(!application.resume().unwrap().paused);
}

#[test]
fn manual_cycle_is_backgrounded_and_emits_terminal_state() {
    let (_directory, store, account_id, device_id) = store();
    store
        .create_question(question(
            "Pending Cloud edit",
            ReplicationScope::CloudSynced,
        ))
        .unwrap();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let events = Arc::new(Mutex::new(Vec::new()));
    let application = SyncControlApplication::new({
        let events = Arc::clone(&events);
        Arc::new(move |snapshot| lock(&events).push(snapshot.status))
    });
    application
        .install_session(
            store,
            account_id,
            device_id,
            Arc::new({
                let entered = Arc::clone(&entered);
                let release = Arc::clone(&release);
                move || {
                    entered.wait();
                    release.wait();
                    Ok(SyncRunReport::default())
                }
            }),
        )
        .unwrap();

    let initial = application.sync_now().unwrap();
    assert_eq!(initial.status, SyncStatus::Syncing);
    entered.wait();
    assert_eq!(application.snapshot().unwrap().status, SyncStatus::Syncing);
    release.wait();

    let deadline = Instant::now() + Duration::from_secs(2);
    while application.snapshot().unwrap().status == SyncStatus::Syncing {
        assert!(Instant::now() < deadline, "sync control did not settle");
        thread::sleep(Duration::from_millis(1));
    }
    assert!(lock(&events).contains(&SyncStatus::Syncing));
}

#[test]
fn deferred_transport_reason_maps_without_exposing_payloads() {
    let (_directory, store, account_id, device_id) = store();
    let application = SyncControlApplication::new(Arc::new(|_| {}));
    application
        .install_session(
            store,
            account_id,
            device_id,
            Arc::new(|| {
                Ok(SyncRunReport {
                    deferred: true,
                    deferred_reason: Some(TransportErrorKind::Offline),
                    ..SyncRunReport::default()
                })
            }),
        )
        .unwrap();
    application.sync_now().unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let snapshot = application.snapshot().unwrap();
        if snapshot.status != SyncStatus::Syncing {
            assert_eq!(snapshot.status, SyncStatus::Offline);
            assert_eq!(snapshot.recommended_action, SyncRecommendedAction::Retry);
            assert!(snapshot.last_error_code.is_none());
            break;
        }
        assert!(Instant::now() < deadline, "offline state was not emitted");
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn unstable_remote_error_codes_are_normalized_at_the_public_boundary() {
    assert_eq!(stable_error_code("SYNC_CONFLICT"), "SYNC_CONFLICT");
    assert_eq!(
        stable_error_code("server said no: payload=secret"),
        "SYNC_REMOTE_FAILURE"
    );
}
