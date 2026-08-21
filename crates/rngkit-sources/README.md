# rngkit-sources

Adapters that implement `rngkit_core::EntropySource` for BitBabbler, TrueRNG
v1/v2/v3, Intel RDSEED, and OS-seeded ChaCha20 PseudoRNG.

Hardware discovery never silently selects the first device when multiple
devices are present. BitBabbler collection uses `get_bits_with_fold`; the other
adapters use `get_bits`. There is no fallback between source kinds.

Default tests do not enumerate or open hardware. Physical tests live in
`tests/hardware.rs` behind `#[ignore]`.
