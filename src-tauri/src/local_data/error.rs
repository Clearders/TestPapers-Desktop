use std::{error::Error, fmt, io, path::PathBuf};

#[derive(Debug)]
pub(crate) enum LocalDataError {
    Io(io::Error),
    Sqlite(rusqlite::Error),
    Json(serde_json::Error),
    Csv(csv::Error),
    Validation(Vec<String>),
    NotFound {
        entity: &'static str,
        id: String,
    },
    StaleBase {
        current_version: i64,
        current_content_hash: String,
        candidate_id: String,
    },
    EntityDeleted {
        entity: &'static str,
        id: String,
    },
    UnsupportedSchema {
        found: u32,
        supported: u32,
    },
    WorkspaceMismatch {
        field: &'static str,
    },
    Busy(String),
    Corrupt(String),
    UnsafePath(PathBuf),
}

impl fmt::Display for LocalDataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "local data I/O failed: {error}"),
            Self::Sqlite(error) => write!(formatter, "local database operation failed: {error}"),
            Self::Json(error) => write!(formatter, "local JSON data is invalid: {error}"),
            Self::Csv(error) => write!(formatter, "question CSV is invalid: {error}"),
            Self::Validation(errors) => write!(formatter, "validation failed: {}", errors.join("; ")),
            Self::NotFound { entity, id } => write!(formatter, "{entity} {id} was not found"),
            Self::StaleBase {
                current_version,
                current_content_hash,
                candidate_id,
            } => write!(
                formatter,
                "the mutation base is stale (current version {current_version}, hash {current_content_hash}); candidate {candidate_id} was preserved"
            ),
            Self::EntityDeleted { entity, id } => {
                write!(formatter, "{entity} {id} is deleted and must be restored before editing")
            }
            Self::UnsupportedSchema { found, supported } => write!(
                formatter,
                "database schema {found} is newer than supported schema {supported}"
            ),
            Self::WorkspaceMismatch { field } => {
                write!(formatter, "the database {field} does not match the selected workspace")
            }
            Self::Busy(message) => write!(formatter, "local data is busy: {message}"),
            Self::Corrupt(message) => write!(formatter, "local data is corrupt: {message}"),
            Self::UnsafePath(path) => write!(formatter, "unsafe local-data path: {}", path.display()),
        }
    }
}

impl Error for LocalDataError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Sqlite(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Csv(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for LocalDataError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<rusqlite::Error> for LocalDataError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<serde_json::Error> for LocalDataError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<csv::Error> for LocalDataError {
    fn from(error: csv::Error) -> Self {
        Self::Csv(error)
    }
}

pub(crate) type LocalDataResult<T> = Result<T, LocalDataError>;
