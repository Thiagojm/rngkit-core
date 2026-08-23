# rngkit-sources

Adapters that implement `rngkit_core::EntropySource` for BitBabbler, TrueRNG
v1/v2/v3, Intel RDSEED, and OS-seeded ChaCha20 PseudoRNG.

`discover()` returns a best-effort snapshot of currently selectable candidates
and per-family issues. Empty hardware lists, `NotAvailable`, unsupported
RDSEED, and compile-time-disabled features are omitted. Other enumeration
failures stay in `issues` and do not hide later families. BitBabbler serials
and TrueRNG ports are transient selectors only; they are never persisted.
Discovery does not open a source or read entropy. PseudoRNG is advertised when
its feature is compiled in; OS entropy is checked only when a caller explicitly
opens it. Callers map a chosen candidate to `SourceConfig` themselves,
including BitBabbler fold. This crate does not serialize discovery types; a
Tauri adapter must define its own DTOs.

With all source features disabled, the crate still compiles and `discover()`
returns an empty report; no source can be constructed or opened.

Hardware listing never silently selects the first device when multiple devices
are present. BitBabbler collection uses `get_bits_with_fold`; the other
adapters use `get_bits`. There is no fallback between source kinds.

Default tests do not enumerate or open hardware. They inject a private fake
discovery backend. Physical tests live in `tests/hardware.rs` behind
`#[ignore]`.
