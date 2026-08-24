# TODO

## Done

- Six-crate workspace from the approved 2026-08-21 design/plan (`first impl`)
- Review safety and terminal-state corrections: UTF-8 timestamps, CSV
  one-count bounds, streaming legacy BIN, manifest stem/path checks,
  no-follow native artifact opens, race-safe XLSX temps/`ErrorIfExists`,
  and failed-session finalization for sink and completed-manifest errors
- Unified `rngkit_sources::discover()` snapshot API from the approved
  2026-08-21 source-discovery design/plan
- PseudoRNG discovery advertises compiled capability without constructing an
  OS-seeded adapter; explicit `open()` remains authoritative
- Physical BitBabbler, TrueRNG3, RDSEED, and `physical_discover` tests
  passed serially on this Windows host
- CI passed on Windows and Ubuntu, stable and Rust 1.85:
  `371f287` https://github.com/Thiagojm/rngkit-core/actions/runs/32539081921
  `dce82be` https://github.com/Thiagojm/rngkit-core/actions/runs/32548183132
- Derived concatenation inspect/create/`open_concatenation`/XLSX (Checkpoints
  1–2), validated on Windows stable and Rust 1.85; the Unix no-replace branch
  also checks and passes Clippy for `aarch64-unknown-linux-gnu`
- Format-neutral standalone legacy CSV/current CSV/BIN readers and schema-2
  current/legacy/mixed CSV concatenation, including normalized XLSX generation,
  validated on Windows stable and Rust 1.85

## In progress

- No implementation phase is currently authorized.

## Next steps

- Use the reachable Phase 1 `rngkit-core` revision in the separate Tauri app
  only after explicit Phase 2 authorization

## Backlog

- TrueRNGpro adapter (needs hardware and protocol validation)
- RngKitPSG version 2 import
- Sequential e-values / confidence sequences
- crates.io publication (blocked on Git-pinned source crates)
