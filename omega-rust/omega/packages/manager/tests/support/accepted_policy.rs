//! Explicit project acceptance fixtures using the ordinary policy workflow.

use package_manager::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLockTarget,
};
use package_manager::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
    ExactTargetPackageSourceClosure,
};
use package_manager::review::{
    CompilerIssuedPackageReviewSet, PackagePolicyChangeLimits, PackagePolicyDecision,
    PackagePolicyDecisionSubject, ReviewOnlyRootPolicyDisposition, compare_package_policy_changes,
    resolve_package_policy_decisions,
};

pub fn accepted_policy(
    target: &ExactTargetPackageSourceClosure<'_>,
    reviews: &CompilerIssuedPackageReviewSet,
) -> PackageLockTarget {
    let changes =
        compare_package_policy_changes(None, reviews, target, PackagePolicyChangeLimits::default())
            .expect("compare explicit initial project acceptance");
    let decisions = changes
        .packages()
        .iter()
        .flat_map(|package| package.rows())
        .filter(|row| row.requires_decision())
        .map(|row| PackagePolicyDecision {
            subject: PackagePolicyDecisionSubject::Row(row.fingerprint().digest()),
            disposition: ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
        })
        .collect::<Vec<_>>();
    assert!(changes.root_role_change().is_none());
    assert!(changes.source_replacements().is_empty());
    let resolution =
        resolve_package_policy_decisions(&changes, changes.fingerprint().digest(), &decisions)
            .expect("resolve explicit project choices");
    assert!(resolution.all_required_changes_accepted());
    let source = CanonicalSourceClosureSubject::from_resolved(
        target,
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .expect("capture accepted source subject");
    let baselines = source
        .packages()
        .iter()
        .map(|package| {
            reviews
                .review(package.key())
                .expect("accepted package review")
                .policy()
                .clone()
        })
        .collect();
    let history = HistoricalPackagePolicyDecisions::capture_policy(
        &source,
        &changes,
        &resolution,
        HistoricalPackagePolicyLimits::default(),
    )
    .expect("retain explicit project choices");
    PackageLockTarget::from_parts(source, baselines, history).expect("accepted project policy")
}
