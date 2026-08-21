# TODO

## Done

- Implemented the six-crate workspace from the approved 2026-08-21 design/plan
- Deterministic stable and MSRV 1.85 validation passed on Windows without hardware
- Physical adapter tests exist behind `#[ignore]` and were not run

## In progress

- None in this workspace. Next product phase is the Tauri application, in a
  separate project.

## Next steps

- After explicit authorization: commit (this work did not commit)
- Run ignored hardware tests serially when BitBabbler / TrueRNG3 / RDSEED are
  attached: `cargo test -p rngkit-sources --test hardware -- --ignored --test-threads=1 --nocapture`
- Treat Linux CI as unverified until a remote job actually passes
- Start the Tauri app only after this library contract is accepted as the base

## Backlog

- TrueRNGpro adapter (needs hardware and protocol validation)
- RngKitPSG version 2 import
- Sequential e-values / confidence sequences
- crates.io publication (blocked on Git-pinned source crates)
