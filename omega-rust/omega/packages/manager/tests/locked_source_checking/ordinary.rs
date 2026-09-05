use super::*;
use omega_package_evidence::encoding::PackagePolicyTextRecoveryLimits;
use omega_package_evidence::record::PackagePolicyBaseline;
use omega_package_manager::resolution::graph::{
    ResolveLockedPackageClosureError, resolve_external_local_project_closure_with_storage,
};
use omega_package_manager::review::{
    CompileResolvedPackageReviewsError, LockedPolicyComparisonError,
    compare_locked_package_policies,
};

fn resolve(tree: &Tree, storage: &SourceResolverStorage) -> ResolvedPackageSourceClosure {
    resolve_external_local_project_closure_with_storage(
        tree.path("sources/root"),
        ExternalSourceContext::derive(b"locked-source-checking"),
        storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .unwrap()
}

fn fixture(tree: &Tree) -> (PackageLock, PackageRootSourceRequest) {
    package(
        &tree.path("sources/root"),
        "checking-root",
        concat!(
            " builder.depend_as(\"first\", Source::Path { location: \"../first\" });\n",
            " builder.depend_as(\"second\", Source::Path { location: \"../second\" });\n",
        ),
    );
    for directory in ["first", "second"] {
        package(&tree.path(&format!("sources/{directory}")), "same-name", "");
        fs::write(
            tree.path(&format!("sources/{directory}/main.omg")),
            "pub const LIMIT: u64 = 7;\n",
        )
        .unwrap();
    }
    let storage = tree.storage("accepted-cache");
    let closure = resolve(tree, &storage);
    capture_lock(&closure, &tree.path("accepted-build"))
}

fn roundtrip(target: PackageLockTarget) -> PackageLock {
    let lock = PackageLock::from_targets(vec![target]).unwrap();
    let text = lock.canonical_text().unwrap();
    PackageLock::recover_text(&text, PackageLockRecoveryLimits::default()).unwrap()
}

#[test]
fn fresh_reviews_report_only_the_exact_package_with_changed_retained_policy() {
    let tree = Tree::new();
    let (lock, request) = fixture(&tree);
    let original_text = lock.canonical_text().unwrap();
    let storage = tree.storage("checking-cache");
    let checked = check_locked_sources(
        &lock,
        TARGET,
        &request,
        &storage,
        LockedSourceRecoveryOptions::default(),
        &tree.path("checking-build"),
    )
    .unwrap();
    assert!(std::ptr::eq(
        checked.accepted(),
        lock.target(TARGET).unwrap()
    ));
    assert!(checked.changed_policies().is_empty());
    assert_fresh_matches(&lock, checked.source_closure());
    assert_eq!(checked.reviews().reviews().len(), 3);
    for (source, accepted) in checked
        .accepted()
        .source()
        .packages()
        .iter()
        .zip(checked.accepted().baselines())
    {
        let review = checked.reviews().review(source.key()).unwrap();
        assert_eq!(review.policy(), accepted);
        assert_eq!(review.resolution(), source.resolution());
    }

    let accepted = lock.target(TARGET).unwrap();
    let index = accepted
        .source()
        .packages()
        .iter()
        .position(|source| source.key().name().as_str() == "same-name")
        .unwrap();
    let key = accepted.source().packages()[index].key().clone();
    let mut baselines = accepted.baselines().to_vec();
    assert_eq!(baselines[index].public_consts().len(), 1);
    assert_eq!(
        baselines[index].public_consts()[0].identity().path(),
        "LIMIT"
    );
    let policy_text = baselines[index].canonical_text().unwrap();
    let original_name = "string \"LIMIT\"\n";
    assert_eq!(policy_text.matches(original_name).count(), 1);
    // This is an edited inert public API record, not forged compiler evidence.
    // Exact scalar replacement leaves every owner and unrelated field intact.
    let altered_text = policy_text.replace(original_name, "string \"RENAMED_LIMIT\"\n");
    baselines[index] = PackagePolicyBaseline::recover_text(
        &altered_text,
        PackagePolicyTextRecoveryLimits::default(),
    )
    .unwrap();
    assert_eq!(
        baselines[index].public_consts()[0].identity().path(),
        "RENAMED_LIMIT"
    );
    let altered = roundtrip(
        PackageLockTarget::from_parts(
            accepted.source().clone(),
            baselines,
            accepted.decisions().clone(),
        )
        .unwrap(),
    );
    let altered_text = altered.canonical_text().unwrap();
    let rechecked = check_locked_sources(
        &altered,
        TARGET,
        &request,
        &storage,
        LockedSourceRecoveryOptions::default(),
        &tree.path("rechecking-build"),
    )
    .unwrap();
    assert_eq!(rechecked.changed_policies(), &[key]);
    for review in checked.reviews().reviews() {
        assert_eq!(
            rechecked.reviews().review(review.key()).unwrap().policy(),
            review.policy()
        );
    }
    assert_eq!(altered.canonical_text().unwrap(), altered_text);
    assert_eq!(lock.canonical_text().unwrap(), original_text);
}

#[test]
fn target_and_root_request_errors_do_not_start_a_build() {
    let tree = Tree::new();
    let (lock, request) = fixture(&tree);
    let storage = tree.storage("checking-cache");
    fs::rename(tree.path("checking-cache"), tree.path("retired-cache")).unwrap();
    let build = tree.path("must-not-build");
    assert!(matches!(
        check_locked_sources(
            &lock,
            TargetProfile::LinuxArm64,
            &request,
            &storage,
            LockedSourceRecoveryOptions::default(),
            &build
        ),
        Err(CheckLockedSourcesError::Recovery(
            RecoverLockedSourcesError::MissingTarget {
                target: TargetProfile::LinuxArm64
            }
        ))
    ));
    let mut wrong_request = request.clone();
    let PackageRootSourceRequest::ExternalLocal { source_context, .. } = &mut wrong_request else {
        panic!("fixture root is external local");
    };
    *source_context = ExternalSourceContext::derive(b"different-checking-root");
    assert!(matches!(
        check_locked_sources(
            &lock,
            TARGET,
            &wrong_request,
            &storage,
            LockedSourceRecoveryOptions::default(),
            &build
        ),
        Err(CheckLockedSourcesError::Recovery(
            RecoverLockedSourcesError::Resolution(
                ResolveLockedPackageClosureError::RootRequestMismatch
            )
        ))
    ));
    assert!(!build.exists());
    assert!(!tree.path("checking-cache").exists());
}

#[test]
fn a_readable_baseline_does_not_suppress_current_compilation_failure() {
    let tree = Tree::new();
    let (accepted, _) = fixture(&tree);
    fs::write(
        tree.path("sources/root/main.omg"),
        "pub machine value() -> u64 { false }\n",
    )
    .unwrap();
    let storage = tree.storage("checking-cache");
    let closure = resolve(&tree, &storage);
    let request = closure.source_requests().root().request().clone();
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
    // A project can edit pins and retain stale analysis. Format recovery does
    // not certify that analysis; checking must still reject the invalid body.
    let lock = roundtrip(
        PackageLockTarget::from_parts(
            source,
            accepted.target(TARGET).unwrap().baselines().to_vec(),
            decisions,
        )
        .unwrap(),
    );
    let text = lock.canonical_text().unwrap();
    assert!(matches!(
        check_locked_sources(
            &lock,
            TARGET,
            &request,
            &storage,
            LockedSourceRecoveryOptions::default(),
            &tree.path("failed-build")
        ),
        Err(CheckLockedSourcesError::Compilation(
            CompileResolvedPackageReviewsError::Compilation { package, diagnostics }
        )) if package == *closure.graph().root() && !diagnostics.is_empty()
    ));
    assert_eq!(lock.canonical_text().unwrap(), text);
}

#[test]
fn independent_compiler_reviews_require_exact_target_resolution_and_coverage() {
    let tree = Tree::new();
    let (lock, _) = fixture(&tree);
    let accepted = lock.target(TARGET).unwrap();
    let storage = tree.storage("comparison-cache");
    let closure = resolve(&tree, &storage);
    let wrong_target = compile_resolved_package_reviews(
        &closure.for_exact_target(TargetProfile::LinuxArm64),
        &tree.path("wrong-target-build"),
    )
    .unwrap();
    assert_eq!(
        compare_locked_package_policies(accepted, &wrong_target),
        Err(LockedPolicyComparisonError::TargetMismatch {
            package: wrong_target.reviews()[0].key().clone()
        })
    );

    let leaf = resolve_external_local_project_closure_with_storage(
        tree.path("sources/first"),
        ExternalSourceContext::derive(b"locked-source-checking"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .unwrap();
    let only_leaf = compile_resolved_package_reviews(
        &leaf.for_exact_target(TARGET),
        &tree.path("only-leaf-build"),
    )
    .unwrap();
    assert_eq!(only_leaf.reviews().len(), 1);
    assert!(
        accepted
            .source()
            .packages()
            .iter()
            .any(|source| source.key() == leaf.graph().root())
    );
    let missing = accepted
        .source()
        .packages()
        .iter()
        .find(|source| source.key() != leaf.graph().root())
        .unwrap()
        .key();
    assert_eq!(
        compare_locked_package_policies(accepted, &only_leaf),
        Err(LockedPolicyComparisonError::MissingReview {
            package: missing.clone()
        })
    );

    fs::write(
        tree.path("sources/root/main.omg"),
        "pub machine value() -> u64 { 8 }\n",
    )
    .unwrap();
    let advanced = resolve(&tree, &storage);
    assert_eq!(advanced.graph().root(), closure.graph().root());
    let advanced_reviews = compile_resolved_package_reviews(
        &advanced.for_exact_target(TARGET),
        &tree.path("advanced-source-build"),
    )
    .unwrap();
    let index = accepted
        .source()
        .packages()
        .iter()
        .position(|source| source.key() == advanced.graph().root())
        .unwrap();
    assert_eq!(
        advanced_reviews
            .review(advanced.graph().root())
            .unwrap()
            .policy(),
        &accepted.baselines()[index],
        "this implementation body edit leaves normalized public policy unchanged"
    );
    assert_eq!(
        compare_locked_package_policies(accepted, &advanced_reviews),
        Err(LockedPolicyComparisonError::ResolutionMismatch {
            package: advanced.graph().root().clone()
        })
    );
}
