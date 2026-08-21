//! Version 3 session stem generation and parsing.

use std::fmt;
use std::path::{Path, PathBuf};

use rngkit_core::{Fold, IntervalSeconds, SampleBits, SourceId};
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};

use crate::error::RecordingError;

/// Parsed RngKitPSG version 3 filename stem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStem {
    local_start: OffsetDateTime,
    source: SourceId,
    sample_bits: SampleBits,
    interval: IntervalSeconds,
    fold: Option<Fold>,
    display: String,
}

/// Current local wall-clock time and its UTC offset.
///
/// # Errors
///
/// Returns [`RecordingError::LocalOffsetUnavailable`] when the platform cannot
/// provide a local offset.
pub fn now_local() -> Result<(OffsetDateTime, UtcOffset), RecordingError> {
    let local = OffsetDateTime::now_local().map_err(|_| RecordingError::LocalOffsetUnavailable)?;
    Ok((local, local.offset()))
}

impl SessionStem {
    /// Builds a stem from a local wall-clock datetime and session parameters.
    ///
    /// BitBabbler requires `_f0` through `_f4`. Other sources must not have a
    /// fold suffix.
    ///
    /// # Errors
    ///
    /// Returns [`RecordingError::InvalidName`] when fold rules are violated.
    pub fn new(
        local_start: OffsetDateTime,
        source: SourceId,
        sample_bits: SampleBits,
        interval: IntervalSeconds,
        fold: Option<Fold>,
    ) -> Result<Self, RecordingError> {
        validate_fold_rules(&source, fold)?;
        let display = render(&local_start, &source, sample_bits, interval, fold);
        Ok(Self {
            local_start,
            source,
            sample_bits,
            interval,
            fold,
            display,
        })
    }

    /// Parses a version 3 stem, rejecting version 2 hyphenated names.
    ///
    /// # Errors
    ///
    /// Returns [`RecordingError::UnsupportedVersion`] for hyphenated timestamps.
    /// Returns [`RecordingError::InvalidName`] for any other malformed stem.
    pub fn parse(stem: &str) -> Result<Self, RecordingError> {
        if stem.contains('-') {
            return Err(RecordingError::UnsupportedVersion {
                reason: format!("hyphenated timestamp is version 2: {stem}"),
            });
        }
        if stem.contains('/') || stem.contains('\\') || stem.contains("..") {
            return Err(RecordingError::InvalidName {
                reason: "stem must not contain a path separator".into(),
            });
        }
        let Some((ts, rest)) = stem.split_once('_') else {
            return Err(RecordingError::InvalidName {
                reason: "missing source token".into(),
            });
        };
        let local_start = parse_stem_timestamp(ts)?;
        let mut parts = rest.split('_');
        let source_token = parts.next().ok_or_else(|| RecordingError::InvalidName {
            reason: "missing source token".into(),
        })?;
        let source = SourceId::new(source_token).map_err(|err| RecordingError::InvalidName {
            reason: err.to_string(),
        })?;
        let bits_token = parts.next().ok_or_else(|| RecordingError::InvalidName {
            reason: "missing sample-bits token".into(),
        })?;
        let sample_bits = SampleBits::new(parse_prefixed_u32(bits_token, 's')?)?;
        let interval_token = parts.next().ok_or_else(|| RecordingError::InvalidName {
            reason: "missing interval token".into(),
        })?;
        let interval = IntervalSeconds::new(parse_prefixed_u32(interval_token, 'i')?)?;
        let fold = match parts.next() {
            Some(token) => {
                let value = parse_prefixed_u32(token, 'f')?;
                let fold = u8::try_from(value).map_err(|_| RecordingError::InvalidName {
                    reason: format!("invalid fold token {token}"),
                })?;
                Some(Fold::new(fold)?)
            }
            None => None,
        };
        if parts.next().is_some() {
            return Err(RecordingError::InvalidName {
                reason: "unexpected trailing tokens".into(),
            });
        }
        Self::new(local_start, source, sample_bits, interval, fold)
    }

    /// Local wall-clock start encoded in the stem.
    #[must_use]
    pub fn local_start(&self) -> OffsetDateTime {
        self.local_start
    }

    /// Source identifier.
    #[must_use]
    pub fn source(&self) -> &SourceId {
        &self.source
    }

    /// Sample size.
    #[must_use]
    pub fn sample_bits(&self) -> SampleBits {
        self.sample_bits
    }

    /// Interval.
    #[must_use]
    pub fn interval(&self) -> IntervalSeconds {
        self.interval
    }

    /// Fold, present only for BitBabbler.
    #[must_use]
    pub fn fold(&self) -> Option<Fold> {
        self.fold
    }

    /// Filename/session stem string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.display
    }

    /// Session directory under `root`.
    #[must_use]
    pub fn directory(&self, root: &Path) -> PathBuf {
        root.join(&self.display)
    }

    /// Artifact path with `extension` inside the session directory.
    #[must_use]
    pub fn artifact(&self, root: &Path, extension: &str) -> PathBuf {
        self.directory(root)
            .join(format!("{}.{extension}", self.display))
    }
}

impl fmt::Display for SessionStem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display)
    }
}

fn validate_fold_rules(source: &SourceId, fold: Option<Fold>) -> Result<(), RecordingError> {
    if source.is_bitb() {
        if fold.is_none() {
            return Err(RecordingError::InvalidName {
                reason: "bitb stems require _f0 through _f4".into(),
            });
        }
    } else if fold.is_some() {
        return Err(RecordingError::InvalidName {
            reason: format!("{source} stems must not include a fold suffix"),
        });
    }
    Ok(())
}

fn render(
    local_start: &OffsetDateTime,
    source: &SourceId,
    sample_bits: SampleBits,
    interval: IntervalSeconds,
    fold: Option<Fold>,
) -> String {
    let ts = format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}",
        local_start.year(),
        u8::from(local_start.month()),
        local_start.day(),
        local_start.hour(),
        local_start.minute(),
        local_start.second()
    );
    match fold {
        Some(fold) => format!(
            "{ts}_{source}_s{}_i{}_f{}",
            sample_bits.get(),
            interval.get(),
            fold.get()
        ),
        None => format!("{ts}_{source}_s{}_i{}", sample_bits.get(), interval.get()),
    }
}

fn parse_stem_timestamp(ts: &str) -> Result<OffsetDateTime, RecordingError> {
    if ts.len() != 15 || ts.as_bytes().get(8) != Some(&b'T') {
        return Err(RecordingError::InvalidName {
            reason: format!("timestamp must be YYYYMMDDTHHMMSS, got {ts}"),
        });
    }
    if !ts.bytes().enumerate().all(|(i, b)| {
        if i == 8 {
            b == b'T'
        } else {
            b.is_ascii_digit()
        }
    }) {
        return Err(RecordingError::InvalidName {
            reason: format!("timestamp must be YYYYMMDDTHHMMSS, got {ts}"),
        });
    }
    let year: i32 = atoi(&ts[0..4])?;
    let month: u8 = atoi(&ts[4..6])?;
    let day: u8 = atoi(&ts[6..8])?;
    let hour: u8 = atoi(&ts[9..11])?;
    let minute: u8 = atoi(&ts[11..13])?;
    let second: u8 = atoi(&ts[13..15])?;
    let month = Month::try_from(month).map_err(|_| RecordingError::InvalidName {
        reason: format!("invalid month in {ts}"),
    })?;
    let date =
        Date::from_calendar_date(year, month, day).map_err(|_| RecordingError::InvalidName {
            reason: format!("invalid date in {ts}"),
        })?;
    let time = Time::from_hms(hour, minute, second).map_err(|_| RecordingError::InvalidName {
        reason: format!("invalid time in {ts}"),
    })?;
    Ok(PrimitiveDateTime::new(date, time).assume_utc())
}

fn parse_prefixed_u32(token: &str, prefix: char) -> Result<u32, RecordingError> {
    let rest = token
        .strip_prefix(prefix)
        .ok_or_else(|| RecordingError::InvalidName {
            reason: format!("expected '{prefix}<digits>', got {token}"),
        })?;
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return Err(RecordingError::InvalidName {
            reason: format!("expected '{prefix}<digits>', got {token}"),
        });
    }
    rest.parse::<u32>()
        .map_err(|_| RecordingError::InvalidName {
            reason: format!("integer overflow in {token}"),
        })
}

fn atoi<T: std::str::FromStr>(s: &str) -> Result<T, RecordingError> {
    s.parse().map_err(|_| RecordingError::InvalidName {
        reason: format!("invalid integer {s}"),
    })
}
