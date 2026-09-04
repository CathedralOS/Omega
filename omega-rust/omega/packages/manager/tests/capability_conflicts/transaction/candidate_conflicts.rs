use super::fixture::ExactCompilerRowScenario;
use super::*;

pub(super) fn derive_and_assert(
    scenario: &ExactCompilerRowScenario,
) -> omega_package_manager::review::ReviewOnlyCapabilityConflictSet {
    assert_eq!(
        scenario.baseline_sources.graph().root(),
        scenario.candidate_sources.graph().root()
    );
    assert_ne!(
        scenario
            .baseline_sources
            .custody(scenario.baseline_sources.graph().root())
            .unwrap()
            .resolution(),
        scenario
            .candidate_sources
            .custody(scenario.candidate_sources.graph().root())
            .unwrap()
            .resolution()
    );

    let conflicts = compare_review_only_capabilities(
        &scenario.baseline_reviews,
        &scenario.candidate_reviews,
        &scenario
            .candidate_sources
            .for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare exact compiler rows");
    let baseline_capsule = ReviewOnlyBaselineCapsule::capture(
        &scenario.baseline_sources,
        &scenario.baseline_reviews,
        ReviewOnlyBaselineLimits::default(),
    )
    .expect("capture restart-stable review baseline");
    let baseline_bytes = baseline_capsule
        .encode(ReviewOnlyBaselineLimits::default())
        .expect("encode restart-stable review baseline");
    let recovered_baseline =
        ReviewOnlyBaselineCapsule::decode(&baseline_bytes, ReviewOnlyBaselineLimits::default())
            .expect("decode restart-stable review baseline");
    assert_eq!(
        recovered_baseline
            .encode(ReviewOnlyBaselineLimits::default())
            .expect("re-encode canonical review baseline"),
        baseline_bytes
    );
    let recovered_conflicts = compare_review_only_capabilities_from_baseline(
        &recovered_baseline,
        &scenario.candidate_reviews,
        &scenario
            .candidate_sources
            .for_exact_target(omega_target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare candidate with recovered baseline");
    assert_eq!(recovered_conflicts, conflicts);
    assert_eq!(
        triage_review_update_from_baseline(
            &recovered_baseline,
            &scenario.candidate_reviews,
            &scenario.candidate_sources,
            &BTreeSet::new(),
        ),
        triage_review_update(
            &scenario.baseline_reviews,
            &scenario.candidate_reviews,
            &BTreeSet::new()
        )
    );
    let mut corrupted_baseline = baseline_bytes.clone();
    corrupted_baseline[0] ^= 1;
    assert!(
        ReviewOnlyBaselineCapsule::decode(
            &corrupted_baseline,
            ReviewOnlyBaselineLimits::default(),
        )
        .is_err(),
        "corrupted restart capsule must reject"
    );
    let mut wrong_version = baseline_bytes.clone();
    let version_offset = b"OMEGA-PACKAGE-REVIEW-BASELINE\0".len();
    wrong_version[version_offset..version_offset + 2].copy_from_slice(&u16::MAX.to_le_bytes());
    let prefix_length = wrong_version.len() - 32;
    let mut checksum = Sha256::new();
    checksum.update(b"OMEGA-PACKAGE-REVIEW-BASELINE-CAPSULE\0");
    checksum.update((prefix_length as u64).to_le_bytes());
    checksum.update(&wrong_version[..prefix_length]);
    let checksum: [u8; 32] = checksum.finalize().into();
    wrong_version[prefix_length..].copy_from_slice(&checksum);
    assert_eq!(
        ReviewOnlyBaselineCapsule::decode(&wrong_version, ReviewOnlyBaselineLimits::default(),)
            .expect_err("checksummed stale capsule version must reject")
            .message(),
        "unsupported review baseline capsule header"
    );
    assert!(
        ReviewOnlyBaselineCapsule::decode(
            &baseline_bytes,
            ReviewOnlyBaselineLimits::new(
                baseline_bytes.len() - 1,
                1_024,
                16_384,
                128,
                4 * 1024,
                256,
                65_536,
                32 * 1024 * 1024,
            ),
        )
        .is_err(),
        "capsule byte ceiling must reject before parsing"
    );
    let tight_identity_limits = ReviewOnlyBaselineLimits::new(
        64 * 1024 * 1024,
        1_024,
        16_384,
        128,
        4,
        256,
        65_536,
        32 * 1024 * 1024,
    );
    assert!(
        baseline_capsule.encode(tight_identity_limits).is_err(),
        "encoding must enforce the same identity ceiling as decoding"
    );
    assert!(
        ReviewOnlyBaselineCapsule::decode(&baseline_bytes, tight_identity_limits).is_err(),
        "decoding must enforce the configured identity ceiling"
    );
    assert_eq!(conflicts.packages().len(), 1);
    assert_eq!(conflicts.conflict_count(), 2);
    let package = &conflicts.packages()[0];
    assert_eq!(package.key(), scenario.candidate_sources.graph().root());
    assert!(package.dependency_path().steps().is_empty());
    assert_ne!(package.candidate_closure().digest(), [0; 32]);
    let [conflict, second_conflict] = package.conflicts() else {
        panic!("two added public proposition rows")
    };
    assert_eq!(
        conflict.kind(),
        PackageReviewCanonicalRowKind::PublicProposition
    );
    assert_eq!(conflict.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert_eq!(conflict.change(), ReviewOnlyCapabilityConflictChange::Added);
    assert!(conflict.baseline_row().is_none());
    assert!(conflict.candidate_row().is_some());
    assert!(conflict.baseline_source().is_none());
    let candidate_locations = conflict
        .candidate_source()
        .and_then(PackageReviewCanonicalRowSource::authored_locations)
        .expect("added proposition has compiler-issued candidate source");
    assert_eq!(candidate_locations.len(), 1);
    assert_eq!(candidate_locations[0].relative_path(), "main.omg");
    assert!(conflict.is_blocking());
    assert_ne!(conflict.fingerprint().digest(), [0; 32]);
    assert_eq!(
        second_conflict.kind(),
        PackageReviewCanonicalRowKind::PublicProposition
    );
    assert!(second_conflict.is_blocking());

    conflicts
}
