use super::*;
use package_manager::lock::HistoricalPackagePolicyError;
use package_manager::operations::{PackageChangeError, PackageChangeReview, review_package_change};
use package_manager::review::{
    CompileResolvedPackageReviewsError, PackagePolicyChangeError, PackagePolicyResolution,
    PackagePolicyReviewError, recover_package_policy_review, render_package_policy_review,
};

#[path = "operation/commands.rs"]
mod commands;
#[path = "operation/lock_file.rs"]
mod lock_file;
#[path = "operation/publication.rs"]
mod publication;
#[path = "operation/semantic.rs"]
mod semantic;
#[path = "operation/staging.rs"]
mod staging;
#[path = "operation/transitive.rs"]
mod transitive;

const MAXIMUM_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;
const PURE: &str = "pub const VALUE: u64 = 7;\n";
const ASSUMPTION: &str = "boundary machine trusted_zero() -> u64 ensures result == 0;\n";

fn review(tree: &Tree, label: &str, accepted: Option<&PackageLockTarget>) -> PackageChangeReview {
    review_package_change(
        resolve(tree, label),
        TARGET,
        accepted,
        &tree.path(&format!("{label}-build")),
    )
    .unwrap()
}

fn decisions(review: &PackageChangeReview, choice: &str) -> PackagePolicyResolution {
    let template = render_package_policy_review(review.changes(), MAXIMUM_DOCUMENT_BYTES).unwrap();
    // Edit only decision lines, leaving the compiler findings byte-identical.
    let document: String = template
        .split_inclusive('\n')
        .map(|line| {
            if line.starts_with("decision ") {
                line.replace(" pending\n", &format!(" {choice}\n"))
            } else {
                line.to_owned()
            }
        })
        .collect();
    recover_package_policy_review(review.changes(), &document, MAXIMUM_DOCUMENT_BYTES).unwrap()
}

fn propose(review: &PackageChangeReview) -> PackageLockTarget {
    review
        .propose_lock_target(&decisions(review, "accept"))
        .unwrap()
}

fn assert_round_trip(review: &PackageChangeReview, proposed: PackageLockTarget) {
    let source = CanonicalSourceClosureSubject::from_resolved(
        &review.source_closure().for_exact_target(review.target()),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap();
    assert_eq!(proposed.source(), &source);
    assert_eq!(proposed.target(), review.target());
    assert_eq!(proposed.baselines().len(), review.reviews().reviews().len());
    for (baseline, package) in proposed.baselines().iter().zip(source.packages()) {
        assert_eq!(
            baseline,
            review.reviews().review(package.key()).unwrap().policy()
        );
    }
    assert_eq!(
        proposed.decisions().comparison(),
        Some(review.changes().fingerprint().digest())
    );
    let lock = PackageLock::from_targets(vec![proposed]).unwrap();
    let text = lock.canonical_text().unwrap();
    let recovered = PackageLock::recover_text(&text, PackageLockRecoveryLimits::default()).unwrap();
    assert_eq!(recovered, lock);
    assert_eq!(recovered.canonical_text().unwrap(), text);
}

#[test]
fn pure_initial_proposal_round_trips_without_publishing_project_files() {
    let tree = Tree::new();
    source(&tree, PURE, "");
    let build = fs::read(tree.path("sources/root/build.omg")).unwrap();
    let lock_path = tree.path("sources/root/omega.lock");
    let initial = review(&tree, "pure", None);
    assert_eq!(initial.target(), TARGET);
    assert!(initial.changes().baseline_source_subject().is_none());
    assert!(!initial.changes().requires_decision());
    assert!(decisions(&initial, "accept").decisions().is_empty());
    assert_round_trip(&initial, propose(&initial));
    assert!(!lock_path.exists());
    assert_eq!(
        fs::read(tree.path("sources/root/build.omg")).unwrap(),
        build
    );
    assert_eq!(fs::read_dir(tree.path("pure-build")).unwrap().count(), 0);

    fs::write(&lock_path, b"existing project lock\n").unwrap();
    assert_round_trip(&initial, propose(&initial));
    assert_eq!(fs::read(lock_path).unwrap(), b"existing project lock\n");
    assert_eq!(
        fs::read(tree.path("sources/root/build.omg")).unwrap(),
        build
    );
}

#[test]
fn assumptions_remain_pending_until_explicit_choices_and_rejection_prevents_proposal() {
    let tree = Tree::new();
    source(&tree, ASSUMPTION, "");
    let initial = review(&tree, "assumption", None);
    let root = initial.source_closure().graph().root();
    assert_eq!(
        initial
            .reviews()
            .review(root)
            .unwrap()
            .obligation_results()
            .open_accepted_claims()
            .len(),
        1
    );
    assert!(initial.changes().requires_decision());
    let template = render_package_policy_review(initial.changes(), MAXIMUM_DOCUMENT_BYTES).unwrap();
    assert!(matches!(
        recover_package_policy_review(initial.changes(), &template, MAXIMUM_DOCUMENT_BYTES),
        Err(PackagePolicyReviewError::UnresolvedDecision(_))
    ));
    let rejected = decisions(&initial, "reject");
    assert!(matches!(
        initial.propose_lock_target(&rejected),
        Err(PackageChangeError::RejectedChanges)
    ));
    let accepted = decisions(&initial, "accept");
    assert!(!accepted.decisions().is_empty());
    let proposed = initial.propose_lock_target(&accepted).unwrap();
    assert_eq!(
        proposed.decisions().decisions().len(),
        accepted.decisions().len()
    );
    assert_round_trip(&initial, proposed);
    assert!(!tree.path("sources/root/omega.lock").exists());
}

#[test]
fn updates_use_retained_baselines_after_old_sources_disappear() {
    let tree = Tree::new();
    source(
        &tree,
        PURE,
        " builder.depend(Source::Path { location: \"../old\" });\n",
    );
    package(&tree.path("sources/old"), "removed-package", "");
    let accepted = propose(&review(&tree, "old", None));
    let old_key = accepted
        .source()
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "removed-package")
        .unwrap()
        .key()
        .clone();
    // Keep only the inert accepted section at its original locations.
    fs::rename(tree.path("sources/old"), tree.path("unavailable-old")).unwrap();
    fs::rename(tree.path("old-cache"), tree.path("unavailable-cache")).unwrap();
    source(&tree, "pub const VALUE: u64 = 8;\n", "");
    let updated = review(&tree, "updated", Some(&accepted));
    assert!(updated.changes().requires_decision());
    let removed = updated
        .changes()
        .packages()
        .iter()
        .find(|package| package.key() == &old_key)
        .unwrap();
    assert!(removed.candidate_resolution().is_none());
    assert!(
        removed
            .rows()
            .iter()
            .all(|row| row.change() == PackagePolicyChangeKind::Removed)
    );
    assert!(removed.rows().iter().any(|row| row.requires_decision()));
    let proposed = propose(&updated);
    assert_eq!(proposed.source().packages().len(), 1);
    assert_eq!(
        proposed.decisions().baseline_source_subject(),
        Some(*accepted.source().fingerprint().as_bytes())
    );
    assert_round_trip(&updated, proposed);

    // Missing history is an explicit initial review, including assumptions.
    source(&tree, ASSUMPTION, "");
    let fresh = review(&tree, "missing-baseline", None);
    assert!(fresh.changes().baseline_source_subject().is_none());
    assert!(fresh.changes().requires_decision());
    assert_round_trip(&fresh, propose(&fresh));
}

#[test]
fn source_only_edits_invalidate_an_otherwise_identical_resolution() {
    let tree = Tree::new();
    source(&tree, ASSUMPTION, "");
    let original = review(&tree, "original", None);
    let resolution = decisions(&original, "accept");
    source(&tree, &format!("// source-only edit\n{ASSUMPTION}"), "");
    let edited = review(&tree, "edited", None);
    assert_eq!(
        original.reviews().reviews()[0].policy(),
        edited.reviews().reviews()[0].policy()
    );
    assert_ne!(
        original.changes().fingerprint(),
        edited.changes().fingerprint()
    );
    assert!(matches!(
        edited.propose_lock_target(&resolution),
        Err(PackageChangeError::Decisions(
            HistoricalPackagePolicyError::ResolutionMismatch
        ))
    ));
    assert_round_trip(&edited, propose(&edited));
}

#[test]
#[cfg_attr(not(unix), allow(clippy::permissions_set_readonly_false))]
fn proposal_rechecks_the_reviewed_source_snapshot() {
    let tree = Tree::new();
    source(&tree, PURE, "");
    let checked = review(&tree, "tamper", None);
    let resolution = decisions(&checked, "accept");
    let root = checked.source_closure().graph().root();
    let snapshot = checked
        .source_closure()
        .source_root(root)
        .unwrap()
        .join("main.omg");
    let original_permissions = fs::metadata(&snapshot).unwrap().permissions();
    let mut writable = original_permissions.clone();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        writable.set_mode(writable.mode() | 0o200);
    }
    #[cfg(not(unix))]
    writable.set_readonly(false);
    fs::set_permissions(&snapshot, writable).unwrap();
    fs::write(&snapshot, "pub const VALUE: u64 = 9;\n").unwrap();
    fs::set_permissions(&snapshot, original_permissions).unwrap();
    assert!(matches!(
        checked.propose_lock_target(&resolution),
        Err(PackageChangeError::Compilation(CompileResolvedPackageReviewsError::SourceCustody { source_package, .. })) if &source_package == root
    ));
    assert!(!tree.path("sources/root/omega.lock").exists());
}

#[test]
fn target_mismatch_rejects_before_creating_a_build_directory() {
    let tree = Tree::new();
    source(&tree, PURE, "");
    let accepted = propose(&review(&tree, "accepted", None));
    let build_root = tree.path("wrong-target-build");
    assert!(matches!(
        review_package_change(
            resolve(&tree, "wrong-target"),
            TargetProfile::LinuxArm64,
            Some(&accepted),
            &build_root
        ),
        Err(PackageChangeError::Comparison(
            PackagePolicyChangeError::TargetMismatch
        ))
    ));
    assert!(!build_root.exists());
}
