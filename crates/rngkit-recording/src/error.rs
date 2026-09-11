//! Recording, naming, and import errors.

use std::fmt;
use std::path::PathBuf;

/// Compatibility field that differed between concatenation inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcatenationCompatibilityField {
    /// Source identifier.
    Source,
    /// Sample bit length.
    SampleBits,
    /// Sample interval.
    Interval,
    /// BitBabbler fold.
    Fold,
}

impl fmt::Display for ConcatenationCompatibilityField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Source => "source",
            Self::SampleBits => "sample bits",
            Self::Interval => "interval",
            Self::Fold => "fold",
        })
    }
}

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
    /// A legacy CSV one-count exceeds the sample bit length from the filename.
    #[error("ones count {ones} exceeds sample bits {sample_bits}")]
    OnesExceedSampleBits {
        /// Observed one-count.
        ones: u64,
        /// Declared sample size in bits.
        sample_bits: u32,
    },
    /// No concatenation inputs were supplied.
    #[error("concatenation requires at least one csv input")]
    EmptyConcatenationInputs,
    /// A concatenation input file contains no rows.
    #[error("concatenation input {basename} is empty")]
    EmptyConcatenationInput {
        /// Input basename.
        basename: String,
    },
    /// The same canonical concatenation input was supplied more than once.
    #[error("concatenation input {basename} was supplied more than once")]
    DuplicateConcatenationInput {
        /// Input basename.
        basename: String,
    },
    /// A concatenation input is not a `.csv` file.
    #[error("concatenation input {basename} is not a csv file")]
    ConcatenationInputNotCsv {
        /// Input basename.
        basename: String,
    },
    /// A native session CSV was supplied as a concatenation input.
    #[error("concatenation input {basename} is a native csv")]
    NativeConcatenationInput {
        /// Input basename.
        basename: String,
    },
    /// A CSV begins like a current native file but does not have its exact
    /// seven-column header.
    #[error("current csv header is invalid in {basename}")]
    InvalidNativeCsvHeader {
        /// Input basename.
        basename: String,
    },
    /// Concatenation inputs do not share a required compatibility field.
    #[error("concatenation inputs {left_basename} and {right_basename} have incompatible {field}")]
    IncompatibleConcatenationInputs {
        /// Field that differed.
        field: ConcatenationCompatibilityField,
        /// Basename of the first incompatible input.
        left_basename: String,
        /// Basename of the second incompatible input.
        right_basename: String,
    },
    /// Timestamps inside one concatenation input decreased.
    #[error("concatenation input {basename} has a decreasing timestamp")]
    DecreasingConcatenationTimestamp {
        /// Input basename.
        basename: String,
    },
    /// Two concatenation inputs have overlapping or equal timestamp boundaries.
    #[error(
        "concatenation inputs {left_basename} and {right_basename} have overlapping timestamps"
    )]
    OverlappingConcatenationRanges {
        /// Basename of the earlier input.
        left_basename: String,
        /// Basename of the later input.
        right_basename: String,
    },
    /// A concatenation row or index count overflowed `u64`.
    #[error("concatenation row count overflow")]
    ConcatenationCountOverflow,
    /// A concatenation SHA-256 value is not 64 lowercase hex characters.
    #[error("concatenation sha-256 must be 64 lowercase hex characters")]
    InvalidConcatenationHash,
    /// An output row range does not match its row count or is not contiguous.
    #[error("concatenation output range is inconsistent")]
    InconsistentConcatenationRange,
    /// Concatenation manifest `kind` is not `legacy_csv_concatenation`.
    #[error("unsupported concatenation kind {kind}")]
    UnsupportedConcatenationKind {
        /// Observed kind.
        kind: String,
    },
    /// An input file changed after inspection and before derived bytes were written.
    #[error("concatenation input {basename} changed after inspection")]
    ConcatenationInputChanged {
        /// Input basename.
        basename: String,
    },
    /// Derived concatenation writing failed at a named stage.
    #[error("concatenation failed at {stage}: {reason}")]
    ConcatenationWrite {
        /// Write stage that failed.
        stage: &'static str,
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
