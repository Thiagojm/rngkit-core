# rngkit-xlsx

Excel analysis report generation for native and legacy RngKit sessions.

The workbook has two user-visible sheets, `Summary` and `Samples`, plus a
cumulative signed-Z line chart with a zero line and dashed visual references at
`+1.96` and `-1.96`. Those series are not significance, confidence, acceptance,
rejection, pass, or fail boundaries. No p-value is written.

This crate consumes normalized readers and `rngkit-analysis`. It does not parse
BIN or CSV itself. Native report paths are built from a validated session stem
and must remain inside the session directory. Workbooks are written to a unique
create-new temporary file in the destination directory and promoted only after
a successful close; `ErrorIfExists` never replaces a destination that appears
concurrently.
