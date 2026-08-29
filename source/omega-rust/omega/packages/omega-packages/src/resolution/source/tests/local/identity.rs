use super::*;

#[test]
fn package_fixtures_resolve_as_distinct_local_sources() {
    let fixtures_root = package_fixtures_root();
    let mut identities = BTreeSet::new();
    for package in PACKAGE_FIXTURES {
        PackageName::parse(*package).expect("fixture package names must be kebab-case");
        let root = fixtures_root.join(package);
        assert!(root.join("build.omg").is_file());
        assert!(root.join("main.omg").is_file());

        let resolved =
            resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve fixture");
        assert!(resolved.file_count >= 3);
        assert!(identities.insert(resolved.content_identity));
    }
    assert_eq!(identities.len(), PACKAGE_FIXTURES.len());
}

#[test]
fn local_source_identity_is_order_independent_and_ignores_git_dir() {
    let root = temp_root("identity");
    std::fs::create_dir_all(root.join("src")).expect("create source tree");
    std::fs::create_dir_all(root.join(".git")).expect("create git dir");
    std::fs::write(root.join("src/lib.omg"), "machine Lib::id() {}\n").expect("write source");
    std::fs::write(root.join("README.md"), "package\n").expect("write readme");
    std::fs::write(root.join(".git/index"), "ignored").expect("write ignored git data");

    let first = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve");
    std::fs::write(root.join(".git/index"), "ignored but changed")
        .expect("change ignored git data");
    let second = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve");

    assert_eq!(first.file_count, 2);
    assert_eq!(first.content_identity, second.content_identity);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn local_package_identity_excludes_only_root_build_output() {
    let root = temp_root("root-build-output");
    std::fs::create_dir_all(root.join("build")).expect("create root build output");
    std::fs::create_dir_all(root.join("src/build")).expect("create nested source directory");
    std::fs::write(root.join("main.omg"), "machine Main::main() {}\n")
        .expect("write package source");
    std::fs::write(
        root.join("build/00_pipeline.html"),
        "first generated report",
    )
    .expect("write generated report");
    std::fs::write(
        root.join("src/build/rules.omg"),
        "machine Rules::apply() {}\n",
    )
    .expect("write nested source");

    let first =
        resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve local package");
    std::fs::write(
        root.join("build/00_pipeline.html"),
        "changed generated report",
    )
    .expect("change generated report");
    let changed_output = resolve_local_source(&root, LocalSourceLimits::default())
        .expect("resolve package after output change");
    assert_eq!(first.file_count, 2);
    assert_eq!(first.content_identity, changed_output.content_identity);

    std::fs::write(
        root.join("src/build/rules.omg"),
        "machine Rules::replace() {}\n",
    )
    .expect("change nested source");
    let changed_source = resolve_local_source(&root, LocalSourceLimits::default())
        .expect("resolve package after source change");
    assert_ne!(
        changed_output.content_identity,
        changed_source.content_identity
    );

    let exact = resolve_materialized_source(&root, LocalSourceLimits::default())
        .expect("resolve exact materialized tree");
    assert_eq!(exact.file_count, 3);
    assert_ne!(changed_source.content_identity, exact.content_identity);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn local_source_identity_changes_when_source_bytes_change() {
    let root = temp_root("bytes");
    std::fs::create_dir_all(&root).expect("create source tree");
    std::fs::write(root.join("main.omg"), "machine Main::a() {}\n").expect("write source");
    let first = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve");

    std::fs::write(root.join("main.omg"), "machine Main::b() {}\n").expect("rewrite source");
    let second = resolve_local_source(&root, LocalSourceLimits::default()).expect("resolve");

    assert_ne!(first.content_identity, second.content_identity);

    let _ = std::fs::remove_dir_all(&root);
}
