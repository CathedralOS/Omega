use super::*;
use omega_package_manager::declarations::DependencySourceRequest;
use omega_package_manager::declarations::{
    BuildDependencyEditPlan, BuildFileReplacement, plan_dependency_addition,
    plan_dependency_replacement,
};
use omega_package_manager::operations::{
    LockedSourceRecoveryOptions, check_locked_sources, stage_build_dependency_edit,
};
use omega_package_manager::resolution::graph::resolve_staged_external_local_project_closure_with_storage;
use omega_package_source::SourceResolveError;
use omega_package_source::local::staging::StagedLocalSnapshot;

fn request(location: &str) -> DependencySourceRequest {
    DependencySourceRequest::Path {
        explicit_alias: None,
        location: location.to_owned(),
    }
}

fn automatic(plan: BuildDependencyEditPlan) -> BuildFileReplacement {
    let BuildDependencyEditPlan::Automatic(replacement) = plan else {
        panic!("expected an automatic dependency edit: {plan:?}")
    };
    replacement
}

fn staged_review(
    tree: &Tree,
    replacement: &BuildFileReplacement,
    accepted: Option<&PackageLockTarget>,
) -> (StagedLocalSnapshot, PackageChangeReview) {
    let storage = tree.storage("staged-cache");
    let staged =
        stage_build_dependency_edit(replacement, &storage, LocalSourceLimits::default()).unwrap();
    let closure = resolve_staged_external_local_project_closure_with_storage(
        &staged,
        ExternalSourceContext::derive(b"complete-package-policy-changes"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .unwrap();
    let review =
        review_package_change(closure, TARGET, accepted, &tree.path("staged-build")).unwrap();
    (staged, review)
}

#[test]
fn planned_addition_checks_imports_before_live_edit_and_matches_landed_lock() {
    let tree = Tree::new();
    source(&tree, PURE, "");
    let initial = review(&tree, "initial", None);
    let accepted = propose(&initial);
    let root = tree.path("sources/root");
    package(&tree.path("sources/arithmetic"), "arithmetic-kernels", "");
    fs::write(
        root.join("main.omg"),
        "use arithmetic_kernels::main;\npub machine calculate() -> u64 { value() }\n",
    )
    .unwrap();
    let original_build = fs::read(root.join("build.omg")).unwrap();
    let original_lock = PackageLock::from_targets(vec![accepted.clone()]).unwrap();
    fs::write(
        root.join("omega.lock"),
        original_lock.canonical_text().unwrap(),
    )
    .unwrap();
    let replacement =
        automatic(plan_dependency_addition(&root, &request("../arithmetic")).unwrap());
    let (staged, checked) = staged_review(&tree, &replacement, Some(&accepted));
    assert_eq!(
        checked.source_closure().graph().root(),
        initial.source_closure().graph().root()
    );
    assert_eq!(
        checked.source_closure().source_requests().root().request(),
        initial.source_closure().source_requests().root().request()
    );
    assert_eq!(checked.reviews().reviews().len(), 2);
    assert_eq!(fs::read(root.join("build.omg")).unwrap(), original_build);
    assert_eq!(
        fs::read_to_string(root.join("omega.lock")).unwrap(),
        original_lock.canonical_text().unwrap()
    );
    staged.verify_live_source_unchanged().unwrap();
    let proposed = propose(&checked);
    let lock = PackageLock::from_targets(vec![proposed]).unwrap();

    // Simulate landing the reviewed bytes; this is not the file transaction.
    fs::write(replacement.build_path(), replacement.replacement_source()).unwrap();
    fs::write(root.join("omega.lock"), lock.canonical_text().unwrap()).unwrap();
    let fresh = check_locked_sources(
        &lock,
        TARGET,
        checked.source_closure().source_requests().root().request(),
        &tree.storage("landed-cache"),
        LockedSourceRecoveryOptions::default(),
        &tree.path("landed-build"),
    )
    .unwrap();
    assert!(fresh.changed_policies().is_empty());
    assert_fresh_matches(&lock, fresh.source_closure());
    assert!(staged.verify_live_source_unchanged().is_err());
}

#[test]
fn staged_new_assumptions_require_decisions_without_changing_project_files() {
    let tree = Tree::new();
    source(&tree, PURE, "");
    let root = tree.path("sources/root");
    package(&tree.path("sources/device"), "device-access", "");
    fs::write(tree.path("sources/device/main.omg"), ASSUMPTION).unwrap();
    let original = fs::read(root.join("build.omg")).unwrap();
    let replacement = automatic(plan_dependency_addition(&root, &request("../device")).unwrap());
    let (staged, checked) = staged_review(&tree, &replacement, None);
    assert!(checked.changes().requires_decision());
    let document = render_package_policy_review(checked.changes(), MAXIMUM_DOCUMENT_BYTES).unwrap();
    assert!(matches!(
        recover_package_policy_review(checked.changes(), &document, MAXIMUM_DOCUMENT_BYTES),
        Err(PackagePolicyReviewError::UnresolvedDecision(_))
    ));
    assert!(matches!(
        checked.propose_lock_target(&decisions(&checked, "reject")),
        Err(PackageChangeError::RejectedChanges)
    ));
    assert_eq!(propose(&checked).baselines().len(), 2);
    assert_eq!(fs::read(root.join("build.omg")).unwrap(), original);
    assert!(!root.join("omega.lock").exists());
    staged.verify_live_source_unchanged().unwrap();
}

#[test]
fn stale_planner_bytes_reject_without_overwriting_a_concurrent_edit() {
    let tree = Tree::new();
    source(&tree, PURE, "");
    let root = tree.path("sources/root");
    let replacement =
        automatic(plan_dependency_addition(&root, &request("../arithmetic")).unwrap());
    let concurrent = format!(
        "// concurrent edit\n{}",
        fs::read_to_string(root.join("build.omg")).unwrap()
    );
    fs::write(root.join("build.omg"), &concurrent).unwrap();
    assert!(matches!(
        stage_build_dependency_edit(
            &replacement,
            &tree.storage("stale-cache"),
            LocalSourceLimits::default()
        ),
        Err(SourceResolveError::LocalSourceChanged { .. })
    ));
    assert_eq!(
        fs::read_to_string(root.join("build.omg")).unwrap(),
        concurrent
    );
    assert!(!root.join("omega.lock").exists());
}

#[test]
fn planned_source_replacement_reviews_candidate_when_old_checkout_is_unavailable() {
    let tree = Tree::new();
    source(
        &tree,
        PURE,
        " builder.depend(Source::Path { location: \"../old\" });\n",
    );
    package(&tree.path("sources/old"), "arithmetic-kernels", "");
    package(&tree.path("sources/new"), "arithmetic-kernels", "");
    let initial = review(&tree, "initial", None);
    let accepted = propose(&initial);
    fs::rename(tree.path("sources/old"), tree.path("unavailable-old")).unwrap();
    let root = tree.path("sources/root");
    let original_build = fs::read(root.join("build.omg")).unwrap();
    let replacement = automatic(
        plan_dependency_replacement(&root, &request("../old"), &request("../new")).unwrap(),
    );
    let (staged, checked) = staged_review(&tree, &replacement, Some(&accepted));
    assert_eq!(checked.changes().source_replacements().len(), 1);
    assert!(checked.changes().requires_decision());
    assert_eq!(propose(&checked).baselines().len(), 2);
    assert_eq!(fs::read(root.join("build.omg")).unwrap(), original_build);
    assert!(!root.join("omega.lock").exists());
    staged.verify_live_source_unchanged().unwrap();
}
