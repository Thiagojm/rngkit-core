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

### Unified source discovery (2026-08-21)

- Status: accepted
- Contract:
  - `rngkit_sources::discover()` returns `DiscoveryReport` with present
    `SourceCandidate` values and non-blocking `DiscoveryIssue` values
  - Family order is BitBabbler, TrueRNG, RDSEED, PseudoRNG; hardware listings
    keep the underlying crate order
  - Empty hardware lists, hardware `NotAvailable`, unsupported RDSEED, and
    disabled features are normal absence
  - With all source features disabled, the crate remains usable for composition
    and `discover()` returns an empty report
  - Any other per-family failure is retained as an issue and does not hide
    later families
  - BitBabbler/TrueRNG are listed, never opened, and never implicitly reduced
    to the first device
  - PseudoRNG is advertised when its feature is compiled in; discovery does
    not construct the adapter, and OS entropy is checked only at explicit
    `open()`
  - Serials and port names exist only on transient candidates; they are not
    added to descriptors, manifests, sessions, reports, or serde types
  - Tauri must map this API to its own DTOs; the library adds no serialization
    or async runtime
  - Deterministic tests inject a private fake backend; only the ignored
    physical smoke test calls public `discover()`
- Why: keep adapter discovery policy in the library so Tauri and later
  consumers do not duplicate it, while keeping discovery free of source opens
  and entropy acquisition
- Impact: discovery does not reserve a device; `open()` remains authoritative
  after the snapshot. Remote CI passed on Windows and Ubuntu with stable and
  Rust 1.85 for commit `dce82be`

### Derived legacy CSV concatenation (2026-08-22)

- Status: accepted
- Contract:
  - Derived names use `YYYYMMDDTHHMMSS_concat_<source>_s<bits>_i<seconds>[_f<fold>]`
    and are independent of `SessionStem`
  - Manifest schema version 1, kind `legacy_csv_concatenation`; input entries
    store basename, SHA-256, row count, first/last timestamp, and output range
  - `inspect_legacy_csvs(&[PathBuf])` streams files, hashes bytes, and returns
    a preview with no absolute-path serialization
  - Inputs must be distinct nonempty readable RngKitPSG v3 CSVs (`bitb` /
    `trng` / `pseudo`) with matching source, bits, interval, and fold
  - Timestamps must be nondecreasing within a file; equal values inside one
    file are accepted; equal or overlapping boundaries between files are not
  - Native CSV, v2, empty, duplicate canonical, mixed, and malformed inputs
    fail with explicit `RecordingError` variants
  - Manifest parsing revalidates every input's nonzero row count, timestamp
    order, and inclusive output span after deserialization
  - Preview is advisory; `create_legacy_csv_concatenation` reopens, rehashes,
    and revalidates inputs, then streams
    `sample_index,captured_at_utc,ones,input_index,input_sample_index`
  - Creation uses a unique contained staging directory, syncs CSV and manifest,
    and promotes with atomic no-replace primitives on Windows and supported
    Unix targets; unsupported Unix targets fail closed. Failure cleans owned
    staging and does not mutate inputs
  - `open_concatenation` validates the contained same-stem CSV, ranges, and
    one-count bounds, then returns a `NormalizedSession`
  - `derived_report_path` stays inside the validated bundle directory
- Why: the approved Tauri Combine workflow needs reusable provenance-bearing
  concatenation without copying the legacy sort-and-append behavior
- Impact: `rngkit-recording` owns SHA-256, derived bundles, and
  `open_concatenation`; `rngkit-xlsx` consumes the normalized view

### Standalone and format-neutral CSV inputs (2026-08-24)

- Status: accepted, implemented, and validated; publication authorized
- Contract:
  - `open_standalone` detects one current native CSV, legacy v3 CSV, or
    fixed-size BIN from its exact content and validated filename stem without
    requiring a manifest
  - Current CSV requires the exact seven-column native header, contiguous
    one-based indexes and byte offsets, RFC 3339 timestamps, sample-sized byte
    lengths, and bounded one-counts. Current standalone source IDs are
    `bitb`, `trng`, `rdseed`, and `pseudo`
  - Legacy v3 CSV accepts the observed compact timestamp
    `YYYYMMDDTHHMMSS,<ones>` and retains the older colon-bearing form for
    compatibility; v2/space-delimited input remains rejected
  - A same-stem CSV/BIN pair is validated read-only; standalone BIN timestamps
    are estimated from the filename start and interval
  - `inspect_csv_inputs` and `create_csv_concatenation` accept compatible
    legacy-only, current-only, and mixed CSV sets
  - New derived manifests use schema 2, kind `csv_concatenation`, and a
    per-input `current_csv` or `legacy_v3_csv` format. Existing schema-1
    `legacy_csv_concatenation` manifests remain readable without migration
  - Legacy-only public wrappers remain restrictive and preserve schema-1
    behavior. Preview, debug, and manifests contain basenames/hashes only
- Why: centralize parsing, normalization, compatibility, and provenance before
  Tauri Reports and Combine integration
- Impact: no Tauri or source-adapter changes; application integration remains a
  separately authorized phase and must use an exact reachable revision

### MSRV-compatible Excel stack (2026-08-21)

- Status: accepted
- Contract:
  - `rust_xlsxwriter = 0.96.0` (MSRV 1.83); not 0.97+ (MSRV 1.88)
  - Test reader `calamine = 0.35.0` (MSRV 1.83); not 0.36+ (MSRV 1.88)
- Why: workspace MSRV is 1.85; latest Excel crates require 1.88
- Impact: pin these versions in the workspace table; bump only after MSRV review

### Flat legacy concatenation and contextual report charts (2026-08-25)

- Status: implemented and locally validated; publication remains separately
  unauthorized
- Contract:
  - A canonical `YYYYMMDDTHHMMSS_concat_<source>_s<bits>_i<seconds>[_f<fold>].csv`
    is a reportable headerless legacy artifact without `manifest.json`.
  - `ConcatenationStem` remains independent from `SessionStem`; the flat reader
    validates legacy source/fold rules, nonempty rows, timestamp order, and
    one-count bounds, then preserves recorded row timestamps in a normalized
    session.
  - XLSX callers pass a validated source basename and `RecordedTimestamp` or
    `SampleIndex` through `ReportOptions`. Recorded charts use a hidden
    `HH:mm:ss` category helper and BIN-only charts use the sample-index column.
    Titles and axes identify the source, interval, sample size, and descriptive
    cumulative signed Z without inferential language.
- Why: support the older flat concatenation artifact without inventing a
  manifest or provenance, and prevent estimated BIN timestamps from appearing
  as recorded chart times.
- Impact: Tauri resolver integration, dependency pinning, library publication,
  and native Excel rendering remain later/separate work.

### Local clock labels in recorded-time charts (2026-08-25)

- Status: accepted, implemented, and validated
- Contract:
  - Native sessions convert recorded UTC timestamps with the local UTC offset
    preserved by `manifest.json`.
  - A current standalone CSV without a manifest infers the offset from the
    local wall-clock start in its canonical filename and its first recorded UTC
    row, rounded to a valid 15-minute offset.
  - Legacy CSV rows already represent local wall-clock values and are not
    shifted a second time. Flat and manifest-backed concatenations retain their
    normalized per-row clocks; BIN-only charts remain sample-index based.
  - The full normalized timestamp column is unchanged; only the hidden chart
    category clock labels are localized.
- Why: chart labels should match the collection machine's local clock without
  discarding UTC data or inventing times for BIN-only inputs.
- Impact: consumers must pin an exact reachable `rngkit-core` revision that
  contains this behavior.
