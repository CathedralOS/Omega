use super::*;

#[test]
fn resolves_repository_root_git_closure_and_retains_the_exact_request() {
    let repository = temp_root("git-root-repository");
    let cache = temp_root("git-root-cache");
    write_package(&repository, "network-root", None);
    run_test_git(&repository, ["init", "--quiet"]);
    run_test_git(
        &repository,
        ["config", "user.email", "omega@example.invalid"],
    );
    run_test_git(&repository, ["config", "user.name", "Omega Tests"]);
    run_test_git(&repository, ["add", "."]);
    run_test_git(&repository, ["commit", "--quiet", "-m", "root"]);
    let request = GitSourceRequest::for_local_test_repository_with_lineage(
        &repository,
        None,
        "https://github.com/CathedralOS/network-root.git",
    )
    .expect("validated local Git root request");
    let resolved = resolve_git_package_source(
        &request,
        cache.join("git-sources"),
        LocalSourceLimits::default(),
    )
    .expect("resolve root for exact request validation");
    assert!(git_root_request_matches(
        &request,
        resolved.source(),
        resolved.key().source_lineage()
    ));
    let wrong_revision = GitSourceRequest::for_local_test_repository_with_lineage(
        &repository,
        Some("different-revision".to_owned()),
        "https://github.com/CathedralOS/network-root.git",
    )
    .expect("alternate revision request");
    assert!(!git_root_request_matches(
        &wrong_revision,
        resolved.source(),
        resolved.key().source_lineage()
    ));
    let wrong_locator = GitSourceRequest::for_local_test_repository_with_lineage(
        &repository,
        None,
        "https://github.com/CathedralOS/other-root.git",
    )
    .expect("alternate locator request");
    assert!(!git_root_request_matches(
        &wrong_locator,
        resolved.source(),
        resolved.key().source_lineage()
    ));

    let closure = resolve_git_package_closure(
        &request,
        &cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve repository-root Git closure");

    let root_binding = closure.source_requests().root();
    let PackageRootSourceRequest::Git(retained) = root_binding.request() else {
        panic!("Git adapter retains its root request")
    };
    assert_eq!(
        retained.requested_locator(),
        "https://github.com/CathedralOS/network-root.git"
    );
    assert_eq!(retained.requested_revision(), "HEAD");
    assert_eq!(retained.transport_profile(), GitTransportProfile::TestFile);
    assert_eq!(
        root_binding.selected().key().name().as_str(),
        "network-root"
    );
    assert!(closure.source_requests().dependencies().next().is_none());

    let _ = std::fs::remove_dir_all(repository);
    let _ = std::fs::remove_dir_all(cache);
}
