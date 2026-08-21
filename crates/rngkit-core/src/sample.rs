//! Validated sample bit counts and dependency-free popcount.

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// Positive bit count that is divisible by eight.
///
/// Construction fails before any allocation or source call. The inner value is
/// a `u32` so a single sample cannot request more than `u32::MAX` bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct SampleBits(u32);

impl SampleBits {
    /// Validates a positive, byte-aligned bit count.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::ZeroBitLength`] when `bits` is zero.
    /// Returns [`CoreError::BitLengthNotByteAligned`] when `bits` is not
    /// divisible by eight.
    ///
    /// # Examples
    ///
    /// ```
    /// use rngkit_core::SampleBits;
    ///
    /// let bits = SampleBits::new(2048)?;
    /// assert_eq!(bits.get(), 2048);
    /// assert_eq!(bits.bytes()?, 256);
    /// assert!(SampleBits::new(0).is_err());
    /// assert!(SampleBits::new(7).is_err());
    /// # Ok::<(), rngkit_core::CoreError>(())
    /// ```
    pub fn new(bits: u32) -> Result<Self, CoreError> {
        if bits == 0 {
            return Err(CoreError::ZeroBitLength);
        }
        if bits % 8 != 0 {
            return Err(CoreError::BitLengthNotByteAligned {
                requested_bits: bits,
            });
        }
        Ok(Self(bits))
    }

    /// Bit count.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Byte length of one complete sample.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::SizeOverflow`] if the byte count does not fit in
    /// `usize`.
    pub fn bytes(self) -> Result<usize, CoreError> {
        let bytes = u64::from(self.0) / 8;
        usize::try_from(bytes).map_err(|_| CoreError::SizeOverflow { value: bytes })
    }
}

impl TryFrom<u32> for SampleBits {
    type Error = CoreError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<SampleBits> for u32 {
    fn from(value: SampleBits) -> Self {
        value.0
    }
}

impl std::fmt::Display for SampleBits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Counts one-bits in `bytes` using the platform popcount.
///
/// # Errors
///
/// Returns [`CoreError::PopcountOverflow`] if the running total exceeds `u64`.
///
/// # Examples
///
/// ```
/// use rngkit_core::count_ones;
///
/// assert_eq!(count_ones(&[0x00])?, 0);
/// assert_eq!(count_ones(&[0xFF])?, 8);
/// assert_eq!(count_ones(&[0xAA, 0x55])?, 8);
/// # Ok::<(), rngkit_core::CoreError>(())
/// ```
pub fn count_ones(bytes: &[u8]) -> Result<u64, CoreError> {
    let mut total = 0u64;
    for byte in bytes {
        total = total
            .checked_add(u64::from(byte.count_ones()))
            .ok_or(CoreError::PopcountOverflow)?;
    }
    Ok(total)
}
