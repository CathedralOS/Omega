use super::fixture::ExactCompilerRowScenario;
use super::*;

pub(super) fn derive_and_assert(
    scenario: &ExactCompilerRowScenario,
) -> package_manager::review::ReviewOnlyCapabilityConflictSet {
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
            .for_exact_target(target::TargetProfile::WindowsX64),
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare exact compiler rows");
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
