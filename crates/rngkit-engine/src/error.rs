//! Engine errors.

use rngkit_analysis::AnalysisError;
use rngkit_core::{CoreError, SourceError};
use rngkit_recording::RecordingError;

/// Terminal collection errors.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EngineError {
    /// Invalid configuration, rejected before artifacts are created.
    #[error(transparent)]
    Config(#[from] CoreError),
    /// Source failure.
    #[error(transparent)]
    Source(#[from] SourceError),
    /// Recording failure.
    #[error(transparent)]
    Recording(#[from] RecordingError),
    /// Analysis failure.
    #[error(transparent)]
    Analysis(#[from] AnalysisError),
    /// Event sink failure.
    #[error("event sink failed: {0}")]
    Sink(String),
}

impl EngineError {
    /// Stable kind label for failed-manifest storage.
    #[must_use]
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Config(_) => "config",
            Self::Source(_) => "source",
            Self::Recording(_) => "recording",
            Self::Analysis(_) => "analysis",
            Self::Sink(_) => "sink",
        }
    }
}
