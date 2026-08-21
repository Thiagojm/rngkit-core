# RngKit Core Workspace Design

**Status:** Approved
**Date:** 2026-08-21  
**Workspace root:** `D:\Projetos\rustie\libs\rngkit-core`

## 1. Context and problem

The legacy `RngKitPSG` application combines device access, sampling loops, file
writing, statistical analysis, chart preparation, spreadsheet export, process
management, and GUI state in the same Python application. Its principal flow is
to collect a fixed number of bits from one selected source at a user-defined
interval, save raw bytes and per-sample one-bit counts, calculate cumulative
Z-scores, and export an Excel workbook with a chart.

Four reusable Rust source crates already exist outside this workspace:

- `bitb-rs` for BitBabbler White/Black, including explicit folds 0 through 4;
- `trng3-rs` for TrueRNG v1/v2/v3 identified by USB VID/PID `04D8:F5FE`;
- `intel_seed` for Intel RDSEED without fallback;
- `pseudo_rng` for an OS-seeded ChaCha20 CSPRNG.

Those crates intentionally remain synchronous and exclude Tauri, async runtimes,
file I/O, analysis, and charts. The missing reusable layer is therefore the
application-independent orchestration and data pipeline between those sources
and a future Tauri backend.

The new libraries must preserve the familiar version 3 collection filename
convention while using a self-describing native session bundle. They must also
read version 3 legacy `.bin` and `.csv` files without modifying them.

## 2. Goals

1. Provide a small, typed abstraction over the four existing entropy sources
   without changing their public contracts.
2. Run a cancellable, single-source sampling session without depending on
   Tauri, Tokio, or another async runtime.
3. persist each native session as a portable directory containing a manifest,
   raw `.bin` data, and per-sample `.csv` records.
4. Preserve the RngKitPSG version 3 filename stem for all new collection files.
5. Read RngKitPSG version 3 legacy `.bin` and `.csv` files as normalized,
   read-only session data.
6. Calculate one-bit counts and cumulative Z-scores incrementally and in batch.
7. Generate a separate `.xlsx` analysis report with summary, sample data, and a
   cumulative Z-score chart.
8. Keep each concern independently reusable by command-line programs, tests,
   other desktop applications, and the future Tauri app.
9. Use Rust edition 2024 and MSRV 1.85 across the workspace, matching the
   existing source crates.

## 3. Non-goals

- Building or scaffolding the Tauri application or frontend.
- Supporting TrueRNGpro (`16D0:0AA0`) in version 1.
- Importing RngKitPSG version 2 filenames or space-delimited CSV files.
- Running more than one entropy source in a session.
- Synchronizing, comparing, or XOR-combining live sources.
- Dynamically loading third-party source plugins through a stable ABI.
- Resuming an interrupted recording in place.
- Treating statistical output as an entropy health gate or certification.
- Presenting conventional fixed-horizon p-values, confidence intervals, or
  visual reference lines as formal significance thresholds for manually stopped
  sessions.
- Providing anytime-valid p-values, e-values, confidence sequences, or another
  formal sequential-inference method in version 1.
- Installing drivers, changing USB permissions, or managing operating-system
  device configuration.
- Storing device serial numbers or operating-system device paths in session
  files.

## 4. Workspace and crate boundaries

The workspace is one Git repository containing six packages under `crates/`:

```text
rngkit-core/
├── Cargo.toml
├── crates/
│   ├── rngkit-core/
│   ├── rngkit-sources/
│   ├── rngkit-analysis/
│   ├── rngkit-recording/
│   ├── rngkit-engine/
│   └── rngkit-xlsx/
└── docs/
```

Package names use hyphens. Their Rust import names use underscores, for example
`rngkit_core` and `rngkit_recording`.

### 4.1 `rngkit-core`

Owns dependency-light domain contracts shared by the other packages:

- `SampleBits`, a validated positive bit count divisible by eight;
- `SourceId`, a validated stable identifier such as `trng` or `rdseed`;
- `SourceDescriptor`, containing a stable ID, display label, and safe variant
  metadata;
- `EntropySource`, the synchronous source trait;
- normalized `SourceError` and `SourceErrorKind` values;
- session configuration and common sample/session value types;
- the dependency-free byte popcount used to create and validate a recorded
  sample's one-bit count.

The essential source contract is conceptually:

```rust
pub trait EntropySource: Send {
    fn descriptor(&self) -> &SourceDescriptor;
    fn read_bits(&mut self, bits: SampleBits) -> Result<Vec<u8>, SourceError>;
}
```

`read_bits` must return exactly `bits.bytes()` bytes or an error. It must never
expose partial entropy. The trait deliberately excludes `random_u64` and
`random_range`, because the collection pipeline only needs byte-aligned samples.

### 4.2 `rngkit-sources`

Depends on `rngkit-core` and the four existing device/source crates. It owns:

- adapters implementing `EntropySource` for wrapped BitBabbler, TrueRNG3,
  RDSEED, and PseudoRNG instances;
- source discovery for sources that can be enumerated;
- typed source configuration and explicit device selection;
- the factory that opens a configured source and returns an owned source handle;
- mapping of each source crate's typed errors into normalized `SourceErrorKind`
  values while retaining a diagnostic error chain.

Stable source IDs and filename tokens are:

| Source | Stable ID | Source-specific setting |
|---|---|---|
| BitBabbler | `bitb` | explicit fold `0..=4` |
| TrueRNG v1/v2/v3 | `trng` | optional explicit recognized path |
| Intel RDSEED | `rdseed` | retry policy |
| PseudoRNG | `pseudo` | sampling policy |

BitBabbler collection calls `get_bits_with_fold`; the other adapters call their
source crate's `get_bits`. The adapter layer does not add a fallback when a
source is unavailable.

The registry is compile-time Rust composition, not a dynamic plugin ABI. Adding
a future TrueRNGpro adapter may extend `rngkit-sources` without changing the
recording, analysis, engine, or spreadsheet contracts.

### 4.3 `rngkit-analysis`

Depends only on `rngkit-core`. It owns pure, deterministic statistical
calculations:

- incremental total bits, total one-bits, observed one proportion, deviation
  from `0.5`, and signed cumulative Z-score;
- batch analysis over normalized sample records;
- validation that `ones <= sample_bits` and that all records use the session's
  fixed sample size.

For sample number `n`, sample size `b`, total evaluated bits `N_n = n * b`, and
cumulative one-bit count `C_n`, the observed one proportion and signed cumulative
Z-score are:

```text
p_hat_n = C_n / N_n
delta_n = p_hat_n - 0.5
Z_n = (C_n - N_n / 2) / sqrt(N_n / 4)
    = (2 * C_n - N_n) / sqrt(N_n)
```

The implementation keeps integral counts in checked integer types and converts
to `f64` only for proportion, deviation, and Z-score calculations. Analysis
never accepts or rejects entropy and never changes the raw data.

#### Statistical interpretation

`Z_n` is the signed form of the frequency/monobit statistic. Positive values
mean an excess of ones and negative values mean an excess of zeroes. Under the
null model, its standard-normal interpretation assumes that all evaluated bits
are independent and identically distributed Bernoulli observations with
`P(1) = 0.5`.

The workspace does not establish that assumption. In particular, a monobit
statistic does not test serial independence, runs, min-entropy, source health,
or causal effects. Folding or conditioning performed by a source does not turn
the chart into a certification of randomness.

The session has no predetermined sample count and may be stopped while the live
chart is visible. Consequently, the sequence of cumulative Z values is treated
as a correlated descriptive trajectory. A point crossing a familiar
fixed-horizon threshold such as `+/-1.96` is not reported as a 5% significance
result. The chart may show horizontal `+1.96` and `-1.96` reference lines, but
their labels and legend must identify them only as visual references. Version 1
does not calculate a conventional final p-value or confidence interval and does
not use the Z trajectory or a reference-line crossing to accept or reject a
hypothesis.

Formal inference under optional stopping requires a separate time-uniform
method, such as an e-process or confidence sequence, with its own documented
assumptions. That method is deferred rather than approximated with repeated
fixed-horizon tests.

### 4.4 `rngkit-recording`

Depends on `rngkit-core`. It owns:

- native session name construction and parsing;
- manifest serialization and schema versioning;
- append-only `.bin` and `.csv` recording;
- session consistency inspection;
- normalized read-only access to native bundles;
- read-only import of RngKitPSG version 3 legacy files.

It does not calculate Z-scores and does not generate spreadsheets.

### 4.5 `rngkit-engine`

Depends on `rngkit-core`, `rngkit-analysis`, and `rngkit-recording`. It owns the
single-source collection state machine:

1. validate configuration;
2. create the native recording bundle;
3. read one complete sample;
4. count one-bits;
5. commit the raw bytes and CSV record;
6. update incremental analysis;
7. publish a typed event;
8. wait for the remainder of the configured interval;
9. repeat until cancellation or failure;
10. finalize the manifest.

The engine is synchronous and blocking. It does not create a Tokio runtime and
does not decide which application thread runs it. A caller may run it directly,
on a standard thread, or with Tauri's blocking-task mechanism.

### 4.6 `rngkit-xlsx`

Depends on `rngkit-core`, `rngkit-recording`, and `rngkit-analysis`. It owns only
Excel report generation. Spreadsheet-specific dependencies do not leak into
collection, recording, source, or analysis packages.

## 5. Source selection and session configuration

A session selects exactly one already configured source. Hardware discovery must
show all recognized devices and must not silently choose the first device when
multiple devices are present. PseudoRNG and RDSEED appear as local sources when
their constructors/runtime capability checks succeed.

The version 1 session configuration contains:

- one source configuration;
- `SampleBits`;
- an integer interval in seconds, minimum `1`;
- an output root directory.

The version 1 engine runs until cancellation or a terminal error. Fixed-duration
and fixed-sample-count user modes are outside this design.

## 6. Timing and cancellation

Each cycle measures total elapsed time using a monotonic clock. The measured
cycle begins immediately before source acquisition and ends after the sample has
been durably committed, analyzed, and delivered to the event sink.

```text
remaining = configured_interval.saturating_sub(cycle_elapsed)
```

If `remaining` is positive, the engine performs a cancellation-aware wait for
that duration. If the cycle took at least the configured interval, the engine
emits a timing-overrun event and starts the next read immediately. It does not
create queued tasks, maintain a backlog, overlap reads, discard scheduled
samples, or terminate solely because of an overrun.

Cancellation is checked before a read and during the interval wait. A blocking
hardware read already in progress cannot be forcefully interrupted by the
engine. If cancellation arrives during a read and the source returns a complete
sample, that sample is committed before the session stops. Stop latency is
therefore bounded by the underlying source operation, not only by the interval.

## 7. Native session bundle

### 7.1 Naming

The session uses local wall-clock time for its legacy-compatible filename stem:

```text
YYYYMMDDTHHMMSS_<source>_s<bits>_i<seconds>[_f<fold>]
```

Examples:

```text
20260821T183000_trng_s2048_i1
20260821T183000_bitb_s2048_i1_f0
20260821T183000_pseudo_s2048_i1
20260821T183000_rdseed_s2048_i1
```

The `_f0` through `_f4` suffix is mandatory for BitBabbler and forbidden for all
other sources. A pre-existing path with the same stem causes a typed
`AlreadyExists` error; no file is overwritten and no suffix that changes the
legacy convention is invented.

### 7.2 Directory layout

The output root contains one directory per session:

```text
20260821T183000_trng_s2048_i1/
├── 20260821T183000_trng_s2048_i1.bin
├── 20260821T183000_trng_s2048_i1.csv
└── manifest.json
```

The `.bin` and `.csv` files are the collection artifacts. The manifest makes the
bundle self-describing and versionable.

### 7.3 Manifest schema version 1

`manifest.json` contains:

- `schema_version: 1`;
- the filename/session stem;
- status: `recording`, `completed`, or `failed`;
- stable source ID and safe source variant metadata;
- BitBabbler fold when applicable;
- sample size in bits;
- interval in integer seconds;
- start time in UTC RFC 3339;
- local UTC offset used to create the filename;
- optional completion time in UTC RFC 3339;
- optional terminal failure kind and diagnostic;
- finalized committed-sample and timing-overrun counts;
- the relative `.bin` and `.csv` filenames.

The manifest never stores raw OS device paths, USB serial numbers, PRNG seed or
state, or entropy bytes. Manifest replacement uses a sibling temporary file and
an atomic rename within the session directory. Readers derive the authoritative
committed count from the CSV and do not trust a stale manifest count after a
crash.

### 7.4 Native CSV schema

New native CSV files have one header row and these columns in this order:

```text
sample_index,captured_at_utc,elapsed_ms,acquisition_ms,ones,byte_offset,byte_length
```

- `sample_index` is one-based and contiguous.
- `captured_at_utc` is an RFC 3339 UTC timestamp captured after the complete
  source read.
- `elapsed_ms` is monotonic time since the session started.
- `acquisition_ms` measures the source read only.
- `ones` is the popcount of the complete sample.
- `byte_offset` is zero-based in the `.bin` file.
- `byte_length` must equal `sample_bits / 8`.

Z-scores are derived data and are not stored in the collection CSV.

### 7.5 Commit and consistency contract

For each sample, the recorder:

1. verifies the byte length and one-bit count;
2. appends raw bytes at the expected offset;
3. flushes and durably syncs the `.bin` data;
4. appends the corresponding CSV row;
5. flushes and durably syncs the CSV data;
6. reports the sample as committed.

A CSV row is the commit marker for the corresponding raw range. A crash after
step 3 may leave an uncommitted binary tail. Native readers ignore that tail and
report a consistency warning. They do not truncate or repair files
automatically. A CSV row whose declared bytes are absent from the binary file is
hard corruption; analysis and export fail rather than silently using incomplete
data.

Sessions are one-shot. Version 1 does not resume or append to a previously
interrupted bundle. A manifest left in `recording` state is exposed to readers as
an interrupted session, while its fully committed sample prefix remains
available if consistency checks pass.

## 8. Legacy RngKitPSG version 3 import

The legacy reader accepts only version 3 stems using `YYYYMMDDTHHMMSS` and the
source IDs `bitb`, `trng`, or `pseudo`. Version 2 hyphenated timestamps and
space-delimited CSV are rejected with a typed unsupported-version/format error.

Legacy version 3 CSV files are headerless, comma-delimited rows containing the
timestamp and one-bit count. Legacy binary files contain contiguous fixed-size
samples whose size is obtained from `_s<bits>` in the filename.

Import is read-only:

- selecting a CSV provides timestamps and one-bit counts for analysis;
- selecting a BIN splits it into exact fixed-size samples and calculates counts;
- when both sibling files exist, the importer combines them only after sample
  counts and popcounts agree;
- an incomplete trailing binary sample is an error, not silently discarded;
- a legacy BIN without CSV uses the filename start plus the configured interval
  only as an explicitly marked estimated timestamp sequence;
- legacy files are never renamed, rewritten, moved, or automatically converted
  into a native bundle.

The normalized reader interface allows `rngkit-analysis` and `rngkit-xlsx` to
consume native and legacy sessions without duplicating parsers.

## 9. Excel analysis report

For a native bundle, `rngkit-xlsx` writes
`<session-directory>/<session-stem>.xlsx`. For a legacy input, it writes the same
stem beside the selected legacy file. It writes to a temporary sibling and
renames only after the workbook closes successfully. Existing output causes a
typed conflict unless the caller explicitly chooses an overwrite policy.

The workbook contains:

### 9.1 `Summary` sheet

- source and safe source variant;
- BitBabbler fold when applicable;
- sample size and interval;
- start, completion, and duration when known;
- session status;
- committed sample count;
- total bits and total one-bits;
- observed one proportion and its signed deviation from `0.5`;
- final signed cumulative Z-score, labeled as descriptive;
- timing-overrun count when known;
- timestamp provenance (`recorded` or `estimated`) for legacy data.

### 9.2 `Samples` sheet

- sample index;
- captured/estimated timestamp;
- elapsed time when available;
- acquisition time when available;
- one-bit count;
- cumulative one-bit count;
- cumulative one proportion;
- signed cumulative Z-score.

The workbook stores calculated values rather than spreadsheet formulas. It also
contains a line chart of signed cumulative Z-score against sample index, with a
zero reference line and dashed horizontal reference lines at `+1.96` and
`-1.96`. The two reference series use hidden constant helper values and are
labeled `Reference +1.96` and `Reference -1.96`; they are not labeled as
significance, confidence, acceptance, rejection, pass, or fail boundaries. The
same labeling rule applies when a future Tauri frontend renders the live graph.
The exporter never changes the source `.bin`, `.csv`, or manifest.

If the sample count exceeds the supported Excel worksheet row capacity, export
fails with a typed size error and leaves no partial final workbook. Version 1
does not silently truncate or split one analysis across multiple sample sheets.

## 10. Engine events and failure behavior

The engine emits typed events only after the relevant state transition:

- `SessionStarted` after the native bundle is initialized;
- `SampleCommitted` after both raw bytes and the CSV commit row are durable and
  incremental analysis has been updated;
- `TimingOverrun` when a complete cycle meets or exceeds the interval;
- `SessionStopped` after cancellation and clean manifest finalization;
- `SessionFailed` after a terminal source, recording, analysis, or event-sink
  error and best-effort failed-manifest finalization.

Events carry domain values, not Tauri DTOs. A future Tauri adapter will convert
them to serializable frontend events.

Failures follow these rules:

- invalid configuration fails before source reads or session file creation;
- source errors stop the session; there is no automatic reconnect or fallback;
- partial source buffers are never recorded;
- recording failure stops acquisition immediately;
- a failed manifest update does not hide the original terminal error;
- event-sink failure is terminal so the caller cannot unknowingly lose updates;
- cancellation is a normal stop, not an error.

## 11. Security, privacy, and resource limits

- Session paths are generated from validated values, not arbitrary caller
  fragments. All created paths must remain below the canonical output root.
- Existing session directories and reports are never overwritten implicitly.
- Legacy inputs are opened read-only.
- Device paths and hardware serials may be used transiently to open a selected
  source but are not persisted in collection artifacts or diagnostics intended
  for export.
- PseudoRNG seed/state is never exposed or persisted.
- `SampleBits`, file sizes, row counts, offsets, and cumulative counts use
  checked conversions and arithmetic before allocation or workbook creation.
- The engine holds only the current raw sample and cumulative analysis state in
  memory; it does not retain the whole session during collection.
- Raw random bytes and descriptive statistics are not represented as proof of
  cryptographic quality, causal effects, or device health.

## 12. Compatibility and rollout

- Core, analysis, recording, engine, and XLSX packages are platform-neutral and
  must build on Windows and Linux.
- `rngkit-sources` inherits the actual platform and hardware constraints of the
  four existing source crates; automated builds do not claim physical hardware
  validation.
- The native schema starts at version 1. Readers reject unknown future major
  schema versions rather than guessing.
- Implementation proceeds library-first. The Tauri project begins only after
  the workspace contracts and deterministic test suites are complete.
- No existing source crate is moved into this workspace. Initial integration
  uses explicit dependencies on their current repositories/paths.

## 13. Alternatives considered

### SQLite-backed `.rngkit` file

Rejected by user decision in favor of a transparent directory containing JSON,
BIN, and CSV artifacts.

### One crate with modules and feature flags

Rejected because spreadsheet, device, and file dependencies would be harder to
isolate and reuse independently.

### One repository per new crate

Rejected because coordinated API evolution and end-to-end testing would require
unnecessary cross-repository version management.

### Tokio-based engine or engine-owned worker threads

Rejected to keep runtime and thread ownership in the consumer. The synchronous
engine works in Tauri blocking tasks, command-line tools, and ordinary tests.

### Multi-source sessions or XOR-combined streams

Rejected for version 1. A session owns one source, making recording and failure
semantics unambiguous.

### Conventional p-values after manual stopping

Rejected because the live chart is continuously observed and the stopping time
is not predetermined. Repeatedly interpreting cumulative fixed-horizon tests
would not preserve the nominal false-positive rate.

### Sequential e-values or confidence sequences in version 1

Deferred to a separate design. They can support inference at arbitrary stopping
times, but require an explicit inferential contract beyond legacy-compatible
descriptive Z-score analysis.

### Exact legacy CSV as the native CSV schema

Rejected because the two-column legacy format cannot represent byte offsets,
monotonic elapsed time, or acquisition duration. Only the filename convention is
preserved for new artifacts; the manifest and headered CSV provide the modern
self-describing contract.

## 14. Validation strategy

### 14.1 Deterministic unit tests

- Validate every accepted and rejected `SampleBits`, source ID, fold, interval,
  and filename case.
- Verify core popcount and analysis cumulative Z-score against hand-calculated
  vectors, including observed proportion, deviation, zero/one extremes, sign,
  and long checked accumulations.
- Verify that the XLSX chart contains dashed `Reference +1.96` and
  `Reference -1.96` series and that the report contains no p-value or
  significance decision.
- Verify native CSV and manifest round trips without hardware.
- Inject failures after each recorder commit step and verify consistency
  classification and absence of false committed samples.
- Use fake clocks, waits, sources, cancellation, and sinks to verify cycle timing,
  immediate next reads after overruns, prompt cancellation during waits, and
  completion of an in-progress full sample.
- Verify normalized mappings for representative typed errors from every source
  crate without requiring attached devices.
- Verify XLSX sheet names, cell values, chart series, atomic output behavior, and
  row-limit errors by reopening generated workbooks in tests.

### 14.2 Integration tests

- Run a finite mock session through engine, recording, native reader, batch
  analysis, and XLSX export; compare incremental and batch results.
- Import representative version 3 CSV-only, BIN-only, and paired fixtures.
- Reject version 2 names/CSV, mismatched sibling files, partial final samples,
  binary tails, and CSV references beyond binary EOF as specified.
- Build every workspace package with the Rust 1.85 toolchain.

### 14.3 Physical validation boundary

Source hardware tests remain in the existing device repositories and retain
their ignored/serial execution rules. Workspace integration may be exercised
manually with available TrueRNG3 and BitBabbler hardware, but deterministic CI
must not open hardware or claim physical support.

## 15. Acceptance criteria

1. The workspace contains the six packages and dependency directions described
   in section 4, with no dependency on Tauri or Tokio.
2. Each existing source can be opened through `rngkit-sources` and used through
   the same `EntropySource::read_bits` contract without modifying its source
   crate contract.
3. A configured session rejects zero/non-byte-aligned samples and intervals
   below one second before creating collection artifacts.
4. One mock session produces exactly one legacy-named directory, one same-stem
   BIN, one same-stem CSV, and one schema-versioned manifest.
5. Every reported committed sample has a durable complete raw range and one
   matching contiguous CSV row.
6. Cancellation during the interval wait is prompt; cancellation during a
   successful blocking read commits that complete sample and then stops.
7. A cycle over one second starts the next cycle immediately, emits an overrun,
   and never creates overlapping or queued reads.
8. Native reader consistency checks detect binary tails and reject CSV records
   that reference missing bytes.
9. Version 3 legacy CSV-only, BIN-only, and consistent sibling pairs are readable
   without mutation; version 2 inputs are rejected.
10. Incremental and batch cumulative Z-score results match for the same sample
    sequence.
11. The XLSX report contains `Summary`, `Samples`, and a valid cumulative Z-score
    chart, while leaving collection files unchanged; the Z-score is labeled
    descriptive, `+1.96` and `-1.96` appear only as visual reference lines, and
    no conventional p-value or significance decision is presented.
12. Multiple hardware devices are never resolved by silently selecting the
    first one.
13. Session artifacts contain no hardware serial, OS device path, PRNG state, or
    seed.
14. Deterministic workspace formatting, tests, lints, docs, and MSRV builds pass
    without attached hardware.

## 16. Decisions and assumptions

- The user selected a native directory bundle rather than SQLite.
- The user selected one source per session.
- The user selected a synchronous caller-owned execution model.
- Timing follows measured cycle duration and waits only for the remaining part
  of an interval whose minimum is one second.
- New collection filenames preserve the RngKitPSG version 3 stem, including
  `rdseed` as the new Intel source token.
- Only RngKitPSG version 3 legacy input is supported.
- Excel analysis is a separate crate from BIN/CSV collection and uses the full
  summary/samples/chart layout in section 9.
- Sessions have no predetermined sample count. Signed cumulative Z-score remains
  the central descriptive graph, while conventional fixed-horizon inference is
  excluded and time-uniform sequential inference is deferred.
- Cumulative Z-score charts include `+1.96` and `-1.96` visual reference lines;
  crossings have no pass/fail or significance meaning in version 1.
- TrueRNGpro support is deferred until hardware and protocol validation are
  available.

## 17. Statistical references

- NIST SP 800-22 Rev. 1a, Section 2.1, Frequency (Monobit) Test:
  `https://doi.org/10.6028/NIST.SP.800-22r1a`.
- NIST SP 800-90B, Section 5, Testing the IID Assumption:
  `https://doi.org/10.6028/NIST.SP.800-90B`.
- Johari, Koomen, Pekelis, and Walsh, *Always Valid Inference: Continuous
  Monitoring of A/B Tests*, Operations Research 70(3):1806-1821:
  `https://doi.org/10.1287/opre.2021.2135`.
- Howard, Ramdas, McAuliffe, and Sekhon, *Time-uniform, nonparametric,
  nonasymptotic confidence sequences*, Annals of Statistics 49(2):1055-1080:
  `https://doi.org/10.1214/20-AOS1991`.
