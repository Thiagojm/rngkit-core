# Project context

## Purpose

Library workspace that sits between the existing synchronous source crates
(`bitb-rs`, `trng3-rs`, `intel_seed`, `pseudo_rng`) and a future Tauri app.
It collects from exactly one source, records a native session bundle, computes
descriptive cumulative statistics, and can export an Excel report.

## Main flows

1. Open one configured source through `rngkit-sources` / `EntropySource`.
2. Run `rngkit-engine` until cancellation or a terminal error.
3. Persist a native directory: `<stem>.bin`, `<stem>.csv`, `manifest.json`.
4. Read native bundles or RngKitPSG v3 files through normalized records.
5. Analyze incrementally or in batch with the same accumulator.
6. Export `<stem>.xlsx` with Summary, Samples, and a descriptive Z chart.

## Domain terms

- **Stem:** `YYYYMMDDTHHMMSS_<source>_s<bits>_i<seconds>[_f<fold>]`
- **Commit marker:** the native CSV row; BIN-first, CSV-last
- **Descriptive Z:** `(2*C - N) / sqrt(N)`; not a significance decision
- **Reference ±1.96:** visual chart lines only

## Stable constraints

- Edition 2024, MSRV 1.85, version 0.1.0, MIT
- No Tauri, Tokio, GUI, or other async runtime
- Source crates stay external, revision-pinned Git dependencies
- Default tests never enumerate or open hardware
- Do not persist serials, OS device paths, seeds, or PRNG state
