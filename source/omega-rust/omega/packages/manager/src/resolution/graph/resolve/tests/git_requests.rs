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
    let storage = SourceResolverStorage::for_hardened_base(&cache)
        .expect("create retained Git resolver storage");
    let resolved = crate::resolution::source::resolve_git_package_source_with_storage(
        &request,
        &storage,
        LocalSourceLimits::default(),
    )
    .expect("resolve root for exact request validation");
    assert!(git_root_request_matches(&request, resolved.source()));
    let wrong_revision = GitSourceRequest::for_local_test_repository_with_lineage(
        &repository,
        Some("different-revision".to_owned()),
        "https://github.com/CathedralOS/network-root.git",
    )
    .expect("alternate revision request");
    assert!(!git_root_request_matches(
        &wrong_revision,
        resolved.source()
    ));
    let wrong_locator = GitSourceRequest::for_local_test_repository_with_lineage(
        &repository,
        None,
        "https://github.com/CathedralOS/other-root.git",
    )
    .expect("alternate locator request");
    assert!(!git_root_request_matches(&wrong_locator, resolved.source()));

    let closure = resolve_git_package_closure_with_storage(
        &request,
        omega_target::TargetProfile::CrossPlatformCli,
        &storage,
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

#[test]
fn repository_root_project_retains_application_role_and_package_entry_rejects() {
    let repository = temp_root("git-application-root-repository");
    let cache = temp_root("git-application-root-cache");
    write_application(&repository, "network-console", None);
    run_test_git(&repository, ["init", "--quiet"]);
    run_test_git(
        &repository,
        ["config", "user.email", "omega@example.invalid"],
    );
    run_test_git(&repository, ["config", "user.name", "Omega Tests"]);
    run_test_git(&repository, ["add", "."]);
    run_test_git(&repository, ["commit", "--quiet", "-m", "application"]);
    let request = GitSourceRequest::for_local_test_repository_with_lineage(
        &repository,
        None,
        "https://github.com/CathedralOS/network-console.git",
    )
    .expect("validated local Git application request");
    let storage = SourceResolverStorage::for_hardened_base(&cache)
        .expect("create retained Git resolver storage");

    crate::resolution::graph::resolve_git_package_closure_with_storage(
        &request,
        omega_target::TargetProfile::CrossPlatformCli,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect_err("package-only Git entry rejects an application root");
    let closure = crate::resolution::graph::resolve_git_project_closure_with_storage(
        &request,
        omega_target::TargetProfile::CrossPlatformCli,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("project Git entry accepts an application root");

    assert_eq!(
        closure.root_role(),
        crate::declarations::BuildDeclarationKind::Application
    );
    assert_eq!(closure.graph().root().name().as_str(), "network-console");

    let _ = std::fs::remove_dir_all(repository);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
fn named_git_selection_resolves_only_the_declared_matching_member() {
    let repository = temp_root("git-named-selection-root");
    let cache = temp_root("git-named-selection-cache");
    std::fs::create_dir_all(repository.join("packages")).expect("create root repository");
    std::fs::write(
        repository.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.member("packages/matrix");
}
"#,
    )
    .expect("write root build");
    write_package(&repository.join("packages/matrix"), "matrix", None);
    run_test_git(&repository, ["init", "--quiet"]);
    run_test_git(
        &repository,
        ["config", "user.email", "omega@example.invalid"],
    );
    run_test_git(&repository, ["config", "user.name", "Omega Tests"]);
    run_test_git(&repository, ["add", "."]);
    run_test_git(&repository, ["commit", "--quiet", "-m", "root"]);
    let request = crate::resolution::source::GitPackageSourceRequest::new(
        GitSourceRequest::for_local_test_repository_with_lineage(
            &repository,
            None,
            "https://github.com/CathedralOS/workspace.git",
        )
        .expect("validated local Git root request"),
        crate::declarations::PackageSelection::Named(
            crate::declarations::PackageName::parse("matrix").expect("package name"),
        ),
    );
    let storage = SourceResolverStorage::for_hardened_base(&cache)
        .expect("create retained Git resolver storage");

    let closure = crate::resolution::graph::resolve_selected_git_package_closure_with_storage(
        &request,
        omega_target::TargetProfile::CrossPlatformCli,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve named Git workspace member");
    let root = closure
        .custody(closure.graph().root())
        .expect("root custody");
    assert_eq!(root.key().name().as_str(), "matrix");
    assert!(matches!(
        root.navigation(),
        crate::resolution::source::PackageSourceNavigation::Member(path)
            if path.as_str() == "packages/matrix"
    ));
    let selection = root
        .selection_evidence()
        .git_workspace()
        .expect("named Git source retains workspace declaration evidence");
    assert_eq!(selection.selected_member_path().as_str(), "packages/matrix");
    assert_eq!(selection.members().len(), 1);
    assert_eq!(
        selection.workspace_declaration().repository_path(),
        "build.omg"
    );
    let PackageRootSourceRequest::Git(retained) = closure.source_requests().root().request() else {
        panic!("named Git request retained")
    };
    assert_eq!(retained, &request);

    root.selection_evidence()
        .revalidate()
        .expect("retained declaration bytes replay outside compilation root");
    assert!(!root.snapshot_root().join("packages/matrix").exists());

    let _ = std::fs::remove_dir_all(repository);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
fn named_git_project_selects_an_application_from_a_mixed_workspace() {
    let repository = temp_root("git-named-application-root");
    let cache = temp_root("git-named-application-cache");
    std::fs::create_dir_all(repository.join("projects")).expect("create root repository");
    std::fs::write(
        repository.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.member("projects/console");
    builder.member("projects/protocol");
}
"#,
    )
    .expect("write root build");
    write_application(&repository.join("projects/console"), "driver-console", None);
    write_package(
        &repository.join("projects/protocol"),
        "driver-protocol",
        None,
    );
    run_test_git(&repository, ["init", "--quiet"]);
    run_test_git(
        &repository,
        ["config", "user.email", "omega@example.invalid"],
    );
    run_test_git(&repository, ["config", "user.name", "Omega Tests"]);
    run_test_git(&repository, ["add", "."]);
    run_test_git(&repository, ["commit", "--quiet", "-m", "workspace"]);
    let request = crate::resolution::source::GitPackageSourceRequest::new(
        GitSourceRequest::for_local_test_repository_with_lineage(
            &repository,
            None,
            "https://github.com/CathedralOS/driver-workspace.git",
        )
        .expect("validated local Git workspace request"),
        crate::declarations::PackageSelection::Named(
            crate::declarations::PackageName::parse("driver-console").expect("project name"),
        ),
    );
    let storage = SourceResolverStorage::for_hardened_base(&cache)
        .expect("create retained Git resolver storage");

    let closure = crate::resolution::graph::resolve_selected_git_project_closure_with_storage(
        &request,
        omega_target::TargetProfile::CrossPlatformCli,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("select application member as project root");

    assert_eq!(
        closure.root_role(),
        crate::declarations::BuildDeclarationKind::Application
    );
    let evidence = closure
        .custody(closure.graph().root())
        .expect("root custody")
        .selection_evidence()
        .git_workspace()
        .expect("workspace evidence");
    assert_eq!(evidence.members().len(), 2);
    assert!(evidence.members().iter().any(|member| {
        member.package_name().as_str() == "driver-console"
            && member.role() == crate::declarations::BuildDeclarationKind::Application
    }));
    assert!(evidence.members().iter().any(|member| {
        member.package_name().as_str() == "driver-protocol"
            && member.role() == crate::declarations::BuildDeclarationKind::Package
    }));

    let _ = std::fs::remove_dir_all(repository);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
fn named_git_member_path_dependencies_keep_repository_custody() {
    let repository = temp_root("git-member-path-repository");
    let cache = temp_root("git-member-path-cache");
    std::fs::create_dir_all(repository.join("packages")).expect("create repository");
    std::fs::write(
        repository.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.member("packages/left");
    builder.member("packages/right");
}
"#,
    )
    .expect("write workspace build");
    write_package(&repository.join("packages/left"), "left", Some("../right"));
    write_package(&repository.join("packages/right"), "right", None);
    run_test_git(&repository, ["init", "--quiet"]);
    run_test_git(
        &repository,
        ["config", "user.email", "omega@example.invalid"],
    );
    run_test_git(&repository, ["config", "user.name", "Omega Tests"]);
    run_test_git(&repository, ["add", "."]);
    run_test_git(&repository, ["commit", "--quiet", "-m", "workspace"]);
    let request = crate::resolution::source::GitPackageSourceRequest::new(
        GitSourceRequest::for_local_test_repository_with_lineage(
            &repository,
            None,
            "https://github.com/CathedralOS/member-path.git",
        )
        .expect("validated local Git root request"),
        crate::declarations::PackageSelection::Named(
            crate::declarations::PackageName::parse("left").expect("package name"),
        ),
    );
    let storage = SourceResolverStorage::for_hardened_base(&cache)
        .expect("create retained Git resolver storage");

    let closure = crate::resolution::graph::resolve_selected_git_package_closure_with_storage(
        &request,
        omega_target::TargetProfile::CrossPlatformCli,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve member-relative Git closure");
    let left = closure
        .custodies()
        .iter()
        .find(|custody| custody.key().name().as_str() == "left")
        .expect("left custody");
    let right = closure
        .custodies()
        .iter()
        .find(|custody| custody.key().name().as_str() == "right")
        .expect("right custody");
    assert_eq!(left.key().source_lineage(), right.key().source_lineage());
    assert_eq!(left.resolution(), right.resolution());
    assert_ne!(
        left.materialization().content(),
        right.materialization().content(),
        "one repository resolution carries distinct selected package bytes"
    );
    assert_ne!(left.snapshot_root(), right.snapshot_root());
    assert!(matches!(
        right.navigation(),
        crate::resolution::source::PackageSourceNavigation::Member(path)
            if path.as_str() == "packages/right"
    ));
    assert_eq!(
        left.selection_evidence()
            .git_workspace()
            .expect("left selection evidence")
            .selected_member_path()
            .as_str(),
        "packages/left"
    );
    assert_eq!(
        right
            .selection_evidence()
            .git_workspace()
            .expect("right selection evidence")
            .selected_member_path()
            .as_str(),
        "packages/right"
    );
    let canonical = crate::resolution::graph::CanonicalSourceClosureSubject::from_resolved(
        &closure,
        crate::resolution::graph::CanonicalSourceClosureSubjectLimits::default(),
    )
    .expect("canonicalize member-relative Git closure");
    assert_eq!(
        canonical.package_navigation(right.key()),
        Some(right.navigation())
    );
    crate::resolution::package_compilation_inputs(&closure)
        .expect("compiler handoff revalidates repository and selected member commitments");

    let _ = std::fs::remove_dir_all(repository);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
fn git_member_path_dependency_rejects_an_undeclared_directory() {
    let repository = temp_root("git-undeclared-member-repository");
    let cache = temp_root("git-undeclared-member-cache");
    std::fs::create_dir_all(repository.join("packages")).expect("create repository");
    std::fs::write(
        repository.join("build.omg"),
        r#"
machine build(builder: &mut Build) {
    builder.member("packages/left");
}
"#,
    )
    .expect("write workspace build");
    write_package(&repository.join("packages/left"), "left", Some("../hidden"));
    write_package(&repository.join("packages/hidden"), "hidden", None);
    run_test_git(&repository, ["init", "--quiet"]);
    run_test_git(
        &repository,
        ["config", "user.email", "omega@example.invalid"],
    );
    run_test_git(&repository, ["config", "user.name", "Omega Tests"]);
    run_test_git(&repository, ["add", "."]);
    run_test_git(&repository, ["commit", "--quiet", "-m", "workspace"]);
    let request = crate::resolution::source::GitPackageSourceRequest::new(
        GitSourceRequest::for_local_test_repository_with_lineage(
            &repository,
            None,
            "https://github.com/CathedralOS/undeclared-member.git",
        )
        .expect("validated local Git root request"),
        crate::declarations::PackageSelection::Named(
            crate::declarations::PackageName::parse("left").expect("package name"),
        ),
    );
    let storage = SourceResolverStorage::for_hardened_base(&cache)
        .expect("create retained Git resolver storage");

    let error = crate::resolution::graph::resolve_selected_git_package_closure_with_storage(
        &request,
        omega_target::TargetProfile::CrossPlatformCli,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect_err("undeclared Git workspace directory rejects");
    assert!(matches!(
        error,
        ResolveGitPackageClosureError::Closure(
            PackageSourceClosureResolutionError::Adapter {
                error: ResolveDependencySourceError::UndeclaredGitWorkspaceMember {
                    member_path,
                    ..
                },
                ..
            }
        ) if member_path.as_str() == "packages/hidden"
    ));

    let _ = std::fs::remove_dir_all(repository);
    let _ = std::fs::remove_dir_all(cache);
}
