# TODO

## Done

- Six-crate workspace from the approved 2026-08-21 design/plan (`first impl`)
- Review safety and terminal-state corrections: UTF-8 timestamps, CSV
  one-count bounds, streaming legacy BIN, manifest stem/path checks,
  no-follow native artifact opens, race-safe XLSX temps/`ErrorIfExists`,
  and failed-session finalization for sink and completed-manifest errors
- Unified `rngkit_sources::discover()` snapshot API from the approved
  2026-08-21 source-discovery design/plan
- Windows deterministic stable and MSRV 1.85 validation after discovery:
  92 regular tests, 7 doctests, all-features and zero/single-feature builds
- Physical BitBabbler, TrueRNG3, RDSEED, and `physical_discover` tests
  passed serially on this Windows host
- Base library CI passed on Windows and Ubuntu, stable and Rust 1.85, for
  commit `371f287`:
  https://github.com/Thiagojm/rngkit-core/actions/runs/32539081921
- Unified discovery CI passed on Windows and Ubuntu, stable and Rust 1.85, for
  commit `dce82be`:
  https://github.com/Thiagojm/rngkit-core/actions/runs/32548183132

## In progress

- None in this workspace. Next product phase is the Tauri application, in a
  separate project.

## Next steps

- Start the Tauri app only after this library contract is accepted as the base

## Backlog

- TrueRNGpro adapter (needs hardware and protocol validation)
- RngKitPSG version 2 import
- Sequential e-values / confidence sequences
- crates.io publication (blocked on Git-pinned source crates)
