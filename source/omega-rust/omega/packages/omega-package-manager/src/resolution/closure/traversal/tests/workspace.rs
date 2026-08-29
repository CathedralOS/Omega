use super::*;

#[test]
fn resolves_explicit_workspace_path_closure() {
    let cache_base = temp_root("fixture-cache");
    std::fs::create_dir_all(&cache_base).expect("create private storage base");
    let storage = SourceResolverStorage::for_hardened_base(&cache_base)
        .expect("create production-shaped private resolver storage");
    let closure = resolve_workspace_package_closure_with_storage(
        &fixture_lineage(),
        SourceRelativePath::parse("graph-workbench").expect("root member"),
        fixture_root(),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve local fixture closure");

    assert_eq!(closure.graph().packages().len(), 3);
    let root = closure
        .graph()
        .package(closure.graph().root())
        .expect("root package");
    let aliases = root
        .dependencies()
        .iter()
        .map(|dependency| dependency.alias().as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        aliases,
        std::collections::BTreeSet::from(["arithmetic_kernels", "file_journal"])
    );
    let root_binding = closure.source_requests().root();
    let PackageRootSourceRequest::WorkspaceMember {
        workspace_root_source,
        member_path,
        requested_workspace_root,
    } = root_binding.request()
    else {
        panic!("workspace adapter retains the workspace root request")
    };
    assert_eq!(workspace_root_source, &fixture_lineage());
    assert_eq!(member_path.as_str(), "graph-workbench");
    assert_eq!(requested_workspace_root, &fixture_root());
    assert_eq!(root_binding.selected().key(), closure.graph().root());
    assert_eq!(closure.source_requests().dependencies().count(), 2);

    drop(storage);
    let _ = std::fs::remove_dir_all(cache_base);
}

#[test]
fn resolves_nested_paths_relative_to_each_requester() {
    let workspace = temp_root("nested-workspace");
    let cache = temp_root("nested-cache");
    write_package(
        &workspace.join("packages/root"),
        "root-package",
        Some("../middle"),
    );
    write_package(
        &workspace.join("packages/middle"),
        "middle-package",
        Some("../leaf"),
    );
    write_package(&workspace.join("packages/leaf"), "leaf-package", None);

    let closure = resolve_workspace_package_closure(
        &fixture_lineage(),
        SourceRelativePath::parse("packages/root").expect("root member"),
        &workspace,
        &cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve nested workspace closure");

    assert_eq!(closure.graph().packages().len(), 3);
    assert!(closure.custodies().iter().any(|custody| {
        custody.key().name().as_str() == "leaf-package"
            && matches!(custody.key().source_lineage(), SourceLineage::Workspace(_))
    }));

    let _ = std::fs::remove_dir_all(workspace);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
fn contextual_workspace_escape_becomes_external_local_lineage() {
    let sources = temp_root("contextual-workspace-sources");
    let workspace = sources.join("workspace");
    let root = workspace.join("packages/root");
    let external = sources.join("external");
    let cache = temp_root("contextual-workspace-cache");
    write_package(&root, "root-package", Some("../../../external"));
    write_package(&external, "external-package", None);
    let source_context = ExternalSourceContext::derive(b"workspace-consuming-lock");

    let closure = resolve_workspace_package_closure_in_context(
        &fixture_lineage(),
        SourceRelativePath::parse("packages/root").expect("root member"),
        &workspace,
        source_context.clone(),
        &cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("explicit context should route the workspace escape");

    assert_eq!(closure.graph().packages().len(), 2);
    let external = closure
        .custodies()
        .iter()
        .find(|custody| custody.key().name().as_str() == "external-package")
        .expect("external dependency custody");
    assert!(matches!(
        external.key().source_lineage(),
        SourceLineage::ExternalLocal(lineage)
            if lineage.source_context() == &source_context
    ));

    write_package(&root, "root-package", Some("../../../external/"));
    let malformed = resolve_workspace_package_closure_in_context(
        &fixture_lineage(),
        SourceRelativePath::parse("packages/root").expect("root member"),
        &workspace,
        source_context,
        &cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect_err("a malformed workspace spelling must not switch source lanes");
    assert!(matches!(
        malformed,
        ResolveWorkspacePackageClosureError::Closure(
            PackageSourceClosureResolutionError::Adapter {
                error: ResolveDependencySourceError::InvalidPath { .. },
                ..
            }
        )
    ));

    let _ = std::fs::remove_dir_all(sources);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
fn rejects_workspace_escape_before_resolving_the_target() {
    let workspace = temp_root("escape-workspace");
    let package = workspace.join("packages/root");
    let cache = temp_root("escape-cache");
    std::fs::create_dir_all(&package).expect("create package");
    std::fs::write(
        package.join("build.omg"),
        r#"
            machine build(builder: &mut Build) {
                builder.package("root-package");
                builder.depend(Source::Path { location: "../../../outside" });
            }
            "#,
    )
    .expect("write build file");
    std::fs::write(package.join("main.omg"), "machine root() {}\n").expect("write source");

    let error = resolve_workspace_package_closure(
        &fixture_lineage(),
        SourceRelativePath::parse("packages/root").expect("root member"),
        &workspace,
        &cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect_err("escaping dependency must reject");

    assert!(matches!(
        error,
        ResolveWorkspacePackageClosureError::Closure(
            PackageSourceClosureResolutionError::Adapter {
                error: ResolveDependencySourceError::InvalidPath { .. },
                ..
            }
        )
    ));

    let _ = std::fs::remove_dir_all(workspace);
    let _ = std::fs::remove_dir_all(cache);
}
