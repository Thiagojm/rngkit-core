//! Source identifiers, safe descriptors, and the synchronous entropy trait.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, SourceError};
use crate::sample::SampleBits;

/// Stable source identifier used in filenames and manifests.
///
/// The type is not closed: unknown future IDs are accepted when they match the
/// filename-token grammar. Known constants are [`SOURCE_ID_BITB`],
/// [`SOURCE_ID_TRNG`], [`SOURCE_ID_RDSEED`], and [`SOURCE_ID_PSEUDO`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SourceId(String);

/// BitBabbler filename and manifest token.
pub const SOURCE_ID_BITB: &str = "bitb";
/// TrueRNG v1/v2/v3 filename and manifest token.
pub const SOURCE_ID_TRNG: &str = "trng";
/// Intel RDSEED filename and manifest token.
pub const SOURCE_ID_RDSEED: &str = "rdseed";
/// OS-seeded ChaCha20 PseudoRNG filename and manifest token.
pub const SOURCE_ID_PSEUDO: &str = "pseudo";

impl SourceId {
    /// Parses a stable source identifier.
    ///
    /// Accepted values are `^[a-z][a-z0-9]{0,31}$` so they can appear as a
    /// single filename token without `_` or uppercase letters.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidSourceId`] when the string is empty, too
    /// long, or contains characters outside the grammar.
    ///
    /// # Examples
    ///
    /// ```
    /// use rngkit_core::{SourceId, SOURCE_ID_TRNG};
    ///
    /// let id = SourceId::new(SOURCE_ID_TRNG)?;
    /// assert_eq!(id.as_str(), "trng");
    /// assert!(SourceId::new("TRNG").is_err());
    /// assert!(SourceId::new("bit_b").is_err());
    /// # Ok::<(), rngkit_core::CoreError>(())
    /// ```
    pub fn new(id: impl AsRef<str>) -> Result<Self, CoreError> {
        let id = id.as_ref();
        if id.is_empty() {
            return Err(CoreError::InvalidSourceId {
                reason: "must not be empty",
            });
        }
        if id.len() > 32 {
            return Err(CoreError::InvalidSourceId {
                reason: "must be at most 32 characters",
            });
        }
        let mut chars = id.chars();
        let Some(first) = chars.next() else {
            return Err(CoreError::InvalidSourceId {
                reason: "must not be empty",
            });
        };
        if !first.is_ascii_lowercase() {
            return Err(CoreError::InvalidSourceId {
                reason: "must start with a lowercase ascii letter",
            });
        }
        if !chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit()) {
            return Err(CoreError::InvalidSourceId {
                reason: "must contain only lowercase ascii letters and digits",
            });
        }
        Ok(Self(id.to_owned()))
    }

    /// BitBabbler identifier.
    #[must_use]
    pub fn bitb() -> Self {
        Self(SOURCE_ID_BITB.to_owned())
    }

    /// TrueRNG identifier.
    #[must_use]
    pub fn trng() -> Self {
        Self(SOURCE_ID_TRNG.to_owned())
    }

    /// Intel RDSEED identifier.
    #[must_use]
    pub fn rdseed() -> Self {
        Self(SOURCE_ID_RDSEED.to_owned())
    }

    /// PseudoRNG identifier.
    #[must_use]
    pub fn pseudo() -> Self {
        Self(SOURCE_ID_PSEUDO.to_owned())
    }

    /// Returns the token as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether this identifier is BitBabbler (`bitb`).
    #[must_use]
    pub fn is_bitb(&self) -> bool {
        self.0 == SOURCE_ID_BITB
    }
}

impl TryFrom<String> for SourceId {
    type Error = CoreError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for SourceId {
    type Error = CoreError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<SourceId> for String {
    fn from(value: SourceId) -> Self {
        value.0
    }
}

impl AsRef<str> for SourceId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Explicit BitBabbler XOR fold depth in `0..=4`.
///
/// Fold is a persistable session parameter, not a device serial or path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Fold(u8);

impl Fold {
    /// Raw, unfolded stream (`0`).
    pub const RAW: Self = Self(0);

    /// Validates a fold depth.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidFold`] when `value` is outside `0..=4`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rngkit_core::Fold;
    ///
    /// assert_eq!(Fold::new(0)?.get(), 0);
    /// assert!(Fold::new(5).is_err());
    /// # Ok::<(), rngkit_core::CoreError>(())
    /// ```
    pub fn new(value: u8) -> Result<Self, CoreError> {
        if value > 4 {
            return Err(CoreError::InvalidFold { value });
        }
        Ok(Self(value))
    }

    /// Numeric fold depth.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Fold {
    type Error = CoreError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Fold> for u8 {
    fn from(value: Fold) -> Self {
        value.0
    }
}

impl std::fmt::Display for Fold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Safe source metadata that may be persisted in manifests and reports.
///
/// The type has no field that can hold an OS device path, USB serial, PRNG
/// seed, or generator state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceDescriptor {
    id: SourceId,
    label: String,
    variant: Option<String>,
    fold: Option<Fold>,
}

impl SourceDescriptor {
    /// Builds a descriptor from validated parts.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::EmptyDescriptorField`] when `label` is empty or
    /// `variant` is `Some("")`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rngkit_core::{SourceDescriptor, SourceId};
    ///
    /// let desc = SourceDescriptor::new(
    ///     SourceId::trng(),
    ///     "TrueRNG v1/v2/v3",
    ///     Some("TrueRNG".into()),
    ///     None,
    /// )?;
    /// assert_eq!(desc.id().as_str(), "trng");
    /// # Ok::<(), rngkit_core::CoreError>(())
    /// ```
    pub fn new(
        id: SourceId,
        label: impl Into<String>,
        variant: Option<String>,
        fold: Option<Fold>,
    ) -> Result<Self, CoreError> {
        let label = label.into();
        if label.trim().is_empty() {
            return Err(CoreError::EmptyDescriptorField { field: "label" });
        }
        if let Some(variant) = &variant {
            if variant.trim().is_empty() {
                return Err(CoreError::EmptyDescriptorField { field: "variant" });
            }
        }
        Ok(Self {
            id,
            label,
            variant,
            fold,
        })
    }

    /// Stable source identifier.
    #[must_use]
    pub fn id(&self) -> &SourceId {
        &self.id
    }

    /// Human-readable label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Safe variant label such as `White` or `TrueRNG`, if known.
    #[must_use]
    pub fn variant(&self) -> Option<&str> {
        self.variant.as_deref()
    }

    /// BitBabbler fold, if this descriptor is for BitBabbler.
    #[must_use]
    pub fn fold(&self) -> Option<Fold> {
        self.fold
    }
}

/// Synchronous entropy source used by collection.
///
/// `read_bits` is all-or-error: it returns exactly [`SampleBits::bytes`] bytes
/// or an error. Partial entropy is never exposed. The trait excludes
/// `random_u64` and `random_range`.
///
/// # Examples
///
/// ```
/// use rngkit_core::{
///     EntropySource, SampleBits, SourceDescriptor, SourceError, SourceErrorKind, SourceId,
/// };
///
/// struct Ones {
///     descriptor: SourceDescriptor,
/// }
///
/// impl EntropySource for Ones {
///     fn descriptor(&self) -> &SourceDescriptor {
///         &self.descriptor
///     }
///
///     fn read_bits(&mut self, bits: SampleBits) -> Result<Vec<u8>, SourceError> {
///         let n = bits.bytes().map_err(|err| {
///             SourceError::new(SourceErrorKind::InvalidRequest, err.to_string())
///         })?;
///         Ok(vec![0xFF; n])
///     }
/// }
///
/// let mut src = Ones {
///     descriptor: SourceDescriptor::new(SourceId::pseudo(), "mock", None, None)?,
/// };
/// let bits = SampleBits::new(16)?;
/// let bytes = src.read_bits(bits)?;
/// assert_eq!(bytes.len(), 2);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub trait EntropySource: Send {
    /// Safe descriptor for this source instance.
    fn descriptor(&self) -> &SourceDescriptor;

    /// Reads exactly one complete sample of `bits` bits.
    ///
    /// # Errors
    ///
    /// Returns a [`SourceError`] when the source cannot supply a complete
    /// buffer. Callers must treat any error as terminal for the current
    /// session; there is no partial payload.
    fn read_bits(&mut self, bits: SampleBits) -> Result<Vec<u8>, SourceError>;
}
