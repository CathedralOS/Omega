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
    let mut decisions = Vec::new();
    if changes.root_role_change().is_some() {
        decisions.push(PackagePolicyDecision {
            subject: PackagePolicyDecisionSubject::RootRole,
            disposition,
        });
    }
    for replacement in changes.source_replacements() {
        decisions.push(PackagePolicyDecision {
            subject: PackagePolicyDecisionSubject::SourceReplacement(
                replacement.fingerprint().digest(),
            ),
            disposition,
        });
    }
    for package in changes.packages() {
        for row in package.rows().iter().filter(|row| row.requires_decision()) {
            decisions.push(PackagePolicyDecision {
                subject: PackagePolicyDecisionSubject::Row(row.fingerprint().digest()),
                disposition,
            });
        }
    }
    decisions
}

pub(super) fn resolution(
    changes: &PackagePolicyChangeSet,
    disposition: ReviewOnlyRootPolicyDisposition,
) -> PackagePolicyResolution {
    let resolution = resolve_package_policy_decisions(
        changes,
        changes.fingerprint().digest(),
        &decisions(changes, disposition),
    )
    .unwrap();
    resolution
}

pub(super) fn history_lock(
    sources: &ResolvedPackageSourceClosure,
    reviews: &CompilerIssuedPackageReviewSet,
    changes: &PackagePolicyChangeSet,
    resolved: &PackagePolicyResolution,
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
