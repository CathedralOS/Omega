use super::{make_tree_owner_writable, temp_root, write_package};
use crate::manifest::dependencies::read::DependencySourceRequest;
use crate::resolution::source::{
    resolve_workspace_member_package_source, ResolvePackageSourceError,
};
#[cfg(unix)]
use omega_package_source::SourceResolveError;
use omega_package_source::{
    IdentityError, ImmutableSourceResolution, LocalSourceLimits, SourceLineage,
    WorkspaceLineageIdentity, WorkspaceMemberLineage, WorkspaceMemberPath,
};

#[test]
fn workspace_member_resolution_binds_root_lineage_path_and_member_snapshot() {
    let workspace = temp_root("workspace");
    let member = workspace.join("packages/arithmetic-kernels");
    let cache = temp_root("workspace-cache");
    write_package(&member, "arithmetic-kernels");
    std::fs::write(workspace.join("workspace-only.txt"), "not package source")
        .expect("write workspace-only file");
    std::fs::write(
        member.join("build.omg"),
        r#"
            machine build(builder: &mut Build) {
                builder.package("arithmetic-kernels");
                builder.depend(Source::Git {
                    repository: "https://github.com/CathedralOS/exact-math.git",
                    revision: "main"
                });
            }
            "#,
    )
    .expect("write workspace package declaration and dependency");

    let workspace_root_source =
        SourceLineage::git("https://github.com/CathedralOS/omega-workspace.git")
            .expect("workspace root lineage");
    let member_path =
        WorkspaceMemberPath::parse("packages/arithmetic-kernels").expect("normalized member path");
    let expected_workspace_identity =
        WorkspaceLineageIdentity::from_root_source(&workspace_root_source)
            .expect("workspace identity");
    let source_limits = LocalSourceLimits {
        max_files: 32,
        max_bytes: 4096,
        max_depth: 8,
    };
    let resolved = resolve_workspace_member_package_source(
        &workspace_root_source,
        member_path.clone(),
        &workspace,
        &cache,
        source_limits,
    )
    .expect("resolve workspace member");

    assert_eq!(resolved.key().name().as_str(), "arithmetic-kernels");
    let SourceLineage::Workspace(lineage) = resolved.key().source_lineage() else {
        panic!("workspace member must retain workspace lineage");
    };
    assert_eq!(lineage.workspace_identity(), &expected_workspace_identity);
    assert_eq!(lineage.member_path(), &member_path);
    assert!(matches!(
        resolved.resolution(),
        ImmutableSourceResolution::Workspace { .. }
    ));
    assert_eq!(
        resolved.dependency_requests(),
        [DependencySourceRequest::Git {
            explicit_alias: None,
            repository: "https://github.com/CathedralOS/exact-math.git".to_owned(),
            revision: "main".to_owned(),
            selection: crate::manifest::PackageSelection::Root,
        }]
    );
    assert_eq!(
        resolved.source().canonical_live_root(),
        member.canonicalize().expect("canonical member")
    );
    assert!(resolved.snapshot_root().join("main.omg").is_file());
    assert!(!resolved.snapshot_root().join("workspace-only.txt").exists());
    assert_eq!(resolved.source_limits(), source_limits);
    assert_eq!(
        resolved.clone().into_custody().source_limits(),
        source_limits
    );

    let _ = std::fs::remove_dir_all(&workspace);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn workspace_member_resolution_rejects_member_path_symlink_escape() {
    use std::os::unix::fs::symlink;

    let workspace = temp_root("workspace-member-escape");
    let outside = temp_root("workspace-member-outside");
    let cache = temp_root("workspace-member-escape-cache");
    std::fs::create_dir_all(workspace.join("packages")).expect("create workspace packages");
    write_package(&outside, "outside-package");
    let member = workspace.join("packages/escaped");
    symlink(&outside, &member).expect("create escaping member symlink");

    let storage = omega_package_source::SourceResolverStorage::for_hardened_base(&cache)
        .expect("create retained workspace storage");
    let error = crate::resolution::resolve_workspace_member_package_source_with_storage(
        &SourceLineage::git("https://github.com/CathedralOS/workspace.git")
            .expect("workspace lineage"),
        WorkspaceMemberPath::parse("packages/escaped").expect("member path"),
        &workspace,
        &storage,
        LocalSourceLimits::default(),
    )
    .expect_err("member symlink escape must reject");

    assert!(matches!(
        error,
        ResolvePackageSourceError::WorkspaceMemberEscapesRoot { .. }
    ));
    assert!(
        !storage
            .workspace_members()
            .path()
            .join("local-snapshots")
            .exists(),
        "rejection must occur before snapshot publication custody"
    );

    drop(storage);
    let _ = std::fs::remove_dir_all(&workspace);
    let _ = std::fs::remove_dir_all(&outside);
}

#[cfg(unix)]
#[test]
fn workspace_member_resolution_retains_member_tree_symlink_containment() {
    use std::os::unix::fs::symlink;

    let workspace = temp_root("workspace-tree-escape");
    let member = workspace.join("packages/member");
    let outside = temp_root("workspace-tree-outside");
    let cache = temp_root("workspace-tree-escape-cache");
    write_package(&member, "member-package");
    std::fs::create_dir_all(&outside).expect("create outside directory");
    std::fs::write(outside.join("secret.omg"), "machine Secret::read() {}\n")
        .expect("write outside source");
    symlink(outside.join("secret.omg"), member.join("escaped.omg"))
        .expect("create escaping source symlink");

    let error = resolve_workspace_member_package_source(
        &SourceLineage::git("https://github.com/CathedralOS/workspace.git")
            .expect("workspace lineage"),
        WorkspaceMemberPath::parse("packages/member").expect("member path"),
        &workspace,
        &cache,
        LocalSourceLimits::default(),
    )
    .expect_err("member source symlink escape must reject");

    assert!(matches!(
        error,
        ResolvePackageSourceError::Source(SourceResolveError::SymlinkEscapesRoot { .. })
    ));

    let _ = std::fs::remove_dir_all(&workspace);
    let _ = std::fs::remove_dir_all(&outside);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn workspace_member_resolution_rejects_recursive_workspace_lineage() {
    let root_source = SourceLineage::git("https://github.com/CathedralOS/workspace.git")
        .expect("root source lineage");
    let recursive_source = SourceLineage::Workspace(WorkspaceMemberLineage::new(
        WorkspaceLineageIdentity::from_root_source(&root_source).expect("workspace identity"),
        WorkspaceMemberPath::parse("packages/parent").expect("parent member path"),
    ));

    let error = resolve_workspace_member_package_source(
        &recursive_source,
        WorkspaceMemberPath::parse("packages/child").expect("child member path"),
        temp_root("recursive-workspace"),
        temp_root("recursive-cache"),
        LocalSourceLimits::default(),
    )
    .expect_err("workspace member cannot become a workspace root lineage");

    assert!(matches!(
        error,
        ResolvePackageSourceError::Identity(IdentityError::RecursiveWorkspaceLineage)
    ));
}
