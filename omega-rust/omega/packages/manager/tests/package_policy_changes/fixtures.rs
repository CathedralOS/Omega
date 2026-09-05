use super::*;

pub(super) fn resolve(tree: &Tree, label: &str) -> ResolvedPackageSourceClosure {
    let storage = tree.storage(&format!("{label}-cache"));
    resolve_external_local_project_closure_with_storage(
        tree.path("sources/root"),
        ExternalSourceContext::derive(b"complete-package-policy-changes"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .unwrap()
}

pub(super) fn candidate(
    tree: &Tree,
    label: &str,
) -> (ResolvedPackageSourceClosure, CompilerIssuedPackageReviewSet) {
    let closure = resolve(tree, label);
    let reviews = compile_resolved_package_candidate_reviews(
        &closure.for_exact_target(TARGET),
        &tree.path(&format!("{label}-build")),
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
    let baselines = source
        .packages()
        .iter()
        .map(|package| reviews.review(package.key()).unwrap().policy().clone())
        .collect();
    let lock = PackageLock::from_targets(vec![
        PackageLockTarget::from_parts(source, baselines, decisions).unwrap(),
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
