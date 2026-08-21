//! Recording, naming, and import errors.

use std::path::PathBuf;

/// Errors from native recording, consistency checks, and legacy import.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RecordingError {
    /// A session path with the same stem already exists.
    #[error("session path already exists: {path}")]
    AlreadyExists {
        /// Conflicting path.
        path: PathBuf,
    },
    /// A generated path escaped the output root.
    #[error("path {path} is not under output root {root}")]
    PathEscapesRoot {
        /// Output root.
        root: PathBuf,
        /// Offending path.
        path: PathBuf,
    },
    /// The session name does not match the version 3 stem grammar.
    #[error("invalid session name: {reason}")]
    InvalidName {
        /// Why the name was rejected.
        reason: String,
    },
    /// A version 2 hyphenated name or space-delimited CSV was supplied.
    #[error("unsupported legacy format: {reason}")]
    UnsupportedVersion {
        /// Why the input is unsupported.
        reason: String,
    },
    /// Manifest schema is not version 1.
    #[error("unsupported manifest schema version {version}")]
    UnsupportedSchema {
        /// Observed schema version.
        version: u32,
    },
    /// Native CSV/BIN consistency failed with hard corruption.
    #[error("session is corrupt: {reason}")]
    Corrupt {
        /// Diagnostic.
        reason: String,
    },
    /// A domain value failed validation.
    #[error(transparent)]
    Core(#[from] rngkit_core::CoreError),
    /// Filesystem failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// CSV failure.
    #[error("csv error: {0}")]
    Csv(#[from] csv::Error),
    /// Local UTC offset could not be determined.
    #[error("local utc offset is unavailable")]
    LocalOffsetUnavailable,
    /// Injected or unexpected commit failure.
    #[error("commit failed at {stage}: {reason}")]
    Commit {
        /// Commit stage that failed.
        stage: &'static str,
        /// Diagnostic.
        reason: String,
    },
}
