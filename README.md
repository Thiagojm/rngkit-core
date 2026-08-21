# rngkit-core

Reusable Rust workspace for modular entropy collection, recording, analysis,
source adapters, the synchronous engine, and XLSX reports. It is the library
layer before a future Tauri application.

## Packages

| Package | Role |
| --- | --- |
| `rngkit-core` | Domain contracts, `EntropySource`, popcount |
| `rngkit-sources` | BitBabbler, TrueRNG3, RDSEED, PseudoRNG adapters |
| `rngkit-analysis` | Incremental and batch descriptive cumulative statistics |
| `rngkit-recording` | Native JSON/BIN/CSV sessions and read-only v3 import |
| `rngkit-engine` | Synchronous, caller-owned, cancellable collection |
| `rngkit-xlsx` | Excel analysis report generation |

## Constraints

- Rust edition 2024, MSRV 1.85, MIT
- No Tauri, Tokio, GUI, or other async runtime
- One source per session; no reconnect, resume, or XOR-combined sources
- Cumulative Z is descriptive; `±1.96` lines are visual references only

## License

MIT
