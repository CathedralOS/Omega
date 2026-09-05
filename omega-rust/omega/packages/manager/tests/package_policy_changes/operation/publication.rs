use super::*;
use package_evidence::encoding::PackagePolicyTextRecoveryLimits;
use package_evidence::record::PackagePolicyBaseline;
use package_manager::declarations::{
    BuildDependencyEditPlan, BuildFileReplacement, DependencySourceRequest,
    plan_dependency_addition,
};
use package_manager::operations::{
    LockedSourceRecoveryOptions, PackageFileTransaction, PackagePublicationLimits,
    PublishReviewedPackageChangeError, check_locked_sources, prepare_local_project_for_target,
    publish_reviewed_package_change, stage_build_dependency_edit,
};
use package_manager::resolution::graph::resolve_staged_external_local_project_closure_with_storage;
use package_source::SourceRelativePath;
use package_source::local::staging::{StagedLocalSnapshot, stage_local_source_replacement_in_lane};
use std::time::SystemTime;

fn addition(root: &Path, location: &str) -> BuildFileReplacement {
    let plan = plan_dependency_addition(
        root,
        &DependencySourceRequest::Path {
            explicit_alias: None,
            location: location.to_owned(),
        },
    )
    .unwrap();
    let BuildDependencyEditPlan::Automatic(replacement) = plan else {
        panic!("expected automatic edit: {plan:?}");
    };
    replacement
}

fn stage(tree: &Tree, replacement: &BuildFileReplacement) -> StagedLocalSnapshot {
    stage_build_dependency_edit(
        replacement,
        &tree.storage("publication-stage-cache"),
        LocalSourceLimits::default(),
    )
    .unwrap()
}

fn review_stage(
    tree: &Tree,
    staged: &StagedLocalSnapshot,
    target: TargetProfile,
    accepted: Option<&PackageLockTarget>,
) -> PackageChangeReview {
    let closure = resolve_staged_external_local_project_closure_with_storage(
        staged,
        ExternalSourceContext::derive(b"complete-package-policy-changes"),
        &tree.storage("publication-review-cache"),
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .unwrap();
    review_package_change(
        closure,
        target,
        accepted,
        &tree.path(&format!("publication-{}-build", target.target_name())),
    )
    .unwrap()
}

fn fixture(
    main: &str,
) -> (
    Tree,
    BuildFileReplacement,
    StagedLocalSnapshot,
    PackageChangeReview,
) {
    let tree = Tree::new();
    source(&tree, main, "");
    package(&tree.path("sources/dependency"), "dependency", "");
    fs::write(tree.path("sources/dependency/main.omg"), PURE).unwrap();
    let replacement = addition(&tree.path("sources/root"), "../dependency");
    let staged = stage(&tree, &replacement);
    let checked = review_stage(&tree, &staged, TARGET, None);
    (tree, replacement, staged, checked)
}

fn write_lock(tree: &Tree, targets: Vec<PackageLockTarget>) -> (PackageLock, String) {
    let lock = PackageLock::from_targets(targets).unwrap();
    let text = lock.canonical_text().unwrap();
    fs::write(tree.path("sources/root/omega.lock"), &text).unwrap();
    (lock, text)
}

// Include timestamps so even rewriting identical project bytes fails the check.
fn files(root: &Path) -> Vec<(PathBuf, Vec<u8>, SystemTime)> {
    fn visit(root: &Path, path: &Path, result: &mut Vec<(PathBuf, Vec<u8>, SystemTime)>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                visit(root, &path, result);
            } else {
                result.push((
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(&path).unwrap(),
                    fs::metadata(&path).unwrap().modified().unwrap(),
                ));
            }
        }
    }
    let mut result = Vec::new();
    visit(root, root, &mut result);
    result.sort_by(|left, right| left.0.cmp(&right.0));
    result
}

fn rejected_without_writes(
    root: &Path,
    replacement: &BuildFileReplacement,
    staged: &StagedLocalSnapshot,
    reviews: &[(&PackageChangeReview, &PackagePolicyResolution)],
    accepted: Option<&str>,
) -> PublishReviewedPackageChangeError {
    let mut transaction =
        PackageFileTransaction::open(root, PackagePublicationLimits::default()).unwrap();
    assert!(!transaction.has_pending().unwrap());
    let before = files(root);
    let error =
        publish_reviewed_package_change(&mut transaction, replacement, staged, reviews, accepted)
            .expect_err("publication must reject this input");
    assert_eq!(
        files(root),
        before,
        "rejection wrote project files: {error}"
    );
    assert!(!transaction.has_pending().unwrap());
    error
}

fn publish(
    root: &Path,
    replacement: &BuildFileReplacement,
    staged: &StagedLocalSnapshot,
    reviews: &[(&PackageChangeReview, &PackagePolicyResolution)],
    accepted: Option<&str>,
) -> PackageLock {
    let mut transaction =
        PackageFileTransaction::open(root, PackagePublicationLimits::default()).unwrap();
    let lock =
        publish_reviewed_package_change(&mut transaction, replacement, staged, reviews, accepted)
            .unwrap();
    assert!(!transaction.has_pending().unwrap());
    assert_eq!(
        fs::read_to_string(root.join("build.omg")).unwrap(),
        replacement.replacement_source()
    );
    assert_eq!(
        fs::read_to_string(root.join("omega.lock")).unwrap(),
        lock.canonical_text().unwrap()
    );
    lock
}

fn assert_locked(tree: &Tree, lock: &PackageLock, checked: &PackageChangeReview) {
    let target = checked.target();
    let fresh = check_locked_sources(
        lock,
        target,
        checked.source_closure().source_requests().root().request(),
        &tree.storage(&format!("landed-{}-cache", target.target_name())),
        LockedSourceRecoveryOptions::default(),
        &tree.path(&format!("landed-{}-build", target.target_name())),
    )
    .unwrap();
    assert!(fresh.changed_policies().is_empty());
    assert_eq!(
        fresh.source_closure().graph().root(),
        checked.source_closure().graph().root()
    );
    assert_eq!(
        CanonicalSourceClosureSubject::from_resolved(
            &fresh.source_closure().for_exact_target(target),
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .unwrap(),
        *lock.target(target).unwrap().source()
    );
}

#[test]
fn planned_publication_preserves_original_lineage_and_landed_source_pin() {
    let (tree, replacement, staged, checked) = fixture(PURE);
    let original = review(&tree, "original", None);
    assert_eq!(
        original.source_closure().graph().root(),
        checked.source_closure().graph().root()
    );
    assert_eq!(
        original.source_closure().source_requests().root().request(),
        checked.source_closure().source_requests().root().request()
    );
    assert!(staged.verify_live_source_unchanged().is_ok());
    let lock = publish(
        &tree.path("sources/root"),
        &replacement,
        &staged,
        &[(&checked, &decisions(&checked, "accept"))],
        None,
    );
    assert_eq!(lock.target(TARGET).unwrap(), &propose(&checked));
    assert_locked(&tree, &lock, &checked);
    assert!(staged.verify_live_source_unchanged().is_err());
}

#[test]
fn executable_build_publication_preserves_mode_and_landed_source_pin() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let (tree, replacement, _, _) = fixture(PURE);
        let root = tree.path("sources/root");
        fs::set_permissions(root.join("build.omg"), fs::Permissions::from_mode(0o751)).unwrap();
        let staged = stage(&tree, &replacement);
        let checked = review_stage(&tree, &staged, TARGET, None);
        let lock = publish(
            &root,
            &replacement,
            &staged,
            &[(&checked, &decisions(&checked, "accept"))],
            None,
        );
        assert_eq!(
            fs::metadata(root.join("build.omg"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o751
        );
        assert_locked(&tree, &lock, &checked);
    }
    #[cfg(not(unix))]
    eprintln!("SKIP: executable build-file modes require Unix");
}

#[test]
fn unstaged_review_and_different_candidate_stage_reject_without_writes() {
    let (tree, replacement, staged, checked) = fixture(PURE);
    let root = tree.path("sources/root");
    let original = review(&tree, "unstaged", None);
    assert!(matches!(
        rejected_without_writes(
            &root,
            &replacement,
            &staged,
            &[(&original, &decisions(&original, "accept"))],
            None,
        ),
        PublishReviewedPackageChangeError::Association(_)
    ));
    package(&tree.path("sources/other"), "other", "");
    let other_replacement = addition(&root, "../other");
    let other_stage = stage(&tree, &other_replacement);
    assert!(matches!(
        rejected_without_writes(
            &root,
            &replacement,
            &other_stage,
            &[(&checked, &decisions(&checked, "accept"))],
            None,
        ),
        PublishReviewedPackageChangeError::Association(_)
    ));
    let other_review = review_stage(&tree, &other_stage, TARGET, None);
    assert!(matches!(
        rejected_without_writes(
            &root,
            &replacement,
            &staged,
            &[(&other_review, &decisions(&other_review, "accept"))],
            None,
        ),
        PublishReviewedPackageChangeError::Association(_)
    ));
}

#[test]
fn non_build_stage_with_matching_edit_digests_rejects_without_writes() {
    let (tree, replacement, _, checked) = fixture(PURE);
    let root = tree.path("sources/root");
    fs::copy(root.join("build.omg"), root.join("other.omg")).unwrap();
    let staged = stage_local_source_replacement_in_lane(
        &root,
        &SourceRelativePath::parse("other.omg").unwrap(),
        replacement.expected_sha256(),
        replacement.replacement_source().as_bytes(),
        tree.storage("non-build-cache").external_local_sources(),
        LocalSourceLimits::default(),
    )
    .unwrap();
    assert_eq!(staged.expected_sha256(), replacement.expected_sha256());
    assert!(matches!(
        rejected_without_writes(
            &root,
            &replacement,
            &staged,
            &[(&checked, &decisions(&checked, "accept"))],
            None,
        ),
        PublishReviewedPackageChangeError::Association(_)
    ));
}

#[test]
fn identical_edit_bytes_from_another_project_and_wrong_transaction_root_reject() {
    let (tree, replacement, staged, checked) = fixture(PURE);
    let root = tree.path("sources/root");
    let other = tree.path("sources/other-root");
    package(&other, "policy-fixture", "");
    fs::write(other.join("main.omg"), PURE).unwrap();
    let other_replacement = addition(&other, "../dependency");
    assert_eq!(
        replacement.expected_sha256(),
        other_replacement.expected_sha256()
    );
    assert_eq!(
        replacement.replacement_source(),
        other_replacement.replacement_source()
    );
    let other_before = files(&other);
    assert!(matches!(
        rejected_without_writes(
            &root,
            &other_replacement,
            &staged,
            &[(&checked, &decisions(&checked, "accept"))],
            None,
        ),
        PublishReviewedPackageChangeError::Association(_)
    ));
    assert_eq!(files(&other), other_before);
    let other_stage = stage(&tree, &other_replacement);
    assert_eq!(
        staged.normalized().content_identity,
        other_stage.normalized().content_identity
    );
    let other_review = review_stage(&tree, &other_stage, TARGET, None);
    assert!(matches!(
        rejected_without_writes(
            &root,
            &replacement,
            &staged,
            &[(&other_review, &decisions(&other_review, "accept"))],
            None,
        ),
        PublishReviewedPackageChangeError::Association(_)
    ));
    assert_eq!(files(&other), other_before);
    let root_before = files(&root);
    assert!(matches!(
        rejected_without_writes(
            &other,
            &replacement,
            &staged,
            &[(&checked, &decisions(&checked, "accept"))],
            None,
        ),
        PublishReviewedPackageChangeError::Association(_)
    ));
    assert_eq!(files(&root), root_before);
}

#[test]
fn edited_lock_and_changed_policy_with_the_same_source_pin_reject_stale_review() {
    let (tree, replacement, staged, _) = fixture(PURE);
    let accepted = propose(&review(&tree, "accepted", None));
    let (_, accepted_text) = write_lock(&tree, vec![accepted.clone()]);
    let checked = review_stage(&tree, &staged, TARGET, Some(&accepted));
    let choices = decisions(&checked, "accept");
    let mut baselines = accepted.baselines().to_vec();
    let text = baselines[0].canonical_text().unwrap();
    assert!(text.contains("string \"VALUE\"\n"));
    baselines[0] = PackagePolicyBaseline::recover_text(
        &text.replace("string \"VALUE\"\n", "string \"RENAMED_VALUE\"\n"),
        PackagePolicyTextRecoveryLimits::default(),
    )
    .unwrap();
    let edited = PackageLockTarget::from_parts(
        accepted.source().clone(),
        baselines,
        accepted.decisions().clone(),
    )
    .unwrap();
    assert_eq!(edited.source(), accepted.source());
    assert_ne!(edited.baselines(), accepted.baselines());
    let (_, edited_text) = write_lock(&tree, vec![edited]);
    for supplied in [&accepted_text, &edited_text] {
        assert!(matches!(
            rejected_without_writes(
                &tree.path("sources/root"),
                &replacement,
                &staged,
                &[(&checked, &choices)],
                Some(supplied),
            ),
            PublishReviewedPackageChangeError::Association(_)
        ));
    }
}

#[test]
fn rejected_and_stale_decisions_never_publish() {
    let (tree, replacement, staged, checked) = fixture(ASSUMPTION);
    let root = tree.path("sources/root");
    assert!(checked.changes().requires_decision());
    assert!(matches!(
        rejected_without_writes(
            &root,
            &replacement,
            &staged,
            &[(&checked, &decisions(&checked, "reject"))],
            None,
        ),
        PublishReviewedPackageChangeError::Review(PackageChangeError::RejectedChanges)
    ));
    let stale_choices = decisions(&checked, "accept");
    fs::write(
        root.join("main.omg"),
        format!("// source-only edit\n{ASSUMPTION}"),
    )
    .unwrap();
    let new_stage = stage(&tree, &replacement);
    let new_review = review_stage(&tree, &new_stage, TARGET, None);
    assert_ne!(
        checked.changes().fingerprint(),
        new_review.changes().fingerprint()
    );
    assert_eq!(
        checked.reviews().reviews()[0].policy(),
        new_review.reviews().reviews()[0].policy()
    );
    assert!(matches!(
        rejected_without_writes(
            &root,
            &replacement,
            &new_stage,
            &[(&new_review, &stale_choices)],
            None,
        ),
        PublishReviewedPackageChangeError::Review(PackageChangeError::Decisions(
            HistoricalPackagePolicyError::ResolutionMismatch
        ))
    ));
}

#[test]
fn concurrent_build_and_other_source_edits_are_preserved_on_rejection() {
    for name in ["build.omg", "main.omg"] {
        let (tree, replacement, staged, checked) = fixture(PURE);
        let root = tree.path("sources/root");
        let path = root.join(name);
        fs::write(
            &path,
            format!("// concurrent edit\n{}", fs::read_to_string(&path).unwrap()),
        )
        .unwrap();
        let error = rejected_without_writes(
            &root,
            &replacement,
            &staged,
            &[(&checked, &decisions(&checked, "accept"))],
            None,
        );
        match name {
            "build.omg" => assert!(matches!(
                error,
                PublishReviewedPackageChangeError::Association(_)
            )),
            _ => assert!(matches!(
                error,
                PublishReviewedPackageChangeError::Source(_)
            )),
        }
    }
}

#[test]
fn changed_path_dependency_rejects_without_publishing_stale_local_pins() {
    let (tree, replacement, staged, _) = fixture(PURE);
    let accepted = propose(&review(&tree, "accepted", None));
    let (_, accepted_text) = write_lock(&tree, vec![accepted.clone()]);
    let checked = review_stage(&tree, &staged, TARGET, Some(&accepted));
    let choices = decisions(&checked, "accept");
    let dependency = tree.path("sources/dependency");
    fs::write(dependency.join("main.omg"), "pub const VALUE: u64 = 99;\n").unwrap();
    let dependency_before = files(&dependency);
    assert!(matches!(
        rejected_without_writes(
            &tree.path("sources/root"),
            &replacement,
            &staged,
            &[(&checked, &choices)],
            Some(&accepted_text),
        ),
        PublishReviewedPackageChangeError::Source(_)
    ));
    assert_eq!(files(&dependency), dependency_before);
}

#[test]
fn empty_and_duplicate_target_reviews_reject_without_writes() {
    let (tree, replacement, staged, checked) = fixture(PURE);
    let root = tree.path("sources/root");
    assert!(matches!(
        rejected_without_writes(&root, &replacement, &staged, &[], None),
        PublishReviewedPackageChangeError::Association(_)
    ));
    let choices = decisions(&checked, "accept");
    assert!(matches!(
        rejected_without_writes(
            &root,
            &replacement,
            &staged,
            &[(&checked, &choices), (&checked, &choices)],
            None,
        ),
        PublishReviewedPackageChangeError::Lock(_)
    ));
}

#[test]
fn retained_targets_cannot_be_dropped_and_both_reviewed_targets_publish() {
    let (tree, replacement, staged, _) = fixture(PURE);
    let windows = propose(&review(&tree, "accepted-windows", None));
    let linux = propose(
        &review_package_change(
            resolve(&tree, "accepted-linux"),
            TargetProfile::LinuxArm64,
            None,
            &tree.path("accepted-linux-build"),
        )
        .unwrap(),
    );
    let (_, accepted_text) = write_lock(&tree, vec![linux.clone(), windows.clone()]);
    let windows_review = review_stage(&tree, &staged, TARGET, Some(&windows));
    let linux_review = review_stage(&tree, &staged, TargetProfile::LinuxArm64, Some(&linux));
    let windows_choices = decisions(&windows_review, "accept");
    let linux_choices = decisions(&linux_review, "accept");
    let root = tree.path("sources/root");
    assert!(matches!(
        rejected_without_writes(
            &root,
            &replacement,
            &staged,
            &[(&windows_review, &windows_choices)],
            Some(&accepted_text),
        ),
        PublishReviewedPackageChangeError::Association(_)
    ));
    let lock = publish(
        &root,
        &replacement,
        &staged,
        &[
            (&linux_review, &linux_choices),
            (&windows_review, &windows_choices),
        ],
        Some(&accepted_text),
    );
    assert_eq!(lock.targets().len(), 2);
    for checked in [&windows_review, &linux_review] {
        assert_eq!(lock.target(checked.target()).unwrap(), &propose(checked));
        assert_locked(&tree, &lock, checked);
    }
}

#[test]
fn preparation_recovers_pending_declarations_before_compiler_input_resolution() {
    let tree = Tree::new();
    source(&tree, PURE, "");
    let root = tree.path("sources/root");
    let after_build = fs::read_to_string(root.join("build.omg")).unwrap();
    let storage = SourceResolverStorage::for_hardened_base(tree.path("recovered-cache")).unwrap();
    let closure =
        package_manager::resolution::graph::resolve_external_local_project_closure_with_storage(
            &root,
            ExternalSourceContext::derive(b"omega-local-project-v1"),
            &storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .unwrap();
    let reviewed =
        review_package_change(closure, TARGET, None, &tree.path("recovered-build")).unwrap();
    let after_lock = PackageLock::from_targets(vec![propose(&reviewed)])
        .unwrap()
        .canonical_text()
        .unwrap();
    let before_build = "this is not a valid build declaration\n";
    fs::write(root.join("build.omg"), before_build).unwrap();
    let transaction =
        PackageFileTransaction::open(&root, PackagePublicationLimits::default()).unwrap();
    // A recorded intent, before either accepted file was replaced. The raw
    // transaction tests own fault injection; this fixture pins prepare ordering.
    let journal = format!(
        "omega-package-transaction 1\nbefore-build {}\n{}\nafter-build {}\n{}\nbefore-lock absent\nafter-lock {}\n{}\n",
        before_build.len(),
        before_build,
        after_build.len(),
        after_build,
        after_lock.len(),
        after_lock,
    );
    fs::write(root.join("build/package-manager/pending"), journal).unwrap();
    assert!(transaction.has_pending().unwrap());
    drop(transaction);
    let prepared = prepare_local_project_for_target(&root.join("main.omg"), TARGET)
        .unwrap()
        .unwrap();
    assert_eq!(
        fs::read_to_string(root.join("build.omg")).unwrap(),
        after_build
    );
    assert_eq!(
        fs::read_to_string(root.join("omega.lock")).unwrap(),
        after_lock
    );
    let transaction =
        PackageFileTransaction::open(&root, PackagePublicationLimits::default()).unwrap();
    assert!(!transaction.has_pending().unwrap());
    drop(transaction);
    let (entry, inputs) = prepared.into_parts();
    assert_ne!(entry, root.join("main.omg"));
    assert_eq!(inputs.packages().count(), 1);
    compiler::compile_to_checked_with_packages_in_build_dir(
        &entry,
        &tree.path("recovered-compilation"),
        Some(TARGET.target_name()),
        inputs,
    )
    .unwrap_or_else(|diagnostics| panic!("recovered project must compile: {diagnostics:#?}"));
}
