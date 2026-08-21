//! Native session consistency classification.

use std::path::PathBuf;

/// Warning that does not block reading the committed prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConsistencyWarning {
    /// BIN is longer than the last committed CSV range; the tail is ignored.
    UncommittedBinaryTail {
        /// Path of the BIN file.
        path: PathBuf,
        /// Declared committed length in bytes.
        committed_bytes: u64,
        /// Observed file length in bytes.
        file_bytes: u64,
    },
}

/// Result of inspecting a native bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsistencyReport {
    /// Committed sample count derived from CSV.
    pub committed_samples: u64,
    /// Non-fatal warnings.
    pub warnings: Vec<ConsistencyWarning>,
}

impl ConsistencyReport {
    /// Empty report.
    #[must_use]
    pub fn new(committed_samples: u64) -> Self {
        Self {
            committed_samples,
            warnings: Vec::new(),
        }
    }
}
