use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn sha256(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn run_scenario(scenario: &Value) -> Value {
    let mut entities = BTreeMap::<String, Value>::new();
    let mut conflicts = Vec::new();
    let mut results = Vec::new();
    for operation in scenario["operations"].as_array().expect("operations") {
        let entity_type = operation["entityType"].as_str().expect("entity type");
        let entity_id = operation["entityId"].as_str().expect("entity id");
        let operation_id = operation["operationId"].as_str().expect("operation id");
        let kind = operation["kind"].as_str().expect("kind");
        let key = format!("{entity_type}:{entity_id}");
        let Some(entity) = entities.get_mut(&key) else {
            assert!(matches!(kind, "create" | "attach"));
            assert!(operation.get("baseVersion").is_none());
            entities.insert(
                key,
                json!({
                    "entityType": entity_type,
                    "entityId": entity_id,
                    "version": 1,
                    "tombstone": false,
                    "payload": operation.get("payload").cloned().unwrap_or(Value::Null)
                }),
            );
            results.push(
                json!({"operationId": operation_id, "status": "applied", "acceptedVersion": 1}),
            );
            continue;
        };
        let cloud_version = entity["version"].as_u64().expect("version");
        let base_version = operation.get("baseVersion").and_then(Value::as_u64);
        if base_version != Some(cloud_version) {
            let reason = if entity["tombstone"] == true {
                "tombstoneDivergence"
            } else {
                "divergentContent"
            };
            conflicts.push(json!({
                "operationId": operation_id,
                "device": operation["device"],
                "entityType": entity_type,
                "entityId": entity_id,
                "kind": kind,
                "baseVersion": operation["baseVersion"],
                "cloudVersion": cloud_version,
                "reason": reason
            }));
            results.push(json!({"operationId": operation_id, "status": "conflict", "cloudVersion": cloud_version, "reason": reason}));
            continue;
        }
        assert_ne!(kind, "create");
        let accepted_version = cloud_version + 1;
        entity["version"] = json!(accepted_version);
        if matches!(kind, "delete" | "detach") {
            entity["tombstone"] = json!(true);
        } else {
            entity["tombstone"] = json!(false);
            entity["payload"] = operation.get("payload").cloned().unwrap_or(Value::Null);
        }
        results.push(json!({"operationId": operation_id, "status": "applied", "acceptedVersion": accepted_version}));
    }
    json!({
        "entities": entities.into_values().collect::<Vec<_>>(),
        "conflicts": conflicts,
        "operationResults": results
    })
}

fn failure_message(scenario: &Value, actual: &Value) -> String {
    format!(
        "sync consistency mismatch seed={} operations={} diff={}",
        scenario["seed"],
        scenario["operations"],
        json!({"expected": scenario["expected"], "actual": actual})
    )
}

fn bundle() -> (Value, Value, Value) {
    let schema_bytes = include_bytes!("../../contracts/sync-consistency-v1.schema.json");
    let fixture_bytes = include_bytes!("../../contracts/sync-consistency-v1.fixtures.json");
    let schema: Value = serde_json::from_slice(schema_bytes).expect("scenario schema");
    let fixtures: Value = serde_json::from_slice(fixture_bytes).expect("scenario fixtures");
    let lock: Value = serde_json::from_str(include_str!(
        "../../contracts/sync-consistency-v1.lock.json"
    ))
    .expect("scenario lock");
    let schema_hash = sha256(schema_bytes);
    let fixture_hash = sha256(fixture_bytes);
    assert_eq!(lock["schemaSha256"], schema_hash);
    assert_eq!(lock["fixturesSha256"], fixture_hash);
    assert_eq!(
        lock["semanticFingerprint"],
        sha256(format!("{schema_hash}:{fixture_hash}").as_bytes())
    );
    assert_eq!(
        schema["properties"]["dslVersion"]["const"],
        fixtures["dslVersion"]
    );
    assert_eq!(fixtures["dslVersion"], lock["dslVersion"]);
    (schema, fixtures, lock)
}

#[test]
fn fixed_scenarios_converge_to_the_pinned_cross_runtime_result() {
    let (_, fixtures, _) = bundle();
    let scenarios = fixtures["scenarios"].as_array().expect("scenarios");
    let mut scenario_ids = BTreeSet::new();
    let mut operation_ids = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    for scenario in scenarios {
        assert!(scenario_ids.insert(scenario["id"].as_str().unwrap()));
        let devices = scenario["devices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<BTreeSet<_>>();
        for operation in scenario["operations"].as_array().unwrap() {
            assert!(operation_ids.insert(operation["operationId"].as_str().unwrap()));
            assert!(devices.contains(operation["device"].as_str().unwrap()));
            kinds.insert(operation["kind"].as_str().unwrap());
        }
        let actual = run_scenario(scenario);
        assert_eq!(
            actual,
            scenario["expected"],
            "{}",
            failure_message(scenario, &actual)
        );
    }
    assert_eq!(
        kinds,
        BTreeSet::from(["attach", "create", "delete", "detach", "restore", "update"])
    );
}

#[test]
fn mismatch_diagnostic_contains_seed_operations_and_state_diff() {
    let (_, fixtures, _) = bundle();
    let scenario = &fixtures["scenarios"][0];
    let mut actual = run_scenario(scenario);
    actual["entities"][0]["version"] = json!(999);
    let diagnostic = failure_message(scenario, &actual);
    assert!(diagnostic.contains(&format!("seed={}", scenario["seed"])));
    assert!(diagnostic.contains("operations="));
    assert!(diagnostic.contains("\"expected\"") && diagnostic.contains("\"actual\""));
}
