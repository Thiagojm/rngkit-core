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
- Artifact feedback Phase 1: manifest-free flat legacy concatenation reader and
  explicit contextual XLSX chart presentation, focused and complete stable/MSRV
  validation passed and published at `7c79814`
- Local-clock chart labels use manifest offsets, infer current standalone CSV
  offsets from canonical filenames, preserve legacy clocks, and pass the
  complete stable/MSRV validation suite.
- Explicitly selected report basenames remain authoritative even when a `.bin`
  is normalized through a valid recorded CSV sibling; stable/MSRV validation
  passed.

## In progress

- Mixed-source concatenation Phase 1 (core + XLSX) implemented. Homogeneous
  schema 2 and mixed schema 3 (`mixed` / `Mixed sources`) are covered by
  locked tests. Awaiting user validation of mixed vs homogeneous bundles
  and optional mixed XLSX. Optional Excel inspection of recorded-time and
  BIN/index workbooks remains unverified.

## Next steps

- After Phase 1 validation and a separate Phase 2 request: pin the published
  exact `rngkit-core` revision in the Tauri app and update Combine/Help.

## Backlog

- TrueRNGpro adapter (needs hardware and protocol validation)
- RngKitPSG version 2 import
- Sequential e-values / confidence sequences
- crates.io publication (blocked on Git-pinned source crates)

## Source freshness integration (2026-09-10)

- TrueRNG per-acquisition purge is integrated; the user reported successful
  native app acceptance on 2026-09-10, without an independent hardware log.
- BitBabbler e4cc6c6 prevents reuse after acquisition failures and bounds sync/purge;
  Drop no longer drains input. No adapter API or RDSEED change is needed.
- Locked workspace all-target tests, clippy and MSRV 1.85 check passed on Windows.
  Four physical tests remained ignored; native BitBabbler retest is pending.
