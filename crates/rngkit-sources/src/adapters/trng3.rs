//! TrueRNG v1/v2/v3 adapter.

use rngkit_core::{
    EntropySource, SOURCE_ID_TRNG, SampleBits, SourceDescriptor, SourceError, SourceErrorKind,
    SourceId,
};
use trng3_rs::{DeviceInfo, TrueRng3};

use crate::error_mapping::{enforce_len, map_trng3};

/// Transient listing of a recognized TrueRNG port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrngListing {
    /// OS port name. Transient; not stored on the descriptor.
    pub port_name: String,
}

/// [`EntropySource`] wrapper around [`TrueRng3`].
pub struct Trng3Adapter {
    inner: TrueRng3,
    descriptor: SourceDescriptor,
}

impl Trng3Adapter {
    /// Lists recognized `04D8:F5FE` ports. Does not select the first device.
    ///
    /// # Errors
    ///
    /// Returns mapped serial enumeration errors.
    pub fn list() -> Result<Vec<TrngListing>, SourceError> {
        let devices = TrueRng3::list_devices().map_err(map_trng3)?;
        Ok(devices.into_iter().map(listing).collect())
    }

    /// Opens one device. Multiple devices require an explicit path.
    ///
    /// # Errors
    ///
    /// Returns [`SourceErrorKind::MultipleDevices`] when more than one device is
    /// present and `path` is `None`.
    pub fn open(path: Option<&str>) -> Result<Self, SourceError> {
        let inner = match path {
            Some(path) => TrueRng3::open_path(path).map_err(map_trng3)?,
            None => TrueRng3::open().map_err(map_trng3)?,
        };
        let descriptor = SourceDescriptor::new(
            SourceId::new(SOURCE_ID_TRNG).expect("trng id"),
            "TrueRNG v1/v2/v3",
            Some("TrueRNG".into()),
            None,
        )
        .map_err(|err| SourceError::new(SourceErrorKind::InvalidRequest, err.to_string()))?;
        let _ = inner.device_info();
        Ok(Self { inner, descriptor })
    }
}

impl EntropySource for Trng3Adapter {
    fn descriptor(&self) -> &SourceDescriptor {
        &self.descriptor
    }

    fn read_bits(&mut self, bits: SampleBits) -> Result<Vec<u8>, SourceError> {
        let n = bits
            .bytes()
            .map_err(|err| SourceError::new(SourceErrorKind::InvalidRequest, err.to_string()))?;
        let bytes = self
            .inner
            .get_bits(bits.get() as usize)
            .map_err(map_trng3)?;
        enforce_len(n, bytes)
    }
}

fn listing(info: DeviceInfo) -> TrngListing {
    TrngListing {
        port_name: info.port_name,
    }
}
