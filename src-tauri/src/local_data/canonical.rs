use std::fmt::Write as _;

use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::error::LocalDataResult;

pub(crate) fn canonical_json<T: Serialize>(value: &T) -> LocalDataResult<String> {
    let value = serde_json::to_value(value)?;
    Ok(serde_json::to_string(&sort_value(value))?)
}

pub(crate) fn content_hash<T: Serialize>(value: &T) -> LocalDataResult<String> {
    Ok(sha256_hex(canonical_json(value)?.as_bytes()))
}

pub(crate) fn domain_content_hash<T: Serialize>(
    entity: &str,
    schema_version: u32,
    content: &T,
) -> LocalDataResult<String> {
    let mut value = serde_json::to_value(content)?;
    let object = value.as_object_mut().ok_or_else(|| {
        super::error::LocalDataError::Corrupt(
            "domain hash payload must serialize as a JSON object".into(),
        )
    })?;
    object.insert("entity".into(), Value::String(entity.into()));
    object.insert("schemaVersion".into(), Value::from(schema_version));
    content_hash(&value)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    digest_hex(Sha256::digest(bytes).as_slice())
}

pub(crate) fn digest_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn sort_value(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut entries: Vec<_> = object.into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let mut sorted = Map::new();
            for (key, value) in entries {
                sorted.insert(key, sort_value(value));
            }
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(sort_value).collect()),
        primitive => primitive,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn canonical_json_sorts_every_object_level() {
        let left = json!({"z": 1, "a": {"z": 2, "a": 3}});
        let right = json!({"a": {"a": 3, "z": 2}, "z": 1});
        assert_eq!(
            canonical_json(&left).unwrap(),
            canonical_json(&right).unwrap()
        );
        assert_eq!(content_hash(&left).unwrap(), content_hash(&right).unwrap());
    }

    #[test]
    fn domain_hash_includes_entity_and_schema_identity() {
        let content = json!({"value": null});
        assert_eq!(
            domain_content_hash("question", 1, &content).unwrap(),
            "dfbc3f4bec97819c4905aad70b6abedb7eb461555d51cd34f0424f33e03c8e36"
        );
        assert_ne!(
            domain_content_hash("question", 1, &content).unwrap(),
            domain_content_hash("paper", 1, &content).unwrap()
        );
        assert_ne!(
            domain_content_hash("question", 1, &content).unwrap(),
            domain_content_hash("question", 2, &content).unwrap()
        );
    }
}
