//! Workbook cell and chart OOXML verification.

use std::io::Read;

use calamine::{Data, Reader, Xlsx, open_workbook};
use rngkit_core::{
    IntervalSeconds, SampleBits, SampleIndex, SampleRecord, SessionStatus, SourceId,
    TimestampProvenance, UtcTimestamp,
};
use rngkit_recording::{NormalizedMeta, NormalizedSession, SessionStem};
use rngkit_xlsx::{
    EXCEL_MAX_SAMPLE_ROWS, Overwrite, REF_MINUS, REF_PLUS, SAMPLES_SHEET, SUMMARY_SHEET, XlsxError,
    native_report_path, with_report_promote_hook, with_workbook_write_failure, write_report,
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

fn try_file_symlink(target: &std::path::Path, link: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).expect("unix file symlink");
        true
    }
    #[cfg(windows)]
    {
        match std::os::windows::fs::symlink_file(target, link) {
            Ok(()) => true,
            Err(err) => {
                eprintln!("skipping real file-symlink coverage: {err}");
                false
            }
        }
    }
}

#[test]
fn preexisting_predictable_temp_is_not_reused() {
    let dir = tempdir().unwrap();
    let dest = dir.path().join("report.xlsx");
    let predictable = dir.path().join(".report.xlsx.tmp");
    let stale = b"stale-temp-must-not-be-truncated";
    std::fs::write(&predictable, stale).unwrap();
    let session = session_with(&[4]);
    write_report(&session, &dest, Overwrite::ErrorIfExists).unwrap();
    assert!(dest.exists());
    let dest_bytes = std::fs::read(&dest).unwrap();
    assert_ne!(&dest_bytes, stale);
    assert!(dest_bytes.len() > 8);
    assert_eq!(std::fs::read(&predictable).unwrap(), stale);
}

#[test]
fn preexisting_temp_symlink_is_not_followed() {
    let dir = tempdir().unwrap();
    let dest = dir.path().join("report.xlsx");
    let outside = dir.path().join("outside.bin");
    let secret = b"xlsx-must-not-touch-this";
    std::fs::write(&outside, secret).unwrap();
    let predictable = dir.path().join(".report.xlsx.tmp");
    let linked = try_file_symlink(&outside, &predictable);
    if !linked {
        std::fs::write(&predictable, secret).unwrap();
    }
    write_report(&session_with(&[4]), &dest, Overwrite::ErrorIfExists).unwrap();
    assert!(dest.exists());
    assert_ne!(std::fs::read(&dest).unwrap(), secret);
    assert_eq!(std::fs::read(&outside).unwrap(), secret);
    if linked {
        assert!(predictable.exists());
    }
}

#[test]
fn error_if_exists_rejects_destination_created_before_promote() {
    let dir = tempdir().unwrap();
    let dest = dir.path().join("report.xlsx");
    let dest_for_hook = dest.clone();
    let concurrent = b"concurrent-destination";
    let err = with_report_promote_hook(
        move |_tmp| {
            std::fs::write(&dest_for_hook, concurrent).unwrap();
        },
        || write_report(&session_with(&[4]), &dest, Overwrite::ErrorIfExists),
    )
    .unwrap_err();
    assert!(matches!(err, XlsxError::AlreadyExists { .. }));
    assert_eq!(std::fs::read(&dest).unwrap(), concurrent);
}

#[test]
fn replace_overwrites_existing_destination() {
    let dir = tempdir().unwrap();
    let dest = dir.path().join("report.xlsx");
    std::fs::write(&dest, b"old-report").unwrap();
    write_report(&session_with(&[4]), &dest, Overwrite::Replace).unwrap();
    let bytes = std::fs::read(&dest).unwrap();
    assert_ne!(&bytes, b"old-report");
    assert!(bytes.len() > 8);
}

#[test]
fn workbook_generation_failure_cleans_temp_and_preserves_neighbors() {
    let dir = tempdir().unwrap();
    let dest = dir.path().join("report.xlsx");
    let outside = dir.path().join("outside.bin");
    let secret = b"neighbor-must-survive";
    std::fs::write(&outside, secret).unwrap();
    std::fs::write(&dest, b"pre-existing").unwrap();
    let err = with_workbook_write_failure(|| {
        write_report(&session_with(&[4]), &dest, Overwrite::Replace)
    })
    .unwrap_err();
    assert!(matches!(err, XlsxError::Workbook(_)));
    assert_eq!(std::fs::read(&dest).unwrap(), b"pre-existing");
    assert_eq!(std::fs::read(&outside).unwrap(), secret);
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.starts_with(".rngkit-xlsx-") && name.ends_with(".tmp"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "owned xlsx temp leftover: {leftovers:?}"
    );
}

#[test]
fn native_report_path_stays_inside_session_dir() {
    let dir = tempdir().unwrap();
    let session_dir = dir.path().join("session");
    std::fs::create_dir(&session_dir).unwrap();
    let stem = SessionStem::parse("20260821T183000_pseudo_s8_i1").unwrap();
    let path = native_report_path(&session_dir, &stem).unwrap();
    let root = session_dir.canonicalize().unwrap();
    assert!(path.starts_with(&root));
    assert_eq!(
        path.file_name().and_then(|n| n.to_str()),
        Some("20260821T183000_pseudo_s8_i1.xlsx")
    );
    assert!(SessionStem::parse("../outside").is_err());
    assert!(SessionStem::parse("..\\outside").is_err());

    write_report(&session_with(&[4]), &path, Overwrite::ErrorIfExists).unwrap();
    assert!(path.exists());
    assert!(!dir.path().join("outside.xlsx").exists());
}
