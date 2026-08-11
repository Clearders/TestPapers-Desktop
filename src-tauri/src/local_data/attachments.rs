use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
};

use rusqlite::{params, OptionalExtension, Row, Transaction, TransactionBehavior};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    canonical::{digest_hex, domain_content_hash},
    error::{LocalDataError, LocalDataResult},
    migration::{now_micros, validate_canonical_uuid},
    model::{
        AttachmentRecord, HistoryAction, NewQuestionAttachment, ReplicationScope,
        ENTITY_SCHEMA_VERSION,
    },
    questions::{append_history, append_pending, HistoryWrite, PendingWrite},
    LocalDataStore,
};

pub(crate) const MAX_ATTACHMENT_BYTES: u64 = 30 * 1024 * 1024;

const ATTACHMENT_COLUMNS: &str = "
    a.id, a.owner_id, a.replication_scope, a.schema_version, a.version,
    a.content_hash, a.created_at, a.updated_at, a.deleted_at, a.deleted_by_id,
    a.target_type, a.target_id, a.file_name, a.media_type, a.byte_size,
    a.blob_hash, a.caption, a.position, a.uploaded_by_id";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AttachmentContent<'a> {
    target_type: &'a str,
    target_id: &'a str,
    file_name: &'a str,
    media_type: &'a str,
    byte_size: i64,
    blob_hash: &'a str,
    caption: &'a Option<String>,
    position: u32,
}

struct StagedBlob {
    path: PathBuf,
    hash: String,
    byte_size: i64,
}

impl LocalDataStore {
    pub(crate) fn add_question_attachment<R: Read>(
        &self,
        mut request: NewQuestionAttachment,
        reader: R,
    ) -> LocalDataResult<AttachmentRecord> {
        validate_canonical_uuid(&request.question_id, "questionId")?;
        if let Some(uploaded_by_id) = &request.uploaded_by_id {
            validate_canonical_uuid(uploaded_by_id, "uploadedById")?;
        }
        if request.position > i32::MAX as u32 {
            return Err(LocalDataError::Validation(vec![
                "attachment position exceeds the supported range".into(),
            ]));
        }
        request.file_name = validate_file_name(&request.file_name)?;
        request.media_type = validate_media_type(&request.media_type)?;
        request.caption = request
            .caption
            .map(|caption| caption.trim().to_owned())
            .filter(|caption| !caption.is_empty());

        let staged = stage_blob(&self.blob_root, reader);
        let staged = staged?;
        let result = self.commit_question_attachment(request, &staged);
        let _ = fs::remove_file(&staged.path);
        result
    }

    pub(crate) fn list_question_attachments(
        &self,
        question_id: &str,
        include_deleted: bool,
    ) -> LocalDataResult<Vec<AttachmentRecord>> {
        validate_canonical_uuid(question_id, "questionId")?;
        let connection = self.connection();
        let mut sql = format!(
            "SELECT {ATTACHMENT_COLUMNS}
             FROM attachments a
             JOIN question_attachment_links link ON link.attachment_id = a.id
             WHERE link.question_id = ?1"
        );
        if !include_deleted {
            sql.push_str(" AND a.deleted_at IS NULL");
        }
        sql.push_str(" ORDER BY a.position, a.id");
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map([question_id], read_attachment_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub(crate) fn attachment_blob_path(&self, attachment_id: &str) -> LocalDataResult<PathBuf> {
        validate_canonical_uuid(attachment_id, "attachmentId")?;
        let connection = self.connection();
        let blob_hash: String = connection
            .query_row(
                "SELECT blob_hash FROM attachments WHERE id = ?1 AND deleted_at IS NULL",
                [attachment_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| LocalDataError::NotFound {
                entity: "attachment",
                id: attachment_id.into(),
            })?;
        validate_blob_hash(&blob_hash)?;
        let path = blob_path(&self.blob_root, &blob_hash);
        if !path.is_file() {
            return Err(LocalDataError::Corrupt(format!(
                "attachment {attachment_id} references a missing blob"
            )));
        }
        Ok(path)
    }

    fn commit_question_attachment(
        &self,
        request: NewQuestionAttachment,
        staged: &StagedBlob,
    ) -> LocalDataResult<AttachmentRecord> {
        let mut connection = self.connection();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (owner_id, scope, deleted_at): (String, String, Option<i64>) = transaction
            .query_row(
                "SELECT owner_id, replication_scope, deleted_at FROM questions WHERE id = ?1",
                [&request.question_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or_else(|| LocalDataError::NotFound {
                entity: "question",
                id: request.question_id.clone(),
            })?;
        if deleted_at.is_some() {
            return Err(LocalDataError::EntityDeleted {
                entity: "question",
                id: request.question_id,
            });
        }
        let scope = ReplicationScope::from_str(&scope)?;
        let relative_path = relative_blob_path(&staged.hash);
        let destination = self.blob_root.join(&relative_path);
        let now = now_micros();
        let content = AttachmentContent {
            target_type: "question",
            target_id: &request.question_id,
            file_name: &request.file_name,
            media_type: &request.media_type,
            byte_size: staged.byte_size,
            blob_hash: &staged.hash,
            caption: &request.caption,
            position: request.position,
        };
        let record = AttachmentRecord {
            id: Uuid::now_v7().to_string(),
            owner_id,
            replication_scope: scope,
            schema_version: ENTITY_SCHEMA_VERSION,
            version: 1,
            content_hash: domain_content_hash("attachment", ENTITY_SCHEMA_VERSION, &content)?,
            created_at: now,
            updated_at: now,
            deleted_at: None,
            deleted_by_id: None,
            target_type: "question".into(),
            target_id: request.question_id,
            file_name: request.file_name,
            media_type: request.media_type,
            byte_size: staged.byte_size,
            blob_hash: staged.hash.clone(),
            caption: request.caption,
            position: request.position,
            uploaded_by_id: request.uploaded_by_id,
        };
        let created_blob = install_blob(staged, &destination)?;
        let insert_result = (|| {
            transaction.execute(
                "INSERT INTO attachments(
                    id, owner_id, replication_scope, schema_version, version, content_hash,
                    created_at, updated_at, deleted_at, deleted_by_id, target_type, target_id,
                    file_name, media_type, byte_size, blob_hash, caption, position,
                    uploaded_by_id, relative_path
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, ?9, ?10,
                    ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18
                 )",
                params![
                    record.id,
                    record.owner_id,
                    record.replication_scope.to_string(),
                    record.schema_version,
                    record.version,
                    record.content_hash,
                    record.created_at,
                    record.updated_at,
                    record.target_type,
                    record.target_id,
                    record.file_name,
                    record.media_type,
                    record.byte_size,
                    record.blob_hash,
                    record.caption,
                    record.position,
                    record.uploaded_by_id,
                    relative_path_to_text(&relative_path)?,
                ],
            )?;
            transaction.execute(
                "INSERT INTO question_attachment_links(attachment_id, question_id)
                 VALUES (?1, ?2)",
                params![record.id, record.target_id],
            )?;
            append_history(
                &transaction,
                HistoryWrite {
                    entity_type: "attachment",
                    entity_id: &record.id,
                    version: record.version,
                    hash: &record.content_hash,
                    action: HistoryAction::Create,
                    snapshot: &record,
                    created_at: record.created_at,
                },
            )?;
            append_pending(
                &transaction,
                PendingWrite {
                    scope: record.replication_scope,
                    entity_type: "attachment",
                    entity_id: &record.id,
                    base: None,
                    mutation_kind: "create",
                    candidate: &record,
                    created_at: record.created_at,
                },
            )?;
            transaction.commit()?;
            LocalDataResult::Ok(())
        })();
        if let Err(error) = insert_result {
            if created_blob {
                let _ = fs::remove_file(&destination);
            }
            return Err(error);
        }
        Ok(record)
    }
}

pub(super) fn tombstone_question_attachments(
    transaction: &Transaction<'_>,
    question_id: &str,
    actor_id: &str,
    deleted_at: i64,
) -> LocalDataResult<()> {
    let mut statement = transaction.prepare(&format!(
        "SELECT {ATTACHMENT_COLUMNS}
         FROM attachments a
         JOIN question_attachment_links link ON link.attachment_id = a.id
         WHERE link.question_id = ?1 AND a.deleted_at IS NULL
         ORDER BY a.position, a.id"
    ))?;
    let rows = statement.query_map([question_id], read_attachment_row)?;
    let attachments = rows.collect::<Result<Vec<_>, _>>()?;
    drop(statement);

    for current in attachments {
        let next_version = current
            .version
            .checked_add(1)
            .ok_or_else(|| LocalDataError::Corrupt("attachment version overflow".into()))?;
        let next = AttachmentRecord {
            version: next_version,
            updated_at: deleted_at,
            deleted_at: Some(deleted_at),
            deleted_by_id: Some(actor_id.into()),
            ..current.clone()
        };
        transaction.execute(
            "UPDATE attachments
             SET version = ?2, updated_at = ?3, deleted_at = ?4, deleted_by_id = ?5
             WHERE id = ?1",
            params![
                next.id,
                next.version,
                next.updated_at,
                next.deleted_at,
                next.deleted_by_id,
            ],
        )?;
        append_history(
            transaction,
            HistoryWrite {
                entity_type: "attachment",
                entity_id: &next.id,
                version: next.version,
                hash: &next.content_hash,
                action: HistoryAction::Delete,
                snapshot: &next,
                created_at: deleted_at,
            },
        )?;
        append_pending(
            transaction,
            PendingWrite {
                scope: next.replication_scope,
                entity_type: "attachment",
                entity_id: &next.id,
                base: Some((&current.version, &current.content_hash)),
                mutation_kind: "delete",
                candidate: &next,
                created_at: deleted_at,
            },
        )?;
    }
    Ok(())
}

fn stage_blob<R: Read>(blob_root: &Path, mut reader: R) -> LocalDataResult<StagedBlob> {
    let staging_dir = blob_root.join(".staging");
    fs::create_dir_all(&staging_dir)?;
    let path = staging_dir.join(format!("{}.part", Uuid::now_v7()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let mut digest = Sha256::new();
        let mut byte_size = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            byte_size = byte_size.checked_add(read as u64).ok_or_else(|| {
                LocalDataError::Validation(vec!["attachment is too large".into()])
            })?;
            if byte_size > MAX_ATTACHMENT_BYTES {
                return Err(LocalDataError::Validation(vec![format!(
                    "attachment exceeds the {} byte limit",
                    MAX_ATTACHMENT_BYTES
                )]));
            }
            digest.update(&buffer[..read]);
            file.write_all(&buffer[..read])?;
        }
        file.flush()?;
        file.sync_all()?;
        Ok(StagedBlob {
            path: path.clone(),
            hash: digest_hex(&digest.finalize()),
            byte_size: i64::try_from(byte_size).unwrap_or(i64::MAX),
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&path);
    }
    result
}

fn install_blob(staged: &StagedBlob, destination: &Path) -> LocalDataResult<bool> {
    if destination.exists() {
        verify_existing_blob(destination, &staged.hash, staged.byte_size)?;
        return Ok(false);
    }
    let parent = destination
        .parent()
        .ok_or_else(|| LocalDataError::UnsafePath(destination.to_owned()))?;
    fs::create_dir_all(parent)?;
    match fs::rename(&staged.path, destination) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            verify_existing_blob(destination, &staged.hash, staged.byte_size)?;
            Ok(false)
        }
        Err(error) => Err(error.into()),
    }
}

fn verify_existing_blob(
    path: &Path,
    expected_hash: &str,
    expected_size: i64,
) -> LocalDataResult<()> {
    let metadata = fs::metadata(path)?;
    if metadata.len() != expected_size as u64 {
        return Err(LocalDataError::Corrupt(format!(
            "content-addressed blob {} has an unexpected size",
            path.display()
        )));
    }
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    if digest_hex(&digest.finalize()) != expected_hash {
        return Err(LocalDataError::Corrupt(format!(
            "content-addressed blob {} failed SHA-256 verification",
            path.display()
        )));
    }
    Ok(())
}

fn read_attachment_row(row: &Row<'_>) -> rusqlite::Result<AttachmentRecord> {
    let scope: String = row.get(2)?;
    let position: i64 = row.get(17)?;
    Ok(AttachmentRecord {
        id: row.get(0)?,
        owner_id: row.get(1)?,
        replication_scope: ReplicationScope::from_str(&scope).map_err(sql_decode_error)?,
        schema_version: row.get(3)?,
        version: row.get(4)?,
        content_hash: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        deleted_at: row.get(8)?,
        deleted_by_id: row.get(9)?,
        target_type: row.get(10)?,
        target_id: row.get(11)?,
        file_name: row.get(12)?,
        media_type: row.get(13)?,
        byte_size: row.get(14)?,
        blob_hash: row.get(15)?,
        caption: row.get(16)?,
        position: u32::try_from(position).map_err(sql_decode_error)?,
        uploaded_by_id: row.get(18)?,
    })
}

fn validate_file_name(value: &str) -> LocalDataResult<String> {
    let value = value.trim();
    if value.is_empty()
        || value.chars().count() > 255
        || value.chars().any(char::is_control)
        || value.contains('/')
        || value.contains('\\')
    {
        return Err(LocalDataError::Validation(vec![
            "fileName must be a 1..255 character display name without path separators".into(),
        ]));
    }
    Ok(value.into())
}

fn validate_media_type(value: &str) -> LocalDataResult<String> {
    let value = value.trim().to_lowercase();
    let valid = value.len() <= 255
        && value.split_once('/').is_some_and(|(kind, subtype)| {
            !kind.is_empty()
                && !subtype.is_empty()
                && kind.bytes().all(media_token_byte)
                && subtype.bytes().all(media_token_byte)
        });
    if !valid {
        return Err(LocalDataError::Validation(vec![
            "mediaType must be a valid type/subtype token".into(),
        ]));
    }
    Ok(value)
}

fn media_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#' | b'$' | b'&' | b'^' | b'_' | b'.' | b'+' | b'-'
        )
}

fn validate_blob_hash(value: &str) -> LocalDataResult<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(LocalDataError::Corrupt(
            "invalid attachment blob hash".into(),
        ))
    }
}

fn relative_blob_path(hash: &str) -> PathBuf {
    PathBuf::from("sha256").join(&hash[..2]).join(hash)
}

fn blob_path(blob_root: &Path, hash: &str) -> PathBuf {
    blob_root.join(relative_blob_path(hash))
}

fn relative_path_to_text(path: &Path) -> LocalDataResult<String> {
    let components = path
        .components()
        .map(|component| component.as_os_str().to_str())
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| LocalDataError::UnsafePath(path.to_owned()))?;
    Ok(components.join("/"))
}

fn sql_decode_error(error: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(io::Error::new(
            io::ErrorKind::InvalidData,
            error.to_string(),
        )),
    )
}
