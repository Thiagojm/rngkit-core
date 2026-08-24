# rngkit-core

Reusable Rust workspace for modular entropy collection, recording, analysis,
source adapters, the synchronous engine, and XLSX reports. It is the library
layer before a future Tauri application.

## Packages

| Package | Role |
| --- | --- |
| `rngkit-core` | Domain contracts, `EntropySource`, popcount |
| `rngkit-sources` | BitBabbler, TrueRNG3, RDSEED, PseudoRNG adapters and `discover()` |
| `rngkit-analysis` | Incremental and batch descriptive cumulative statistics |
| `rngkit-recording` | Native JSON/BIN/CSV sessions, standalone current/legacy input readers, and CSV concatenation |
| `rngkit-engine` | Synchronous, caller-owned, cancellable collection |
| `rngkit-xlsx` | Excel analysis report generation |

## Constraints

- Rust edition 2024, MSRV 1.85, MIT
- No Tauri, Tokio, GUI, or other async runtime
- One source per session; no reconnect, resume, or XOR-combined sources
- Cumulative Z is descriptive; `±1.96` lines are visual references only

rngkit-recording::open_standalone reads a selected current CSV, legacy v3 CSV,
or fixed-size BIN without requiring a manifest. Current CSV rows use the exact
native seven-column header and are validated for indexes, RFC 3339 timestamps,
one-count bounds, byte lengths, and byte offsets. Current standalone inputs
support bitb, trng, rdseed, and pseudo; headerless legacy CSV remains limited
to the represented v3 families. A same-stem CSV/BIN pair is checked read-only
when present.

inspect_csv_inputs and create_csv_concatenation accept compatible legacy,
current, or mixed CSV sets. New derived bundles use schema 2 and
csv_concatenation, record each input format, and contain no absolute input
paths. The legacy-only inspection and creation wrappers remain available and
continue to read and write schema-1 bundles. open_concatenation reads both
schema versions.

## License

MIT
