//! Schema-version-1 derived concatenation manifest.

use std::fmt::Write as _;

use rngkit_core::{
    Fold, IntervalSeconds, SOURCE_ID_BITB, SOURCE_ID_PSEUDO, SOURCE_ID_RDSEED, SOURCE_ID_TRNG,
    SampleBits, SampleIndex, SourceId, UtcTimestamp,
};
use serde::{Deserialize, Serialize};
use time::UtcOffset;

use crate::concatenation::naming::ConcatenationStem;
use crate::error::RecordingError;
use crate::naming::{SessionStem, validate_fold_rules};
use crate::normalized::StandaloneInputFormat;

/// Concatenation manifest schema version implemented by this crate.
pub const CONCATENATION_SCHEMA_VERSION: u32 = 1;

/// Artifact kind stored in a concatenation manifest.
pub const CONCATENATION_KIND: &str = "legacy_csv_concatenation";

/// Current schema version for homogeneous format-neutral CSV concatenation
/// bundles.
pub const CSV_CONCATENATION_SCHEMA_VERSION: u32 = 2;

/// Artifact kind stored in a current CSV concatenation manifest.
pub const CSV_CONCATENATION_KIND: &str = "csv_concatenation";

/// Schema version for heterogeneous (mixed-source or mixed-fold) concatenations.
pub const MIXED_CSV_CONCATENATION_SCHEMA_VERSION: u32 = 3;

/// Derived-output source token for heterogeneous concatenations.
///
/// This is not a collectable entropy source. Collection and
/// [`crate::naming::SessionStem`] validation still accept only `bitb` /
/// `trng` / `rdseed` / `pseudo` as appropriate for the input format.
pub const MIXED_SOURCE_ID: &str = "mixed";

/// User-facing label for heterogeneous concatenations.
pub const MIXED_SOURCE_LABEL: &str = "Mixed sources";

pub(crate) fn mixed_source_id() -> SourceId {
    SourceId::new(MIXED_SOURCE_ID).expect("BUG: mixed matches the SourceId filename-token grammar")
}

pub(crate) fn is_mixed_source(id: &SourceId) -> bool {
    id.as_str() == MIXED_SOURCE_ID
}

/// Lowercase hex SHA-256 of an input file's bytes.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContentSha256(String);

impl ContentSha256 {
    /// Encodes a 32-byte digest as lowercase hex.
    #[must_use]
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        let mut hex = String::with_capacity(64);
        for byte in bytes {
            let _ = write!(&mut hex, "{byte:02x}");
        }
        Self(hex)
    }

    /// Parses a 64-character lowercase hex digest.
    ///
    /// # Errors
    ///
    /// Returns [`RecordingError::InvalidConcatenationHash`] when `value` is not
    /// exactly 64 lowercase hexadecimal characters.
    pub fn parse(value: &str) -> Result<Self, RecordingError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(RecordingError::InvalidConcatenationHash);
        }
        Ok(Self(value.to_owned()))
    }

    /// Hex digest.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ContentSha256 {
    type Error = RecordingError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<ContentSha256> for String {
    fn from(value: ContentSha256) -> Self {
        value.0
    }
}

impl std::fmt::Debug for ContentSha256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::fmt::Display for ContentSha256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One validated concatenation input: basename, hash, rows, range, and times.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConcatenationInputEntry {
    basename: String,
    sha256: ContentSha256,
    row_count: u64,
    first_timestamp: UtcTimestamp,
    last_timestamp: UtcTimestamp,
    output_start: SampleIndex,
    output_end: SampleIndex,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    format: Option<StandaloneInputFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_id: Option<SourceId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fold: Option<Fold>,
}

impl ConcatenationInputEntry {
    /// Builds a typed input entry.
    ///
    /// `basename` must be a single path segment. `output_start` and
    /// `output_end` are one-based inclusive indexes whose span equals
    /// `row_count`. `first_timestamp` must not be after `last_timestamp`.
    ///
    /// # Errors
    ///
    /// Returns [`RecordingError::InvalidName`] for an unsafe basename,
    /// [`RecordingError::EmptyConcatenationInput`] when `row_count` is zero,
    /// and [`RecordingError::InconsistentConcatenationRange`] when the
    /// timestamps or output span do not match `row_count`.
    pub fn new(
        basename: String,
        sha256: ContentSha256,
        row_count: u64,
        first_timestamp: UtcTimestamp,
        last_timestamp: UtcTimestamp,
        output_start: SampleIndex,
        output_end: SampleIndex,
    ) -> Result<Self, RecordingError> {
        let entry = Self {
            basename,
            sha256,
            row_count,
            first_timestamp,
            last_timestamp,
            output_start,
            output_end,
            format: None,
            source_id: None,
            fold: None,
        };
        entry.validate()?;
        Ok(entry)
    }

    /// Builds an input entry with a current or legacy CSV format label.
    ///
    /// The format is serialized in schema-2 manifests and omitted from the
    /// compatibility schema-1 representation.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_format(
        basename: String,
        sha256: ContentSha256,
        row_count: u64,
        first_timestamp: UtcTimestamp,
        last_timestamp: UtcTimestamp,
        output_start: SampleIndex,
        output_end: SampleIndex,
        format: StandaloneInputFormat,
    ) -> Result<Self, RecordingError> {
        if format == StandaloneInputFormat::Bin {
            return Err(RecordingError::InvalidName {
                reason: "concatenation inputs must be csv formats".into(),
            });
        }
        let mut entry = Self::new(
            basename,
            sha256,
            row_count,
            first_timestamp,
            last_timestamp,
            output_start,
            output_end,
        )?;
        entry.format = Some(format);
        Ok(entry)
    }

    /// Records this input's source and fold from its session stem.
    ///
    /// Format-neutral preview entries always carry provenance in memory.
    /// Schema-2 serialization strips these fields. Schema 3 requires them.
    ///
    /// # Errors
    ///
    /// Returns [`RecordingError::InvalidName`] when `source_id` is `mixed` or
    /// fold rules for that source are violated.
    pub fn with_provenance(
        mut self,
        source_id: SourceId,
        fold: Option<Fold>,
    ) -> Result<Self, RecordingError> {
        if is_mixed_source(&source_id) {
            return Err(RecordingError::InvalidName {
                reason: "concatenation input source must not be mixed".into(),
            });
        }
        validate_fold_rules(&source_id, fold)?;
        self.source_id = Some(source_id);
        self.fold = fold;
        Ok(self)
    }

    pub(crate) fn without_provenance(mut self) -> Self {
        self.source_id = None;
        self.fold = None;
        self
    }

    fn validate(&self) -> Result<(), RecordingError> {
        validate_basename(&self.basename)?;
        if self.row_count == 0 {
            return Err(RecordingError::EmptyConcatenationInput {
                basename: self.basename.clone(),
            });
        }
        if self.first_timestamp > self.last_timestamp {
            return Err(RecordingError::InconsistentConcatenationRange);
        }
        let span = self
            .output_end
            .get()
            .checked_sub(self.output_start.get())
            .and_then(|delta| delta.checked_add(1));
        if span != Some(self.row_count) {
            return Err(RecordingError::InconsistentConcatenationRange);
        }
        Ok(())
    }

    /// Input basename. Never an absolute path.
    #[must_use]
    pub fn basename(&self) -> &str {
        &self.basename
    }

    /// SHA-256 of the input file bytes.
    #[must_use]
    pub fn sha256(&self) -> &ContentSha256 {
        &self.sha256
    }

    /// Row count in this input.
    #[must_use]
    pub fn row_count(&self) -> u64 {
        self.row_count
    }

    /// First row timestamp.
    #[must_use]
    pub fn first_timestamp(&self) -> UtcTimestamp {
        self.first_timestamp
    }

    /// Last row timestamp.
    #[must_use]
    pub fn last_timestamp(&self) -> UtcTimestamp {
        self.last_timestamp
    }

    /// Inclusive one-based output start index.
    #[must_use]
    pub fn output_start(&self) -> SampleIndex {
        self.output_start
    }

    /// Inclusive one-based output end index.
    #[must_use]
    pub fn output_end(&self) -> SampleIndex {
        self.output_end
    }

    /// CSV format, when this entry belongs to a schema-2 or schema-3 manifest.
    #[must_use]
    pub fn format(&self) -> Option<StandaloneInputFormat> {
        self.format
    }

    /// Input source from the session stem, when recorded.
    #[must_use]
    pub fn source_id(&self) -> Option<&SourceId> {
        self.source_id.as_ref()
    }

    /// Input fold from the session stem, when BitBabbler provenance is present.
    #[must_use]
    pub fn fold(&self) -> Option<Fold> {
        self.fold
    }
}

/// Schema-version-1 concatenation manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConcatenationManifest {
    schema_version: u32,
    kind: String,
    stem: String,
    source_id: SourceId,
    sample_bits: SampleBits,
    interval_seconds: IntervalSeconds,
    #[serde(skip_serializing_if = "Option::is_none")]
    fold: Option<Fold>,
    created_at_utc: UtcTimestamp,
    local_utc_offset: String,
    csv_file: String,
    total_rows: u64,
    inputs: Vec<ConcatenationInputEntry>,
}

impl ConcatenationManifest {
    /// Builds a schema-version-1 manifest from a stem and ordered inputs.
    ///
    /// # Errors
    ///
    /// Returns [`RecordingError::EmptyConcatenationInputs`] when `inputs` is
    /// empty, [`RecordingError::ConcatenationCountOverflow`] if row counts
    /// overflow, and [`RecordingError::InconsistentConcatenationRange`] when
    /// output ranges are not contiguous from 1.
    pub fn new(
        stem: &ConcatenationStem,
        created_at: UtcTimestamp,
        local_offset: UtcOffset,
        inputs: Vec<ConcatenationInputEntry>,
    ) -> Result<Self, RecordingError> {
        let mut manifest = Self {
            schema_version: CONCATENATION_SCHEMA_VERSION,
            kind: CONCATENATION_KIND.to_owned(),
            stem: stem.as_str().to_owned(),
            source_id: stem.source().clone(),
            sample_bits: stem.sample_bits(),
            interval_seconds: stem.interval(),
            fold: stem.fold(),
            created_at_utc: created_at,
            local_utc_offset: format_offset(local_offset),
            csv_file: stem.csv_basename(),
            total_rows: 0,
            inputs,
        };
        manifest.total_rows = manifest.validate_contents()?;
        Ok(manifest)
    }

    /// Builds a schema-2 format-neutral CSV concatenation manifest.
    ///
    /// Every input must carry a current or legacy CSV format label.
    pub fn new_csv(
        stem: &ConcatenationStem,
        created_at: UtcTimestamp,
        local_offset: UtcOffset,
        inputs: Vec<ConcatenationInputEntry>,
    ) -> Result<Self, RecordingError> {
        let mut manifest = Self {
            schema_version: CSV_CONCATENATION_SCHEMA_VERSION,
            kind: CSV_CONCATENATION_KIND.to_owned(),
            stem: stem.as_str().to_owned(),
            source_id: stem.source().clone(),
            sample_bits: stem.sample_bits(),
            interval_seconds: stem.interval(),
            fold: stem.fold(),
            created_at_utc: created_at,
            local_utc_offset: format_offset(local_offset),
            csv_file: stem.csv_basename(),
            total_rows: 0,
            inputs,
        };
        manifest.total_rows = manifest.validate_contents()?;
        Ok(manifest)
    }

    /// Builds a schema-3 mixed-source CSV concatenation manifest.
    ///
    /// Top-level identity is `mixed` with no fold. Every input must carry a
    /// CSV format and per-input source/fold provenance.
    pub fn new_mixed_csv(
        stem: &ConcatenationStem,
        created_at: UtcTimestamp,
        local_offset: UtcOffset,
        inputs: Vec<ConcatenationInputEntry>,
    ) -> Result<Self, RecordingError> {
        let mut manifest = Self {
            schema_version: MIXED_CSV_CONCATENATION_SCHEMA_VERSION,
            kind: CSV_CONCATENATION_KIND.to_owned(),
            stem: stem.as_str().to_owned(),
            source_id: stem.source().clone(),
            sample_bits: stem.sample_bits(),
            interval_seconds: stem.interval(),
            fold: stem.fold(),
            created_at_utc: created_at,
            local_utc_offset: format_offset(local_offset),
            csv_file: stem.csv_basename(),
            total_rows: 0,
            inputs,
        };
        manifest.total_rows = manifest.validate_contents()?;
        Ok(manifest)
    }

    /// Parses JSON and rejects unknown schema versions and kinds.
    ///
    /// # Errors
    ///
    /// Returns [`RecordingError::UnsupportedSchema`],
    /// [`RecordingError::UnsupportedConcatenationKind`], JSON errors, or
    /// consistency errors for a tampered document.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, RecordingError> {
        let manifest: Self = serde_json::from_slice(bytes)?;
        let total = manifest.validate_contents()?;
        if manifest.total_rows != total {
            return Err(RecordingError::InconsistentConcatenationRange);
        }
        Ok(manifest)
    }

    fn validate_contents(&self) -> Result<u64, RecordingError> {
        let schema1 =
            self.schema_version == CONCATENATION_SCHEMA_VERSION && self.kind == CONCATENATION_KIND;
        let schema2 = self.schema_version == CSV_CONCATENATION_SCHEMA_VERSION
            && self.kind == CSV_CONCATENATION_KIND;
        let schema3 = self.schema_version == MIXED_CSV_CONCATENATION_SCHEMA_VERSION
            && self.kind == CSV_CONCATENATION_KIND;
        if !schema1 && !schema2 && !schema3 {
            if self.schema_version != CONCATENATION_SCHEMA_VERSION {
                return Err(RecordingError::UnsupportedSchema {
                    version: self.schema_version,
                });
            }
            return Err(RecordingError::UnsupportedConcatenationKind {
                kind: self.kind.clone(),
            });
        }
        let mixed = is_mixed_source(&self.source_id);
        if schema3 {
            if !mixed || self.fold.is_some() {
                return Err(RecordingError::Corrupt {
                    reason: "schema-3 concatenation must use mixed source without fold".into(),
                });
            }
        } else if mixed {
            return Err(RecordingError::Corrupt {
                reason: "mixed concatenation requires schema 3".into(),
            });
        }
        if self.inputs.is_empty() {
            return Err(RecordingError::EmptyConcatenationInputs);
        }
        let parsed = ConcatenationStem::parse(&self.stem)?;
        if self.csv_file != parsed.csv_basename() {
            return Err(RecordingError::InvalidName {
                reason: "concatenation csv file must be the same-stem basename".into(),
            });
        }
        if self.source_id != *parsed.source() {
            return Err(RecordingError::Corrupt {
                reason: "concatenation manifest source_id does not match stem".into(),
            });
        }
        if self.sample_bits != parsed.sample_bits() {
            return Err(RecordingError::Corrupt {
                reason: "concatenation manifest sample_bits do not match stem".into(),
            });
        }
        if self.interval_seconds != parsed.interval() {
            return Err(RecordingError::Corrupt {
                reason: "concatenation manifest interval does not match stem".into(),
            });
        }
        if self.fold != parsed.fold() {
            return Err(RecordingError::Corrupt {
                reason: "concatenation manifest fold does not match stem".into(),
            });
        }
        let mut expected_start = 1u64;
        let mut total = 0u64;
        let mut prev_last: Option<(UtcTimestamp, &str)> = None;
        for input in &self.inputs {
            input.validate()?;
            if schema1
                && (input.format.is_some() || input.source_id.is_some() || input.fold.is_some())
            {
                return Err(RecordingError::Corrupt {
                    reason: "schema-1 concatenation input unexpectedly has format or provenance"
                        .into(),
                });
            }
            if schema2 {
                if !matches!(
                    input.format,
                    Some(StandaloneInputFormat::CurrentCsv | StandaloneInputFormat::LegacyV3Csv)
                ) {
                    return Err(RecordingError::Corrupt {
                        reason: "schema-2 concatenation input is missing a csv format".into(),
                    });
                }
                if input.source_id.is_some() || input.fold.is_some() {
                    return Err(RecordingError::Corrupt {
                        reason: "schema-2 concatenation input unexpectedly has source provenance"
                            .into(),
                    });
                }
            }
            if schema3 {
                self.validate_schema3_input(input)?;
            }
            if input.output_start.get() != expected_start {
                return Err(RecordingError::InconsistentConcatenationRange);
            }
            if let Some((prev, left_basename)) = prev_last {
                if prev >= input.first_timestamp {
                    return Err(RecordingError::OverlappingConcatenationRanges {
                        left_basename: left_basename.to_owned(),
                        right_basename: input.basename.clone(),
                    });
                }
            }
            total = total
                .checked_add(input.row_count)
                .ok_or(RecordingError::ConcatenationCountOverflow)?;
            expected_start = input
                .output_end
                .get()
                .checked_add(1)
                .ok_or(RecordingError::ConcatenationCountOverflow)?;
            prev_last = Some((input.last_timestamp, input.basename.as_str()));
        }
        if schema3 {
            let first = &self.inputs[0];
            let heterogeneous = self
                .inputs
                .iter()
                .any(|input| input.source_id != first.source_id || input.fold != first.fold);
            if !heterogeneous {
                return Err(RecordingError::Corrupt {
                    reason: "schema-3 concatenation inputs must not share one source and fold"
                        .into(),
                });
            }
        }
        Ok(total)
    }

    fn validate_schema3_input(
        &self,
        input: &ConcatenationInputEntry,
    ) -> Result<(), RecordingError> {
        let format = match input.format {
            Some(
                format @ (StandaloneInputFormat::CurrentCsv | StandaloneInputFormat::LegacyV3Csv),
            ) => format,
            _ => {
                return Err(RecordingError::Corrupt {
                    reason: "schema-3 concatenation input is missing a csv format".into(),
                });
            }
        };
        let Some(source_id) = input.source_id.as_ref() else {
            return Err(RecordingError::Corrupt {
                reason: "schema-3 concatenation input is missing source provenance".into(),
            });
        };
        if is_mixed_source(source_id) {
            return Err(RecordingError::Corrupt {
                reason: "concatenation input source must not be mixed".into(),
            });
        }
        validate_fold_rules(source_id, input.fold)?;
        validate_input_source(source_id, format)?;
        let stem = parse_input_session_stem(&input.basename)?;
        if stem.source() != source_id || stem.fold() != input.fold {
            return Err(RecordingError::Corrupt {
                reason: format!(
                    "schema-3 concatenation input {} provenance does not match basename",
                    input.basename
                ),
            });
        }
        if stem.sample_bits() != self.sample_bits || stem.interval() != self.interval_seconds {
            return Err(RecordingError::Corrupt {
                reason: format!(
                    "schema-3 concatenation input {} bits or interval do not match output metadata",
                    input.basename
                ),
            });
        }
        Ok(())
    }

    /// Schema version.
    #[must_use]
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Artifact kind.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Concatenation stem string.
    #[must_use]
    pub fn stem(&self) -> &str {
        &self.stem
    }

    /// Source identifier.
    #[must_use]
    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    /// Sample size.
    #[must_use]
    pub fn sample_bits(&self) -> SampleBits {
        self.sample_bits
    }

    /// Interval.
    #[must_use]
    pub fn interval(&self) -> IntervalSeconds {
        self.interval_seconds
    }

    /// Fold, if BitBabbler.
    #[must_use]
    pub fn fold(&self) -> Option<Fold> {
        self.fold
    }

    /// UTC creation time of the derived artifact.
    #[must_use]
    pub fn created_at_utc(&self) -> UtcTimestamp {
        self.created_at_utc
    }

    /// Local offset used to build the filename, as `±HH:MM`.
    #[must_use]
    pub fn local_utc_offset(&self) -> &str {
        &self.local_utc_offset
    }

    /// Contained CSV basename.
    #[must_use]
    pub fn csv_file(&self) -> &str {
        &self.csv_file
    }

    /// Total output rows.
    #[must_use]
    pub fn total_rows(&self) -> u64 {
        self.total_rows
    }

    /// Ordered input entries.
    #[must_use]
    pub fn inputs(&self) -> &[ConcatenationInputEntry] {
        &self.inputs
    }
}

pub(crate) fn parse_input_session_stem(basename: &str) -> Result<SessionStem, RecordingError> {
    let stem = basename
        .strip_suffix(".csv")
        .ok_or_else(|| RecordingError::InvalidName {
            reason: "concatenation input basename must end with .csv".into(),
        })?;
    SessionStem::parse(stem)
}

pub(crate) fn validate_input_source(
    source: &SourceId,
    format: StandaloneInputFormat,
) -> Result<(), RecordingError> {
    if is_mixed_source(source) {
        return Err(RecordingError::UnsupportedVersion {
            reason: format!(
                "standalone {} does not include source {source}",
                format_name(format)
            ),
        });
    }
    let supported = match format {
        StandaloneInputFormat::LegacyV3Csv => {
            matches!(
                source.as_str(),
                SOURCE_ID_BITB | SOURCE_ID_TRNG | SOURCE_ID_PSEUDO
            )
        }
        StandaloneInputFormat::CurrentCsv => matches!(
            source.as_str(),
            SOURCE_ID_BITB | SOURCE_ID_TRNG | SOURCE_ID_RDSEED | SOURCE_ID_PSEUDO
        ),
        StandaloneInputFormat::Bin => false,
        StandaloneInputFormat::FlatLegacyConcatenation => false,
    };
    if !supported {
        return Err(RecordingError::UnsupportedVersion {
            reason: format!(
                "standalone {} does not include source {source}",
                format_name(format)
            ),
        });
    }
    Ok(())
}

fn format_name(format: StandaloneInputFormat) -> &'static str {
    match format {
        StandaloneInputFormat::CurrentCsv => "current csv",
        StandaloneInputFormat::LegacyV3Csv => "legacy v3 csv",
        StandaloneInputFormat::Bin => "bin",
        StandaloneInputFormat::FlatLegacyConcatenation => "flat legacy concatenation",
    }
}

pub(crate) fn validate_basename(name: &str) -> Result<(), RecordingError> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name.contains(':')
    {
        return Err(RecordingError::InvalidName {
            reason: "concatenation input basename must be a single path segment".into(),
        });
    }
    Ok(())
}

fn format_offset(offset: UtcOffset) -> String {
    let whole = offset.whole_seconds();
    let sign = if whole < 0 { '-' } else { '+' };
    let abs = whole.unsigned_abs();
    let hours = abs / 3600;
    let minutes = (abs % 3600) / 60;
    format!("{sign}{hours:02}:{minutes:02}")
}
