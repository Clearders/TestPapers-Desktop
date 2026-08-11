use std::{fs, io::Cursor};

use rusqlite::Connection;
use serde_json::json;
use tempfile::TempDir;
use uuid::Uuid;

use crate::workspace_features::paper::{
    AnswerSnapshot as PaperAnswer, Difficulty as PaperDifficulty, PaperItemSnapshot, PaperSnapshot,
    PaperSnapshotStore, PaperStatus, QuestionSnapshot as PaperQuestion,
    QuestionType as PaperQuestionType, ReplicationScope as PaperScope, StoreCandidateError,
    PAPER_SCHEMA_VERSION,
};

use super::*;

fn test_store() -> (TempDir, LocalDataStore) {
    let directory = tempfile::tempdir().unwrap();
    let store = LocalDataStore::open(StoreConfig {
        database_path: directory.path().join("workspace.sqlite3"),
        blob_root: directory.path().join("blobs"),
        workspace_id: Uuid::now_v7().to_string(),
        local_principal_id: Uuid::now_v7().to_string(),
    })
    .unwrap();
    (directory, store)
}

fn content(text: &str) -> QuestionContent {
    QuestionContent {
        question_type: QuestionType::ShortAnswer,
        subjects: vec!["Mathematics".into()],
        difficulty: Difficulty::Medium,
        tags: vec!["algebra".into()],
        text: text.into(),
        options: None,
        answer: json!("x = 1"),
        has_latex: true,
        source: Some("Fixture".into()),
        essay_blank_space: None,
        score_weight: "1.0000".into(),
    }
}

fn mutation_base(question: &QuestionRecord) -> MutationBase {
    MutationBase {
        base_version: question.version,
        base_content_hash: question.content_hash.clone(),
    }
}

fn paper_candidate(owner_id: &str, scope: PaperScope) -> PaperSnapshot {
    let question = PaperQuestion {
        id: "018f0000-0000-7000-8000-000000000103".into(),
        version: 1,
        content_hash: "a".repeat(64),
        question_type: PaperQuestionType::ShortAnswer,
        subjects: vec!["Mathematics".into()],
        difficulty: PaperDifficulty::Medium,
        tags: vec!["algebra".into()],
        text: "What is 2 + 2?".into(),
        options: None,
        answer: PaperAnswer::Text("4".into()),
        has_latex: false,
        source: None,
        essay_blank_space: None,
        score_weight: "1".into(),
        attachments: vec![],
    };
    PaperSnapshot {
        id: "018f0000-0000-7000-8000-000000000101".into(),
        owner_id: owner_id.into(),
        replication_scope: scope,
        schema_version: PAPER_SCHEMA_VERSION,
        version: 99,
        content_hash: "b".repeat(64),
        created_at_micros: 123,
        updated_at_micros: 456,
        deleted_at_micros: None,
        title: "Midterm".into(),
        subject: "Mathematics".into(),
        duration_minutes: 60,
        total_marks: "10".into(),
        status: PaperStatus::Draft,
        items: vec![
            PaperItemSnapshot {
                id: "018f0000-0000-7000-8000-000000000104".into(),
                question_id: None,
                order: 0,
                marks: Some("4".into()),
                question_snapshot: question.clone(),
            },
            PaperItemSnapshot {
                id: "018f0000-0000-7000-8000-000000000105".into(),
                question_id: None,
                order: 1,
                marks: Some("6".into()),
                question_snapshot: question,
            },
        ],
    }
}

#[test]
fn new_database_is_migrated_and_bound_to_workspace_identity() {
    let (directory, store) = test_store();
    assert_eq!(store.migration_report().from_version, 0);
    assert_eq!(store.migration_report().to_version, LATEST_SCHEMA_VERSION);
    assert!(store.migration_report().rollback_path.is_none());
    store.verify_integrity().unwrap();

    let tables = store
        .connection()
        .prepare(
            "SELECT name FROM sqlite_master
             WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'",
        )
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    for required in [
        "workspace_meta",
        "questions",
        "question_subjects",
        "question_tags",
        "questions_fts",
        "papers",
        "paper_items",
        "drafts",
        "attachments",
        "comments",
        "favorites",
        "settings",
        "entity_history",
        "pending_mutations",
    ] {
        assert!(
            tables.iter().any(|table| table == required),
            "missing {required}"
        );
    }

    let mismatched = LocalDataStore::open(StoreConfig {
        database_path: directory.path().join("workspace.sqlite3"),
        blob_root: directory.path().join("blobs"),
        workspace_id: Uuid::now_v7().to_string(),
        local_principal_id: store.local_principal_id().into(),
    });
    assert!(matches!(
        mismatched,
        Err(LocalDataError::WorkspaceMismatch {
            field: "workspaceId"
        })
    ));
}

#[test]
fn version_zero_database_migrates_through_staging_and_keeps_rollback() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("workspace.sqlite3");
    let connection = Connection::open(&database_path).unwrap();
    connection
        .execute_batch("CREATE TABLE legacy_marker(value TEXT); PRAGMA user_version = 0;")
        .unwrap();
    drop(connection);

    let store = LocalDataStore::open(StoreConfig {
        database_path,
        blob_root: directory.path().join("blobs"),
        workspace_id: Uuid::now_v7().to_string(),
        local_principal_id: Uuid::now_v7().to_string(),
    })
    .unwrap();
    let rollback = store.migration_report().rollback_path.as_ref().unwrap();
    assert!(rollback.is_file());
    assert_eq!(
        Connection::open(rollback)
            .unwrap()
            .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
            .unwrap(),
        0
    );
    store.verify_integrity().unwrap();
}

#[test]
fn interrupted_swap_recovers_rollback_before_migrating() {
    let directory = tempfile::tempdir().unwrap();
    let database_path = directory.path().join("workspace.sqlite3");
    let interrupted_rollback = directory
        .path()
        .join("workspace.sqlite3.rollback-v0-interrupted");
    let connection = Connection::open(&interrupted_rollback).unwrap();
    connection
        .execute_batch("CREATE TABLE before_interruption(value TEXT); PRAGMA user_version = 0;")
        .unwrap();
    drop(connection);

    let store = LocalDataStore::open(StoreConfig {
        database_path,
        blob_root: directory.path().join("blobs"),
        workspace_id: Uuid::now_v7().to_string(),
        local_principal_id: Uuid::now_v7().to_string(),
    })
    .unwrap();
    assert_eq!(store.migration_report().from_version, 0);
    assert_eq!(
        store
            .connection()
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'before_interruption'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );
}

#[test]
fn question_lifecycle_is_versioned_and_stale_candidate_is_preserved() {
    let (_directory, store) = test_store();
    let created = store
        .create_question(CreateQuestion {
            owner_id: None,
            replication_scope: ReplicationScope::CloudSynced,
            content: content("Solve $x + 1 = 2$."),
        })
        .unwrap();
    assert_eq!(created.version, 1);
    assert_eq!(created.content.score_weight, "1");
    assert_eq!(store.list_pending_mutations(10).unwrap().len(), 1);

    let no_op = store
        .update_question(
            &created.id,
            UpdateQuestion {
                mutation_base: mutation_base(&created),
                content: created.content.clone(),
            },
        )
        .unwrap();
    assert_eq!(no_op.version, 1);

    let mut changed_content = created.content.clone();
    changed_content.text = "Solve $x + 2 = 4$.".into();
    let updated = store
        .update_question(
            &created.id,
            UpdateQuestion {
                mutation_base: mutation_base(&created),
                content: changed_content,
            },
        )
        .unwrap();
    assert_eq!(updated.version, 2);
    assert_ne!(updated.content_hash, created.content_hash);

    let stale = store.update_question(
        &created.id,
        UpdateQuestion {
            mutation_base: mutation_base(&created),
            content: content("A competing edit"),
        },
    );
    assert!(matches!(stale, Err(LocalDataError::StaleBase { .. })));
    assert_eq!(
        store
            .connection()
            .query_row("SELECT count(*) FROM conflict_candidates", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        1
    );

    let reverted = store
        .revert_question(&created.id, 1, mutation_base(&updated))
        .unwrap();
    assert_eq!(reverted.version, 3);
    assert_eq!(reverted.content.text, created.content.text);
    assert_eq!(store.list_question_revisions(&created.id).unwrap().len(), 3);

    let deleted = store
        .delete_question(
            &created.id,
            mutation_base(&reverted),
            store.local_principal_id(),
        )
        .unwrap();
    assert_eq!(deleted.version, 4);
    assert!(deleted.deleted_at.is_some());
    let restored = store
        .restore_question(&created.id, mutation_base(&deleted))
        .unwrap();
    assert_eq!(restored.version, 5);
    assert!(restored.deleted_at.is_none());
    assert_eq!(store.list_pending_mutations(20).unwrap().len(), 5);
}

#[test]
fn malformed_mutation_bases_are_rejected_without_preserving_conflicts() {
    let (_directory, store) = test_store();
    let question = store
        .create_question(CreateQuestion {
            owner_id: None,
            replication_scope: ReplicationScope::LocalPrivate,
            content: content("Mutation base validation"),
        })
        .unwrap();
    assert!(matches!(
        store.update_question(
            &question.id,
            UpdateQuestion {
                mutation_base: MutationBase {
                    base_version: 0,
                    base_content_hash: "not-a-hash".into(),
                },
                content: content("Invalid competing edit"),
            },
        ),
        Err(LocalDataError::Validation(_))
    ));

    let paper = store
        .create_paper_snapshot(&paper_candidate(
            store.local_principal_id(),
            PaperScope::LocalPrivate,
        ))
        .unwrap();
    assert!(matches!(
        PaperSnapshotStore::accept_candidate(&store, 0, "not-a-hash", &paper),
        Err(StoreCandidateError::Rejected(_))
    ));
    assert_eq!(
        store
            .connection()
            .query_row("SELECT count(*) FROM conflict_candidates", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        0
    );
}

#[test]
fn fts_and_exact_filters_return_cursor_pages() {
    let (_directory, store) = test_store();
    for (text, subject, tag, difficulty) in [
        (
            "Quadratic equation roots",
            "Mathematics",
            "algebra",
            Difficulty::Hard,
        ),
        (
            "Linear equation",
            "Mathematics",
            "algebra",
            Difficulty::Easy,
        ),
        ("Cell membrane", "Biology", "cells", Difficulty::Medium),
    ] {
        let mut question = content(text);
        question.subjects = vec![subject.into()];
        question.tags = vec![tag.into()];
        question.difficulty = difficulty;
        store
            .create_question(CreateQuestion {
                owner_id: None,
                replication_scope: ReplicationScope::LocalPrivate,
                content: question,
            })
            .unwrap();
    }

    let result = store
        .search_questions(QuestionSearch {
            query: Some("Quadratic equation".into()),
            subjects: vec!["Mathematics".into()],
            tags: vec!["algebra".into()],
            difficulties: vec![Difficulty::Hard],
            ..QuestionSearch::default()
        })
        .unwrap();
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].content.text, "Quadratic equation roots");

    let first = store
        .search_questions(QuestionSearch {
            page_size: Some(2),
            ..QuestionSearch::default()
        })
        .unwrap();
    assert_eq!(first.items.len(), 2);
    let second = store
        .search_questions(QuestionSearch {
            cursor: first.next_cursor,
            page_size: Some(2),
            ..QuestionSearch::default()
        })
        .unwrap();
    assert_eq!(second.items.len(), 1);
}

#[test]
fn csv_import_matches_web_aliases_and_commits_valid_rows_atomically() {
    let (_directory, store) = test_store();
    let csv = concat!(
        "type,subject,difficulty,tags,text,options,answer,weight,latex,essay_lines,essay_line_height\n",
        "true_false,Science,easy,logic,\"A quoted, multiline\nstatement\",,True,2,yes,,\n",
        "essay,English,medium,writing,Explain the theme,,A response,1,,,100\n",
        "unknown,Math,hard,bad,Invalid row,,answer,1,,,\n"
    );
    let inspection = store
        .inspect_question_import(Cursor::new(csv), "questions.csv")
        .unwrap();
    assert!(inspection.fatal_error.is_none());
    assert_eq!(inspection.valid_count(), 2);
    assert_eq!(inspection.invalid_count(), 1);
    assert_eq!(
        inspection.rows[0]
            .input
            .as_ref()
            .unwrap()
            .options
            .as_ref()
            .unwrap(),
        &["True", "False"]
    );
    assert_eq!(
        inspection.rows[1]
            .input
            .as_ref()
            .unwrap()
            .essay_blank_space
            .as_ref()
            .unwrap()
            .line_height,
        48
    );

    let committed = store.commit_question_import(&inspection).unwrap();
    assert_eq!(committed.created_ids.len(), 2);
    assert_eq!(committed.invalid_rows, 1);
    assert_eq!(
        store
            .search_questions(QuestionSearch::default())
            .unwrap()
            .items
            .len(),
        2
    );
}

#[test]
fn json_import_accepts_array_and_questions_wrapper() {
    let (_directory, store) = test_store();
    let row = json!({
        "type": "short_answer",
        "subjects": ["Mathematics"],
        "difficulty": "easy",
        "tags": "latex|algebra",
        "text": "Solve $x=1$",
        "answer": "x=1",
        "scoreWeight": 0
    });
    let array = serde_json::to_vec(&json!([row.clone()])).unwrap();
    let wrapped = serde_json::to_vec(&json!({"questions": [row]})).unwrap();
    let first = store
        .inspect_question_import(Cursor::new(array), "questions.json")
        .unwrap();
    let second = store
        .inspect_question_import(Cursor::new(wrapped), "questions.json")
        .unwrap();
    assert_eq!(first.valid_count(), 1);
    assert_eq!(second.valid_count(), 1);
    assert_eq!(first.rows[0].input.as_ref().unwrap().score_weight, "1");
    assert!(first.rows[0].input.as_ref().unwrap().has_latex);
}

#[test]
fn attachment_bytes_are_deduplicated_and_tombstoned_with_question() {
    let (_directory, store) = test_store();
    let question = store
        .create_question(CreateQuestion {
            owner_id: None,
            replication_scope: ReplicationScope::CloudSynced,
            content: content("Question with two references to one image"),
        })
        .unwrap();
    let first = store
        .add_question_attachment(
            NewQuestionAttachment {
                question_id: question.id.clone(),
                file_name: "diagram.png".into(),
                media_type: "image/png".into(),
                caption: Some("Diagram".into()),
                position: 0,
                uploaded_by_id: Some(store.local_principal_id().into()),
            },
            Cursor::new(b"same bytes"),
        )
        .unwrap();
    let second = store
        .add_question_attachment(
            NewQuestionAttachment {
                question_id: question.id.clone(),
                file_name: "diagram-copy.png".into(),
                media_type: "image/png".into(),
                caption: None,
                position: 1,
                uploaded_by_id: None,
            },
            Cursor::new(b"same bytes"),
        )
        .unwrap();
    assert_eq!(first.blob_hash, second.blob_hash);
    assert_eq!(
        store.attachment_blob_path(&first.id).unwrap(),
        store.attachment_blob_path(&second.id).unwrap()
    );
    assert_eq!(
        store
            .list_question_attachments(&question.id, false)
            .unwrap()
            .len(),
        2
    );

    store
        .delete_question(
            &question.id,
            mutation_base(&question),
            store.local_principal_id(),
        )
        .unwrap();
    assert!(store
        .list_question_attachments(&question.id, false)
        .unwrap()
        .is_empty());
    assert_eq!(
        store
            .list_question_attachments(&question.id, true)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn paper_store_stamps_envelope_and_accepts_items_atomically() {
    let (_directory, store) = test_store();
    let candidate = paper_candidate(store.local_principal_id(), PaperScope::CloudSynced);
    let created = store.create_paper_snapshot(&candidate).unwrap();
    assert_eq!(created.version, 1);
    assert_ne!(created.content_hash, candidate.content_hash);
    assert_eq!(created.created_at_micros, created.updated_at_micros);
    assert_eq!(
        store.load_paper_snapshot(&created.id).unwrap(),
        Some(created.clone())
    );

    let mut changed = created.clone();
    changed.title = "Updated Midterm".into();
    changed.items.reverse();
    let accepted = PaperSnapshotStore::accept_candidate(
        &store,
        created.version,
        &created.content_hash,
        &changed,
    )
    .unwrap();
    assert_eq!(accepted.version, 2);
    assert_eq!(accepted.items[0].order, 0);
    assert_eq!(accepted.items[1].order, 1);
    assert_eq!(
        store
            .connection()
            .query_row(
                "SELECT count(*) FROM paper_items WHERE paper_id = ?1",
                [&accepted.id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        2
    );
    assert_eq!(store.list_pending_mutations(10).unwrap().len(), 2);

    let no_op = PaperSnapshotStore::accept_candidate(
        &store,
        accepted.version,
        &accepted.content_hash,
        &accepted,
    )
    .unwrap();
    assert_eq!(no_op, accepted);
    assert_eq!(
        store
            .connection()
            .query_row(
                "SELECT count(*) FROM entity_history WHERE entity_type = 'paper'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        2
    );
}

#[test]
fn paper_store_preserves_stale_candidate_and_rejects_missing_provenance_atomically() {
    let (_directory, store) = test_store();
    let candidate = paper_candidate(store.local_principal_id(), PaperScope::LocalPrivate);
    let created = store.create_paper_snapshot(&candidate).unwrap();
    let mut changed = created.clone();
    changed.title = "First edit".into();
    let accepted = store
        .accept_paper_candidate(created.version, &created.content_hash, &changed)
        .unwrap();

    let mut stale_candidate = created.clone();
    stale_candidate.title = "Competing edit".into();
    assert_eq!(
        PaperSnapshotStore::accept_candidate(
            &store,
            created.version,
            &created.content_hash,
            &stale_candidate,
        ),
        Err(StoreCandidateError::StaleBase)
    );
    assert_eq!(
        store
            .connection()
            .query_row(
                "SELECT count(*) FROM conflict_candidates WHERE entity_type = 'paper'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1
    );

    let mut invalid = paper_candidate(store.local_principal_id(), PaperScope::LocalPrivate);
    invalid.id = "018f0000-0000-7000-8000-000000000201".into();
    invalid.items[0].question_id = Some(invalid.items[0].question_snapshot.id.clone());
    assert!(store.create_paper_snapshot(&invalid).is_err());
    assert!(store.load_paper_snapshot(&invalid.id).unwrap().is_none());
    assert_eq!(
        store.load_paper_snapshot(&accepted.id).unwrap(),
        Some(accepted)
    );

    let mut out_of_range = paper_candidate(store.local_principal_id(), PaperScope::LocalPrivate);
    out_of_range.id = "018f0000-0000-7000-8000-000000000202".into();
    out_of_range.total_marks = "0.001".into();
    assert!(matches!(
        store.create_paper_snapshot(&out_of_range),
        Err(LocalDataError::Validation(_))
    ));
}

#[test]
fn online_snapshot_is_self_contained_and_inventories_verified_blobs() {
    let (directory, store) = test_store();
    let question = store
        .create_question(CreateQuestion {
            owner_id: None,
            replication_scope: ReplicationScope::LocalPrivate,
            content: content("Backed up question"),
        })
        .unwrap();
    let attachment = store
        .add_question_attachment(
            NewQuestionAttachment {
                question_id: question.id,
                file_name: "backup.png".into(),
                media_type: "image/png".into(),
                caption: None,
                position: 0,
                uploaded_by_id: None,
            },
            Cursor::new(b"backup blob"),
        )
        .unwrap();
    let destination = directory.path().join("backup").join("workspace.sqlite3");
    let inventory = store.snapshot_to(&destination).unwrap();
    assert_eq!(inventory.live_entity_counts["questions"], 1);
    assert_eq!(inventory.live_entity_counts["attachments"], 1);
    assert_eq!(inventory.blobs.len(), 1);
    assert_eq!(inventory.blobs[0].blob_hash, attachment.blob_hash);
    assert_eq!(
        inventory.blobs[0].archive_relative_path,
        format!(
            "blobs/sha256/{}/{}",
            &attachment.blob_hash[..2],
            attachment.blob_hash
        )
    );
    assert!(inventory.blobs[0].source_path.is_file());
    assert_eq!(
        Connection::open(&destination)
            .unwrap()
            .query_row("SELECT count(*) FROM questions", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        1
    );
    assert!(matches!(
        store.snapshot_to(&destination),
        Err(LocalDataError::UnsafePath(_))
    ));

    fs::write(&inventory.blobs[0].source_path, b"tampered").unwrap();
    assert!(matches!(
        store.backup_inventory(),
        Err(LocalDataError::Corrupt(_))
    ));
}
