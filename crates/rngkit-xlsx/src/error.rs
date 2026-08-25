//! XLSX export errors.

use std::path::PathBuf;

/// Spreadsheet export failures.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum XlsxError {
    /// The source name used for report presentation is not a safe basename.
    #[error("invalid report source basename: {basename}")]
    InvalidSourceBasename {
        /// Rejected source basename.
        basename: String,
    },
    /// Output already exists and overwrite was not selected.
    #[error("xlsx already exists: {path}")]
    AlreadyExists {
        /// Conflicting path.
        path: PathBuf,
    },
    /// Sample count exceeds the Excel worksheet row limit.
    #[error("sample count {count} exceeds excel row limit {limit}")]
    RowLimit {
        /// Requested sample count.
        count: u64,
        /// Maximum data rows (excluding header).
        limit: u64,
    },
    /// Analysis failed.
    #[error(transparent)]
    Analysis(#[from] rngkit_analysis::AnalysisError),
    /// Recording/import failed.
    #[error(transparent)]
    Recording(#[from] rngkit_recording::RecordingError),
    /// Workbook write failed.
    #[error("xlsx write failed: {0}")]
    Workbook(String),
    /// Filesystem failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Core domain error.
    #[error(transparent)]
    Core(#[from] rngkit_core::CoreError),
}
