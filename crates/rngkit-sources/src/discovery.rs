//! Unified, best-effort snapshot of currently selectable entropy sources.

#[cfg(any(feature = "bitb", feature = "trng3"))]
use rngkit_core::SourceErrorKind;
use rngkit_core::{SourceError, SourceId};

/// A currently selectable entropy source.
///
/// Hardware variants carry transient selectors used later to build an explicit
/// [`crate::SourceConfig`]. Those selectors are not persistable descriptors and
/// must not be written into manifests, session files, or reports.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SourceCandidate {
    /// A recognized BitBabbler. `serial` is transient selection only.
    #[cfg(feature = "bitb")]
    Bitb {
        /// White or Black, from the USB product string.
        variant: String,
        /// Transient serial used only to open this device.
        serial: String,
    },
    /// A recognized TrueRNG v1/v2/v3 port. `port_name` is transient.
    #[cfg(feature = "trng3")]
    Trng {
        /// OS port name used only to open this device.
        port_name: String,
    },
    /// Intel RDSEED is supported in this process.
    #[cfg(feature = "rdseed")]
    Rdseed,
    /// OS-seeded ChaCha20 PseudoRNG constructed successfully.
    #[cfg(feature = "pseudo")]
    Pseudo,
}

impl SourceCandidate {
    /// Stable typed source identity for this candidate.
    #[must_use]
    pub fn source_id(&self) -> SourceId {
        #[cfg(not(any(
            feature = "bitb",
            feature = "trng3",
            feature = "rdseed",
            feature = "pseudo"
        )))]
        unreachable!("SourceCandidate has no variants without source features");

        #[cfg(any(
            feature = "bitb",
            feature = "trng3",
            feature = "rdseed",
            feature = "pseudo"
        ))]
        match self {
            #[cfg(feature = "bitb")]
            Self::Bitb { .. } => SourceId::bitb(),
            #[cfg(feature = "trng3")]
            Self::Trng { .. } => SourceId::trng(),
            #[cfg(feature = "rdseed")]
            Self::Rdseed => SourceId::rdseed(),
            #[cfg(feature = "pseudo")]
            Self::Pseudo => SourceId::pseudo(),
        }
    }

    /// Safe static display label. Does not include serials, ports, or state.
    #[must_use]
    pub fn label(&self) -> &'static str {
        #[cfg(not(any(
            feature = "bitb",
            feature = "trng3",
            feature = "rdseed",
            feature = "pseudo"
        )))]
        unreachable!("SourceCandidate has no variants without source features");

        #[cfg(any(
            feature = "bitb",
            feature = "trng3",
            feature = "rdseed",
            feature = "pseudo"
        ))]
        match self {
            #[cfg(feature = "bitb")]
            Self::Bitb { .. } => "BitBabbler",
            #[cfg(feature = "trng3")]
            Self::Trng { .. } => "TrueRNG v1/v2/v3",
            #[cfg(feature = "rdseed")]
            Self::Rdseed => "Intel RDSEED",
            #[cfg(feature = "pseudo")]
            Self::Pseudo => "PseudoRNG",
        }
    }
}

/// A non-blocking failure scoped to one source family.
#[derive(Debug)]
pub struct DiscoveryIssue {
    source_id: SourceId,
    error: SourceError,
}

impl DiscoveryIssue {
    /// Typed identity of the family that failed.
    #[must_use]
    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    /// Normalized error. The kind is suitable for application mapping; the
    /// diagnostic text is not a stable IPC contract.
    #[must_use]
    pub fn error(&self) -> &SourceError {
        &self.error
    }
}

/// Best-effort discovery snapshot: present candidates plus per-family issues.
///
/// This type does not derive serialization traits. A Tauri adapter must map it
/// to its own DTOs.
#[derive(Debug)]
#[must_use]
pub struct DiscoveryReport {
    candidates: Vec<SourceCandidate>,
    issues: Vec<DiscoveryIssue>,
}

impl DiscoveryReport {
    /// Currently selectable sources, in stable family order.
    #[must_use]
    pub fn candidates(&self) -> &[SourceCandidate] {
        &self.candidates
    }

    /// Real per-family failures that did not stop discovery of later families.
    #[must_use]
    pub fn issues(&self) -> &[DiscoveryIssue] {
        &self.issues
    }
}

/// Discovers currently selectable entropy sources.
///
/// Families are evaluated independently in this order: BitBabbler, TrueRNG,
/// RDSEED, PseudoRNG. Empty hardware lists, [`SourceErrorKind::NotAvailable`]
/// during hardware enumeration, unsupported RDSEED, and compile-time-disabled
/// features are omitted. Any other per-family failure is recorded as a
/// [`DiscoveryIssue`] and discovery continues.
///
/// BitBabbler and TrueRNG devices are listed without opening them or reading
/// entropy. PseudoRNG is probed by constructing and immediately dropping a
/// default adapter. Multiple hardware devices remain separate candidates;
/// callers choose an explicit selector when opening.
///
/// Call this from a blocking context: enumeration may perform operating-system
/// I/O. The result is a snapshot, not a reservation, and is not cached.
pub fn discover() -> DiscoveryReport {
    discover_with(&LiveBackend)
}

fn discover_with(backend: &impl DiscoveryBackend) -> DiscoveryReport {
    #[cfg_attr(
        not(any(
            feature = "bitb",
            feature = "trng3",
            feature = "rdseed",
            feature = "pseudo"
        )),
        allow(unused_mut)
    )]
    let mut candidates = Vec::new();
    #[cfg_attr(
        not(any(feature = "bitb", feature = "trng3", feature = "pseudo")),
        allow(unused_mut)
    )]
    let mut issues = Vec::new();

    #[cfg(not(any(
        feature = "bitb",
        feature = "trng3",
        feature = "rdseed",
        feature = "pseudo"
    )))]
    let _ = backend;

    #[cfg(feature = "bitb")]
    match backend.list_bitb() {
        Ok(devices) => candidates.extend(devices.into_iter().map(|dev| SourceCandidate::Bitb {
            variant: dev.variant,
            serial: dev.serial,
        })),
        Err(err) if err.kind() == SourceErrorKind::NotAvailable => {}
        Err(err) => issues.push(DiscoveryIssue {
            source_id: SourceId::bitb(),
            error: err,
        }),
    }

    #[cfg(feature = "trng3")]
    match backend.list_trng() {
        Ok(ports) => candidates.extend(ports.into_iter().map(|dev| SourceCandidate::Trng {
            port_name: dev.port_name,
        })),
        Err(err) if err.kind() == SourceErrorKind::NotAvailable => {}
        Err(err) => issues.push(DiscoveryIssue {
            source_id: SourceId::trng(),
            error: err,
        }),
    }

    #[cfg(feature = "rdseed")]
    if backend.rdseed_supported() {
        candidates.push(SourceCandidate::Rdseed);
    }

    #[cfg(feature = "pseudo")]
    match backend.probe_pseudo() {
        Ok(()) => candidates.push(SourceCandidate::Pseudo),
        Err(err) => issues.push(DiscoveryIssue {
            source_id: SourceId::pseudo(),
            error: err,
        }),
    }

    DiscoveryReport { candidates, issues }
}

trait DiscoveryBackend {
    #[cfg(feature = "bitb")]
    fn list_bitb(&self) -> Result<Vec<crate::adapters::bitb::BitbListing>, SourceError>;

    #[cfg(feature = "trng3")]
    fn list_trng(&self) -> Result<Vec<crate::adapters::trng3::TrngListing>, SourceError>;

    #[cfg(feature = "rdseed")]
    fn rdseed_supported(&self) -> bool;

    #[cfg(feature = "pseudo")]
    fn probe_pseudo(&self) -> Result<(), SourceError>;
}

struct LiveBackend;

impl DiscoveryBackend for LiveBackend {
    #[cfg(feature = "bitb")]
    fn list_bitb(&self) -> Result<Vec<crate::adapters::bitb::BitbListing>, SourceError> {
        crate::adapters::BitbAdapter::list()
    }

    #[cfg(feature = "trng3")]
    fn list_trng(&self) -> Result<Vec<crate::adapters::trng3::TrngListing>, SourceError> {
        crate::adapters::Trng3Adapter::list()
    }

    #[cfg(feature = "rdseed")]
    fn rdseed_supported(&self) -> bool {
        crate::adapters::RdseedAdapter::is_supported()
    }

    #[cfg(feature = "pseudo")]
    fn probe_pseudo(&self) -> Result<(), SourceError> {
        let _adapter = crate::adapters::PseudoAdapter::open(None)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(feature = "bitb", feature = "trng3", feature = "pseudo"))]
    use rngkit_core::SourceErrorKind;

    struct FakeBackend {
        #[cfg(feature = "bitb")]
        bitb: Result<Vec<crate::adapters::bitb::BitbListing>, SourceError>,
        #[cfg(feature = "trng3")]
        trng: Result<Vec<crate::adapters::trng3::TrngListing>, SourceError>,
        #[cfg(feature = "rdseed")]
        rdseed: bool,
        #[cfg(feature = "pseudo")]
        pseudo: Result<(), SourceError>,
    }

    // With no features this is an empty struct and Clippy suggests deriving;
    // enabled features contain Result fields that do not implement Default.
    #[allow(clippy::derivable_impls)]
    impl Default for FakeBackend {
        fn default() -> Self {
            Self {
                #[cfg(feature = "bitb")]
                bitb: Ok(Vec::new()),
                #[cfg(feature = "trng3")]
                trng: Ok(Vec::new()),
                #[cfg(feature = "rdseed")]
                rdseed: false,
                #[cfg(feature = "pseudo")]
                pseudo: Ok(()),
            }
        }
    }

    impl DiscoveryBackend for FakeBackend {
        #[cfg(feature = "bitb")]
        fn list_bitb(&self) -> Result<Vec<crate::adapters::bitb::BitbListing>, SourceError> {
            match &self.bitb {
                Ok(items) => Ok(items.clone()),
                Err(err) => Err(SourceError::new(err.kind(), err.message().to_owned())),
            }
        }

        #[cfg(feature = "trng3")]
        fn list_trng(&self) -> Result<Vec<crate::adapters::trng3::TrngListing>, SourceError> {
            match &self.trng {
                Ok(items) => Ok(items.clone()),
                Err(err) => Err(SourceError::new(err.kind(), err.message().to_owned())),
            }
        }

        #[cfg(feature = "rdseed")]
        fn rdseed_supported(&self) -> bool {
            self.rdseed
        }

        #[cfg(feature = "pseudo")]
        fn probe_pseudo(&self) -> Result<(), SourceError> {
            match &self.pseudo {
                Ok(()) => Ok(()),
                Err(err) => Err(SourceError::new(err.kind(), err.message().to_owned())),
            }
        }
    }

    #[cfg(any(feature = "bitb", feature = "trng3", feature = "pseudo"))]
    fn err(kind: SourceErrorKind, message: &str) -> SourceError {
        SourceError::new(kind, message)
    }

    #[cfg(any(feature = "bitb", feature = "trng3", feature = "pseudo"))]
    fn issue_kinds(report: &DiscoveryReport) -> Vec<(String, SourceErrorKind)> {
        report
            .issues()
            .iter()
            .map(|issue| (issue.source_id().as_str().to_owned(), issue.error().kind()))
            .collect()
    }

    #[cfg(any(feature = "bitb", feature = "trng3", feature = "rdseed"))]
    fn candidate_ids(report: &DiscoveryReport) -> Vec<SourceId> {
        report
            .candidates()
            .iter()
            .map(SourceCandidate::source_id)
            .collect()
    }

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn report_types_are_send_sync() {
        assert_send_sync::<DiscoveryReport>();
        assert_send_sync::<DiscoveryIssue>();
        assert_send_sync::<SourceCandidate>();
    }

    #[test]
    fn all_families_absent() {
        let report = discover_with(&FakeBackend::default());
        #[cfg(feature = "pseudo")]
        {
            assert_eq!(report.candidates(), &[SourceCandidate::Pseudo]);
        }
        #[cfg(not(feature = "pseudo"))]
        {
            assert!(report.candidates().is_empty());
        }
        assert!(report.issues().is_empty());
        #[cfg(feature = "bitb")]
        assert!(!candidate_ids(&report).iter().any(SourceId::is_bitb));
        #[cfg(feature = "trng3")]
        assert!(
            !candidate_ids(&report)
                .iter()
                .any(|id| id.as_str() == "trng")
        );
        #[cfg(feature = "rdseed")]
        assert!(
            !candidate_ids(&report)
                .iter()
                .any(|id| id.as_str() == "rdseed")
        );
    }

    #[test]
    fn empty_lists_and_not_available_are_absence() {
        let empty = discover_with(&FakeBackend::default());
        assert!(empty.issues().is_empty());

        #[cfg(any(feature = "bitb", feature = "trng3"))]
        {
            let missing = FakeBackend {
                #[cfg(feature = "bitb")]
                bitb: Err(err(SourceErrorKind::NotAvailable, "no bitb")),
                #[cfg(feature = "trng3")]
                trng: Err(err(SourceErrorKind::NotAvailable, "no trng")),
                ..FakeBackend::default()
            };
            let report = discover_with(&missing);
            assert!(report.issues().is_empty());
            #[cfg(feature = "bitb")]
            assert!(!candidate_ids(&report).iter().any(SourceId::is_bitb));
            #[cfg(feature = "trng3")]
            assert!(
                !candidate_ids(&report)
                    .iter()
                    .any(|id| id.as_str() == "trng")
            );
        }
    }

    #[cfg(all(
        feature = "bitb",
        feature = "trng3",
        feature = "rdseed",
        feature = "pseudo"
    ))]
    #[test]
    fn multiple_devices_keep_family_order_and_explicit_selectors() {
        let backend = FakeBackend {
            bitb: Ok(vec![
                crate::adapters::bitb::BitbListing {
                    variant: "White".into(),
                    serial: "serial-a".into(),
                },
                crate::adapters::bitb::BitbListing {
                    variant: "Black".into(),
                    serial: "serial-b".into(),
                },
            ]),
            trng: Ok(vec![
                crate::adapters::trng3::TrngListing {
                    port_name: "port-a".into(),
                },
                crate::adapters::trng3::TrngListing {
                    port_name: "port-b".into(),
                },
            ]),
            rdseed: true,
            pseudo: Ok(()),
        };
        let report = discover_with(&backend);
        assert!(report.issues().is_empty());
        assert_eq!(
            candidate_ids(&report),
            vec![
                SourceId::bitb(),
                SourceId::bitb(),
                SourceId::trng(),
                SourceId::trng(),
                SourceId::rdseed(),
                SourceId::pseudo(),
            ]
        );
        match &report.candidates()[0] {
            SourceCandidate::Bitb { variant, serial } => {
                assert_eq!(variant, "White");
                assert_eq!(serial, "serial-a");
            }
            other => panic!("expected first BitBabbler, got {other:?}"),
        }
        match &report.candidates()[1] {
            SourceCandidate::Bitb { variant, serial } => {
                assert_eq!(variant, "Black");
                assert_eq!(serial, "serial-b");
            }
            other => panic!("expected second BitBabbler, got {other:?}"),
        }
        match &report.candidates()[2] {
            SourceCandidate::Trng { port_name } => assert_eq!(port_name, "port-a"),
            other => panic!("expected first TrueRNG, got {other:?}"),
        }
        match &report.candidates()[3] {
            SourceCandidate::Trng { port_name } => assert_eq!(port_name, "port-b"),
            other => panic!("expected second TrueRNG, got {other:?}"),
        }
        assert_eq!(report.candidates()[4], SourceCandidate::Rdseed);
        assert_eq!(report.candidates()[5], SourceCandidate::Pseudo);
    }

    #[cfg(all(
        feature = "bitb",
        feature = "trng3",
        feature = "rdseed",
        feature = "pseudo"
    ))]
    #[test]
    fn each_family_failure_is_independent() {
        let backend = FakeBackend {
            bitb: Err(err(SourceErrorKind::PermissionDenied, "bitb denied")),
            trng: Err(err(SourceErrorKind::Timeout, "trng timeout")),
            rdseed: true,
            pseudo: Err(err(
                SourceErrorKind::EntropyUnavailable,
                "os entropy unavailable",
            )),
        };
        let report = discover_with(&backend);
        assert_eq!(report.candidates(), &[SourceCandidate::Rdseed]);
        assert_eq!(
            issue_kinds(&report),
            vec![
                ("bitb".into(), SourceErrorKind::PermissionDenied),
                ("trng".into(), SourceErrorKind::Timeout),
                ("pseudo".into(), SourceErrorKind::EntropyUnavailable),
            ]
        );
    }

    #[cfg(all(feature = "bitb", feature = "trng3"))]
    #[test]
    fn bitb_failure_does_not_hide_trng_candidates() {
        let backend = FakeBackend {
            bitb: Err(err(SourceErrorKind::DeviceBusy, "bitb busy")),
            trng: Ok(vec![crate::adapters::trng3::TrngListing {
                port_name: "port-a".into(),
            }]),
            ..FakeBackend::default()
        };
        let report = discover_with(&backend);
        assert!(
            report
                .candidates()
                .iter()
                .any(|c| matches!(c, SourceCandidate::Trng { port_name } if port_name == "port-a"))
        );
        assert_eq!(
            issue_kinds(&report),
            vec![("bitb".into(), SourceErrorKind::DeviceBusy)]
        );
    }

    #[cfg(all(feature = "bitb", feature = "trng3"))]
    #[test]
    fn trng_failure_does_not_hide_bitb_candidates() {
        let backend = FakeBackend {
            bitb: Ok(vec![crate::adapters::bitb::BitbListing {
                variant: "White".into(),
                serial: "serial-a".into(),
            }]),
            trng: Err(err(SourceErrorKind::Other, "usb enumeration")),
            ..FakeBackend::default()
        };
        let report = discover_with(&backend);
        assert!(
            report
                .candidates()
                .iter()
                .any(|c| matches!(c, SourceCandidate::Bitb { serial, .. } if serial == "serial-a"))
        );
        assert_eq!(
            issue_kinds(&report),
            vec![("trng".into(), SourceErrorKind::Other)]
        );
    }

    #[cfg(feature = "rdseed")]
    #[test]
    fn unsupported_rdseed_is_omitted() {
        let report = discover_with(&FakeBackend::default());
        assert!(
            !candidate_ids(&report)
                .iter()
                .any(|id| id.as_str() == "rdseed")
        );
        assert!(report.issues().is_empty());

        let supported = FakeBackend {
            rdseed: true,
            ..FakeBackend::default()
        };
        let report = discover_with(&supported);
        assert!(
            report
                .candidates()
                .iter()
                .any(|c| matches!(c, SourceCandidate::Rdseed))
        );
    }

    #[cfg(feature = "pseudo")]
    #[test]
    fn successful_pseudo_probe_adds_one_candidate() {
        let report = discover_with(&FakeBackend::default());
        let pseudos: Vec<_> = report
            .candidates()
            .iter()
            .filter(|c| matches!(c, SourceCandidate::Pseudo))
            .collect();
        assert_eq!(pseudos.len(), 1);
        assert!(
            report
                .issues()
                .iter()
                .all(|issue| issue.source_id().as_str() != "pseudo")
        );
    }

    #[cfg(feature = "pseudo")]
    #[test]
    fn failed_pseudo_probe_adds_issue_without_state() {
        let backend = FakeBackend {
            pseudo: Err(err(
                SourceErrorKind::EntropyUnavailable,
                "os entropy unavailable",
            )),
            ..FakeBackend::default()
        };
        let report = discover_with(&backend);
        assert!(
            !report
                .candidates()
                .iter()
                .any(|c| matches!(c, SourceCandidate::Pseudo))
        );
        assert_eq!(
            issue_kinds(&report),
            vec![("pseudo".into(), SourceErrorKind::EntropyUnavailable)]
        );
        let debug = format!("{:?}", report.issues()[0].error());
        assert!(!debug.to_ascii_lowercase().contains("seed"));
    }

    #[test]
    fn candidate_ids_and_safe_labels() {
        #[cfg(feature = "bitb")]
        {
            let candidate = SourceCandidate::Bitb {
                variant: "White".into(),
                serial: "fake-serial".into(),
            };
            assert_eq!(candidate.source_id(), SourceId::bitb());
            assert_eq!(candidate.label(), "BitBabbler");
        }
        #[cfg(feature = "trng3")]
        {
            let candidate = SourceCandidate::Trng {
                port_name: "fake-port".into(),
            };
            assert_eq!(candidate.source_id(), SourceId::trng());
            assert_eq!(candidate.label(), "TrueRNG v1/v2/v3");
        }
        #[cfg(feature = "rdseed")]
        {
            assert_eq!(SourceCandidate::Rdseed.source_id(), SourceId::rdseed());
            assert_eq!(SourceCandidate::Rdseed.label(), "Intel RDSEED");
        }
        #[cfg(feature = "pseudo")]
        {
            assert_eq!(SourceCandidate::Pseudo.source_id(), SourceId::pseudo());
            assert_eq!(SourceCandidate::Pseudo.label(), "PseudoRNG");
        }
    }

    #[cfg(feature = "bitb")]
    #[test]
    fn bitb_candidate_maps_to_explicit_source_config() {
        use crate::SourceConfig;
        use rngkit_core::Fold;

        let candidate = SourceCandidate::Bitb {
            variant: "Black".into(),
            serial: "fake-serial".into(),
        };
        let fold = Fold::new(2).unwrap();
        let config = match candidate {
            SourceCandidate::Bitb { serial, .. } => SourceConfig::Bitb {
                fold,
                serial: Some(serial),
            },
            #[allow(unreachable_patterns)]
            _ => panic!("expected bitb candidate"),
        };
        match config {
            SourceConfig::Bitb { fold, serial } => {
                assert_eq!(fold.get(), 2);
                assert_eq!(serial.as_deref(), Some("fake-serial"));
            }
            #[allow(unreachable_patterns)]
            _ => panic!("expected bitb config"),
        }
    }

    #[cfg(feature = "trng3")]
    #[test]
    fn trng_candidate_maps_to_explicit_source_config() {
        use crate::SourceConfig;

        let candidate = SourceCandidate::Trng {
            port_name: "fake-port".into(),
        };
        let config = match candidate {
            SourceCandidate::Trng { port_name } => SourceConfig::Trng {
                path: Some(port_name),
            },
            #[allow(unreachable_patterns)]
            _ => panic!("expected trng candidate"),
        };
        match config {
            SourceConfig::Trng { path } => {
                assert_eq!(path.as_deref(), Some("fake-port"));
            }
            #[allow(unreachable_patterns)]
            _ => panic!("expected trng config"),
        }
    }
}
