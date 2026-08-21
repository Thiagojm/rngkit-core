//! Concrete [`rngkit_core::EntropySource`] adapters.

#[cfg(feature = "bitb")]
pub mod bitb;
#[cfg(feature = "pseudo")]
pub mod pseudo;
#[cfg(feature = "rdseed")]
pub mod rdseed;
#[cfg(feature = "trng3")]
pub mod trng3;

#[cfg(feature = "bitb")]
pub use bitb::BitbAdapter;
#[cfg(feature = "pseudo")]
pub use pseudo::PseudoAdapter;
#[cfg(feature = "rdseed")]
pub use rdseed::RdseedAdapter;
#[cfg(feature = "trng3")]
pub use trng3::Trng3Adapter;
