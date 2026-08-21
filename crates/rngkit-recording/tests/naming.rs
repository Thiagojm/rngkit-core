//! Golden tests for version 3 stems.

use rngkit_core::{Fold, IntervalSeconds, SampleBits, SourceId};
use rngkit_recording::{RecordingError, SessionStem};
use time::{Date, Month, PrimitiveDateTime, Time, UtcOffset};

fn bits() -> SampleBits {
    SampleBits::new(2048).expect("bits")
}

fn interval() -> IntervalSeconds {
    IntervalSeconds::new(1).expect("interval")
}

fn local(offset_hours: i8) -> time::OffsetDateTime {
    let date = Date::from_calendar_date(2026, Month::August, 21).expect("date");
    let time = Time::from_hms(18, 30, 0).expect("time");
    let offset = UtcOffset::from_hms(offset_hours, 0, 0).expect("offset");
    PrimitiveDateTime::new(date, time).assume_offset(offset)
}

#[test]
fn four_source_stems() {
    let t = local(-3);
    let trng = SessionStem::new(t, SourceId::trng(), bits(), interval(), None).expect("trng");
    assert_eq!(trng.as_str(), "20260821T183000_trng_s2048_i1");
    let bitb = SessionStem::new(
        t,
        SourceId::bitb(),
        bits(),
        interval(),
        Some(Fold::new(0).expect("fold")),
    )
    .expect("bitb");
    assert_eq!(bitb.as_str(), "20260821T183000_bitb_s2048_i1_f0");
    let pseudo = SessionStem::new(t, SourceId::pseudo(), bits(), interval(), None).expect("pseudo");
    assert_eq!(pseudo.as_str(), "20260821T183000_pseudo_s2048_i1");
    let rdseed = SessionStem::new(t, SourceId::rdseed(), bits(), interval(), None).expect("rdseed");
    assert_eq!(rdseed.as_str(), "20260821T183000_rdseed_s2048_i1");
}

#[test]
fn bitb_folds_zero_through_four() {
    let t = local(0);
    for fold in 0..=4 {
        let stem = SessionStem::new(
            t,
            SourceId::bitb(),
            bits(),
            interval(),
            Some(Fold::new(fold).expect("fold")),
        )
        .expect("stem");
        assert!(stem.as_str().ends_with(&format!("_f{fold}")));
        let parsed = SessionStem::parse(stem.as_str()).expect("parse");
        assert_eq!(parsed.fold().map(|f| f.get()), Some(fold));
    }
}

#[test]
fn timezone_offset_does_not_change_local_digits() {
    let west = SessionStem::new(local(-5), SourceId::trng(), bits(), interval(), None).unwrap();
    let east = SessionStem::new(local(9), SourceId::trng(), bits(), interval(), None).unwrap();
    assert_eq!(west.as_str(), east.as_str());
    assert_eq!(west.as_str(), "20260821T183000_trng_s2048_i1");
}

#[test]
fn rejects_version_two_hyphenated_names() {
    let err = SessionStem::parse("20260821-183000_trng_s2048_i1").unwrap_err();
    assert!(matches!(err, RecordingError::UnsupportedVersion { .. }));
}

#[test]
fn rejects_bitb_without_fold_and_fold_on_others() {
    let t = local(0);
    assert!(SessionStem::new(t, SourceId::bitb(), bits(), interval(), None).is_err());
    assert!(
        SessionStem::new(
            t,
            SourceId::trng(),
            bits(),
            interval(),
            Some(Fold::new(0).unwrap())
        )
        .is_err()
    );
}

#[test]
fn round_trip_parse() {
    for name in [
        "20260821T183000_trng_s2048_i1",
        "20260821T183000_bitb_s2048_i1_f4",
        "20260821T183000_pseudo_s8_i60",
        "20260821T183000_rdseed_s2048_i1",
    ] {
        let parsed = SessionStem::parse(name).expect(name);
        assert_eq!(parsed.as_str(), name);
    }
}

#[test]
fn rejects_path_separators() {
    assert!(SessionStem::parse("20260821T183000_trng_s2048_i1/../x").is_err());
}
