# Mixed-source CSV concatenation implementation plan

**Date:** 2026-09-11
**Approved design:** `docs/specs/2026-09-11-mixed-source-concatenation-design.md`
**Library root:** `D:\Projetos\rustie\libs\rngkit-core`
**Application root:** `D:\Projetos\rustie\rngkit-tauri`

This plan is phased. An implementation request authorizes **Phase 1 only**.
Phase 2 starts only after Phase 1 user validation and a separate authorization
that includes an exact reachable core revision. Commit, push, core publication,
app pin, and release remain separately authorized and are not implied by either
phase.

## 1. Goal

Format-neutral Combine concatenates CSVs when `sample_bits` and `interval`
match exactly, including when source or fold differ. Heterogeneous output uses
filename token `mixed`, label `Mixed sources`, and schema 3. Homogeneous
schema-2 writing, schema-1 wrappers, and collected values stay unchanged.

## 2. Out of scope

- XOR, resampling, interpolation, BIN inputs, or already-derived `_concat_`
  CSVs as Combine inputs.
- Device crates, collection, discovery, and live source selection.
- Changes to `inspect_legacy_csvs` / `create_legacy_csv_concatenation`.
- Per-row source columns on the derived CSV or XLSX Samples sheet.
- Local-path `rngkit-core` dependencies.
- Commit, push, publication, pin, or release without later authorization.

## 3. Prerequisites

1. Treat the approved spec as the source of truth.
2. Both worktrees start clean on `main`. Preserve unrelated changes if any
   appear. Do not silently install toolchains.
3. Core MSRV `1.85.0` and locked `Cargo.lock` versions. App pin remains
   `4e0e43d360b91887fe457a5a984b14d225a82db9` until Phase 2 is authorized.
4. Default tests must not enumerate or open hardware.
5. Follow existing concatenation patterns in `rngkit-recording`; do not add
   mixed as a collectable `SourceId` in `rngkit-core`.

## 4. Phase 1 — Core contract, provenance, and XLSX

**Goal:** `inspect_csv_inputs` / `create_csv_concatenation` accept mixed source
or fold when bits and interval match; mixed bundles are schema 3 with explicit
identity and per-input provenance; homogeneous schema 2 and schema 1 stay
readable and writable as today; XLSX Summary does not label mixed data as a
device family.

**Depends on:** nothing.

### Step 1 — Mixed identity constants

**Modify**

- `crates/rngkit-recording/src/concatenation/mod.rs`
- `crates/rngkit-recording/src/concatenation/naming.rs` (docs only if needed)
- `crates/rngkit-recording/src/lib.rs`

**Actions**

1. Add recording-owned constants, not core constructors:
   - `MIXED_SOURCE_ID` = `"mixed"`
   - `MIXED_SOURCE_LABEL` = `"Mixed sources"`
   - `MIXED_CSV_CONCATENATION_SCHEMA_VERSION` = `3`
2. Keep `CSV_CONCATENATION_SCHEMA_VERSION` = `2` and
   `CSV_CONCATENATION_KIND` = `"csv_concatenation"`.
3. Re-export the new constants from `rngkit-recording`.
4. Document that `mixed` is a derived-output token only. Collection and
   `validate_source` continue to accept only `bitb` / `trng` / `rdseed` /
   `pseudo` as appropriate for the input format.

**Acceptance criteria**

- `SourceId::new("mixed")` remains valid via the existing grammar.
- No mixed variant is added to discovery or collection constructors.

### Step 2 — Relax format-neutral compatibility and record per-input provenance

**Modify**

- `crates/rngkit-recording/src/concatenation/inspect.rs`
- `crates/rngkit-recording/src/concatenation/manifest.rs`

**Actions**

1. Store each input's `source_id` and `fold` from `SessionStem` on
   `InspectedFile`.
2. `check_compat` on the format-neutral path (`allow_current == true`) compares
   only `sample_bits` and `interval`. Source and fold are not compatibility
   fields there.
3. The legacy path (`allow_current == false`) still requires matching source,
   bits, interval, and fold.
4. After ordering, set preview identity:
   - Homogeneous: first input's source and fold.
   - Heterogeneous (any source **or** fold difference, including `bitb` `f0` vs
     `f1`): `SourceId` `mixed`, `fold = None`.
5. Format-neutral `ConcatenationInputEntry` values carry that input's
   `source_id` and `fold` in memory. Add optional serde fields with
   `skip_serializing_if = "Option::is_none"` plus a small
   `with_provenance(source_id, fold)` helper rather than expanding
   `new_with_format` further.
6. Update `ConcatenationPreview` docs: top-level source/fold are the output
   identity, not necessarily each row's hardware.
7. Do not invent serials, variants, device paths, or missing folds.

**Acceptance criteria**

- Mixed source or mixed fold inspect succeeds when bits and interval match and
  ranges are disjoint.
- `s2048_i2` with `s1024_i1` fails `SampleBits` even though bits/second match;
  preserve the existing bits-before-interval check order. Equal bits with
  unequal intervals fail `Interval`.
- Unequal bits still fail `SampleBits`.
- Duplicate canonical paths, overlap including equal boundaries, empty files,
  decreasing timestamps, BIN, and overflow still fail.
- `inspect_legacy_csvs` still rejects differing source and fold.

### Step 3 — Schema 3 manifests

**Modify**

- `crates/rngkit-recording/src/concatenation/manifest.rs`

**Actions**

1. Add `ConcatenationManifest::new_mixed_csv` (name may vary) that writes
   schema 3, kind `csv_concatenation`, top-level `source_id` `mixed`, omitted
   fold, and required per-input `format`, `source_id`, and fold consistent with
   `validate_fold_rules` for that input source (`bitb` requires `_f0`–`_f4`;
   other families omit fold).
2. Accept schema 1, 2, and 3 in `from_slice` / `validate_contents`.
3. Locked schema rules:
   - Schema 3 if and only if top-level source is `mixed` and fold is absent.
   - Schema 2 remains homogeneous: require per-input CSV format; reject
     per-input `source_id`/`fold`; reject top-level `mixed`.
   - Schema 1 still rejects format and per-input source/fold.
   - Schema 4+ remains `UnsupportedSchema`.
   - Kind for schema 3 stays `csv_concatenation`. Wrong kind still fails as
     today.
4. Keep existing row-count, hash, timestamp, and output-range checks.
5. Parse each schema-3 input basename as a session CSV stem. Require source
   and fold to match entry provenance, and bits and interval to match output
   metadata. Validate the source against the declared CSV format's supported
   sources; reject `mixed` as an input source. Reject schema 3 unless at least
   two input identities differ in source or fold.

**Acceptance criteria**

- A mixed document cannot masquerade as schema 2.
- A homogeneous schema-2 fixture still loads.
- Schema 1 fixtures still load and still reject unexpected fields.

### Step 4 — Writer, reader, and naming

**Modify**

- `crates/rngkit-recording/src/concatenation/writer.rs`
- `crates/rngkit-recording/src/concatenation/reader.rs`
- `crates/rngkit-recording/src/concatenation/naming.rs` (comments only unless
  a constructor helper is useful)

**Actions**

1. `create_csv_concatenation` still streams through `for_each_csv_row`. Copy
   ones and timestamps in chronological order. Do not XOR, resample, or rewrite
   values. Derived columns stay
   `sample_index,captured_at_utc,ones,input_index,input_sample_index`.
2. Stem construction uses preview identity. Mixed stems are
   `YYYYMMDDTHHMMSS_concat_mixed_s<bits>_i<seconds>` with no `_f*`. Existing
   `SessionStem` fold rules already forbid a fold suffix on non-`bitb`.
3. `write_manifest`:
   - mixed → schema-3 constructor, keep per-input provenance;
   - homogeneous with format → `new_csv` after stripping per-input
     source/fold so schema-2 JSON stays unchanged;
   - legacy wrapper → schema 1, no format, no per-input source/fold.
4. `open_concatenation` accepts schema 1–3. For schema 3,
   `NormalizedMeta.source_id` is `mixed`, `source_label` is `Mixed sources`,
   `fold` is `None`. Map `mixed` in `concatenation_source_label`.
5. Update writer docs that currently claim every format-neutral bundle is
   schema 2.

**Acceptance criteria**

- Mixed create writes `_concat_mixed_` and schema 3.
- Homogeneous create still writes schema 2 with the original source token and
  BitBabbler `_f<fold>` when applicable.
- Output ones/timestamps equal the corresponding input values in order.
- Opening schema 3 yields label `Mixed sources`. Opening schema 1/2 is
  unchanged.

### Step 5 — XLSX Summary for mixed sessions

**Modify**

- `crates/rngkit-xlsx/src/report.rs`

**Actions**

1. When `meta.source_id` is `mixed`, Summary `Source` writes
   `meta.source_label` (`Mixed sources`) and `Fold` stays empty.
2. Homogeneous Summary `Source` continues to write the source token
   (`bitb`, `trng`, `rdseed`, `pseudo`), not the friendly label.
3. Do not add per-row source columns. Chart title remains the selected
   basename.

**Acceptance criteria**

- Mixed workbook Source is `Mixed sources`; Fold is empty.
- Homogeneous workbook Source remains the device token.

### Step 6 — Library tests

**Modify**

- `crates/rngkit-recording/tests/concatenation_current.rs`
- `crates/rngkit-recording/tests/concatenation_inspection.rs` (legacy
  regressions only if a fixture needs schema-4 coverage)
- `crates/rngkit-xlsx/tests/report.rs`

Add focused tests covering:

1. Mixed-source inspect/create success; stem contains `_concat_mixed_`; ones
   and timestamps match inputs in order.
2. Differing BitBabbler folds succeed; per-input folds preserved; top-level
   identity is mixed with no fold suffix.
3. Unequal bits rejected; unequal interval rejected, including
   `s2048_i2` vs `s1024_i1`.
4. Duplicate and overlap rejection unchanged on the format-neutral path.
5. Schema-3 manifest round-trip; `open_concatenation` label is
   `Mixed sources`; no absolute paths in JSON.
6. Schema-1 and homogeneous schema-2 fixtures still open; homogeneous create
   still writes schema 2 with the original source token and without per-input
   source/fold fields.
7. Schema 2 with per-input source/fold or top-level `mixed` is rejected.
   Schema 3 that is not mixed is rejected. Schema 4 is `UnsupportedSchema`.
8. `inspect_legacy_csvs` still rejects differing source and fold.
9. XLSX mixed vs homogeneous Summary Source/Fold.
10. Reject schema-3 source/fold provenance mismatches against basenames,
    basename bits/interval mismatches against output metadata, unsupported
    source/format combinations (including `mixed` inputs), and homogeneous
    input identities labeled mixed. Test interval-only mismatches separately
    from the equal-throughput pair, which must fail `SampleBits`.

Keep existing homogeneous mixed-format (current+legacy, same source) tests.

### Step 7 — Core project memory

**Modify**

- `docs/DECISIONS.md` (amend Standalone and format-neutral CSV inputs; keep
  schema-1 matching-source/fold rules)
- `docs/PROJECT_CONTEXT.md` (current concatenation compatibility and schema 3)
- `TODO.md` (Phase 1 in progress / next: authorized pin)

Do not add mixed to collectable source lists. Do not commit.

### Phase 1 automated verification

From `D:\Projetos\rustie\libs\rngkit-core`:

```text
cargo test -p rngkit-recording --locked --test concatenation_inspection
cargo test -p rngkit-recording --locked --test concatenation_current
cargo test -p rngkit-recording --locked --test concatenation_roundtrip
cargo test -p rngkit-xlsx --locked --test report
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo test --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --doc --all-features --locked
cargo +1.85.0 check --workspace --all-targets --all-features --locked
cargo +1.85.0 test --workspace --all-targets --all-features --locked
git diff --check
git status --short --branch
```

Hardware tests stay ignored.

### Phase 1 user testing

1. Inspect a test-created mixed bundle: directory name contains
   `_concat_mixed_`, `manifest.json` is schema 3 with per-input source/fold,
   derived CSV ones/timestamps match the inputs.
2. Inspect a homogeneous bundle from the same code: schema 2, original source
   token, no per-input source/fold.
3. Optional: open the mixed XLSX Summary and confirm Source is
   `Mixed sources` and Fold is empty.

The desktop app still pins the old revision until Phase 2.

### Phase 1 completion condition

Locked stable and MSRV suites pass. Spec behavior above is covered by tests.
No app pin, no local-path dependency, no commit unless separately authorized.

**Stop.** Report what changed, which checks passed, and how to inspect a mixed
bundle. Do not start Phase 2.

## 5. Phase 2 — App Combine, Help, pin, and no-bundle validation

**Goal:** After an authorized exact core revision is reachable, the app previews
per-input source/fold, allows mixed sets, never labels mixed data as the first
input's hardware, and Help/incompatibility copy match the bits/interval rule.

**Depends on:** Phase 1 complete, user validation opportunity, explicit Phase 2
authorization, and an exact reachable Git revision of `rngkit-core` (never a
path dependency). Current pin is
`4e0e43d360b91887fe457a5a984b14d225a82db9`.

### Step 8 — Pin the authorized core revision

**Modify**

- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- revision labels in Help / `RNGKIT_CORE_REVISION` as the project already
  surfaces them

**Actions**

1. Pin the authorized exact revision of every `rngkit-*` git dependency.
2. Run `cargo metadata` / lock update only for that pin.
3. Refuse a `path =` dependency.

**Acceptance criteria**

- Lockfile points at the authorized revision only.

### Step 9 — Combine preview, copy, and Help

**Modify**

- `src-tauri/src/combine/mod.rs`
- `src/pages/HelpPage.svelte`
- `src/pages/HelpPage.test.ts`
- `src/pages/CombinePage.test.ts` and/or `src/state/mock-scenarios.ts` if a
  mixed-compatible fixture is the smallest way to lock per-row source labels
- `src-tauri/tests/combine.rs`

**Actions**

1. `row_from_preview` uses each entry's `source_id` and `fold`, not
   `preview.source_id()` / `preview.fold()` for every row. Map `mixed` to
   `Mixed sources` only for output-level identity; input rows use that file's
   friendly label (`BitBabbler`, `TrueRNG v1/v2/v3`, `RDSEED`, `PseudoRNG`).
2. Compatible mixed selections set `compatible: true`.
3. `IncompatibleConcatenationInputs` copy for format-neutral Combine:
   - `SampleBits`: `Sample size must match exactly.`
   - `Interval`: `Sampling interval must match exactly. Matching average bits per second is not enough.`
   - Do not mention source or fold as compatibility requirements.
4. Reports already display `meta.source_label` and `meta.fold` from
   `open_concatenation`; after the pin, mixed derived preview Source is
   `Mixed sources` and Fold is empty. Add a Combine/Reports test that a mixed
   bundle is not labeled with the first input's hardware name.
5. Help Combining files: bits and interval must match exactly; sources and
   folds may differ; mixed output is labeled `Mixed sources`; files are not
   XOR'd or resampled; time ranges still cannot overlap; BIN still rejected.
6. Help “Combine says the files are incompatible”: same bits/interval rule;
   overlap still rejected.
7. File formats: homogeneous Combine output remains schema 2;
   mixed output uses schema 3 `csv_concatenation`; schema 1 remains readable.
8. Update Help tests that currently require “matching source” / only schema 2.

**Acceptance criteria**

- Combine rows for a mixed set show distinct sources/folds and allow create.
- Derived Reports preview and XLSX Summary do not use the first input's
  hardware name.
- Help no longer requires matching source and fold.

### Step 10 — App project memory

**Modify**

- `docs/DECISIONS.md`
- `docs/PROJECT_CONTEXT.md`
- `TODO.md`
- `AGENTS.md` only for the pinned revision label after the pin is real

Record that Combine compatibility is exact bits and interval; mixed output is
`Mixed sources` / schema 3; the app pin is the authorized revision.

### Phase 2 automated verification

From `D:\Projetos\rustie\rngkit-tauri`:

```text
cargo test --locked --manifest-path src-tauri/Cargo.toml --test combine
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --locked --manifest-path src-tauri/Cargo.toml --all-targets
cargo test --locked --manifest-path src-tauri/Cargo.toml --all-targets
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml --doc
cargo +1.85.0 check --locked --manifest-path src-tauri/Cargo.toml --all-targets
cargo +1.85.0 test --locked --manifest-path src-tauri/Cargo.toml --all-targets
npm run format:check
npm run check
npm run lint
npm run test:unit -- --run
npm run tauri -- build --no-bundle -- --locked
git diff --check
git status --short --branch
```

Playwright e2e is optional unless Combine/Help coverage lives there. Hardware
tests stay ignored.

### Phase 2 user testing

Using the no-bundle native build:

1. Combine two disjoint CSVs from different sources with the same bits and
   interval: preview shows each source; create succeeds; folder name contains
   `_concat_mixed_`.
2. Combine two BitBabbler files with different folds and matching bits/interval:
   success, mixed identity, per-row folds preserved.
3. Reject `s2048_i2` with `s1024_i1` with a sample-size mismatch (equal
   throughput, both fields differ). Also reject equal bits with different
   intervals with an interval mismatch.
4. Reject overlapping ranges and duplicate files as today.
5. Open the mixed bundle in Reports and generate XLSX: Source is
   `Mixed sources`, Fold empty, chart title uses the mixed basename.
6. Homogeneous Combine still uses the original source token and schema 2.
7. Read Help Combining files and the incompatibility topic.

### Phase 2 completion condition

Pinned revision is exact and reachable. Focused Combine/Help tests and the
locked checks above pass. No-bundle build exists for the user. No commit,
push, or release unless separately authorized.

**Stop.**

## 6. Test plan (both phases)

| Requirement | Where |
| --- | --- |
| Mixed source success, values preserved | `concatenation_current.rs` |
| Differing folds, per-input provenance | `concatenation_current.rs` |
| Unequal bits; unequal interval including equal throughput | `concatenation_current.rs` |
| Duplicate/overlap unchanged | `concatenation_current.rs` / existing overlap tests |
| Schema 3 round-trip; label `Mixed sources` | `concatenation_current.rs` |
| Schema 1/2 still open; homogeneous still schema 2 | `concatenation_inspection.rs`, `concatenation_current.rs` |
| Schema 2 cannot carry mixed provenance | `concatenation_current.rs` or inspection JSON tests |
| Legacy wrappers still strict | `concatenation_inspection.rs` |
| XLSX mixed vs homogeneous Summary | `rngkit-xlsx/tests/report.rs` |
| Combine per-row source/fold; mixed allowed | `src-tauri/tests/combine.rs` |
| Bits/interval copy; Help | Combine tests, `HelpPage.test.ts` |
| MSRV and lockfile | Phase 1/2 command lists |

## 7. Risks and safeguards

- **Old app opens mixed bundles.** Schema 3 fails closed with
  `UnsupportedSchema`. Do not write mixed as schema 2.
- **Preview copies the first source onto every row.** Phase 2 must read entry
  provenance. Phase 1 must populate those fields in memory even for
  homogeneous inspect.
- **Homogeneous JSON drift.** Strip per-input source/fold before schema-2
  serialize; add a test that those keys are absent.
- **`mixed` mistaken for a device.** Keep the token in `rngkit-recording` only.
  XLSX writes the friendly label, not the token.
- **Path dependency.** Phase 2 is blocked until a revision is reachable.
- **Unrelated worktrees.** Inspect `git status` before editing; do not revert
  unrelated files.

## 8. Phase gates

| Phase | Authorization required to start | Stop after |
| --- | --- | --- |
| 1. Core + XLSX | Explicit implementation request | User can inspect mixed/homogeneous test bundles; locked core suite green |
| 2. App pin + Combine/Help | Phase 1 tested; explicit continue; authorized exact core revision | No-bundle build ready; user can run Combine/Reports/Help |

Commit, push, publication, pin (as a git action), and release each need their
own later authorization. Previous approvals for unrelated fixes do not apply.
