# TODO

## Done

- Six-crate workspace from the approved 2026-08-21 design/plan (`first impl`)
- Review safety and terminal-state corrections: UTF-8 timestamps, CSV
  one-count bounds, streaming legacy BIN, manifest stem/path checks,
  no-follow native artifact opens, race-safe XLSX temps/`ErrorIfExists`,
  and failed-session finalization for sink and completed-manifest errors
- Windows deterministic stable and MSRV 1.85 validation: 77 regular tests,
  7 doctests
- Physical BitBabbler, TrueRNG3, and RDSEED tests passed serially on this
  Windows host

## In progress

- None in this workspace. Next product phase is the Tauri application, in a
  separate project.

## Next steps

- Treat Linux CI as unverified until a remote job actually passes
- Start the Tauri app only after this library contract is accepted as the base

## Backlog

- TrueRNGpro adapter (needs hardware and protocol validation)
- RngKitPSG version 2 import
- Sequential e-values / confidence sequences
- crates.io publication (blocked on Git-pinned source crates)
