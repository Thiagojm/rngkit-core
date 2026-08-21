# rngkit-core

Validated domain contracts for the RngKit workspace: sample sizes, source
identifiers, the synchronous `EntropySource` trait, shared session types, and
a dependency-free byte popcount.

`read_bits` is all-or-error. The collection abstraction does not include
`random_u64` or `random_range`. Persistable types have no field for an OS
device path, hardware serial, PRNG seed, or generator state.
