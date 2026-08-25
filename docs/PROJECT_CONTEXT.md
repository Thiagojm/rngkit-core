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
5. Read native bundles, standalone current CSV/BIN files, or RngKitPSG v3
   files through normalized records.
6. Analyze incrementally or in batch with the same accumulator.
7. Export `<stem>.xlsx` with Summary, Samples, and a descriptive Z chart.
   Canonical flat legacy `_concat_` CSVs are also reportable without a
   manifest.
8. Inspect compatible current/legacy CSVs with `inspect_csv_inputs` for a
   derived concatenation preview (format labels, basenames, and SHA-256, no
   absolute paths).
9. Create a schema-2 derived concatenation bundle with
   `create_csv_concatenation`, reopen it with `open_concatenation`, and export
   XLSX through `derived_report_path` / `write_report`.

## Domain terms

- **Stem:** `YYYYMMDDTHHMMSS_<source>_s<bits>_i<seconds>[_f<fold>]`
- **Concat stem:** `YYYYMMDDTHHMMSS_concat_<source>_s<bits>_i<seconds>[_f<fold>]`
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
- Discovery never opens a source or reads entropy; an enabled PseudoRNG is
  advertised as a compiled capability and checks OS entropy only at `open()`
- With every source feature disabled, `rngkit-sources` still compiles and
  `discover()` returns an empty report
- Do not persist serials, OS device paths, seeds, or PRNG state
- Native manifests and report paths must stay inside the selected session directory
- Native BIN, CSV, and `manifest.json` opens must not follow links out of the session
- Event-sink and completed-manifest failures finalize the session as failed and keep the primary error
- XLSX uses a unique create-new temp; `ErrorIfExists` must not replace a concurrent destination
- Legacy BIN import streams one sample at a time; CSV one-counts cannot exceed sample bits
- Standalone current CSVs use the exact native seven-column header and validate
   contiguous indexes, RFC 3339 timestamps, byte lengths/offsets, and
   one-count bounds. Current standalone inputs support `bitb`, `trng`, `rdseed`,
   and `pseudo`; headerless legacy CSV remains limited to `bitb`, `trng`, and
   `pseudo`. Same-stem CSV/BIN pairs are checked read-only
- Derived concatenation can inspect distinct nonempty current, legacy, or mixed
   CSVs with matching source/bits/interval/fold; equal timestamps are allowed
   inside one file and rejected as overlap between files; previews never
   persist absolute input paths
- New derived creation reopens and revalidates inputs, streams a same-stem CSV
   plus `manifest.json` through unique staging, and promotes with atomic
   no-replace semantics on Windows and supported Unix targets; unsupported Unix
   targets fail closed. Inputs are not mutated; readers validate contained CSV,
   ranges, and one-count bounds. New bundles use schema 2 with per-input format;
   schema-1 legacy bundles remain readable
- Derived report paths stay inside the validated bundle directory
- Flat legacy concatenation CSVs use the independent `ConcatenationStem`, are
  read-only and manifest-free, retain recorded row timestamps, and reject
  current headers, empty/decreasing input, unsupported legacy sources, and
  one-count overflow
- XLSX report presentation accepts an explicit safe source basename and axis
  mode. Recorded-time charts use hidden `HH:mm:ss` categories while BIN-only
  charts use sample indexes; title and axis copy remains descriptive
- Recorded-time chart labels use a native manifest's captured local UTC offset.
  Without a manifest, current CSVs infer that offset from the canonical local
  filename start versus the first UTC row; legacy CSV clocks remain unshifted.
  Full normalized timestamps are preserved in the Samples sheet

## Current validation evidence

- Phase 1 standalone and format-neutral CSV support passed the complete
  deterministic workspace suite on Windows with stable Rust and Rust 1.85 on
  2026-08-24. This includes XLSX generation from standalone legacy CSV,
  current CSV, BIN, schema-1 concatenation, and schema-2 concatenation inputs.
- The four opt-in physical hardware tests were not run during that validation;
  the default suite confirmed that they remain ignored.
- **Artifact feedback and report charts Phase 1 (2026-08-25):** focused flat
  concatenation and XLSX tests, complete stable workspace validation, and the
  complete Rust 1.85 check/test validation passed locally. The library remains
  at its existing reachable baseline because commit, push, and publication are
  separate approvals. Optional Excel rendering review remains unverified.
- **Local clock correction (2026-08-25):** focused XLSX tests and the complete
  stable workspace format/check/test/clippy suite passed locally. Tests cover
  manifest offset conversion, filename inference for manifest-free current
  CSV, unchanged legacy clocks, and the BIN/index boundary. Native Excel
  inspection remains unverified.
- **Selected-basename correction (2026-08-25):** `ReportOptions` can retain the
  explicitly selected `.bin` basename while still deriving recorded-time mode
  and local-clock context from a valid sibling CSV. Focused XLSX tests and the
  complete stable/MSRV workspace matrix passed locally; physical hardware tests
  remained ignored.
