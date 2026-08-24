# rngkit-recording

Native JSON/BIN/CSV session bundles, standalone current/legacy input reading,
read-only RngKitPSG version 3 import, and derived CSV concatenation inspection
and bundle creation.

New sessions use the version 3 filename stem
`YYYYMMDDTHHMMSS_<source>_s<bits>_i<seconds>[_f<fold>]` inside a directory that
also contains `manifest.json`. CSV is the commit marker: a trailing uncommitted
BIN range is ignored with a warning; CSV references beyond BIN EOF are hard
corruption. Files are never truncated, repaired, resumed, or overwritten
automatically.

Legacy access is read-only and limited to version 3 T-formatted names with
source IDs bitb, trng, and pseudo. Both compact
YYYYMMDDTHHMMSS,<ones> and the older colon-bearing timestamp form are
readable; space-delimited and v2 inputs remain rejected. BIN import streams
fixed-size samples without retaining the whole payload. CSV one-counts
greater than the filename sample size are rejected. Native manifests are
validated so stem, bin_file, and csv_file cannot point outside the selected
session directory. Existing BIN, CSV, and manifest.json entries are opened
without following symbolic links or reparse points.

open_standalone accepts a current native CSV, a headerless legacy v3 CSV, or a
fixed-size BIN without requiring a manifest. Current CSV input uses the exact
seven-column native header and validates contiguous indexes, RFC 3339
timestamps, one-count bounds, byte lengths, and byte offsets. The current
source IDs are bitb, trng, rdseed, and pseudo; legacy CSV keeps its historical
family restriction. A same-stem pair is validated without changing either
input.

inspect_csv_inputs streams distinct nonempty current or legacy CSV files,
hashes each file with SHA-256, and returns chronologically ordered preview
metadata with a current_csv or legacy_v3_csv format label. Inputs must share
source, sample bits, interval, and fold. Timestamps may be equal inside one
file but equal or overlapping boundaries between files are rejected. Preview,
debug, and serialized values expose basenames and hashes, never absolute input
paths. The derived stem grammar is
`YYYYMMDDTHHMMSS_concat_<source>_s<bits>_i<seconds>[_f<fold>]` and is not a
collected session name.

create_csv_concatenation reopens and revalidates current, legacy, or mixed
inputs, streams rows into a unique staging directory, and promotes a same-stem
CSV plus manifest.json without replacing an existing destination. New
manifests use schema 2 and csv_concatenation; existing schema-1
legacy_csv_concatenation bundles remain readable. The legacy-only
inspect_legacy_csvs and create_legacy_csv_concatenation wrappers remain
available. Failure removes owned staging data and does not modify inputs.
open_concatenation validates both manifest versions and returns a normalized
session.
