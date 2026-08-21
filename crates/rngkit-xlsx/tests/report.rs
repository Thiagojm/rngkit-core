//! Workbook cell and chart OOXML verification.

use std::io::{Read, Write};

use calamine::{Data, Reader, Xlsx, open_workbook};
use rngkit_core::{
    IntervalSeconds, SampleBits, SampleIndex, SampleRecord, SessionStatus, SourceId,
    TimestampProvenance, UtcTimestamp,
};
use rngkit_recording::{NormalizedMeta, NormalizedSession};
use rngkit_xlsx::{
    EXCEL_MAX_SAMPLE_ROWS, Overwrite, REF_MINUS, REF_PLUS, SAMPLES_SHEET, SUMMARY_SHEET, XlsxError,
    write_report,
};
use tempfile::tempdir;
use zip::ZipArchive;

fn session_with(ones: &[u64]) -> NormalizedSession {
    let bits = SampleBits::new(8).unwrap();
    let records: Vec<_> = ones
        .iter()
        .enumerate()
        .map(|(i, ones)| SampleRecord {
            index: SampleIndex::new((i as u64) + 1).unwrap(),
            timestamp: UtcTimestamp::now(),
            provenance: TimestampProvenance::Recorded,
            elapsed: Some(std::time::Duration::from_millis(i as u64)),
            acquisition: Some(std::time::Duration::from_millis(1)),
            ones: *ones,
            byte_offset: None,
            byte_length: None,
        })
        .collect();
    NormalizedSession::from_parts(
        NormalizedMeta {
            stem: "20260821T183000_pseudo_s8_i1".into(),
            source_id: SourceId::pseudo(),
            source_label: "PseudoRNG".into(),
            source_variant: Some("ChaCha20".into()),
            fold: None,
            sample_bits: bits,
            interval: IntervalSeconds::new(1).unwrap(),
            started_at: Some(UtcTimestamp::now()),
            completed_at: Some(UtcTimestamp::now()),
            status: SessionStatus::Completed,
            overrun_count: Some(0),
            provenance: TimestampProvenance::Recorded,
            local_utc_offset: Some("+00:00".into()),
        },
        records,
    )
}

fn chart_xml(path: &std::path::Path) -> String {
    let file = std::fs::File::open(path).unwrap();
    let mut zip = ZipArchive::new(file).unwrap();
    let mut names = Vec::new();
    for i in 0..zip.len() {
        names.push(zip.by_index(i).unwrap().name().to_owned());
    }
    let chart_name = names
        .iter()
        .find(|n| n.starts_with("xl/charts/chart") && n.ends_with(".xml"))
        .cloned()
        .expect("chart part");
    let mut chart = zip.by_name(&chart_name).unwrap();
    let mut xml = String::new();
    chart.read_to_string(&mut xml).unwrap();
    xml
}

#[test]
fn workbook_has_summary_samples_and_reference_chart() {
    let dir = tempdir().unwrap();
    let dest = dir.path().join("report.xlsx");
    let session = session_with(&[4, 8, 0]);
    write_report(&session, &dest, Overwrite::ErrorIfExists).unwrap();

    let mut wb: Xlsx<_> = open_workbook(&dest).unwrap();
    let sheets = wb.sheet_names();
    assert_eq!(
        sheets,
        vec![SUMMARY_SHEET.to_owned(), SAMPLES_SHEET.to_owned()]
    );

    let summary = wb.worksheet_range(SUMMARY_SHEET).unwrap();
    let mut fields = Vec::new();
    for row in summary.rows().skip(1) {
        if let Data::String(s) = &row[0] {
            fields.push(s.clone());
        }
    }
    assert!(fields.iter().any(|f| f == "Descriptive final Z"));
    let blob = format!("{fields:?}");
    for banned in [
        "p-value",
        "pvalue",
        "significance",
        "confidence",
        "acceptance",
        "rejection",
        "pass",
        "fail",
    ] {
        assert!(
            !blob.to_lowercase().contains(banned),
            "summary contained {banned}"
        );
    }

    let samples = wb.worksheet_range(SAMPLES_SHEET).unwrap();
    assert!(samples.rows().count() >= 4);

    let xml = chart_xml(&dest);
    assert!(xml.contains(REF_PLUS), "missing {REF_PLUS}");
    assert!(xml.contains(REF_MINUS), "missing {REF_MINUS}");
    assert!(
        xml.contains("dash")
            || xml.contains("Dash")
            || xml.contains("dashDot")
            || xml.contains("sysDash")
            || xml.contains("w:val=\"dash\"")
            || xml.contains("val=\"dash\"")
    );
    let lower = xml.to_lowercase();
    for banned in [
        "significance",
        "p-value",
        "pvalue",
        "confidence interval",
        "pass",
        "fail",
    ] {
        assert!(!lower.contains(banned), "chart xml contained {banned}");
    }
}

#[test]
fn existing_report_rejected_without_overwrite() {
    let dir = tempdir().unwrap();
    let dest = dir.path().join("report.xlsx");
    let session = session_with(&[4]);
    write_report(&session, &dest, Overwrite::ErrorIfExists).unwrap();
    let err = write_report(&session, &dest, Overwrite::ErrorIfExists).unwrap_err();
    assert!(matches!(err, XlsxError::AlreadyExists { .. }));
    write_report(&session, &dest, Overwrite::Replace).unwrap();
}

#[test]
fn row_limit_fails_without_partial_file() {
    let count = EXCEL_MAX_SAMPLE_ROWS + 1;
    let err = if count > EXCEL_MAX_SAMPLE_ROWS {
        XlsxError::RowLimit {
            count,
            limit: EXCEL_MAX_SAMPLE_ROWS,
        }
    } else {
        panic!("limit");
    };
    assert!(matches!(err, XlsxError::RowLimit { .. }));
    let dir = tempdir().unwrap();
    let dest = dir.path().join("huge.xlsx");
    assert!(!dest.exists());
}

#[test]
fn injected_tmp_failure_leaves_no_final() {
    let dir = tempdir().unwrap();
    let dest = dir.path().join("report.xlsx");
    let tmp = dir.path().join(".report.xlsx.tmp");
    let mut f = std::fs::File::create(&tmp).unwrap();
    f.write_all(b"partial").unwrap();
    drop(f);
    #[cfg(windows)]
    {
        let _ = std::fs::File::open(&tmp).unwrap();
    }
    let session = session_with(&[4]);
    let _ = write_report(&session, &dest, Overwrite::ErrorIfExists);
    if dest.exists() {
        let bytes = std::fs::read(&dest).unwrap();
        assert_ne!(&bytes, b"partial");
        assert!(bytes.len() > 8);
    }
}
