use super::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
    CompilerIssuedPackageReviewSet, ExternalSourceContext, HistoricalPackagePolicyDecisions,
    HistoricalPackagePolicyLimits, LocalSourceLimits, PackageLock, PackageLockRecoveryLimits,
    PackageLockTarget, PackageSourceClosureLimits, ResolvedPackageSourceClosure, TARGET, Tree,
    compile_resolved_package_reviews, fs, package, resolve_external_local_project_closure,
};
use package_manager::resolution::graph::GitResolutionOptions;
use package_manager::review::SemanticBindingReview;

pub(super) fn resolve(tree: &Tree, label: &str) -> ResolvedPackageSourceClosure {
    let storage = tree.storage(&format!("{label}-cache"));
    resolve_external_local_project_closure(
        tree.path("sources/root"),
        ExternalSourceContext::derive(b"complete-package-policy-changes"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
        GitResolutionOptions::default(),
    )
    .unwrap()
}

pub(super) fn candidate(
    tree: &Tree,
    label: &str,
) -> (ResolvedPackageSourceClosure, CompilerIssuedPackageReviewSet) {
    let closure = resolve(tree, label);
    let reviews = compile_resolved_package_reviews(
        &closure.for_exact_target(TARGET),
        &tree.path(&format!("{label}-build")),
        SemanticBindingReview::Discover,
    )
    .unwrap();
    (closure, reviews)
}

pub(super) fn lock_from_reviews(
    closure: &ResolvedPackageSourceClosure,
    reviews: &CompilerIssuedPackageReviewSet,
) -> PackageLock {
    let source = CanonicalSourceClosureSubject::from_resolved(
        &closure.for_exact_target(TARGET),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap();
    let decisions = HistoricalPackagePolicyDecisions::recover_text(
        &format!(
            "omega-policy-decisions 1\nsource {}\ndecisions 0\nend\n",
            source.fingerprint().to_hex()
        ),
        &source,
        HistoricalPackagePolicyLimits::default(),
    )
    .unwrap();
    let mut policies = Vec::new();
    for package in source.packages() {
        for purpose in package_manager::declarations::DependencyPurpose::ALL {
            if let Some(review) = reviews.review_occurrence(package.key(), purpose) {
                policies.push((review.checked_context(), review.policy()));
            }
        }
    }
    let lock = PackageLock::from_targets(vec![
        PackageLockTarget::from_policies(source, &policies, decisions).unwrap(),
    ])
    .unwrap();
    PackageLock::recover_text(
        &lock.canonical_text().unwrap(),
        PackageLockRecoveryLimits::default(),
    )
    .unwrap()
}

pub(super) fn source(tree: &Tree, main: &str, build: &str) {
    package(&tree.path("sources/root"), "policy-fixture", build);
    fs::write(tree.path("sources/root/main.omg"), main).unwrap();
}
