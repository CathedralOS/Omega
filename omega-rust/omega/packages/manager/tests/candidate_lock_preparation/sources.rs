use super::*;
use omega_package_manager::lock::{PackageLock, PackageLockRecoveryLimits};
use omega_package_manager::resolution::graph::{
    PackageSourceClosureLimits, resolve_workspace_project_closure_with_storage,
};
use omega_package_manager::review::compile_resolved_package_candidate_reviews;
use omega_package_source::{LocalSourceLimits, SourceLineage, SourceRelativePath};

#[test]
fn removed_dependency_requires_no_old_source_for_preparation() {
    let tree = Tree::new();
    source(
        &tree,
        "pub const VALUE: u64 = 7;\n",
        "builder.depend(Source::Path { location: \"../old\" });\n",
    );
    package(&tree.path("sources/old"), "removed-library", "");
    let accepted = {
        let (sources, reviews) = candidate(&tree, "old");
        let changes = compare(None, &sources, &reviews);
        let decisions = resolution(&changes, ACCEPT);
        prepare_candidate_lock_target(
            None,
            &sources.for_exact_target(TARGET),
            reviews,
            &decisions,
            PrepareCandidateLockLimits::default(),
        )
        .unwrap()
    };
    let old_key = accepted
        .source()
        .packages()
        .iter()
        .find(|source| source.key().name().as_str() == "removed-library")
        .unwrap()
        .key()
        .clone();
    fs::rename(tree.path("sources/old"), tree.path("unavailable-old")).unwrap();
    fs::rename(tree.path("old-cache"), tree.path("unavailable-cache")).unwrap();
    source(&tree, "pub const VALUE: u64 = 7;\n", "");
    let (sources, reviews) = candidate(&tree, "current");
    let changes = compare(Some(&accepted), &sources, &reviews);
    let decisions = resolution(&changes, ACCEPT);
    assert!(!decisions.decisions().is_empty());
    let prepared = prepare_candidate_lock_target(
        Some(&accepted),
        &sources.for_exact_target(TARGET),
        reviews,
        &decisions,
        PrepareCandidateLockLimits::default(),
    )
    .expect("accepted normalized policy supports removal without acquiring old source");
    assert!(
        !prepared
            .source()
            .packages()
            .iter()
            .any(|source| source.key() == &old_key)
    );
    assert!(prepared.decisions().decisions().iter().any(|decision| matches!(
        decision.subject(),
        omega_package_manager::lock::HistoricalPackagePolicyDecisionSubject::RemovedPackage { key } if key == &old_key
    )));
    let lock = PackageLock::from_targets(vec![prepared]).unwrap();
    let text = lock.canonical_text().unwrap();
    assert_eq!(
        PackageLock::recover_text(&text, PackageLockRecoveryLimits::default()).unwrap(),
        lock
    );
    assert!(!tree.path("sources/old").exists());
    assert!(!tree.path("old-cache").exists());
}

#[test]
fn changed_current_snapshot_rejects_after_compilation_and_policy_acceptance() {
    let tree = Tree::new();
    source(&tree, "pub const VALUE: u64 = 7;\n", "");
    let (sources, reviews) = candidate(&tree, "snapshot-change");
    let changes = compare(None, &sources, &reviews);
    let decisions = resolution(&changes, ACCEPT);
    let root = sources.graph().root().clone();
    let path = sources
        .custody(&root)
        .unwrap()
        .snapshot_root()
        .join("main.omg");
    make_writable(&path);
    fs::write(path, "pub const VALUE: u64 = 9;\n").unwrap();
    let error = prepare_candidate_lock_target(
        None,
        &sources.for_exact_target(TARGET),
        reviews,
        &decisions,
        PrepareCandidateLockLimits::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, PrepareCandidateLockError::SourceSnapshot { package, .. } if package == root)
    );
}

#[test]
fn unavailable_current_snapshot_rejects_retained_review_material() {
    let tree = Tree::new();
    source(&tree, "pub const VALUE: u64 = 7;\n", "");
    let (sources, reviews) = candidate(&tree, "snapshot-unavailable");
    let changes = compare(None, &sources, &reviews);
    let decisions = resolution(&changes, ACCEPT);
    let root = sources.graph().root().clone();
    let snapshot = sources.custody(&root).unwrap().snapshot_root();
    fs::rename(snapshot, tree.path("retired-current-snapshot")).unwrap();
    let error = prepare_candidate_lock_target(
        None,
        &sources.for_exact_target(TARGET),
        reviews,
        &decisions,
        PrepareCandidateLockLimits::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, PrepareCandidateLockError::SourceSnapshot { package, .. } if package == root)
    );
}

#[cfg_attr(windows, allow(clippy::permissions_set_readonly_false))]
fn make_writable(path: &Path) {
    let mut permissions = fs::metadata(path).unwrap().permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(permissions.mode() | 0o200);
    }
    #[cfg(not(unix))]
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions).unwrap();
}

#[test]
fn generated_producer_and_consumer_final_policies_survive_preparation_and_recovery() {
    let tree = Tree::new();
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|ancestor| ancestor.join("tests/fixtures/packages").is_dir())
        .unwrap()
        .join("tests/fixtures/packages");
    for relative in [
        "generated-table/build.omg",
        "generated-table/main.omg",
        "generated-table/inputs/table.txt",
        "generated-consumer/build.omg",
        "generated-consumer/main.omg",
    ] {
        let destination = tree.path("sources").join(relative);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(fixtures.join(relative), destination).unwrap();
    }
    let storage = tree.storage("generated-cache");
    let sources = resolve_workspace_project_closure_with_storage(
        &SourceLineage::git("https://github.com/CathedralOS/Omega.git").unwrap(),
        SourceRelativePath::parse("generated-consumer").unwrap(),
        tree.path("sources"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .unwrap();
    let reviews = compile_resolved_package_candidate_reviews(
        &sources.for_exact_target(TARGET),
        &tree.path("generated-build"),
    )
    .unwrap();
    let producer = reviews
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "generated-table")
        .unwrap();
    let consumer = reviews
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "generated-consumer")
        .unwrap();
    let [generated] = producer.generated_source_bundle().sources() else {
        panic!("the actual build must generate the source consumed by the dependency");
    };
    assert_eq!(generated.relative_path(), b"table.generated.omg");
    assert_eq!(
        generated.bytes(),
        b"pub machine table_size() -> u64 {\n    3\n}\n"
    );
    assert!(
        consumer
            .projection()
            .callables()
            .iter()
            .any(|callable| callable.identity().path() == "consume_generated_table")
    );
    let expected = reviews
        .reviews()
        .iter()
        .map(|review| review.policy().clone())
        .collect::<Vec<_>>();
    let changes = compare(None, &sources, &reviews);
    let decisions = resolution(&changes, ACCEPT);
    let prepared = prepare_candidate_lock_target(
        None,
        &sources.for_exact_target(TARGET),
        reviews,
        &decisions,
        PrepareCandidateLockLimits::default(),
    )
    .unwrap();
    assert_eq!(prepared.baselines().len(), 2);
    for policy in &expected {
        assert_eq!(
            prepared
                .baselines()
                .iter()
                .find(|candidate| candidate.package() == policy.package()),
            Some(policy)
        );
    }
    let lock = PackageLock::from_targets(vec![prepared]).unwrap();
    let text = lock.canonical_text().unwrap();
    let recovered = PackageLock::recover_text(&text, PackageLockRecoveryLimits::default()).unwrap();
    assert_eq!(recovered, lock);
    for policy in &expected {
        assert_eq!(
            recovered
                .target(TARGET)
                .unwrap()
                .baselines()
                .iter()
                .find(|candidate| candidate.package() == policy.package()),
            Some(policy)
        );
    }
}
