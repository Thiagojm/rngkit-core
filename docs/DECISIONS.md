# Decisions

### Workspace and crate graph (2026-08-21)

- Status: accepted
- Contract:
  - One Cargo workspace, six packages under `crates/`
  - `rngkit-core` ← analysis, recording, sources
  - `rngkit-engine` ← core, analysis, recording
  - `rngkit-xlsx` ← core, analysis, recording
  - Source crates are optional Git deps of `rngkit-sources` (`bitb`, `trng3`,
    `rdseed`, `pseudo`; all default-on)
  - Pinned revisions: bitb-rs `18e586e`, trng3-rs `0bbe914`, intel_seed
    `1827409`, pseudo_rng `1c728bc`
- Why: isolate spreadsheet and hardware deps; keep source crates unmoved
- Impact: no crates.io publication until Git deps are replaced

### Collection, recording, and statistics (2026-08-21)

- Status: accepted
- Contract:
  - One source per session; synchronous caller-owned engine
  - Interval ≥ 1 s; wait `interval.saturating_sub(cycle_elapsed)`
  - Overrun emits an event and starts the next read immediately
  - Cancel during wait is prompt; cancel during a successful read commits that
    sample then stops
  - Native directory bundle with v3 stem; CSV is the commit marker
  - Loaded manifests parse the stem and require exact same-stem `bin`/`csv`
    basenames; native artifacts including `manifest.json` are opened without
    following links or reparse points, and native XLSX paths stay in the
    session dir
  - Legacy import is read-only v3 (`bitb`/`trng`/`pseudo`); reject v2
  - Legacy BIN is streaming and bounded to one sample buffer; CSV one-counts
    greater than `_s<bits>` fail at import
  - Event-sink failures are terminal and keep the primary error. Start, commit,
    overrun, completed-manifest, and `SessionStopped` failures best-effort
    finalize a failed manifest; secondary finalization or `SessionFailed`
    delivery must not replace the primary error
  - XLSX uses a unique create-new temporary file and `ErrorIfExists` promotes
    without replacing a concurrent destination
  - Z is descriptive; no p-values; chart `±1.96` lines are visual references
- Why: match approved design after statistical, reference-line, and review
  safety amendments
- Impact: Tauri, TrueRNGpro, v2 import, multi-source, reconnect, and sequential
  inference stay out of this workspace

### MSRV-compatible Excel stack (2026-08-21)

- Status: accepted
- Contract:
  - `rust_xlsxwriter = 0.96.0` (MSRV 1.83); not 0.97+ (MSRV 1.88)
  - Test reader `calamine = 0.35.0` (MSRV 1.83); not 0.36+ (MSRV 1.88)
- Why: workspace MSRV is 1.85; latest Excel crates require 1.88
- Impact: pin these versions in the workspace table; bump only after MSRV review
