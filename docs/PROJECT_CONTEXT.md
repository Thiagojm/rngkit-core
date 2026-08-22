# Project context

## Purpose

Library workspace that sits between the existing synchronous source crates
(`bitb-rs`, `trng3-rs`, `intel_seed`, `pseudo_rng`) and a future Tauri app.
It collects from exactly one source, records a native session bundle, computes
descriptive cumulative statistics, and can export an Excel report.

## Main flows

1. Snapshot selectable sources with `rngkit_sources::discover()`; map one
   candidate to an explicit `SourceConfig`.
2. Open that source through `rngkit-sources` / `EntropySource`.
3. Run `rngkit-engine` until cancellation or a terminal error.
4. Persist a native directory: `<stem>.bin`, `<stem>.csv`, `manifest.json`.
5. Read native bundles or RngKitPSG v3 files through normalized records.
6. Analyze incrementally or in batch with the same accumulator.
7. Export `<stem>.xlsx` with Summary, Samples, and a descriptive Z chart.

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
- `discover()` is a best-effort snapshot: present candidates plus per-family
  issues; it does not watch hot-plug, cache, or reserve a device
- With every source feature disabled, `rngkit-sources` still compiles and
  `discover()` returns an empty report
- Do not persist serials, OS device paths, seeds, or PRNG state
- Native manifests and report paths must stay inside the selected session directory
- Native BIN, CSV, and `manifest.json` opens must not follow links out of the session
- Event-sink and completed-manifest failures finalize the session as failed and keep the primary error
- XLSX uses a unique create-new temp; `ErrorIfExists` must not replace a concurrent destination
- Legacy BIN import streams one sample at a time; CSV one-counts cannot exceed sample bits
