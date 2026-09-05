//! One source graph retains independently checked policy for exact targets.

use super::fixture::ExactCompilerRowScenario;
use super::historical_policy::resolve_all;
use package_manager::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLock, PackageLockError,
    PackageLockRecoveryLimits, PackageLockTarget,
};
use package_manager::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
};
use package_manager::review::{
    ReviewOnlyCapabilityConflictSet, ReviewOnlyRootPolicyDisposition,
    compile_resolved_package_reviews,
};
use target::TargetProfile;

fn empty_decisions(source: &CanonicalSourceClosureSubject) -> HistoricalPackagePolicyDecisions {
    HistoricalPackagePolicyDecisions::recover_text(
        &format!(
            "omega-policy-decisions 1\nsource {}\ndecisions 0\nend\n",
            source.fingerprint().to_hex()
        ),
        source,
        HistoricalPackagePolicyLimits::default(),
    )
    .unwrap()
}

pub(super) fn assert_complete_lock(
    scenario: &ExactCompilerRowScenario,
    conflicts: &ReviewOnlyCapabilityConflictSet,
) {
    let source_limits = CanonicalSourceClosureSubjectLimits::default();
    let windows_source = CanonicalSourceClosureSubject::from_resolved(
        &scenario
            .candidate_sources
            .for_exact_target(TargetProfile::WindowsX64),
        source_limits,
    )
    .unwrap();
    let resolution = resolve_all(
        conflicts,
        ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
    );
    let decisions = HistoricalPackagePolicyDecisions::capture(
        &windows_source,
        conflicts,
        Some(&resolution),
        HistoricalPackagePolicyLimits::default(),
    )
    .unwrap();
    let baselines = windows_source
        .packages()
        .iter()
        .map(|source| {
            scenario
                .candidate_reviews
                .review(source.key())
                .unwrap()
                .policy()
                .clone()
        })
        .collect::<Vec<_>>();
    let windows =
        PackageLockTarget::from_parts(windows_source.clone(), baselines.clone(), decisions.clone())
            .unwrap();
    assert_eq!(windows.baselines().len(), windows.source().packages().len());
    assert_eq!(windows.decisions(), &decisions);
    assert_eq!(
        PackageLockTarget::from_parts(windows_source.clone(), Vec::new(), decisions.clone()),
        Err(PackageLockError::BaselineCoverage),
    );

    // The second target is genuinely compiled from identical resolver custody;
    // no retained policy target marker is rewritten to fabricate this child.
    let linux_closure = scenario
        .candidate_sources
        .for_exact_target(TargetProfile::LinuxX64);
    let linux_reviews =
        compile_resolved_package_reviews(&linux_closure, &scenario.build_root).unwrap();
    let linux_source =
        CanonicalSourceClosureSubject::from_resolved(&linux_closure, source_limits).unwrap();
    assert!(windows_source.same_source_graph(&linux_source));
    assert_ne!(windows_source.fingerprint(), linux_source.fingerprint());
    let linux_baselines = linux_source
        .packages()
        .iter()
        .map(|source| linux_reviews.review(source.key()).unwrap().policy().clone())
        .collect::<Vec<_>>();
    let linux_decisions = empty_decisions(&linux_source);
    assert_eq!(
        PackageLockTarget::from_parts(linux_source.clone(), baselines, linux_decisions.clone()),
        Err(PackageLockError::TargetMismatch),
    );
    assert_eq!(
        PackageLockTarget::from_parts(linux_source.clone(), linux_baselines.clone(), decisions),
        Err(PackageLockError::DecisionSourceMismatch),
    );
    let linux =
        PackageLockTarget::from_parts(linux_source, linux_baselines, linux_decisions).unwrap();
    assert_eq!(
        PackageLock::from_targets(vec![]),
        Err(PackageLockError::EmptyTargets)
    );
    assert_eq!(
        PackageLock::from_targets(vec![windows.clone(), linux.clone()]),
        Err(PackageLockError::TargetOrder)
    );
    assert_eq!(
        PackageLock::from_targets(vec![windows.clone(), windows.clone()]),
        Err(PackageLockError::TargetOrder)
    );

    let old_source = CanonicalSourceClosureSubject::from_resolved(
        &scenario
            .baseline_sources
            .for_exact_target(TargetProfile::WindowsX64),
        source_limits,
    )
    .unwrap();
    let old_baselines = old_source
        .packages()
        .iter()
        .map(|source| {
            scenario
                .baseline_reviews
                .review(source.key())
                .unwrap()
                .policy()
                .clone()
        })
        .collect();
    let old_decisions = empty_decisions(&old_source);
    let old_windows =
        PackageLockTarget::from_parts(old_source, old_baselines, old_decisions).unwrap();
    assert_eq!(
        PackageLock::from_targets(vec![linux.clone(), old_windows]),
        Err(PackageLockError::SourceGraphMismatch)
    );

    let lock = PackageLock::from_targets(vec![linux, windows]).unwrap();
    assert_eq!(lock.targets().len(), 2);
    assert_eq!(
        lock.target(TargetProfile::WindowsX64)
            .unwrap()
            .decisions()
            .decisions()
            .len(),
        2
    );
    assert!(lock.target(TargetProfile::LinuxArm64).is_none());
    let text = lock.canonical_text().unwrap();
    let recovered = PackageLock::recover_text(&text, PackageLockRecoveryLimits::default()).unwrap();
    assert_eq!(recovered, lock);
    assert_eq!(recovered.canonical_text().unwrap(), text);
    super::lock_framing::assert_canonical_framing(&text);
    super::lock_framing::assert_aggregate_owned_boundary(&lock, &text);
    super::lock_membership_budget::assert_aggregate_identity_boundary(&lock, &text);
    for limits in [
        PackageLockRecoveryLimits {
            maximum_targets: 1,
            ..PackageLockRecoveryLimits::default()
        },
        PackageLockRecoveryLimits {
            maximum_packages: 1,
            ..PackageLockRecoveryLimits::default()
        },
        PackageLockRecoveryLimits {
            maximum_decisions: 1,
            ..PackageLockRecoveryLimits::default()
        },
    ] {
        assert!(lock.canonical_text_with_limits(limits).is_err());
    }
    assert!(
        PackageLock::recover_text(
            &text,
            PackageLockRecoveryLimits {
                maximum_targets: 1,
                ..PackageLockRecoveryLimits::default()
            }
        )
        .is_err()
    );
    assert!(
        PackageLock::recover_text(
            &text,
            PackageLockRecoveryLimits {
                maximum_packages: 1,
                ..PackageLockRecoveryLimits::default()
            }
        )
        .is_err()
    );
    assert!(
        PackageLock::recover_text(
            &text,
            PackageLockRecoveryLimits {
                maximum_decisions: 1,
                ..PackageLockRecoveryLimits::default()
            }
        )
        .is_err()
    );
}
