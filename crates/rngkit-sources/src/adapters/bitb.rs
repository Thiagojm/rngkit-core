//! BitBabbler adapter.

use bitb_rs::{BitBabbler, DeviceInfo, Fold as BitbFold};

use rngkit_core::{
    EntropySource, Fold, SOURCE_ID_BITB, SampleBits, SourceDescriptor, SourceError, SourceId,
};

use crate::error_mapping::{enforce_len, map_bitb};

/// Safe listing entry. Serial is for transient selection only and is not
/// persisted by this adapter's descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitbListing {
    /// White or Black, from the USB product string.
    pub variant: String,
    /// Transient serial used only to open a specific device.
    pub serial: String,
}

/// [`EntropySource`] wrapper around [`BitBabbler`].
pub struct BitbAdapter {
    inner: BitBabbler,
    descriptor: SourceDescriptor,
    fold: BitbFold,
}

impl BitbAdapter {
    /// Lists recognized devices. Does not open or select one.
    ///
    /// # Errors
    ///
    /// Returns mapped USB/enumeration errors from `bitb-rs`.
    pub fn list() -> Result<Vec<BitbListing>, SourceError> {
        let devices = BitBabbler::list_devices().map_err(map_bitb)?;
        Ok(devices.into_iter().map(listing).collect())
    }

    /// Opens one device. Multiple devices require an explicit serial.
    ///
    /// # Errors
    ///
    /// Returns [`rngkit_core::SourceErrorKind::MultipleDevices`] when more than
    /// one device is present and `serial` is `None`.
    pub fn open(fold: Fold, serial: Option<&str>) -> Result<Self, SourceError> {
        let inner = match serial {
            Some(serial) => BitBabbler::open_by_serial(serial).map_err(map_bitb)?,
            None => BitBabbler::open().map_err(map_bitb)?,
        };
        let bitb_fold = BitbFold::try_from(fold.get()).map_err(map_bitb)?;
        let variant = variant_label(inner.device_info());
        let descriptor = SourceDescriptor::new(
            SourceId::new(SOURCE_ID_BITB).expect("bitb id"),
            "BitBabbler",
            Some(variant),
            Some(fold),
        )
        .map_err(|err| {
            rngkit_core::SourceError::new(
                rngkit_core::SourceErrorKind::InvalidRequest,
                err.to_string(),
            )
        })?;
        Ok(Self {
            inner,
            descriptor,
            fold: bitb_fold,
        })
    }
}

impl EntropySource for BitbAdapter {
    fn descriptor(&self) -> &SourceDescriptor {
        &self.descriptor
    }

    fn read_bits(&mut self, bits: SampleBits) -> Result<Vec<u8>, SourceError> {
        let n = bits.bytes().map_err(|err| {
            rngkit_core::SourceError::new(
                rngkit_core::SourceErrorKind::InvalidRequest,
                err.to_string(),
            )
        })?;
        let bytes = self
            .inner
            .get_bits_with_fold(bits.get() as usize, self.fold)
            .map_err(map_bitb)?;
        enforce_len(n, bytes)
    }
}

fn listing(info: DeviceInfo) -> BitbListing {
    BitbListing {
        variant: variant_label(&info),
        serial: info.serial,
    }
}

fn variant_label(info: &DeviceInfo) -> String {
    match info.variant {
        bitb_rs::DeviceVariant::White => "White".into(),
        bitb_rs::DeviceVariant::Black => "Black".into(),
    }
}
