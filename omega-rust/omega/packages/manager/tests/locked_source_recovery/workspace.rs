use super::*;
use package_manager::resolution::graph::{
    resolve_workspace_project_closure_in_context_with_storage,
    resolve_workspace_project_closure_with_storage,
};
use package_source::{ExternalLocalLineage, SourceLineage, SourceRelativePath};

fn workspace(
    tree: &Tree,
    storage: &SourceResolverStorage,
    escaping: bool,
) -> ResolvedPackageSourceClosure {
    let mut dependencies =
        String::from(" builder.depend_as(\"sibling\", Source::Path { location: \"../peer\" });\n");
    if escaping {
        dependencies.push_str(
            " builder.depend_as(\"outside\", Source::Path { location: \"../../outside\" });\n",
        );
        package(&tree.path("outside"), "outside", "");
    }
    package(
        &tree.path("workspace/root"),
        "workspace-root",
        &dependencies,
    );
    package(&tree.path("workspace/peer"), "peer", "");
    let lineage = workspace_lineage(tree, b"explicit-workspace-root");
    let member = SourceRelativePath::parse("root").unwrap();
    if escaping {
        resolve_workspace_project_closure_in_context_with_storage(
            &lineage,
            member,
            tree.path("workspace"),
            ExternalSourceContext::derive(b"workspace-external-dependency-context"),
            storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .unwrap()
    } else {
        resolve_workspace_project_closure_with_storage(
            &lineage,
            member,
            tree.path("workspace"),
            storage,
            LocalSourceLimits::default(),
            PackageSourceClosureLimits::default(),
        )
        .unwrap()
    }
}

fn workspace_lineage(tree: &Tree, context: &[u8]) -> SourceLineage {
    SourceLineage::ExternalLocal(
        ExternalLocalLineage::canonicalize(
            tree.path("workspace"),
            ExternalSourceContext::derive(context),
        )
        .unwrap(),
    )
}

#[test]
fn explicit_workspace_sibling_recovery_uses_fresh_sources_and_rejects_member_drift() {
    let tree = Tree::new();
    let (lock, request) = {
        let storage = tree.storage("old-cache");
        let closure = workspace(&tree, &storage, false);
        capture_lock(&closure, &tree.path("build"))
    };
    let text = lock.canonical_text().unwrap();
    let storage = tree.storage("new-cache");
    let fresh = recover_locked_sources(
        &lock,
        TARGET,
        &request,
        &storage,
        LockedSourceRecoveryOptions::default(),
    )
    .unwrap();
    assert_fresh_matches(&lock, &fresh);
    assert_eq!(fresh.custodies().len(), 2);
    let fresh_cache = fs::canonicalize(tree.path("new-cache")).unwrap();
    for custody in fresh.custodies() {
        assert!(matches!(
            custody.key().source_lineage(),
            SourceLineage::Workspace(_)
        ));
        assert!(custody.snapshot_root().starts_with(&fresh_cache));
    }
    fs::write(
        tree.path("workspace/peer/main.omg"),
        "pub machine value() -> u64 { 8 }\n",
    )
    .unwrap();
    assert!(matches!(
        recover_locked_sources(
            &lock,
            TARGET,
            &request,
            &storage,
            LockedSourceRecoveryOptions::default(),
        ),
        Err(RecoverLockedSourcesError::Resolution(_))
    ));
    fs::rename(tree.path("workspace/peer"), tree.path("unavailable-peer")).unwrap();
    assert!(matches!(
        recover_locked_sources(
            &lock,
            TARGET,
            &request,
            &storage,
            LockedSourceRecoveryOptions::default(),
        ),
        Err(RecoverLockedSourcesError::Resolution(_))
    ));
    assert_eq!(lock.canonical_text().unwrap(), text);
}

#[test]
fn workspace_escape_retains_context_and_rejects_changed_requested_lineage_or_root() {
    let tree = Tree::new();
    let (lock, request) = {
        let storage = tree.storage("old-cache");
        let closure = workspace(&tree, &storage, true);
        capture_lock(&closure, &tree.path("build"))
    };
    let storage = tree.storage("new-cache");
    let fresh = recover_locked_sources(
        &lock,
        TARGET,
        &request,
        &storage,
        LockedSourceRecoveryOptions::default(),
    )
    .unwrap();
    assert_fresh_matches(&lock, &fresh);
    assert_eq!(fresh.custodies().len(), 3);
    let outside = fresh
        .custodies()
        .iter()
        .find(|source| source.key().name().as_str() == "outside")
        .unwrap();
    let SourceLineage::ExternalLocal(lineage) = outside.key().source_lineage() else {
        panic!("escaping dependency must retain external-local context");
    };
    assert_eq!(
        lineage.source_context(),
        &ExternalSourceContext::derive(b"workspace-external-dependency-context")
    );
    assert_eq!(
        lineage.canonical_absolute_path(),
        fs::canonicalize(tree.path("outside")).unwrap()
    );

    let mut changed_context = request.clone();
    let PackageRootSourceRequest::WorkspaceMember {
        workspace_root_source,
        ..
    } = &mut changed_context
    else {
        panic!("fixture must retain an explicit workspace root");
    };
    *workspace_root_source = workspace_lineage(&tree, b"another-consuming-context");
    let mut changed_root = request.clone();
    let PackageRootSourceRequest::WorkspaceMember {
        requested_workspace_root,
        ..
    } = &mut changed_root
    else {
        unreachable!();
    };
    *requested_workspace_root = tree.path("another-workspace");
    for changed in [&changed_context, &changed_root] {
        assert!(matches!(
            recover_locked_sources(
                &lock,
                TARGET,
                changed,
                &storage,
                LockedSourceRecoveryOptions::default(),
            ),
            Err(RecoverLockedSourcesError::Resolution(_))
        ));
    }
    fs::write(
        tree.path("outside/main.omg"),
        "pub machine value() -> u64 { 9 }\n",
    )
    .unwrap();
    assert!(matches!(
        recover_locked_sources(
            &lock,
            TARGET,
            &request,
            &storage,
            LockedSourceRecoveryOptions::default(),
        ),
        Err(RecoverLockedSourcesError::Resolution(_))
    ));
}
