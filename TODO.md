# TODO

## Done

- Six-crate workspace from the approved 2026-08-21 design/plan (`first impl`)
- Review safety and terminal-state corrections: UTF-8 timestamps, CSV
  one-count bounds, streaming legacy BIN, manifest stem/path checks,
  no-follow native artifact opens, race-safe XLSX temps/`ErrorIfExists`,
  and failed-session finalization for sink and completed-manifest errors
- Unified `rngkit_sources::discover()` snapshot API from the approved
  2026-08-21 source-discovery design/plan
- Physical BitBabbler, TrueRNG3, RDSEED, and `physical_discover` tests
  passed serially on this Windows host
- CI passed on Windows and Ubuntu, stable and Rust 1.85:
  `371f287` https://github.com/Thiagojm/rngkit-core/actions/runs/32539081921
  `dce82be` https://github.com/Thiagojm/rngkit-core/actions/runs/32548183132
- Derived concatenation inspect/create/`open_concatenation`/XLSX (Checkpoints
  1–2), validated on Windows stable and Rust 1.85; the Unix no-replace branch
  also checks and passes Clippy for `aarch64-unknown-linux-gnu`

## In progress

- None.

## Next steps

- Authorization gate A: commit and push a reachable `rngkit-core` revision
- After a reachable library revision, start the separate Tauri app (Checkpoint 3)

## Backlog

- TrueRNGpro adapter (needs hardware and protocol validation)
- RngKitPSG version 2 import
- Sequential e-values / confidence sequences
- crates.io publication (blocked on Git-pinned source crates)
