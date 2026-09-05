use super::*;
use package_evidence::encoding::PackagePolicyTextRecoveryLimits;
use package_evidence::record::PackagePolicyBaseline;
use package_manager::operations::{
    LockedSourceRecoveryOptions, check_locked_sources, recover_locked_sources,
};

fn publish(tree: &Tree, target: PackageLockTarget) -> PackageLock {
    let lock = PackageLock::from_targets(vec![target]).unwrap();
    let path = tree.path("sources/root/omega.lock");
    fs::write(&path, lock.canonical_text().unwrap()).unwrap();
    let recovered = PackageLock::recover_text(
        &fs::read_to_string(path).unwrap(),
        PackageLockRecoveryLimits::default(),
    )
    .unwrap();
    assert_eq!(recovered, lock);
    recovered
}

#[test]
fn published_proposal_preserves_source_and_complete_policy_for_review_and_locked_checks() {
    let tree = Tree::new();
    source(&tree, PURE, "");
    fs::create_dir(tree.path("sources/root/src")).unwrap();
    fs::write(
        tree.path("sources/root/src/omega.lock"),
        "ordinary source\n",
    )
    .unwrap();
    let initial = review(&tree, "initial", None);
    let request = initial
        .source_closure()
        .source_requests()
        .root()
        .request()
        .clone();
    let lock = publish(&tree, propose(&initial));
    let accepted = lock.target(TARGET).unwrap();
    let root = initial.source_closure().graph().root();
    let reviewed = review(&tree, "published", Some(accepted));
    assert_eq!(reviewed.source_closure().graph().root(), root);
    assert_fresh_matches(&lock, reviewed.source_closure());
    assert_eq!(
        reviewed.reviews().review(root).unwrap().policy(),
        initial.reviews().review(root).unwrap().policy()
    );
    assert!(!reviewed.changes().requires_decision());
    assert!(
        reviewed
            .changes()
            .packages()
            .iter()
            .all(|package| { !package.source_changed() && package.rows().is_empty() })
    );
    let proposed = propose(&reviewed);
    assert_eq!(proposed.source(), accepted.source());
    assert_eq!(proposed.baselines(), accepted.baselines());

    let snapshot = reviewed.source_closure().source_root(root).unwrap();
    assert!(!snapshot.join("omega.lock").exists());
    assert_eq!(
        fs::read_to_string(snapshot.join("src/omega.lock")).unwrap(),
        "ordinary source\n"
    );
    let storage = tree.storage("locked-cache");
    let recovered = recover_locked_sources(
        &lock,
        TARGET,
        &request,
        &storage,
        LockedSourceRecoveryOptions::default(),
    )
    .unwrap();
    assert_fresh_matches(&lock, &recovered);
    let checked = check_locked_sources(
        &lock,
        TARGET,
        &request,
        &storage,
        LockedSourceRecoveryOptions::default(),
        &tree.path("locked-build"),
    )
    .unwrap();
    assert_fresh_matches(&lock, checked.source_closure());
    assert!(checked.changed_policies().is_empty());
    assert_eq!(
        checked.reviews().review(root).unwrap().policy(),
        &accepted.baselines()[0]
    );

    fs::write(tree.path("sources/root/src/omega.lock"), "edited source\n").unwrap();
    let nested_edit = review(&tree, "nested-edit", Some(accepted));
    let proposed = propose(&nested_edit);
    assert_ne!(proposed.source(), accepted.source());
    assert_eq!(proposed.baselines(), accepted.baselines());
    assert!(nested_edit.changes().packages()[0].source_changed());
    assert_eq!(
        fs::read_to_string(tree.path("sources/root/omega.lock")).unwrap(),
        lock.canonical_text().unwrap()
    );
}

#[test]
fn published_inert_baseline_edits_preserve_source_but_remain_policy_changes() {
    let tree = Tree::new();
    source(&tree, PURE, "");
    let initial = review(&tree, "initial", None);
    let request = initial
        .source_closure()
        .source_requests()
        .root()
        .request()
        .clone();
    let lock = publish(&tree, propose(&initial));
    let accepted = lock.target(TARGET).unwrap();
    let root = initial.source_closure().graph().root();
    let mut baselines = accepted.baselines().to_vec();
    assert_eq!(baselines.len(), 1);
    let text = baselines[0].canonical_text().unwrap();
    assert_eq!(text.matches("string \"VALUE\"\n").count(), 1);
    // Edit retained policy through its codec; the published lock stays valid.
    baselines[0] = PackagePolicyBaseline::recover_text(
        &text.replace("string \"VALUE\"\n", "string \"RENAMED_VALUE\"\n"),
        PackagePolicyTextRecoveryLimits::default(),
    )
    .unwrap();
    let edited = publish(
        &tree,
        PackageLockTarget::from_parts(
            accepted.source().clone(),
            baselines,
            accepted.decisions().clone(),
        )
        .unwrap(),
    );
    assert_ne!(
        edited.canonical_text().unwrap(),
        lock.canonical_text().unwrap()
    );
    let reviewed = review(&tree, "edited-lock", edited.target(TARGET));
    assert_fresh_matches(&lock, reviewed.source_closure());
    assert_eq!(
        reviewed.reviews().review(root).unwrap().policy(),
        &accepted.baselines()[0]
    );
    let [package] = reviewed.changes().packages() else {
        panic!("one root package")
    };
    assert!(!package.source_changed());
    assert_eq!(package.rows().len(), 2);
    for (change, name) in [
        (PackagePolicyChangeKind::Removed, "RENAMED_VALUE"),
        (PackagePolicyChangeKind::Added, "VALUE"),
    ] {
        assert!(package.rows().iter().any(|row| {
            row.kind() == PackagePolicyRowKind::PublicConst
                && row.change() == change
                && row
                    .baseline()
                    .or(row.candidate())
                    .unwrap()
                    .canonical_text()
                    .contains(name)
        }));
    }
    let storage = tree.storage("edited-lock-cache");
    let checked = check_locked_sources(
        &edited,
        TARGET,
        &request,
        &storage,
        LockedSourceRecoveryOptions::default(),
        &tree.path("edited-lock-build"),
    )
    .unwrap();
    assert_fresh_matches(&lock, checked.source_closure());
    assert_eq!(checked.changed_policies(), std::slice::from_ref(root));
    assert_eq!(
        checked.reviews().review(root).unwrap().policy(),
        &accepted.baselines()[0]
    );
    assert_eq!(
        fs::read_to_string(tree.path("sources/root/omega.lock")).unwrap(),
        edited.canonical_text().unwrap()
    );
}
