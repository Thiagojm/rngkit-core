# Mixed-source CSV concatenation

**Status:** Approved

**Date:** 2026-09-11

**Library root:** `D:\Projetos\rustie\libs\rngkit-core`

**Application root:** `D:\Projetos\rustie\rngkit-tauri`

**Supersedes:** The matching-source and matching-fold concatenation rules in
`docs/DECISIONS.md` (Derived legacy CSV concatenation; Standalone and
format-neutral CSV inputs) and the Combine Help/copy that currently require
matching source and fold. Unaffected contracts remain in force: CSV-only
Combine, chronological no-overlap ordering, schema-1 readability, homogeneous
schema-2 output, no XOR, no BIN inputs, and no absolute input paths.

## 1. Context and problem

Combine currently concatenates current, legacy, or mixed-format CSVs only when
every input shares source, sample bits, interval, and fold. That check lives in
`crates/rngkit-recording/src/concatenation/inspect.rs` (`check_compat`) and is reused
by preview, writer, reader, manifests, and XLSX.

Users need to concatenate files from different RNG sources when the recorded
sample size and sampling interval are identical. Equal average throughput is
not enough: `2048` bits every `2` s is not compatible with `1024` bits every
`1` s. Concatenation must copy collected one-counts and timestamps in order. It
must not XOR, resample, interpolate, or rewrite values.

Today a mixed-source or mixed-fold set fails with
`IncompatibleConcatenationInputs`. Preview rows also copy the shared preview
source onto every input, so a mixed output would be mislabeled as the first
file's hardware if those checks were removed without a new identity.

Confirmed current behavior (both worktrees clean on `main`; core HEAD
`4e0e43d360b91887fe457a5a984b14d225a82db9`; app HEAD `33cc82c` pins that
revision):

- Format-neutral APIs: `inspect_csv_inputs` / `create_csv_concatenation`.
- Legacy-only wrappers still require matching source, bits, interval, and fold
  and write schema 1.
- Derived names:
  `YYYYMMDDTHHMMSS_concat_<source>_s<bits>_i<seconds>[_f<fold>]`.
- Schema 2 stores one top-level `source_id` and `fold`; input entries store
  format, not per-input source or fold.
- Normalized readers and XLSX Summary `Source` use that single identity.
- Combine Help states that source, sample size, interval, and fold must match.

## 2. Goals and non-goals

### Goals

1. Allow format-neutral Combine to concatenate CSVs whose sample bits and
   interval match exactly, even when source or fold differ.
2. Preserve each input's source and fold from filename metadata already used
   by `SessionStem`. Do not invent serials, variants, device paths, or missing
   folds.
3. Give heterogeneous output an explicit identity: filename token `mixed`,
   user-facing label `Mixed sources`, matching manifest and normalized-reader
   fields.
4. Keep homogeneous concatenations byte-compatible in contract: schema 2,
   existing stem grammar, existing source/fold identity.
5. Keep schema-1 bundles readable. Reject unknown newer schemas as today.
6. Update Combine preview, incompatibility copy, derived summaries, XLSX
   source/fold presentation, and Help so mixed data is never labeled as the
   first input's hardware.
7. Leave device crates, collection, and live source selection unchanged.

### Non-goals

- XOR, live multi-source collection, reconnect, resume, or resampling to a
  common rate.
- Combining BIN files, raw entropy bytes, or already-derived `_concat_` CSVs.
- Changing `inspect_legacy_csvs` / `create_legacy_csv_concatenation` (schema-1
  wrappers stay strict).
- Migrating or rewriting existing schema-1 or schema-2 bundles.
- Adding per-row source columns to the derived CSV or XLSX Samples sheet.
- Pinning, committing, pushing, or releasing without separate authorization.
- Local-path `rngkit-core` dependencies.

## 3. Requirements and acceptance criteria

1. **Exact bits and interval.** Two inputs are compatible only when
   `sample_bits` and `interval` are equal. A matching bits-per-second ratio
   with different bits or interval is rejected as `SampleBits` or `Interval`
   incompatibility.
2. **Relaxed source and fold (format-neutral only).** `inspect_csv_inputs` and
   `create_csv_concatenation` accept differing `source_id` and `fold` values.
   Duplicate canonical paths, overlap (including equal boundaries), empty
   files, decreasing timestamps, BIN, corrupt rows, and one-count overflow
   still fail.
3. **Heterogeneous identity.** If any input source or fold differs from the
   first, the output stem is
   `YYYYMMDDTHHMMSS_concat_mixed_s<bits>_i<seconds>` with no fold suffix.
   Display label is exactly `Mixed sources`. Top-level manifest `source_id` is
   `mixed` and `fold` is omitted.
4. **Homogeneous identity.** If every input shares source and fold, behavior
   stays as today: schema 2, original source token, BitBabbler `_f<fold>` when
   applicable, friendly source labels unchanged.
5. **Per-input provenance.** Each mixed input entry stores `source_id` and
   `fold` parsed from that file's stem. BitBabbler entries keep `_f0`–`_f4`;
   other families omit fold. Missing device details are not synthesized.
6. **Value preservation.** Derived CSV columns stay
   `sample_index,captured_at_utc,ones,input_index,input_sample_index`. Output
   `ones` and timestamps equal the corresponding input values in chronological
   order.
7. **Schema.** Mixed bundles use schema version 3 and kind
   `csv_concatenation`. Schema 1 and schema 2 remain readable. New homogeneous
   bundles remain schema 2. Readers that do not implement schema 3 keep failing
   with `UnsupportedSchema`.
8. **Preview and UI.** Combine rows show each input's own source and fold.
   Compatible mixed selections are allowed. Derived Reports preview source is
   `Mixed sources` and fold is empty. Incompatibility copy names bits and
   interval, not source/fold, for format-neutral Combine.
9. **XLSX.** For mixed sessions, Summary `Source` is `Mixed sources` and
   `Fold` is empty. Chart title continues to use the selected basename, which
   contains `_concat_mixed_`. Homogeneous workbooks are unchanged.
10. **Help.** Combining files and the Combine incompatibility topic state that
    bits and interval must match exactly, sources and folds may differ, mixed
    output is labeled `Mixed sources`, and files are not XOR'd or resampled.

## 4. Chosen approach

Keep one format-neutral concatenation pipeline. Relax only source and fold
compatibility there. Introduce a dedicated mixed identity and schema 3 for
heterogeneous output. Leave homogeneous schema-2 writing and schema-1 wrappers
alone.

### 4.1 Compatibility

`check_compat` (or its replacement) compares `sample_bits` and `interval` for
every format-neutral set. Source and fold are recorded per inspected file and
are not compatibility fields on that path.

The legacy-only path (`allow_current == false`) continues to require matching
source and fold so existing schema-1 tests and wrappers stay valid.

Overlap, duplicates, and integrity checks stay in `inspect_csv_inputs_ordered`.

### 4.2 Preview

`ConcatenationPreview` keeps top-level `source_id`, `sample_bits`, `interval`,
and `fold`:

- Homogeneous: those fields are the shared input values.
- Heterogeneous: `source_id` is `mixed`, `fold` is `None`.

Each `ConcatenationInputEntry` used by format-neutral preview carries that
input's `source_id` and `fold` from its `SessionStem`. Combine mapping must
read those entry fields. It must not copy `preview.source_id()` onto every
row.

### 4.3 Naming

`mixed` is a valid `SourceId` token (`^[a-z][a-z0-9]{0,31}$`) and is not a
collectable entropy source. Do not add it to discovery or `SourceId`
constructors used by collection.

`ConcatenationStem::new` for mixed uses `SourceId` `mixed`, the shared bits
and interval, and `fold = None`. Existing fold rules already forbid a fold
suffix on non-`bitb` tokens, so the mixed stem has no `_f*`.

Define the token and the user-facing label in `rngkit-recording` (not as a
live source in `rngkit-core`) so collection cannot emit a mixed session.

### 4.4 Manifest

| Output | Schema | Kind | Top-level source/fold | Per-input source/fold |
| --- | --- | --- | --- | --- |
| Schema-1 legacy wrapper | 1 | `legacy_csv_concatenation` | Shared input | Absent |
| Homogeneous Combine | 2 | `csv_concatenation` | Shared input | Absent |
| Heterogeneous Combine | 3 | `csv_concatenation` | `mixed` / omitted | Required; plus existing format |

Schema 3 validation:

- Kind is `csv_concatenation`.
- Stem, `source_id`, bits, interval, and fold match as today (`mixed`, no
  fold).
- Every input has `current_csv` or `legacy_v3_csv` format, a `source_id`, and
  a fold value consistent with that source's stem rules.
- Parse each input basename as a session CSV stem. Its source and fold must
  equal the entry provenance, and its bits and interval must equal the output
  metadata. Require a source supported by the declared input format; `mixed`
  is never an input source.
- At least two inputs must differ in source or fold. Reject schema-3 documents
  whose input identities are all identical.
- Row counts, hashes, timestamps, and output ranges stay as in schema 2.

Schema 1 must still reject unexpected format fields. Schema 2 must still
require format and must reject per-input `source_id`/`fold` so a mixed
document cannot masquerade as schema 2.

### 4.5 Writer and reader

`create_csv_concatenation` inspects, then:

- Homogeneous: existing schema-2 stem and `ConcatenationManifest::new_csv`.
- Heterogeneous: mixed stem and a schema-3 constructor.

Rows are still streamed through `for_each_csv_row`. No resampling and no
combination of one-counts across inputs except ordered append.

`open_concatenation` accepts schema 1, 2, and 3. For schema 3 it builds
`NormalizedMeta` with `source_id = mixed`, `source_label = "Mixed sources"`,
and `fold = None`. Per-input provenance remains on the manifest; the
normalized sample series stays a single chronological list.

### 4.6 XLSX and app

XLSX Summary `Source` currently writes `meta.source_id`. For mixed it must
write `meta.source_label` (`Mixed sources`) so the workbook does not present
`mixed` as if it were a device family. `Fold` stays empty. Homogeneous Summary
rows keep writing the source token.

RngKit Tauri Combine/Reports/Help consume the library identity. After an
authorized exact core pin:

- Preview rows use per-input source/fold.
- Derived preview and outcome summaries use `Mixed sources` when mixed.
- Bits/interval mismatch copy states that both must match exactly.
- Help Combining files and “Combine says the files are incompatible” match
  the new rule.

### 4.7 Integration

Implement and validate in `rngkit-core` first. Publish or otherwise make an
exact Git revision reachable, then pin that revision in the app. Never use a
path dependency. Commit, push, pin, and release stay separately authorized.

## 5. Alternatives considered

**Keep schema 2 and add optional per-input source/fold.** Homogeneous JSON
could stay similar, and current readers ignore unknown fields. Old app builds
would then open mixed bundles, show source token `mixed`, and drop per-input
provenance. Schema 3 fails closed (`UnsupportedSchema`) and keeps homogeneous
schema-2 documents unchanged. Chosen because mixed output is a new stored
contract.

**Keep the first input's source token in the filename and mark mixed only in
the manifest.** Rejected. Mixed data would still look like the first hardware
in stems, Reports, and XLSX titles.

**Treat only differing source IDs as mixed, and keep same-source different
folds as `bitb`.** A BitBabbler stem requires `_f0`–`_f4`. There is no honest
single fold for `f0`+`f1`. Differing folds are therefore mixed, same as
differing source IDs.

**Relax schema-1 wrappers too.** Rejected. Those APIs are documented as
restrictive legacy-only. Combine uses the format-neutral APIs.

## 6. Failure and edge cases

- Bits differ, interval matches: reject `SampleBits`.
- Interval differs, bits match, including equal bits/second: reject
  `Interval`.
- Source and fold differ, bits and interval match, ranges disjoint: success,
  mixed identity.
- Same source, different folds, bits and interval match: success, mixed
  identity.
- Duplicate canonical path: reject as today.
- Equal or overlapping timestamp boundaries across files: reject as today.
- Equal timestamps inside one file: accept as today.
- Derived `_concat_` CSV selected as Combine input: still fails stem/format
  checks; not a session CSV.
- Schema-3 document presented as schema 2: reject.
- Schema 1/2 homogeneous bundles: unchanged read path.
- Missing fold on a BitBabbler input stem: invalid name, as today.
- Fold present on a non-BitBabbler input stem: invalid name, as today.

## 7. Cross-cutting concerns

- **Compatibility:** Schema 1 and 2 remain readable. Only new mixed output
  uses schema 3. Homogeneous Combine output stays schema 2.
- **Privacy:** Still no serials, OS paths, seeds, or absolute input paths.
  Per-input source/fold are already in the input filenames.
- **Security:** Inputs stay read-only. Staging, no-replace promotion, and
  contained opens are unchanged.
- **Rollout:** Library revision first; app pin second; no silent toolchain or
  dependency float.

## 8. Validation strategy

Library (locked stable and MSRV 1.85, hardware tests still ignored):

- Mixed-source inspect and create succeed; output stem contains `_concat_mixed_`;
  ones/timestamps match inputs in order.
- Differing BitBabbler folds succeed with per-input folds preserved and mixed
  top-level identity.
- Unequal bits rejected; unequal interval rejected, including equal-throughput
  pairs such as `s2048_i2` with `s1024_i1`.
- Duplicate and overlap rejection unchanged.
- Schema-3 manifest round-trip; `open_concatenation` label is `Mixed sources`.
- Reject schema-3 provenance that disagrees with the input basename, input
  bits/interval that disagree with output metadata, unsupported source/format
  combinations, and homogeneous input identities presented as mixed.
- Schema-1 and homogeneous schema-2 fixtures still open; homogeneous create
  still writes schema 2 with the original source token.
- XLSX Summary Source/Fold for mixed versus homogeneous.
- `inspect_legacy_csvs` still rejects differing source and fold.

Application, after authorized pin:

- Combine preview rows show distinct sources/folds and allow mixed sets.
- Help and incompatibility copy match the bits/interval rule.
- Derived Reports preview and XLSX do not use the first input's hardware name.
- Focused Combine/Help tests plus the relevant locked frontend and Rust
  checks; no-bundle build prepared for user validation.

## 9. Decisions and assumptions

- Heterogeneous means any difference in source ID or fold.
- Filename token is `mixed`; user-facing label is `Mixed sources`.
- Schema 3 is mixed-only; kind stays `csv_concatenation`.
- Schema-1 wrappers remain strict.
- Per-input provenance comes only from `SessionStem`.
- Interval remains whole seconds; throughput is never a compatibility field.
- Implementation starts only after an explicit request to execute the approved
  plan. Commit, push, core publication, app pin, and release each need their
  own authorization.
