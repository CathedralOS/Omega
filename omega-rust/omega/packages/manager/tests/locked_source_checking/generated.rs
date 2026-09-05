use super::*;
use omega_package_manager::resolution::graph::resolve_workspace_project_closure_with_storage;
use omega_package_source::{SourceLineage, SourceRelativePath};

fn generated_workspace(
    tree: &Tree,
    storage: &SourceResolverStorage,
) -> ResolvedPackageSourceClosure {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|ancestor| ancestor.join("tests/fixtures/packages").is_dir())
        .expect("package manager lives beneath the repository fixtures")
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
    resolve_workspace_project_closure_with_storage(
        &SourceLineage::git("https://github.com/CathedralOS/Omega.git").unwrap(),
        SourceRelativePath::parse("generated-consumer").unwrap(),
        tree.path("sources"),
        storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve the existing generated producer and consumer fixture")
}

#[test]
fn locked_checking_rebuilds_generated_bundles_and_preserves_full_current_reviews() {
    let tree = Tree::new();
    let (lock, request) = {
        let storage = tree.storage("old-cache");
        let closure = generated_workspace(&tree, &storage);
        capture_lock(&closure, &tree.path("old-build"))
    };
    let accepted_text = lock.canonical_text().unwrap();
    // Accepted text retains policy, not a source bundle or current compiler
    // result. Remove prior phase artifacts before asking for fresh checking.
    fs::remove_dir_all(tree.path("old-build")).unwrap();
    let storage = tree.storage("new-cache");
    let checked = check_locked_sources(
        &lock,
        TARGET,
        &request,
        &storage,
        LockedSourceRecoveryOptions::default(),
        &tree.path("fresh-build"),
    )
    .expect("fresh checking reconstructs generated dependencies from exact source");
    assert_eq!(checked.accepted(), lock.target(TARGET).unwrap());
    assert_fresh_matches(&lock, checked.source_closure());
    assert!(checked.changed_policies().is_empty());
    assert_eq!(checked.reviews().reviews().len(), 2);
    let producer = checked
        .reviews()
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "generated-table")
        .unwrap();
    let consumer = checked
        .reviews()
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "generated-consumer")
        .unwrap();
    let [generated] = producer.generated_source_bundle().sources() else {
        panic!("fresh producer must retain the complete generated source bundle");
    };
    assert_eq!(generated.relative_path(), b"table.generated.omg");
    assert_eq!(
        generated.bytes(),
        b"pub machine table_size() -> u64 {\n    3\n}\n"
    );
    assert!(consumer.generated_source_bundle().sources().is_empty());
    assert_eq!(
        producer
            .build_observation_summary()
            .unwrap()
            .filesystem_operation_attempts()
            .len(),
        6
    );
    assert_eq!(
        consumer
            .build_observation_summary()
            .unwrap()
            .filesystem_operation_attempts()
            .len(),
        0
    );
    assert!(
        consumer
            .projection()
            .callables()
            .iter()
            .any(|callable| callable.identity().path() == "consume_generated_table")
    );
    for review in [producer, consumer] {
        let accepted = checked
            .accepted()
            .baselines()
            .iter()
            .find(|policy| policy.package() == review.key().identity())
            .unwrap();
        assert_eq!(review.policy(), accepted);
        assert_eq!(
            review.policy().canonical_bytes().unwrap(),
            accepted.canonical_bytes().unwrap()
        );
    }
    let fresh_cache = fs::canonicalize(tree.path("new-cache")).unwrap();
    for custody in checked.source_closure().custodies() {
        assert!(custody.snapshot_root().starts_with(&fresh_cache));
    }
    assert!(tree.path("fresh-build").is_dir());
    assert!(!tree.path("old-build").exists());

    // Even though this fixture's emitted text is fixed, changed build input is
    // different locked source and must reject before a new compiler run.
    fs::write(
        tree.path("sources/generated-table/inputs/table.txt"),
        b"changed source input\n",
    )
    .unwrap();
    assert!(
        check_locked_sources(
            &lock,
            TARGET,
            &request,
            &storage,
            LockedSourceRecoveryOptions::default(),
            &tree.path("rejected-build"),
        )
        .is_err()
    );
    assert!(!tree.path("rejected-build").exists());
    assert_eq!(lock.canonical_text().unwrap(), accepted_text);
}
