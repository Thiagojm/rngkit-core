# Unified Source Discovery Design

**Status:** Approved
**Date:** 2026-08-21
**Workspace root:** `D:\Projetos\rustie\libs\rngkit-core`

## 1. Context and problem

`rngkit-sources` already provides typed adapters and a unified `open(SourceConfig)`
factory, but discovery is adapter-specific. A consumer currently has to call
`BitbAdapter::list()`, `Trng3Adapter::list()`, `RdseedAdapter::is_supported()`,
and the PseudoRNG constructor separately. Repeating that policy in a Tauri app
would couple the app to adapter details and duplicate it in future CLI or GUI
consumers.

## 2. Goals

1. Add one Tauri-independent discovery entry point to `rngkit-sources`.
2. Return only source candidates that are currently usable or selectable.
3. Treat absent devices and unsupported local sources as normal absence.
4. Preserve useful failures as per-source issues without hiding candidates from
   other source families.
5. Keep hardware selectors transient and strongly typed.
6. Preserve the current feature-gated source composition and MSRV 1.85.

## 3. Non-goals

- Adding Tauri DTOs, serialization, commands, events, threads, or application
  state.
- Opening BitBabbler or TrueRNG devices during discovery.
- Selecting the first hardware device implicitly.
- Persisting serial numbers, port names, device paths, seeds, or generator
  state.
- Changing `SourceConfig`, adapter opening behavior, the collection engine, or
  session formats.
- Polling continuously, watching hot-plug events, caching results, or performing
  automatic reconnect.
- Adding TrueRNGpro support.

## 4. Chosen approach

Add `crates/rngkit-sources/src/discovery.rs` and re-export its public API from
the crate root.

The public entry point is conceptually:

```text
pub fn discover() -> DiscoveryReport
```

Discovery is best-effort and therefore does not return a top-level `Result`.
`DiscoveryReport` contains:

- `candidates: Vec<SourceCandidate>` for sources that can be selected;
- `issues: Vec<DiscoveryIssue>` for real failures scoped to one source family.

`SourceCandidate` is a non-exhaustive enum with feature-gated variants:

- `Bitb { variant, serial }` for every recognized BitBabbler;
- `Trng { port_name }` for every recognized TrueRNG v1/v2/v3;
- `Rdseed` when the runtime capability check succeeds;
- `Pseudo` when OS-seeded PseudoRNG construction succeeds.

The hardware selector fields are intentionally present only in the in-memory
candidate so a caller can later construct an explicit `SourceConfig`. They are
not added to `SourceDescriptor`, manifests, recording files, reports, or
documentation fixtures. The candidate API exposes stable source identity and a
safe display label without requiring callers to match arbitrary strings.

`DiscoveryIssue` owns the stable source identity and normalized `SourceError`.
The error kind is suitable for application mapping; its diagnostic remains
diagnostic-only and is not a stable IPC contract. `DiscoveryReport`, candidates,
and issues do not derive `serde` traits. A Tauri adapter must map them to its own
DTOs and decide what diagnostic text is safe to expose.

## 5. Discovery behavior

Discovery evaluates enabled source families independently in this stable family
order: BitBabbler, TrueRNG, RDSEED, PseudoRNG. Hardware candidates preserve the
order returned by their source crate.

- An empty hardware list produces no candidate and no issue.
- `SourceErrorKind::NotAvailable` during hardware enumeration is normalized to
  absence and produces no issue.
- Any other hardware enumeration error produces one issue for that family and
  does not stop discovery.
- RDSEED is included only when `RdseedAdapter::is_supported()` is true; false is
  normal absence.
- PseudoRNG is probed by constructing and immediately dropping a default
  `PseudoAdapter`. Success adds one candidate. Failure adds one issue; no seed or
  generator state is exposed.
- A source family disabled at compile time produces neither a candidate nor an
  issue.

Discovery never calls `open()` for BitBabbler or TrueRNG, never reads entropy,
and never resolves multiple hardware devices to an implicit first choice.

## 6. Consumer flow

1. The application calls `rngkit_sources::discover()` on a blocking/background
   context because device enumeration may perform operating-system I/O.
2. It renders `report.candidates` and may surface `report.issues` as warnings.
3. The user selects one candidate and any source-specific collection options,
   such as BitBabbler fold.
4. The application constructs `SourceConfig` with the candidate's explicit
   transient selector and calls the existing `open()` factory.

Discovery results are snapshots. Consumers call `discover()` again to refresh;
the library does not retain state or monitor hot-plug events.

## 7. Failure and edge cases

- One failing source family cannot suppress candidates from another family.
- Device disappearance between discovery and `open()` is reported by the
  existing typed open error; discovery does not promise reservation.
- Multiple physical devices are returned as separate candidates.
- PseudoRNG probe success does not guarantee that a later independent OS seed
  cannot fail; the later `open()` result remains authoritative.
- Diagnostics are never persisted automatically and are not treated as stable
  user-facing text.

## 8. Compatibility and security

This is an additive public API. Existing adapters, feature flags, and `open()`
remain source-compatible. No new runtime or serialization dependency is added.

Transient serials and port names are necessary for explicit selection but must
not cross into persisted source descriptors or session/report artifacts. Tests
must not print real selectors into committed fixtures.

## 9. Validation strategy

- Use a private injectable discovery seam so deterministic tests never enumerate
  or open physical hardware.
- Test empty hardware lists, multiple candidates, stable family ordering,
  `NotAvailable` normalization, partial success with one failing family,
  unsupported RDSEED, and failed/successful PseudoRNG probes.
- Verify each candidate can be mapped by a test consumer to the corresponding
  explicit `SourceConfig` without implicit first-device selection.
- Keep any real discovery smoke test behind `#[ignore]` and run it serially.
- Run the full workspace stable/MSRV, Clippy, doctest, feature-subset, Windows,
  and Linux CI validation already required by the repository.

## 10. Acceptance criteria

1. A single `rngkit_sources::discover()` call returns all available candidates
   from enabled source families.
2. Missing hardware and unsupported RDSEED are omitted without errors.
3. A real error in one family appears in `issues` while other families remain
   discoverable.
4. Multiple BitBabbler or TrueRNG devices remain separate candidates with
   explicit transient selectors.
5. Default deterministic tests perform no hardware enumeration or opening.
6. No selector or PseudoRNG state is added to any persisted domain type or
   artifact.
7. No Tauri, Tokio, GUI, async runtime, or serialization dependency is added.
8. Stable, Rust 1.85, Windows, Linux, and isolated source feature builds pass.

## 11. Alternatives considered

### Fail the whole discovery on the first error

Rejected because one USB or serial failure would hide unrelated sources that
remain usable.

### Return disabled entries for every known source

Rejected by the selected behavior: discovery lists present/usable candidates.
Real failures remain available separately as issues rather than being confused
with normal absence.

### Implement discovery only in the Tauri project

Rejected because the same adapter-specific policy would be duplicated by other
frontends and command-line tools.

### Create a new discovery crate

Rejected because discovery depends directly on the adapters and belongs within
the existing `rngkit-sources` responsibility.

## 12. Decisions and assumptions

- The user selected tolerant partial discovery.
- Only currently present/usable candidates are listed.
- Normal absence is not an error or disabled candidate.
- Real per-family failures are retained as non-blocking issues.
- Discovery is an explicit snapshot operation, not a watcher.
