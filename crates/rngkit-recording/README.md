# rngkit-recording

Native JSON/BIN/CSV session bundles and read-only RngKitPSG version 3 import.

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
