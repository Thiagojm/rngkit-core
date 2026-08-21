//! Domain construction and source errors.

use std::error::Error as StdError;
use std::fmt;

/// Errors from constructing or converting core domain values.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CoreError {
    /// A bit length of zero was supplied.
    #[error("sample bits must be greater than zero")]
    ZeroBitLength,
    /// A bit length was not divisible by eight.
    #[error("sample bits must be divisible by 8, got {requested_bits}")]
    BitLengthNotByteAligned {
        /// Requested bit length.
        requested_bits: u32,
    },
    /// A conversion to a platform size type overflowed.
    #[error("value {value} does not fit in the platform size type")]
    SizeOverflow {
        /// Value that overflowed.
        value: u64,
    },
    /// An interval of zero seconds was supplied.
    #[error("interval must be at least one second")]
    ZeroInterval,
    /// A one-based sample index of zero was supplied.
    #[error("sample index must be at least 1")]
    ZeroSampleIndex,
    /// A fold value outside `0..=4` was supplied.
    #[error("fold must be between 0 and 4 inclusive, got {value}")]
    InvalidFold {
        /// Rejected fold value.
        value: u8,
    },
    /// A source identifier failed validation.
    #[error("invalid source id {reason}")]
    InvalidSourceId {
        /// Why the identifier was rejected.
        reason: &'static str,
    },
    /// A persistable descriptor field was empty.
    #[error("source {field} must not be empty")]
    EmptyDescriptorField {
        /// Name of the empty field.
        field: &'static str,
    },
    /// A popcount overflowed `u64`.
    #[error("one-bit count overflowed u64")]
    PopcountOverflow,
}

/// Stable buckets for normalized entropy-source failures.
///
/// Display text on [`SourceError`] is diagnostic only and is not a serialized
/// IPC contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SourceErrorKind {
    /// No matching source is present or the source cannot be constructed.
    NotAvailable,
    /// More than one recognized device is present and none was selected.
    MultipleDevices,
    /// An explicit serial or path is required.
    SelectionRequired,
    /// The requested device was not found.
    DeviceNotFound,
    /// The process lacks permission to open or read the source.
    PermissionDenied,
    /// The device interface is busy.
    DeviceBusy,
    /// The device was removed or the handle is invalid.
    Disconnected,
    /// A transfer or instruction timed out.
    Timeout,
    /// Protocol, framing, or contract rules were violated.
    Protocol,
    /// The caller supplied an invalid bit length or configuration.
    InvalidRequest,
    /// Buffer reservation failed.
    AllocationFailed,
    /// The source did not yield entropy within its retry budget.
    EntropyUnavailable,
    /// The architecture, instruction, or product is unsupported.
    Unsupported,
    /// A failure not mapped to a more specific kind.
    Other,
}

impl fmt::Display for SourceErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotAvailable => "not available",
            Self::MultipleDevices => "multiple devices",
            Self::SelectionRequired => "selection required",
            Self::DeviceNotFound => "device not found",
            Self::PermissionDenied => "permission denied",
            Self::DeviceBusy => "device busy",
            Self::Disconnected => "disconnected",
            Self::Timeout => "timeout",
            Self::Protocol => "protocol",
            Self::InvalidRequest => "invalid request",
            Self::AllocationFailed => "allocation failed",
            Self::EntropyUnavailable => "entropy unavailable",
            Self::Unsupported => "unsupported",
            Self::Other => "other",
        })
    }
}

/// Normalized source error with an optional diagnostic chain.
///
/// The kind is stable for matching. The message and source chain are for
/// diagnostics and must not be persisted in session artifacts.
#[derive(Debug)]
pub struct SourceError {
    kind: SourceErrorKind,
    message: String,
    source: Option<Box<dyn StdError + Send + Sync + 'static>>,
}

impl SourceError {
    /// Builds an error of `kind` with a diagnostic `message`.
    #[must_use]
    pub fn new(kind: SourceErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    /// Builds an error that retains `source` in the diagnostic chain.
    #[must_use]
    pub fn with_source<E>(kind: SourceErrorKind, message: impl Into<String>, source: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Self {
            kind,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Stable error kind.
    #[must_use]
    pub fn kind(&self) -> SourceErrorKind {
        self.kind
    }

    /// Diagnostic message. Not a serialized contract.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl StdError for SourceError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_deref()
            .map(|err| err as &(dyn StdError + 'static))
    }
}
