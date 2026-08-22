# rngkit-recording

Native JSON/BIN/CSV session bundles, read-only RngKitPSG version 3 import, and
derived concatenation inspection.

New sessions use the version 3 filename stem
`YYYYMMDDTHHMMSS_<source>_s<bits>_i<seconds>[_f<fold>]` inside a directory that
also contains `manifest.json`. CSV is the commit marker: a trailing uncommitted
BIN range is ignored with a warning; CSV references beyond BIN EOF are hard
corruption. Files are never truncated, repaired, resumed, or overwritten
automatically.

Legacy access is read-only and limited to version 3 T-formatted names with
source IDs `bitb`, `trng`, and `pseudo`. BIN import streams fixed-size samples
without retaining the whole payload. CSV one-counts greater than the filename
sample size are rejected. Native manifests are validated so `stem`, `bin_file`,
and `csv_file` cannot point outside the selected session directory. Existing
BIN, CSV, and `manifest.json` entries are opened without following symbolic
links or reparse points.

`inspect_legacy_csvs` streams distinct nonempty legacy v3 CSV files, hashes
each file with SHA-256, and returns chronologically ordered preview metadata.
Inputs must share source, sample bits, interval, and fold. Timestamps may be
equal inside one file but equal or overlapping boundaries between files are
rejected. Preview, debug, and serialized values expose basenames and hashes,
never absolute input paths. The derived stem grammar is
`YYYYMMDDTHHMMSS_concat_<source>_s<bits>_i<seconds>[_f<fold>]` and is not a
collected session name. Bundle writing is a separate API.
