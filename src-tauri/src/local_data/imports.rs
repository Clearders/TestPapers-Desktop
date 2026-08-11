use std::{collections::BTreeMap, io::Read, str::FromStr};

use csv::{ReaderBuilder, StringRecord};
use rusqlite::TransactionBehavior;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{
    error::{LocalDataError, LocalDataResult},
    model::{Difficulty, EssayBlankSpace, QuestionContent, QuestionType, ReplicationScope},
    questions::insert_question,
    LocalDataStore,
};

pub(crate) const MAX_IMPORT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportRow {
    pub(crate) row_number: usize,
    pub(crate) input: Option<QuestionContent>,
    pub(crate) errors: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportInspection {
    pub(crate) rows: Vec<ImportRow>,
    pub(crate) fatal_error: Option<String>,
}

impl ImportInspection {
    pub(crate) fn valid_count(&self) -> usize {
        self.rows.iter().filter(|row| row.input.is_some()).count()
    }

    pub(crate) fn invalid_count(&self) -> usize {
        self.rows.len().saturating_sub(self.valid_count())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportCommitResult {
    pub(crate) created_ids: Vec<String>,
    pub(crate) invalid_rows: usize,
}

impl LocalDataStore {
    pub(crate) fn inspect_question_import<R: Read>(
        &self,
        mut reader: R,
        file_name: &str,
    ) -> LocalDataResult<ImportInspection> {
        let mut text = String::new();
        let read_result = reader
            .by_ref()
            .take(MAX_IMPORT_BYTES + 1)
            .read_to_string(&mut text);
        if let Err(error) = read_result {
            if error.kind() == std::io::ErrorKind::InvalidData {
                return Ok(fatal("Question imports must be UTF-8 encoded."));
            }
            return Err(error.into());
        }
        if text.len() as u64 > MAX_IMPORT_BYTES {
            return Ok(fatal(format!(
                "Question import exceeds the {MAX_IMPORT_BYTES} byte limit."
            )));
        }
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(fatal("The selected file is empty."));
        }
        if file_name.to_lowercase().ends_with(".json")
            || trimmed.starts_with('[')
            || trimmed.starts_with('{')
        {
            Ok(inspect_json(trimmed))
        } else {
            inspect_csv(trimmed)
        }
    }

    pub(crate) fn commit_question_import(
        &self,
        inspection: &ImportInspection,
    ) -> LocalDataResult<ImportCommitResult> {
        if let Some(error) = &inspection.fatal_error {
            return Err(LocalDataError::Validation(vec![error.clone()]));
        }
        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut created_ids = Vec::with_capacity(inspection.valid_count());
        for row in &inspection.rows {
            let Some(content) = &row.input else {
                continue;
            };
            let record = insert_question(
                &transaction,
                &self.local_principal_id,
                ReplicationScope::LocalPrivate,
                content.clone().normalize()?,
                None,
            )?;
            created_ids.push(record.id);
        }
        transaction.commit()?;
        Ok(ImportCommitResult {
            created_ids,
            invalid_rows: inspection.invalid_count(),
        })
    }
}

fn inspect_json(text: &str) -> ImportInspection {
    let payload: Value = match serde_json::from_str(text) {
        Ok(payload) => payload,
        Err(error) => return fatal(format!("Failed to parse JSON: {error}")),
    };
    let rows = match payload {
        Value::Array(rows) => rows,
        Value::Object(mut object) => match object.remove("questions") {
            Some(Value::Array(rows)) => rows,
            _ => return fatal("JSON import must be an array or an object with a questions array."),
        },
        _ => return fatal("JSON import must be an array or an object with a questions array."),
    };
    ImportInspection {
        fatal_error: None,
        rows: rows
            .into_iter()
            .enumerate()
            .map(|(index, value)| match value {
                Value::Object(object) => normalize_import_record(object, index + 1),
                _ => ImportRow {
                    row_number: index + 1,
                    input: None,
                    errors: vec!["row must be a JSON object".into()],
                },
            })
            .collect(),
    }
}

fn inspect_csv(text: &str) -> LocalDataResult<ImportInspection> {
    let mut reader = ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::None)
        .from_reader(text.as_bytes());
    let raw_headers = match reader.headers() {
        Ok(headers) => headers.clone(),
        Err(error) => return Ok(fatal(format!("Failed to parse CSV header: {error}"))),
    };
    let headers: Vec<_> = raw_headers.iter().map(normalize_header).collect();
    if !headers.iter().any(|header| header == "type")
        || !headers.iter().any(|header| header == "text")
    {
        return Ok(fatal("CSV import requires at least type and text columns."));
    }

    let mut rows = Vec::new();
    for (index, record) in reader.records().enumerate() {
        let record = match record {
            Ok(record) => record,
            Err(error) => {
                rows.push(ImportRow {
                    row_number: index + 2,
                    input: None,
                    errors: vec![format!("invalid CSV row: {error}")],
                });
                continue;
            }
        };
        if record.iter().all(|cell| cell.trim().is_empty()) {
            continue;
        }
        rows.push(normalize_csv_record(&headers, &record, index + 2));
    }
    if rows.is_empty() {
        return Ok(fatal(
            "CSV import requires a header row and at least one question row.",
        ));
    }
    Ok(ImportInspection {
        rows,
        fatal_error: None,
    })
}

fn normalize_csv_record(headers: &[String], record: &StringRecord, row_number: usize) -> ImportRow {
    let mut object = Map::new();
    for (index, header) in headers.iter().enumerate() {
        object.insert(
            header.clone(),
            Value::String(record.get(index).unwrap_or_default().to_owned()),
        );
    }
    normalize_import_record(object, row_number)
}

fn normalize_import_record(object: Map<String, Value>, row_number: usize) -> ImportRow {
    let object: BTreeMap<String, Value> = object
        .into_iter()
        .map(|(key, value)| (normalize_header(&key), value))
        .collect();
    let mut errors = Vec::new();
    let question_type = parse_question_type(value(&object, "type"), &mut errors);
    let difficulty = parse_difficulty(value(&object, "difficulty"), &mut errors);
    let subjects =
        parse_string_list(value(&object, "subjects").or_else(|| value(&object, "subject")));
    let tags = parse_string_list(value(&object, "tags"))
        .into_iter()
        .map(|tag| tag.to_lowercase())
        .collect::<Vec<_>>();
    let text = string_value(value(&object, "text")).trim().to_owned();
    if subjects.is_empty() {
        errors.push("subjects is required".into());
    }
    if text.is_empty() {
        errors.push("text is required".into());
    }

    let (Some(question_type), Some(difficulty)) = (question_type, difficulty) else {
        return ImportRow {
            row_number,
            input: None,
            errors,
        };
    };

    let mut options = parse_string_list(value(&object, "options"));
    if question_type == QuestionType::TrueFalse && options.is_empty() {
        options = vec!["True".into(), "False".into()];
    }
    if question_type.has_options() && options.is_empty() {
        errors.push("options is required for choice and true/false questions".into());
    }

    let answer_value = value(&object, "answer");
    let answer = if question_type == QuestionType::MultipleChoice {
        Value::Array(
            parse_string_list(answer_value)
                .into_iter()
                .map(Value::String)
                .collect(),
        )
    } else {
        let answer = match answer_value {
            Some(Value::Array(values)) => values
                .iter()
                .map(|value| string_value(Some(value)).trim().to_owned())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join(", "),
            other => string_value(other).trim().to_owned(),
        };
        Value::String(answer)
    };

    let score_weight = import_score_weight(
        value(&object, "scoreWeight")
            .or_else(|| value(&object, "weight"))
            .unwrap_or(&Value::String("1".into())),
    );
    let source = string_value(value(&object, "source")).trim().to_owned();
    let has_latex =
        parse_optional_boolean(value(&object, "hasLatex").or_else(|| value(&object, "latex")))
            .unwrap_or_else(|| detect_latex(&text, &answer, &options));
    let essay_blank_space = (question_type == QuestionType::Essay).then(|| EssayBlankSpace {
        lines: bounded_u32(
            value(&object, "essayLines").or_else(|| value(&object, "lines")),
            6,
            1,
            20,
        ),
        line_height: bounded_u32(
            value(&object, "essayLineHeight").or_else(|| value(&object, "lineHeight")),
            28,
            20,
            48,
        ),
    });
    let content = QuestionContent {
        question_type,
        subjects,
        difficulty,
        tags,
        text,
        options: question_type.has_options().then_some(options),
        answer,
        has_latex,
        source: (!source.is_empty()).then_some(source),
        essay_blank_space,
        score_weight,
    };
    match content.normalize() {
        Ok(content) if errors.is_empty() => ImportRow {
            row_number,
            input: Some(content),
            errors,
        },
        Ok(_) => ImportRow {
            row_number,
            input: None,
            errors,
        },
        Err(LocalDataError::Validation(mut validation)) => {
            errors.append(&mut validation);
            errors.sort();
            errors.dedup();
            ImportRow {
                row_number,
                input: None,
                errors,
            }
        }
        Err(error) => ImportRow {
            row_number,
            input: None,
            errors: vec![error.to_string()],
        },
    }
}

fn parse_question_type(value: Option<&Value>, errors: &mut Vec<String>) -> Option<QuestionType> {
    let raw = string_value(value);
    match QuestionType::from_str(raw.trim()) {
        Ok(value) => Some(value),
        Err(_) => {
            errors.push(
                "type must be one of single_choice, multiple_choice, true_false, blank, short_answer, essay"
                    .into(),
            );
            None
        }
    }
}

fn parse_difficulty(value: Option<&Value>, errors: &mut Vec<String>) -> Option<Difficulty> {
    let raw = string_value(value);
    match Difficulty::from_str(raw.trim()) {
        Ok(value) => Some(value),
        Err(_) => {
            errors.push("difficulty must be easy, medium, or hard".into());
            None
        }
    }
}

fn parse_string_list(value: Option<&Value>) -> Vec<String> {
    let values = match value {
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| string_value(Some(value)).trim().to_owned())
            .collect(),
        Some(Value::String(raw)) if raw.trim_start().starts_with('[') => {
            match serde_json::from_str::<Vec<Value>>(raw) {
                Ok(values) => values
                    .iter()
                    .map(|value| string_value(Some(value)).trim().to_owned())
                    .collect(),
                Err(_) => split_list(raw),
            }
        }
        other => split_list(&string_value(other)),
    };
    let mut unique = Vec::new();
    for value in values {
        if !value.is_empty() && !unique.contains(&value) {
            unique.push(value);
        }
    }
    unique
}

fn split_list(raw: &str) -> Vec<String> {
    raw.split([';', ',', '|'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

fn parse_optional_boolean(value: Option<&Value>) -> Option<bool> {
    if let Some(Value::Bool(value)) = value {
        return Some(*value);
    }
    match string_value(value).trim().to_lowercase().as_str() {
        "true" | "1" | "yes" | "y" => Some(true),
        "false" | "0" | "no" | "n" => Some(false),
        _ => None,
    }
}

fn detect_latex(text: &str, answer: &Value, options: &[String]) -> bool {
    fn contains_pair(value: &str) -> bool {
        value
            .find('$')
            .and_then(|first| value[first + 1..].find('$'))
            .is_some()
    }
    contains_pair(text)
        || match answer {
            Value::String(answer) => contains_pair(answer),
            Value::Array(answers) => answers.iter().filter_map(Value::as_str).any(contains_pair),
            _ => false,
        }
        || options.iter().any(|option| contains_pair(option))
}

fn import_score_weight(value: &Value) -> String {
    let parsed = match value {
        Value::Number(number) => number.as_f64(),
        Value::String(value) => value.parse::<f64>().ok(),
        _ => None,
    }
    .filter(|number| number.is_finite() && *number != 0.0)
    .unwrap_or(1.0)
    .clamp(0.01, 100.0);
    format!("{parsed:.4}")
}

fn bounded_u32(value: Option<&Value>, fallback: u32, minimum: u32, maximum: u32) -> u32 {
    let parsed = match value {
        Some(Value::Number(value)) => value.as_f64(),
        Some(Value::String(value)) => value.parse::<f64>().ok(),
        Some(Value::Bool(value)) => Some(f64::from(*value as u8)),
        Some(Value::Null) => Some(0.0),
        _ => None,
    };
    parsed
        .filter(|value| value.is_finite())
        .map(|value| value.trunc().clamp(minimum as f64, maximum as f64) as u32)
        .unwrap_or(fallback)
}

fn value<'a>(object: &'a BTreeMap<String, Value>, key: &str) -> Option<&'a Value> {
    object.get(key)
}

fn string_value(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(value @ (Value::Array(_) | Value::Object(_))) => value.to_string(),
        Some(Value::Null) | None => String::new(),
    }
}

fn normalize_header(value: &str) -> String {
    let value = value.trim().trim_start_matches('\u{feff}');
    let mut output = String::with_capacity(value.len());
    let mut uppercase_next = false;
    for character in value.chars() {
        if character == '-' || character == '_' || character.is_whitespace() {
            uppercase_next = true;
            continue;
        }
        if uppercase_next && character.is_ascii_alphabetic() {
            output.push(character.to_ascii_uppercase());
        } else {
            output.push(character);
        }
        uppercase_next = false;
    }
    output
}

fn fatal(message: impl Into<String>) -> ImportInspection {
    ImportInspection {
        rows: Vec::new(),
        fatal_error: Some(message.into()),
    }
}
