//! Derived concatenation stem generation and parsing.

use std::fmt;

use rngkit_core::{Fold, IntervalSeconds, SampleBits, SourceId};
use time::OffsetDateTime;

use crate::error::RecordingError;
use crate::naming::SessionStem;

/// Parsed derived concatenation directory/file stem.
///
/// Grammar: `YYYYMMDDTHHMMSS_concat_<source>_s<bits>_i<seconds>[_f<fold>]`.
/// This is independent of [`SessionStem`] and cannot be parsed as a collected
/// session name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcatenationStem {
    inner: SessionStem,
    display: String,
}

impl ConcatenationStem {
    /// Builds a concatenation stem from a local creation datetime and
    /// compatibility parameters.
    ///
    /// The timestamp is the local creation time of the derived artifact, not a
    /// claim about the first input sample. BitBabbler requires `_f0` through
    /// `_f4`. Other sources must not have a fold suffix.
    ///
    /// # Errors
    ///
    /// Returns [`RecordingError::InvalidName`] when fold rules are violated.
    pub fn new(
        local_created: OffsetDateTime,
        source: SourceId,
        sample_bits: SampleBits,
        interval: IntervalSeconds,
        fold: Option<Fold>,
    ) -> Result<Self, RecordingError> {
        let inner = SessionStem::new(local_created, source, sample_bits, interval, fold)?;
        Ok(Self::from_inner(inner))
    }

    /// Parses a derived concatenation stem.
    ///
    /// # Errors
    ///
    /// Returns [`RecordingError::UnsupportedVersion`] for hyphenated timestamps.
    /// Returns [`RecordingError::InvalidName`] for any other malformed stem,
    /// including a collected-session stem without the `concat` token.
    pub fn parse(stem: &str) -> Result<Self, RecordingError> {
        if stem.contains('/') || stem.contains('\\') || stem.contains("..") {
            return Err(RecordingError::InvalidName {
                reason: "stem must not contain a path separator".into(),
            });
        }
        let Some((ts, rest)) = stem.split_once("_concat_") else {
            return Err(RecordingError::InvalidName {
                reason: "missing concat token".into(),
            });
        };
        if rest.contains("_concat_") {
            return Err(RecordingError::InvalidName {
                reason: "unexpected concat token".into(),
            });
        }
        let session_like = format!("{ts}_{rest}");
        let inner = SessionStem::parse(&session_like)?;
        let parsed = Self::from_inner(inner);
        if parsed.display != stem {
            return Err(RecordingError::InvalidName {
                reason: "concatenation stem is not canonical".into(),
            });
        }
        Ok(parsed)
    }

    fn from_inner(inner: SessionStem) -> Self {
        let display = render(&inner);
        Self { inner, display }
    }

    /// Local wall-clock creation time encoded in the stem.
    #[must_use]
    pub fn local_created(&self) -> OffsetDateTime {
        self.inner.local_start()
    }

    /// Source identifier.
    #[must_use]
    pub fn source(&self) -> &SourceId {
        self.inner.source()
    }

    /// Sample size.
    #[must_use]
    pub fn sample_bits(&self) -> SampleBits {
        self.inner.sample_bits()
    }

    /// Interval.
    #[must_use]
    pub fn interval(&self) -> IntervalSeconds {
        self.inner.interval()
    }

    /// Fold, present only for BitBabbler.
    #[must_use]
    pub fn fold(&self) -> Option<Fold> {
        self.inner.fold()
    }

    /// Concatenation stem string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.display
    }

    /// Same-stem CSV basename contained in the derived directory.
    #[must_use]
    pub fn csv_basename(&self) -> String {
        format!("{}.csv", self.display)
    }
}

impl fmt::Display for ConcatenationStem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display)
    }
}

fn render(inner: &SessionStem) -> String {
    let local_start = inner.local_start();
    let local = format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}",
        local_start.year(),
        u8::from(local_start.month()),
        local_start.day(),
        local_start.hour(),
        local_start.minute(),
        local_start.second()
    );
    match inner.fold() {
        Some(fold) => format!(
            "{local}_concat_{}_s{}_i{}_f{}",
            inner.source(),
            inner.sample_bits().get(),
            inner.interval().get(),
            fold.get()
        ),
        None => format!(
            "{local}_concat_{}_s{}_i{}",
            inner.source(),
            inner.sample_bits().get(),
            inner.interval().get()
        ),
    }
}
