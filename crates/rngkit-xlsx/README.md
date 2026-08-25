# rngkit-xlsx

Excel analysis report generation for native, legacy, and derived concatenation
RngKit sessions.

The workbook has two user-visible sheets, `Summary` and `Samples`, plus a
cumulative signed-Z line chart with a zero line and dashed visual references at
`+1.96` and `-1.96`. Those series are not significance, confidence, acceptance,
rejection, pass, or fail boundaries. No p-value is written.

This crate consumes normalized readers and `rngkit-analysis`. It does not parse
BIN or CSV itself. Native and derived report paths are built from a validated
stem and must remain inside the selected directory. Workbooks are written to a
unique create-new temporary file in the destination directory and promoted only
after a successful close; `ErrorIfExists` never replaces a destination that
appears concurrently.

Callers that know the selected artifact should use `ReportOptions` with its
safe `.csv` or `.bin` basename and `ChartXAxisMode`. Recorded CSV timestamps are
shown as `HH:mm:ss` chart categories through a hidden helper column while the
full timestamp remains in `Samples`; BIN-only reports use the one-based sample
index. The chart title includes the source basename and its axes describe the
sample time/index, configured interval, cumulative signed Z, and sample size.
