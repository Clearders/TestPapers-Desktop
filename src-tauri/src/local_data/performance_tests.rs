use std::time::{Duration, Instant};

use rusqlite::params;
use tempfile::TempDir;
use uuid::Uuid;

use super::*;

fn benchmark_store() -> (TempDir, LocalDataStore) {
    let directory = tempfile::tempdir().expect("create benchmark directory");
    let store = LocalDataStore::open(StoreConfig {
        database_path: directory.path().join("workspace.sqlite3"),
        blob_root: directory.path().join("blobs"),
        workspace_id: Uuid::now_v7().to_string(),
        local_principal_id: Uuid::now_v7().to_string(),
    })
    .expect("open benchmark store");
    (directory, store)
}

#[test]
#[ignore = "100,000-question local search performance baseline"]
fn filtered_fts_search_over_100k_questions_stays_interactive() {
    const QUESTION_COUNT: u128 = 100_000;
    const SEARCH_TARGET: Duration = Duration::from_secs(5);

    let (_directory, store) = benchmark_store();
    let owner_id = store.local_principal_id().to_owned();
    let mut connection = store.connection();
    let transaction = connection.transaction().expect("start fixture transaction");
    for index in 0..QUESTION_COUNT {
        let id = Uuid::from_u128(0x018f_0000_0000_7000_8000_0000_0000_0000 + index).to_string();
        let text = if index % 1_000 == 0 {
            format!("benchmark needle quadratic equation {index}")
        } else {
            format!("ordinary algebra question {index}")
        };
        transaction
            .execute(
                "INSERT INTO questions (
                    id, owner_id, replication_scope, schema_version, version, content_hash,
                    created_at, updated_at, deleted_at, deleted_by_id, type, subjects_json,
                    difficulty, tags_json, text, options_json, answer_json, has_latex, source,
                    essay_blank_space_json, score_weight
                 ) VALUES (
                    ?1, ?2, 'local_private', 1, 1, ?3, ?4, ?4, NULL, NULL, 'short_answer',
                    '[\"Mathematics\"]', 'medium', '[\"algebra\"]', ?5, NULL, '\"x = 1\"',
                    0, 'performance-baseline', NULL, '1.0000'
                 )",
                params![id, owner_id, "a".repeat(64), index as i64 + 1, text],
            )
            .expect("insert benchmark question");
    }
    transaction.commit().expect("commit benchmark fixture");
    drop(connection);

    let started = Instant::now();
    let page = store
        .search_questions(QuestionSearch {
            query: Some("benchmark needle".into()),
            subjects: vec!["Mathematics".into()],
            tags: vec!["algebra".into()],
            difficulties: vec![Difficulty::Medium],
            page_size: Some(50),
            ..QuestionSearch::default()
        })
        .expect("search benchmark fixture");
    let elapsed = started.elapsed();
    eprintln!(
        "CLE-26 100k filtered FTS baseline: {} results in {:.3?}",
        page.items.len(),
        elapsed
    );

    assert_eq!(page.items.len(), 50);
    assert!(
        elapsed < SEARCH_TARGET,
        "100k filtered FTS search took {elapsed:.3?}, target is {SEARCH_TARGET:.3?}"
    );
}
