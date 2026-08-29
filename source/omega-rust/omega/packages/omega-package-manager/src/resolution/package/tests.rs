use super::{
    ResolvePackageSourceError, resolve_external_local_package_source, resolve_git_package_source,
    resolve_workspace_member_package_source,
};
use crate::manifest::declaration::PackageDeclarationError;
use crate::manifest::dependency_projection::{DependencyProjectionError, DependencySourceRequest};
#[cfg(unix)]
use crate::resolution::SourceResolveError;
use crate::resolution::identity::{
    ExternalSourceContext, IdentityError, ImmutableSourceResolution, SourceLineage,
    WorkspaceLineageIdentity, WorkspaceMemberLineage, WorkspaceMemberPath,
};
use crate::resolution::{GitSourceRequest, LocalSourceLimits};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should follow Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "omega-package-source-{name}-{}-{stamp}",
        std::process::id()
    ))
}

fn write_package(root: &Path, name: &str) {
    std::fs::create_dir_all(root).expect("create package root");
    std::fs::write(
        root.join("build.omg"),
        format!("machine build(builder: &mut Build) {{\n    builder.package(\"{name}\");\n}}\n"),
    )
    .expect("write package declaration");
    std::fs::write(root.join("main.omg"), "machine Main::main() {}\n")
        .expect("write package source");
}

#[test]
fn external_local_resolution_uses_declared_name_and_immutable_snapshot() {
    let root = temp_root("external");
    let cache = temp_root("external-cache");
    write_package(&root, "arithmetic-kernels");

    let resolved = resolve_external_local_package_source(
        &root,
        &cache,
        LocalSourceLimits::default(),
        ExternalSourceContext::derive(b"consumer-lock"),
    )
    .expect("resolve declared local package");

    assert_eq!(resolved.key().name().as_str(), "arithmetic-kernels");
    assert!(matches!(
        resolved.key().source_lineage(),
        SourceLineage::ExternalLocal(_)
    ));
    assert!(matches!(
        resolved.resolution(),
        ImmutableSourceResolution::ExternalLocal { .. }
    ));
    assert!(resolved.dependency_requests().is_empty());
    assert_ne!(
        resolved.snapshot_root(),
        root.canonicalize().expect("canonical live root")
    );
    assert_eq!(resolved.snapshot_root(), resolved.source().snapshot_root());
    let identity = resolved.identity();
    assert_eq!(identity.key(), resolved.key());
    assert_eq!(identity.resolution(), resolved.resolution());

    let _ = std::fs::remove_dir_all(&root);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn external_local_context_changes_key_without_changing_source_resolution() {
    let root = temp_root("context");
    let cache = temp_root("context-cache");
    write_package(&root, "arithmetic-kernels");

    let first = resolve_external_local_package_source(
        &root,
        &cache,
        LocalSourceLimits::default(),
        ExternalSourceContext::derive(b"consumer-a"),
    )
    .expect("resolve first context");
    let second = resolve_external_local_package_source(
        &root,
        &cache,
        LocalSourceLimits::default(),
        ExternalSourceContext::derive(b"consumer-b"),
    )
    .expect("resolve second context");

    assert_ne!(first.key(), second.key());
    assert_eq!(first.resolution(), second.resolution());
    assert_eq!(first.snapshot_root(), second.snapshot_root());

    let _ = std::fs::remove_dir_all(&root);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

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

    let storage = crate::SourceResolverStorage::for_hardened_base(&cache)
        .expect("create retained workspace storage");
    let error = crate::resolve_workspace_member_package_source_with_storage(
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

#[test]
fn declaration_failure_does_not_fall_back_to_repository_name() {
    let root = temp_root("missing-declaration");
    let cache = temp_root("missing-declaration-cache");
    std::fs::create_dir_all(&root).expect("create source");
    std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");

    let error = resolve_external_local_package_source(
        &root,
        &cache,
        LocalSourceLimits::default(),
        ExternalSourceContext::derive(b"consumer-lock"),
    )
    .expect_err("missing declaration must reject");

    assert!(matches!(
        error,
        ResolvePackageSourceError::Declaration(PackageDeclarationError::MissingBuildFile { .. })
    ));

    let _ = std::fs::remove_dir_all(&root);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn application_role_cannot_be_bound_as_a_package_source() {
    let root = temp_root("application-role");
    let cache = temp_root("application-role-cache");
    std::fs::create_dir_all(&root).expect("create source");
    std::fs::write(
        root.join("build.omg"),
        "machine build(builder: &mut Build) {\n    builder.application(\"artifact-root\");\n}\n",
    )
    .expect("write application declaration");
    std::fs::write(root.join("main.omg"), "machine Main::main() {}\n").expect("write source");

    let error = resolve_external_local_package_source(
        &root,
        &cache,
        LocalSourceLimits::default(),
        ExternalSourceContext::derive(b"consumer-lock"),
    )
    .expect_err("an application must not become an importable package");

    assert!(matches!(
        error,
        ResolvePackageSourceError::Declaration(
            PackageDeclarationError::ExpectedPackageDeclaration {
                found: crate::manifest::declaration::BuildDeclarationKind::Application
            }
        )
    ));

    let _ = std::fs::remove_dir_all(&root);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn source_custody_projects_only_canonical_dependency_rows() {
    let root = temp_root("dependencies");
    let cache = temp_root("dependencies-cache");
    write_package(&root, "application");
    std::fs::write(
        root.join("build.omg"),
        r#"
            machine build(builder: &mut Build) {
                builder.package("application");
                builder.depend(Source::Path { location: "../local-library" });
                builder.depend(Source::Git {
                    repository: "https://github.com/CathedralOS/arithmetic-kernels.git",
                    revision: "main"
                });
            }
            "#,
    )
    .expect("write dependency projection");

    let resolved = resolve_external_local_package_source(
        &root,
        &cache,
        LocalSourceLimits::default(),
        ExternalSourceContext::derive(b"consumer-lock"),
    )
    .expect("resolve package and dependency projection");

    assert_eq!(
        resolved.dependency_requests(),
        [
            DependencySourceRequest::Path {
                explicit_alias: None,
                location: "../local-library".to_owned(),
            },
            DependencySourceRequest::Git {
                explicit_alias: None,
                repository: "https://github.com/CathedralOS/arithmetic-kernels.git".to_owned(),
                revision: "main".to_owned(),
            },
        ]
    );

    let _ = std::fs::remove_dir_all(&root);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn source_custody_rejects_hidden_dependency_requests() {
    let root = temp_root("hidden-dependency");
    let cache = temp_root("hidden-dependency-cache");
    write_package(&root, "application");
    std::fs::write(
        root.join("build.omg"),
        r#"
            machine helper(builder: &mut Build) {
                builder.depend(Source::Path { location: "../hidden" });
            }
            machine build(builder: &mut Build) {
                builder.package("application");
                helper(builder);
            }
            "#,
    )
    .expect("write hidden dependency");

    let error = resolve_external_local_package_source(
        &root,
        &cache,
        LocalSourceLimits::default(),
        ExternalSourceContext::derive(b"consumer-lock"),
    )
    .expect_err("hidden dependency request must reject");
    assert!(matches!(
        error,
        ResolvePackageSourceError::DependencyProjection(
            DependencyProjectionError::UnsupportedDependencyShape
        )
    ));

    let _ = std::fs::remove_dir_all(&root);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_binding_normalizes_known_transport_without_using_repository_name() {
    let repository = temp_root("git-binding-repository");
    let cache = temp_root("git-binding-cache");
    write_package(&repository, "declared-package");
    run_test_git(&repository, ["init", "--quiet"]);
    run_test_git(
        &repository,
        ["config", "user.email", "omega@example.invalid"],
    );
    run_test_git(&repository, ["config", "user.name", "Omega Tests"]);
    run_test_git(&repository, ["add", "."]);
    run_test_git(&repository, ["commit", "--quiet", "-m", "package"]);
    let revision = test_git_head(&repository);
    let https_request = GitSourceRequest::for_local_test_repository_with_lineage(
        &repository,
        Some(revision.clone()),
        "https://github.com/CathedralOS/repository-name-does-not-match.git",
    )
    .expect("HTTPS request");
    let ssh_request = GitSourceRequest::for_local_test_repository_with_lineage(
        &repository,
        Some(revision),
        "git@github.com:cathedralos/repository-name-does-not-match.git",
    )
    .expect("SSH request");
    let https = resolve_git_package_source(&https_request, &cache, LocalSourceLimits::default())
        .expect("resolve HTTPS-lineage source");
    let ssh = resolve_git_package_source(&ssh_request, &cache, LocalSourceLimits::default())
        .expect("resolve SSH-lineage source");

    assert_eq!(https.key(), ssh.key());
    assert_eq!(https.key().name().as_str(), "declared-package");
    assert_eq!(https.resolution(), ssh.resolution());

    let _ = std::fs::remove_dir_all(&repository);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

fn run_test_git<I, S>(directory: &Path, args: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = Command::new("git")
        .current_dir(directory)
        .args(args)
        .output()
        .expect("spawn test Git");
    assert!(
        output.status.success(),
        "test Git command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn test_git_head(directory: &Path) -> String {
    let output = Command::new("git")
        .current_dir(directory)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("read test Git head");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[test]
fn conflicting_git_revisions_report_real_custody_and_both_request_paths() {
    let repository = temp_root("git-reconciliation-repository");
    write_package(&repository, "shared-dependency");
    run_test_git(&repository, ["init", "--quiet"]);
    run_test_git(
        &repository,
        ["config", "user.email", "omega@example.invalid"],
    );
    run_test_git(&repository, ["config", "user.name", "Omega Tests"]);
    run_test_git(&repository, ["add", "."]);
    run_test_git(&repository, ["commit", "--quiet", "-m", "first"]);
    let first_revision = test_git_head(&repository);
    std::fs::write(
        repository.join("main.omg"),
        "machine Main::main() {}\nmachine Main::changed() {}\n",
    )
    .expect("change dependency source");
    run_test_git(&repository, ["add", "main.omg"]);
    run_test_git(&repository, ["commit", "--quiet", "-m", "second"]);
    let second_revision = test_git_head(&repository);

    let root = temp_root("git-reconciliation-root");
    std::fs::create_dir_all(&root).expect("create reconciliation root");
    let canonical_repository = "https://github.com/CathedralOS/reconciliation-probe.git";
    std::fs::write(
        root.join("build.omg"),
        format!(
            r#"machine build(builder: &mut Build) {{
    builder.package("reconciliation-root");
    builder.depend_as("first_revision", Source::Git {{
        repository: "{canonical_repository}",
        revision: "{first_revision}"
    }});
    builder.depend_as("second_revision", Source::Git {{
        repository: "{canonical_repository}",
        revision: "{second_revision}"
    }});
}}
"#,
        ),
    )
    .expect("write conflicting root requests");
    std::fs::write(root.join("main.omg"), "machine Main::main() {}\n")
        .expect("write reconciliation root source");

    let cache = temp_root("git-reconciliation-cache");
    let source_limits = LocalSourceLimits::default();
    let first_request = GitSourceRequest::for_local_test_repository_with_lineage(
        &repository,
        Some(first_revision.clone()),
        canonical_repository,
    )
    .expect("validate first local Git fixture request");
    let first = resolve_git_package_source(&first_request, cache.join("first"), source_limits)
        .expect("bind first declared package custody")
        .into_custody();
    let second_request = GitSourceRequest::for_local_test_repository_with_lineage(
        &repository,
        Some(second_revision.clone()),
        canonical_repository,
    )
    .expect("validate second local Git fixture request");
    let second = resolve_git_package_source(&second_request, cache.join("second"), source_limits)
        .expect("bind second declared package custody")
        .into_custody();
    assert_eq!(first.key(), second.key());
    assert_ne!(first.resolution(), second.resolution());
    assert_ne!(first.snapshot_root(), second.snapshot_root());

    let source_context = ExternalSourceContext::derive(b"real-custody-reconciliation");
    let root_custody = resolve_external_local_package_source(
        &root,
        cache.join("root"),
        source_limits,
        source_context.clone(),
    )
    .expect("resolve root custody")
    .into_custody();
    let error = crate::resolution::closure::reconciliation::resolve_package_source_closure::<
        std::convert::Infallible,
        _,
    >(
        crate::resolution::closure::PackageRootSourceRequest::ExternalLocal {
            requested_root: root.clone(),
            source_context,
        },
        root_custody,
        |_, request| {
            let DependencySourceRequest::Git { revision, .. } = request else {
                unreachable!("root authors only Git requests")
            };
            Ok(if revision == &first_revision {
                first.clone()
            } else {
                assert_eq!(revision, &second_revision);
                second.clone()
            })
        },
    )
    .expect_err("one package key cannot reconcile two immutable revisions");

    let [conflict] = error.conflicts().expect("exact custody conflict") else {
        panic!("one package key must conflict")
    };
    assert_eq!(conflict.key(), first.key());
    let [first_candidate, second_candidate] = conflict.candidates() else {
        panic!("both immutable custodies must be retained")
    };
    assert_ne!(
        first_candidate.custody().resolution(),
        second_candidate.custody().resolution()
    );
    let mut request_rows = conflict
        .candidates()
        .iter()
        .flat_map(|candidate| candidate.requesting_paths())
        .map(|path| {
            let [step] = path.steps() else {
                panic!("dependency conflict path must have one root step")
            };
            (step.dependency_index(), step.alias().as_str().to_owned())
        })
        .collect::<Vec<_>>();
    request_rows.sort();
    assert_eq!(
        request_rows,
        vec![
            (0, "first_revision".to_owned()),
            (1, "second_revision".to_owned())
        ]
    );

    let _ = std::fs::remove_dir_all(&repository);
    let _ = std::fs::remove_dir_all(&root);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
fn make_tree_owner_writable(root: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut directories = vec![root.to_path_buf()];
    let mut cursor = 0;
    while cursor < directories.len() {
        let directory = directories[cursor].clone();
        cursor += 1;
        if let Ok(entries) = std::fs::read_dir(&directory) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    directories.push(path);
                }
            }
        }
    }
    for directory in directories.into_iter().rev() {
        if let Ok(metadata) = std::fs::symlink_metadata(&directory) {
            let mut permissions = metadata.permissions();
            permissions.set_mode(permissions.mode() | 0o700);
            let _ = std::fs::set_permissions(directory, permissions);
        }
    }
}

#[cfg(not(unix))]
fn make_tree_owner_writable(_root: &Path) {}
