# Unified Source Discovery Implementation Plan

**Date:** 2026-08-21
**Approved design:** `docs/specs/2026-08-21-source-discovery-design.md`
**Workspace root:** `D:\Projetos\rustie\libs\rngkit-core`

## 1. Goal

Add a unified, best-effort, feature-gated discovery API to `rngkit-sources` so
Tauri and other consumers can list currently selectable source candidates
without duplicating adapter-specific policy. Completion requires deterministic
tests with no hardware access, an ignored physical smoke test, full stable/MSRV
validation, and passing Windows/Linux CI.

## 2. Out of scope

- Tauri commands, DTOs, serialization, events, frontend code, or application
  state.
- Background workers, async runtimes, hot-plug watching, caching, reconnect, or
  polling.
- Changes to `SourceConfig`, `open()`, source adapters, collection, recording,
  analysis, XLSX, or persisted formats.
- TrueRNGpro, implicit first-device selection, or persistence of selectors.
- Commit, push, release, publication, or Tauri project creation without separate
  authorization.

## 3. Prerequisites

1. Preserve the approved six-crate dependency graph and Rust 1.85 MSRV.
2. Keep all four existing source revisions unchanged.
3. Treat `docs/specs/2026-08-21-source-discovery-design.md` as the source of
   truth.
4. Default tests must not enumerate or open hardware.

## 4. Ordered implementation steps

### Step 1 — Define the discovery domain API

**Create or modify**

- `crates/rngkit-sources/src/discovery.rs`
- `crates/rngkit-sources/src/lib.rs`

**Actions**

1. Add a non-exhaustive, feature-gated `SourceCandidate` enum:
   - `Bitb { variant: String, serial: String }`;
   - `Trng { port_name: String }`;
   - `Rdseed`;
   - `Pseudo`.
2. Add candidate methods returning the stable typed `SourceId` and a safe
   static display label. Do not add `serde` derives or persisted conversion.
3. Add `DiscoveryIssue` containing the affected typed `SourceId` and owned
   normalized `SourceError`, with read-only accessors.
4. Add `DiscoveryReport` containing public/read-only candidate and issue
   collections plus convenience accessors as needed. Keep the type independent
   of Tauri and serialization.
5. Re-export `discover`, `DiscoveryReport`, `DiscoveryIssue`, and
   `SourceCandidate` from `rngkit-sources`.

**Acceptance criteria**

- The public API can represent every enabled source and multiple hardware
  devices independently.
- Candidate identity is typed; callers do not need arbitrary string matching.
- Selectors exist only on transient hardware candidates.
- No dependency or persisted domain type changes.

### Step 2 — Implement independent best-effort discovery

**Create or modify**

- `crates/rngkit-sources/src/discovery.rs`

**Actions**

1. Implement `pub fn discover() -> DiscoveryReport` in stable family order:
   BitBabbler, TrueRNG, RDSEED, PseudoRNG.
2. Call `BitbAdapter::list()` and create one candidate per returned device.
3. Call `Trng3Adapter::list()` and create one candidate per returned port.
4. Include RDSEED only when `RdseedAdapter::is_supported()` is true.
5. Probe PseudoRNG with `PseudoAdapter::open(None)`, immediately drop the
   instance on success, and add one candidate without exposing state.
6. Treat empty hardware lists, hardware `NotAvailable`, unsupported RDSEED, and
   compile-time-disabled features as normal absence.
7. Convert every other family-specific failure into one `DiscoveryIssue` and
   continue discovering later families.
8. Do not call BitBabbler/TrueRNG `open`, read entropy, or implicitly choose one
   device.

**Acceptance criteria**

- Failure in one source family does not suppress candidates from another.
- Only present/usable candidates are returned.
- Hardware enumeration preserves underlying per-family ordering.
- The function retains no global state and each call is a fresh snapshot.

### Step 3 — Add deterministic discovery seams and tests

**Create or modify**

- `crates/rngkit-sources/src/discovery.rs`
- `crates/rngkit-sources/tests/adapters.rs` only if a public-consumer contract
  test adds value beyond module tests

**Actions**

1. Introduce a private backend/closure seam used by `discover()` so unit tests
   can supply hardware listings, capability results, and PseudoRNG probe results
   without touching real devices or OS entropy.
2. Test:
   - all families absent;
   - multiple BitBabbler and TrueRNG candidates;
   - stable family order;
   - empty lists and `NotAvailable` as absence;
   - partial success when each hardware family fails independently;
   - unsupported RDSEED omission;
   - successful and failed PseudoRNG probes;
   - correct typed source IDs and safe labels;
   - candidate matching into explicit `SourceConfig` values, including a chosen
     BitBabbler fold and exact hardware selector.
3. Ensure these deterministic tests call only the private fake backend.

**Acceptance criteria**

- Default tests prove all discovery policy without enumerating/opening physical
  hardware.
- Tests distinguish normal absence from reportable failure.
- Multiple-device results never collapse into an implicit default selection.

### Step 4 — Add an ignored physical discovery smoke test

**Create or modify**

- `crates/rngkit-sources/tests/hardware.rs`

**Actions**

1. Add one `#[ignore]` test that calls the public `discover()` API.
2. Allow normal device absence through the production discovery semantics.
3. Fail on any returned `DiscoveryIssue`; do not skip permission, busy,
   protocol, timeout, USB, serial, or OS-entropy errors.
4. Do not print or snapshot real serials, port paths, seeds, or state.
5. Keep execution compatible with the repository's serial ignored-test command.

**Acceptance criteria**

- Default `cargo test` does not execute real discovery.
- The explicit ignored test validates the public physical path without leaking
  selectors.

### Step 5 — Document the reusable contract and current evidence

**Create or modify**

- `crates/rngkit-sources/README.md`
- `README.md`
- `docs/PROJECT_CONTEXT.md`
- `docs/DECISIONS.md`
- `TODO.md`

**Actions**

1. Document the snapshot/best-effort semantics, candidate/issue split, normal
   absence behavior, and Tauri DTO boundary.
2. Record the additive discovery contract as a durable decision.
3. Update validation claims only after the corresponding local, physical, or CI
   commands actually pass.
4. Mark Linux CI verified using the already successful run for commit
   `371f287`, while keeping new discovery CI evidence separate until its own run
   passes.
5. Keep the Tauri project as the next phase; do not scaffold it here.

**Acceptance criteria**

- Documentation clearly distinguishes deterministic, physical, and CI evidence.
- No documentation claims that discovery reserves a device or continuously
  monitors hot-plug state.
- No real selector appears in committed documentation or fixtures.

### Step 6 — Validate the full workspace

**Modify**

- No additional source files unless validation reveals a defect in the approved
  scope.

**Actions**

Run from the workspace root:

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
cargo check -p rngkit-sources --no-default-features --features bitb
cargo check -p rngkit-sources --no-default-features --features trng3
cargo check -p rngkit-sources --no-default-features --features rdseed
cargo check -p rngkit-sources --no-default-features --features pseudo
git diff --check
```

When the corresponding hardware is available, run explicitly and serially:

```text
cargo test -p rngkit-sources --test hardware -- --ignored --test-threads=1 --nocapture
```

After separately authorized commit/push, require the GitHub Actions Windows and
Ubuntu stable/MSRV jobs to pass before recording cross-platform completion.

**Acceptance criteria**

- Stable and Rust 1.85 deterministic suites pass.
- Every isolated source feature builds without undeclared coupling.
- Default tests access no hardware.
- Ignored physical evidence and remote CI evidence are reported separately.
- The final diff contains only approved discovery and context changes.

## 5. Risks and safeguards

### Hardware access in deterministic tests

Keep production enumeration behind a private backend seam and ensure all normal
tests use fakes. Only the explicitly ignored smoke test calls `discover()`.

### Selector leakage

Keep selectors only in candidate variants, never derive serialization in the
library, and avoid debug-printing physical results in tests or documentation.

### Partial failure ambiguity

Normalize only `NotAvailable` to absence. Preserve every other normalized error
as an issue so permission and transport failures are not silently hidden.

### Discovery/open race

Document that discovery is a snapshot and `open()` remains authoritative if a
device disappears or becomes busy after listing.

### Feature-gated API drift

Compile each source feature independently and keep all candidate variants and
backend operations behind the matching feature flags.
