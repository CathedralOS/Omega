use super::*;

pub(super) fn compare(
    accepted: Option<&PackageLockTarget>,
    sources: &ResolvedPackageSourceClosure,
    reviews: &CompilerIssuedPackageReviewSet,
) -> PackagePolicyChangeSet {
    compare_package_policy_changes(
        accepted,
        reviews,
        &sources.for_exact_target(TARGET),
        PackagePolicyChangeLimits::default(),
    )
    .unwrap()
}

pub(super) fn decisions(
    changes: &PackagePolicyChangeSet,
    disposition: ReviewOnlyRootPolicyDisposition,
) -> Vec<PackagePolicyDecision> {
    changes
        .decision_obligations(PackagePolicyDecisionLimits::default())
        .unwrap()
        .iter()
        .map(|obligation| changes.policy_decision(obligation, disposition).unwrap())
        .collect()
}

pub(super) fn resolution(
    changes: &PackagePolicyChangeSet,
    disposition: ReviewOnlyRootPolicyDisposition,
) -> PackagePolicyDecisionResolution {
    let resolution = resolve_package_policy_decisions(
        changes,
        &decisions(changes, disposition),
        PackagePolicyDecisionLimits::default(),
    )
    .unwrap();
    let text = resolution
        .canonical_text(PackagePolicyDecisionLimits::default())
        .unwrap();
    let recovered =
        recover_package_policy_decisions(&text, changes, PackagePolicyDecisionLimits::default())
            .unwrap();
    assert_eq!(resolution, recovered);
    resolution
}

pub(super) fn history_lock(
    sources: &ResolvedPackageSourceClosure,
    reviews: &CompilerIssuedPackageReviewSet,
    changes: &PackagePolicyChangeSet,
    resolved: &PackagePolicyDecisionResolution,
) -> PackageLock {
    let source = CanonicalSourceClosureSubject::from_resolved(
        &sources.for_exact_target(TARGET),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap();
    let history = HistoricalPackagePolicyDecisions::capture_policy_changes(
        &source,
        changes,
        resolved,
        HistoricalPackagePolicyLimits::default(),
    )
    .unwrap();
    assert_eq!(history.version(), 2);
    assert_eq!(history.comparison(), Some(changes.fingerprint().digest()));
    assert_eq!(history.decisions().len(), resolved.decisions().len());
    let baselines = source
        .packages()
        .iter()
        .map(|source| reviews.review(source.key()).unwrap().policy().clone())
        .collect();
    let lock = PackageLock::from_targets(vec![
        PackageLockTarget::from_parts(source, baselines, history).unwrap(),
    ])
    .unwrap();
    let text = lock.canonical_text().unwrap();
    let recovered = PackageLock::recover_text(&text, PackageLockRecoveryLimits::default()).unwrap();
    assert_eq!(lock, recovered);
    assert_eq!(recovered.canonical_text().unwrap(), text);
    recovered
}
