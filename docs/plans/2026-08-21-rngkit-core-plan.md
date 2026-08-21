# RngKit Core Workspace Implementation Plan

**Date:** 2026-08-21  
**Approved design:** `docs/specs/2026-08-21-rngkit-core-design.md`  
**Workspace root:** `D:\Projetos\rustie\libs\rngkit-core`

## 1. Goal

Implement and verify a Rust 2024/MSRV 1.85 Cargo workspace containing six
independently reusable crates:

- `rngkit-core` for validated domain contracts and popcount;
- `rngkit-sources` for adapters over the four existing RNG crates;
- `rngkit-analysis` for descriptive cumulative statistics;
- `rngkit-recording` for native JSON/BIN/CSV sessions and legacy v3 readers;
- `rngkit-engine` for synchronous cancellable collection;
- `rngkit-xlsx` for the separate Excel analysis report.

Completion means the deterministic workspace suite passes without attached
hardware, the end-to-end mock pipeline produces and reopens a valid session and
workbook, MSRV checks pass, and documentation accurately separates automated
evidence from any later physical-device run.

## 2. Out of scope

- Tauri commands, frontend code, live UI rendering, packaging, or installers.
- TrueRNGpro and RngKitPSG version 2 import.
- Multi-source sessions, live comparison, or XOR-combined sources.
- Async runtimes, engine-owned worker threads, automatic reconnect, or session
  resume.
- Conventional fixed-horizon p-values, confidence intervals, significance
  decisions, or sequential e-values/confidence sequences.
- RNG certification, entropy estimation, health gating, or causal claims.
- Driver installation, USB permission changes, remote repository creation,
  commit, push, release, crates.io publication, or deployment.

## 3. Prerequisites

1. Rust stable and Rust 1.85.0 are available. Do not install a missing toolchain
   silently.
2. The four source repositories remain available at their approved public Git
   revisions:

   | Package | Repository | Revision |
   |---|---|---|
   | `bitb-rs` | `https://github.com/Thiagojm/bitb-rs` | `18e586e5e2cf3e14742d1cd86f593a8ad1adbe3d` |
   | `trng3-rs` | `https://github.com/Thiagojm/trng3-rs` | `0bbe91494aae5f53977db559ce6443b2278b98ba` |
   | `intel_seed` | `https://github.com/Thiagojm/intel_seed-rs` | `182740952a44304302420a5589f134964d5e792a` |
   | `pseudo_rng` | `https://github.com/Thiagojm/pseudo_rng-rs` | `1c728bc7c5dc30f2225649751eb3da35b63aad9b` |

3. Select external serialization, timestamp, CSV, XLSX, and test dependencies
   whose declared MSRV is compatible with Rust 1.85. Pin them through the root
   workspace dependency table and verify them with the MSRV commands in this
   plan. Do not infer compatibility from the newest stable build alone.
4. Use only mock sources in default tests. Attached hardware must not be opened
   by `cargo test --workspace`.

## 4. Ordered implementation steps

### Step 1 — Scaffold the workspace and repository-native context

**Create or modify**

- `Cargo.toml`
- `Cargo.lock` (generated after the first successful workspace resolution)
- `.gitignore`
- `LICENSE`
- `README.md`
- `AGENTS.md`
- `docs/PROJECT_CONTEXT.md`
- `docs/DECISIONS.md`
- `TODO.md`
- `.github/workflows/ci.yml`
- `crates/rngkit-core/Cargo.toml`
- `crates/rngkit-core/src/lib.rs`
- `crates/rngkit-sources/Cargo.toml`
- `crates/rngkit-sources/src/lib.rs`
- `crates/rngkit-analysis/Cargo.toml`
- `crates/rngkit-analysis/src/lib.rs`
- `crates/rngkit-recording/Cargo.toml`
- `crates/rngkit-recording/src/lib.rs`
- `crates/rngkit-engine/Cargo.toml`
- `crates/rngkit-engine/src/lib.rs`
- `crates/rngkit-xlsx/Cargo.toml`
- `crates/rngkit-xlsx/src/lib.rs`

**Actions**

1. Initialize a local Git repository at the workspace root if it is not already
   a worktree. Do not create a commit or remote.
2. Create a resolver-3 Cargo workspace with the six members under `crates/`.
3. Set shared package metadata to version `0.1.0`, edition `2024`,
   `rust-version = "1.85"`, and MIT licensing.
4. Put internal path dependencies and shared external dependency versions in
   `[workspace.dependencies]`.
5. Declare the four source crates as optional, revision-pinned Git dependencies
   of `rngkit-sources`, behind `bitb`, `trng3`, `rdseed`, and `pseudo` features;
   enable all four by default for the initial RngKit consumer.
6. Configure CI to run deterministic formatting, check, test, Clippy, doctest,
   and MSRV jobs on Windows and Linux. Do not include ignored hardware tests.
7. Record the approved crate graph, statistical interpretation, physical-test
   boundary, and current milestones in the repo-native context files.
8. Keep the existing approved design and this plan unchanged except for factual
   corrections discovered during implementation.

**Acceptance criteria**

- `cargo metadata --no-deps` lists exactly the six intended workspace members.
- The root is a local Git worktree with no commit or remote created by this step.
- Every empty crate builds on stable and Rust 1.85.
- No crate depends on Tauri, Tokio, a GUI library, or another async runtime.
- The default test suite contains no hardware enumeration/open call.

### Step 2 — Implement `rngkit-core` domain contracts

**Create or modify**

- `crates/rngkit-core/src/lib.rs`
- `crates/rngkit-core/src/sample.rs`
- `crates/rngkit-core/src/source.rs`
- `crates/rngkit-core/src/session.rs`
- `crates/rngkit-core/src/error.rs`
- `crates/rngkit-core/tests/contracts.rs`
- `crates/rngkit-core/README.md`

**Actions**

1. Implement `SampleBits` with checked construction, positive byte alignment,
   byte conversion, and allocation-safe size conversion.
2. Implement validated `SourceId` values and constants for `bitb`, `trng`,
   `rdseed`, and `pseudo` without closing the type to future IDs.
3. Implement safe `SourceDescriptor` metadata that cannot contain persisted OS
   paths, hardware serials, seeds, or generator state.
4. Define `EntropySource: Send` with `descriptor` and all-or-error `read_bits`.
5. Define non-stringly typed `SourceErrorKind` buckets and a diagnostic
   `SourceError` that retains an error chain without becoming a serialized IPC
   contract.
6. Define common session/sample values used across crate boundaries, including
   UTC timestamps, monotonic elapsed/acquisition durations, one-based sample
   indexes, offsets, and byte lengths.
7. Implement the dependency-free byte popcount used by engine and legacy
   validation.
8. Add rustdoc examples and boundary tests for invalid bits, identifiers,
   overflow, exact lengths, and popcount extremes.

**Dependencies**

- Step 1.

**Acceptance criteria**

- Zero and non-byte-aligned sample sizes fail before allocation/source calls.
- Popcount matches hand-calculated all-zero, all-one, alternating, and mixed
  byte vectors.
- The public trait can be implemented by a deterministic mock without any
  device or runtime dependency.
- Persistable domain types have no field capable of leaking a device selector or
  PRNG state.

### Step 3 — Implement descriptive statistics in `rngkit-analysis`

**Create or modify**

- `crates/rngkit-analysis/src/lib.rs`
- `crates/rngkit-analysis/src/accumulator.rs`
- `crates/rngkit-analysis/src/error.rs`
- `crates/rngkit-analysis/tests/cumulative.rs`
- `crates/rngkit-analysis/README.md`

**Actions**

1. Implement an incremental accumulator with checked sample count, total bits,
   and total one-bits.
2. Produce, for every committed sample, observed one proportion, signed
   deviation from `0.5`, and signed cumulative Z-score:

   ```text
   p_hat = C / N
   delta = p_hat - 0.5
   Z = (2*C - N) / sqrt(N)
   ```

3. Implement batch analysis over common normalized sample records using the same
   accumulator; do not maintain a second formula.
4. Reject `ones > sample_bits`, mismatched sample sizes, and checked-count
   overflow.
5. Document the IID Bernoulli `P(1)=0.5` assumption and that cumulative Z is a
   correlated descriptive trajectory, not a sequential hypothesis decision.
6. Do not expose p-values, confidence intervals, pass/fail states, or entropy
   quality claims.
7. Test exact zero sign, positive/negative symmetry, known legacy-formula
   examples, incremental/batch equality, and large checked accumulations.

**Dependencies**

- Step 2.

**Acceptance criteria**

- Results match the approved formula within documented floating-point tolerance.
- Positive Z always represents excess ones and negative Z excess zeroes.
- No public type or documentation labels `+/-1.96` as significance.
- Incremental and batch output are identical for the same normalized records.

### Step 4 — Implement native naming and manifest contracts

**Create or modify**

- `crates/rngkit-recording/src/lib.rs`
- `crates/rngkit-recording/src/naming.rs`
- `crates/rngkit-recording/src/manifest.rs`
- `crates/rngkit-recording/src/error.rs`
- `crates/rngkit-recording/tests/naming.rs`
- `crates/rngkit-recording/tests/manifest.rs`

**Actions**

1. Implement exact version 3 stem generation and parsing:
   `YYYYMMDDTHHMMSS_<source>_s<bits>_i<seconds>[_f<fold>]`.
2. Require `_f0.._f4` only for BitBabbler, use `rdseed` for Intel, and reject
   unsupported/malformed names without partial parsing.
3. Use local wall-clock time only for the stem; store start UTC and local offset
   separately in manifest schema version 1.
4. Implement manifest states `recording`, `completed`, and `failed`, safe source
   metadata, relative artifact names, and finalized counters.
5. Implement same-directory temporary manifest writes and a platform-correct
   replacement strategy that leaves either the previous complete manifest or
   the new complete manifest after failure.
6. Reject unknown schema versions and existing same-stem session paths without
   overwrite or invented suffixes.
7. Ensure generated/canonicalized paths remain under the selected output root.

**Dependencies**

- Step 2.

**Acceptance criteria**

- Golden tests cover all four source stems, folds, timezone offsets, malformed
  version 2 names, collisions, and path-containment failures.
- Manifest JSON round-trips without persisting hardware selectors or secret
  state.
- Replacing a manifest never exposes a partially serialized final manifest.

### Step 5 — Implement native BIN/CSV recording and reading

**Create or modify**

- `crates/rngkit-recording/src/native/mod.rs`
- `crates/rngkit-recording/src/native/writer.rs`
- `crates/rngkit-recording/src/native/reader.rs`
- `crates/rngkit-recording/src/native/csv.rs`
- `crates/rngkit-recording/src/consistency.rs`
- `crates/rngkit-recording/tests/native_roundtrip.rs`
- `crates/rngkit-recording/tests/failure_injection.rs`
- `crates/rngkit-recording/README.md`

**Actions**

1. Create the session directory, same-stem BIN/CSV files, CSV header, and initial
   recording manifest without overwriting an existing path.
2. Make the writer owned, non-cloneable, and single-session so concurrent writers
   cannot share one recording handle.
3. Commit each sample in the approved order: validate, append/sync BIN,
   append/sync CSV, then return committed status.
4. Write the exact native columns:
   `sample_index,captured_at_utc,elapsed_ms,acquisition_ms,ones,byte_offset,byte_length`.
5. Implement clean completion and best-effort failed finalization.
6. Implement a streaming native reader that treats CSV rows as commit markers,
   derives committed count from CSV, validates contiguous indexes/offsets/lengths,
   and does not load the whole BIN into memory.
7. Report and ignore an uncommitted binary tail; reject CSV references beyond
   BIN EOF. Do not truncate, repair, or resume automatically.
8. Add deterministic failure injection after every commit boundary and verify
   the observable consistency result.

**Dependencies**

- Steps 2 and 4.

**Acceptance criteria**

- A round trip preserves raw bytes and every CSV field exactly.
- No sample is reported committed before its raw range and CSV row are durable.
- A BIN tail produces a warning and a readable committed prefix.
- Missing BIN bytes referenced by CSV produce hard corruption and block analysis.
- Collection memory remains bounded to the active sample plus writer buffers.

### Step 6 — Implement read-only RngKitPSG version 3 import

**Create or modify**

- `crates/rngkit-recording/src/legacy_v3/mod.rs`
- `crates/rngkit-recording/src/legacy_v3/csv.rs`
- `crates/rngkit-recording/src/legacy_v3/bin.rs`
- `crates/rngkit-recording/src/normalized.rs`
- `crates/rngkit-recording/tests/legacy_v3.rs`

**Actions**

1. Parse only version 3 `T` stems for `bitb`, `trng`, and `pseudo`; reject
   version 2 hyphenated stems and space-delimited rows.
2. Read headerless comma-delimited version 3 CSV timestamps and one-bit counts.
3. Stream exact fixed-size BIN samples using `_s<bits>` and core popcount; reject
   a partial trailing sample.
4. When both sibling files exist, verify count and per-sample popcount equality
   before exposing the combined normalized view.
5. For BIN-only input, synthesize explicitly estimated timestamps from filename
   start and `_i<seconds>`; preserve timestamp provenance.
6. Expose native and legacy data through the same normalized streaming record
   contract used by analysis/XLSX.
7. Open all legacy inputs read-only and never convert or modify them implicitly.

**Dependencies**

- Steps 2, 4, and 5.

**Acceptance criteria**

- Generated CSV-only, BIN-only, and paired v3 fixtures normalize as designed.
- Mismatched pairs, malformed timestamps/counts, version 2 files, and partial BIN
  records fail with distinguishable typed errors.
- File hashes before and after import remain identical.

### Step 7 — Implement the four source adapters

**Create or modify**

- `crates/rngkit-sources/src/lib.rs`
- `crates/rngkit-sources/src/config.rs`
- `crates/rngkit-sources/src/registry.rs`
- `crates/rngkit-sources/src/error_mapping.rs`
- `crates/rngkit-sources/src/adapters/mod.rs`
- `crates/rngkit-sources/src/adapters/bitb.rs`
- `crates/rngkit-sources/src/adapters/trng3.rs`
- `crates/rngkit-sources/src/adapters/rdseed.rs`
- `crates/rngkit-sources/src/adapters/pseudo.rs`
- `crates/rngkit-sources/tests/adapters.rs`
- `crates/rngkit-sources/tests/hardware.rs`
- `crates/rngkit-sources/README.md`

**Actions**

1. Implement typed source configuration and factory/open functions for all four
   sources.
2. Enumerate BitBabbler/TrueRNG devices without selecting the first when multiple
   candidates exist; require explicit serial/path selection in that case.
3. Map BitBabbler folds 0 through 4 exactly and call
   `get_bits_with_fold`; use raw `get_bits` for the other adapters.
4. Advertise RDSEED through its runtime capability check and open PseudoRNG only
   after successful OS seeding; never add fallback between source kinds.
5. Validate returned length again at the adapter boundary so a source contract
   violation cannot reach recording.
6. Map each source's typed errors into stable `SourceErrorKind` categories while
   retaining source diagnostics/error chaining.
7. Unit-test adapters with narrow internal seams/mocks. Put physical integration
   tests behind `#[ignore]`, serialize them, avoid recording serials, and allow
   only genuine source absence to skip.

**Dependencies**

- Steps 1 and 2.

**Acceptance criteria**

- Default tests do not enumerate/open hardware.
- Feature combinations compile independently and with all features.
- Multiple recognized devices produce a selection error rather than implicit
  first-device use.
- No adapter returns partial buffers or silently changes BitBabbler fold.

### Step 8 — Implement the synchronous collection engine

**Create or modify**

- `crates/rngkit-engine/src/lib.rs`
- `crates/rngkit-engine/src/cancellation.rs`
- `crates/rngkit-engine/src/event.rs`
- `crates/rngkit-engine/src/runner.rs`
- `crates/rngkit-engine/src/error.rs`
- `crates/rngkit-engine/tests/session_pipeline.rs`
- `crates/rngkit-engine/tests/timing.rs`
- `crates/rngkit-engine/README.md`

**Actions**

1. Implement a reusable cancellation token with cancellation-aware timed wait;
   do not use an uninterruptible interval `thread::sleep`.
2. Implement a typed event sink and events `SessionStarted`, `SampleCommitted`,
   `TimingOverrun`, `SessionStopped`, and `SessionFailed`.
3. Implement the blocking single-source loop in the approved order. Emit
   `SampleCommitted` only after BIN/CSV durability and incremental analysis.
4. Measure each cycle with a monotonic clock from before acquisition through
   successful event delivery. Wait for `interval.saturating_sub(elapsed)`.
5. If elapsed meets/exceeds the interval, emit an overrun and begin the next read
   immediately without queue, overlap, dropped schedule entries, or terminal
   backlog error.
6. Check cancellation before reads and during waits. If cancellation arrives
   during a successful blocking read, commit that full sample, emit its event,
   and then finalize normally.
7. Make source, recorder, analysis, and sink errors terminal and preserve the
   primary cause if failed-manifest finalization also fails.
8. Keep clock/wait injection private or test-only. Test timing deterministically
   without real one-second sleeps.

**Dependencies**

- Steps 2, 3, 4, and 5. The engine accepts `EntropySource`, so Step 7 is not
  required for mock-engine completion.

**Acceptance criteria**

- The mock pipeline produces contiguous durable samples and matching incremental
  events.
- Cancellation during wait is prompt in deterministic tests.
- Cancellation during read commits one complete returned sample and no next
  sample.
- Overrun tests start the next read immediately and prove maximum source-call
  concurrency remains one.
- No engine module depends on a device crate, Tauri, or Tokio.

### Step 9 — Implement the separate XLSX analysis exporter

**Create or modify**

- `crates/rngkit-xlsx/src/lib.rs`
- `crates/rngkit-xlsx/src/report.rs`
- `crates/rngkit-xlsx/src/layout.rs`
- `crates/rngkit-xlsx/src/error.rs`
- `crates/rngkit-xlsx/tests/report.rs`
- `crates/rngkit-xlsx/README.md`

**Actions**

1. Consume only normalized readers and batch analysis; do not parse BIN/CSV again.
2. Write the `Summary` fields from the approved design, including total bits,
   total ones, observed proportion, signed deviation from `0.5`, descriptive
   final Z, provenance, and timing/session metadata when available.
3. Write `Samples` rows with index, timestamp, timing values, one count,
   cumulative count, cumulative proportion, and signed cumulative Z.
4. Add a signed cumulative Z line chart against sample index with a zero line and
   dashed `Reference +1.96` and `Reference -1.96` series backed by hidden constant
   helper values.
5. Do not label the reference lines as significance, confidence, acceptance,
   rejection, pass, or fail, and do not write a p-value.
6. Enforce Excel row limits before creating the final workbook.
7. Write to a sibling temporary file and atomically/best-effort safely promote it
   only after workbook close; reject an existing report unless an explicit
   overwrite policy is supplied.
8. For native sessions, output inside the session directory. For legacy input,
   output beside the selected file without modifying it.
9. Reopen cell data with an XLSX reader in tests and inspect chart OOXML to verify
   series names, values, dashed styling, and absence of significance text.

**Dependencies**

- Steps 3, 5, and 6.

**Acceptance criteria**

- Native and all supported legacy views generate equivalent analysis rows for
  equivalent samples.
- The workbook contains exactly `Summary` and `Samples` user-visible sheets and
  one valid cumulative chart.
- Both `+1.96` and `-1.96` reference series are present and descriptive only.
- Row-limit, collision, and injected-write failures leave no partial final XLSX.

### Step 10 — Add end-to-end regression coverage and finalize context

**Create or modify**

- `tests/` only if a root integration harness package is required; otherwise
  place cross-crate integration tests under `crates/rngkit-engine/tests/` and
  `crates/rngkit-xlsx/tests/`
- `README.md`
- `AGENTS.md`
- `docs/PROJECT_CONTEXT.md`
- `docs/DECISIONS.md`
- `TODO.md`
- `.github/workflows/ci.yml`

**Actions**

1. Run a finite mock source through engine, native recording, native reading,
   batch analysis, and XLSX export. Compare raw data, CSV metadata, incremental
   results, batch results, summary cells, and chart series.
2. Exercise one source-adapter mock per feature through the common trait.
3. Run all deterministic validation commands on stable and Rust 1.85.
4. Run CI-equivalent checks on Windows locally. Record Linux as CI evidence only
   after the remote job actually passes.
5. Optionally run ignored physical adapter tests serially when the corresponding
   available hardware is connected. Record each OS/device-family result
   separately; do not generalize TrueRNG3/BitBabbler White evidence to other
   devices or operating systems.
6. Update context files with only durable decisions, verified commands/results,
   and concrete remaining work. Keep Tauri integration as the next project phase.
7. Inspect `git diff --check` and the full working-tree diff. Do not commit or push
   without separate authorization.

**Dependencies**

- Steps 1 through 9.

**Acceptance criteria**

- All deterministic checks below pass with no attached hardware.
- The end-to-end mock fixture proves the library pipeline is ready for a thin
  Tauri adapter.
- Documentation distinguishes deterministic, CI, physical, and unverified
  evidence.
- No commit, push, release, publication, or Tauri scaffold is performed.

## 5. Test plan

Run from `D:\Projetos\rustie\libs\rngkit-core`:

```text
cargo fmt --all -- --check
cargo metadata --no-deps
cargo check --workspace --all-targets --all-features
cargo test --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --doc --all-features
cargo +1.85.0 check --workspace --all-targets --all-features
cargo +1.85.0 test --workspace --all-targets --all-features
cargo tree --workspace --all-features
git diff --check
```

Also compile meaningful source feature subsets so optional adapters do not hide
undeclared coupling:

```text
cargo check -p rngkit-sources --no-default-features --features bitb
cargo check -p rngkit-sources --no-default-features --features trng3
cargo check -p rngkit-sources --no-default-features --features rdseed
cargo check -p rngkit-sources --no-default-features --features pseudo
```

Physical adapter validation is explicit, ignored, and serial. Run only the
selected test(s) for hardware actually available:

```text
cargo test -p rngkit-sources --test hardware -- --ignored --test-threads=1 --nocapture
```

Expected deterministic outcomes:

- no default test opens hardware;
- all crates build on stable and Rust 1.85;
- native and legacy normalized analysis agree for equivalent data;
- failure injection never creates a false committed sample or partial final XLSX;
- cumulative Z remains descriptive and XLSX contains only reference `+/-1.96`
  lines, not significance claims.

## 6. Risks and safeguards

### Source dependency reproducibility

The source crates do not currently have release tags. Pin Git revisions so
upstream branch movement cannot silently change the workspace. Upgrades require
an explicit revision change and adapter regression run. Git dependencies prevent
immediate crates.io publication of dependent packages; publication is out of
scope until dependency distribution is designed separately.

### MSRV drift in external dependencies

The latest serialization/XLSX dependency may raise its MSRV. Pin compatible
versions, keep `cargo +1.85.0` in local/CI validation, and do not weaken the
workspace MSRV silently.

### Cross-file durability

BIN and CSV cannot be atomically committed together. Preserve BIN-first/CSV-last
ordering, sync both at one-second cadence, treat CSV as the commit marker, inject
failures at every boundary, and never auto-truncate.

### Windows file replacement and locking

Windows replacement semantics differ from Unix and Excel may hold workbooks
open. Use same-directory temporary files and a tested platform-correct replace
operation; surface locked/existing targets as typed errors without deleting the
valid prior file.

### Blocking cancellation latency

The engine cannot preempt a source read safely. Document that cancellation waits
for the current source operation, commit a complete successful result, and rely
on the source crate's bounded I/O behavior.

### Statistical misinterpretation

Cumulative points are correlated and sessions stop manually. Keep signed Z as a
descriptive trajectory, include observed proportion/effect size, show `+/-1.96`
only as labeled references, omit p-values/pass-fail language, and defer formal
sequential inference.

### Device identity leakage

Selectors are required transiently for hardware choice but must not enter
persisted descriptors, manifests, exported sheets, test fixtures, logs committed
to the repository, or documentation.

### Excel scale

At one sample per second, very long sessions can exceed worksheet limits. Check
the row count before final output and fail explicitly; do not truncate or split
without a new approved design.
