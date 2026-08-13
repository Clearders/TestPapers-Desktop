use sha2::{Digest, Sha256};

fn sha256(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn canonical_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        serde_json::Value::Object(values) => {
            let mut entries = values.iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(right.0));
            format!(
                "{{{}}}",
                entries
                    .into_iter()
                    .map(|(key, value)| format!(
                        "{}:{}",
                        serde_json::to_string(key).expect("key serializes"),
                        canonical_json(value)
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        value => serde_json::to_string(value).expect("scalar serializes"),
    }
}

#[test]
fn sync_v1_contract_and_canonical_vectors_are_consumable_from_rust() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../contracts/sync-v1.schema.json"))
            .expect("valid schema JSON");
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("../../contracts/sync-v1.fixtures.json"))
            .expect("valid fixture JSON");

    assert_eq!(schema["protocolVersion"], 1);
    assert_eq!(fixtures["protocolVersion"], 1);
    assert_eq!(
        schema["$defs"]["entityType"]["enum"]
            .as_array()
            .unwrap()
            .len(),
        7
    );

    for test_case in fixtures["canonicalCases"].as_array().unwrap() {
        let canonical = canonical_json(&test_case["input"]);
        assert_eq!(canonical, test_case["canonical"].as_str().unwrap());
        assert_eq!(sha256(canonical.as_bytes()), test_case["sha256"]);
    }
}
