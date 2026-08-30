use super::{make_tree_owner_writable, temp_root, write_package};
use crate::manifest::dependencies::read::DependencySourceRequest;
use crate::resolution::source::{
    GitPackageSourceRequest, ResolvePackageSourceError, resolve_external_local_package_source,
    resolve_git_package_source, resolve_selected_git_package_source_with_storage,
};
use omega_package_source::{ExternalSourceContext, GitSourceRequest, LocalSourceLimits};
use std::path::Path;
use std::process::Command;

fn write_workspace(root: &Path, members: &[&str]) {
    std::fs::create_dir_all(root).expect("create workspace root");
    let declarations = members
        .iter()
        .map(|member| format!("    builder.member(\"{member}\");\n"))
        .collect::<String>();
    std::fs::write(
        root.join("build.omg"),
        format!("machine build(builder: &mut Build) {{\n{declarations}}}\n"),
    )
    .expect("write workspace declaration");
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

#[test]
fn named_git_binding_rejects_missing_and_duplicate_declared_names() {
    let repository = temp_root("git-named-errors-repository");
    let cache = temp_root("git-named-errors-cache");
    write_workspace(&repository, &["packages/first", "packages/second"]);
    write_package(&repository.join("packages/first"), "same-name");
    write_package(&repository.join("packages/second"), "same-name");
    run_test_git(&repository, ["init", "--quiet"]);
    run_test_git(
        &repository,
        ["config", "user.email", "omega@example.invalid"],
    );
    run_test_git(&repository, ["config", "user.name", "Omega Tests"]);
    run_test_git(&repository, ["add", "."]);
    run_test_git(&repository, ["commit", "--quiet", "-m", "workspace"]);
    let acquisition = GitSourceRequest::for_local_test_repository_with_lineage(
        &repository,
        None,
        "https://github.com/CathedralOS/named-errors.git",
    )
    .expect("local Git request");
    let storage = omega_package_source::SourceResolverStorage::for_hardened_base(&cache)
        .expect("retained storage");

    let missing = resolve_selected_git_package_source_with_storage(
        &GitPackageSourceRequest::new(
            acquisition.clone(),
            crate::manifest::PackageSelection::Named(
                crate::manifest::PackageName::parse("missing").unwrap(),
            ),
        ),
        &storage,
        LocalSourceLimits::default(),
    )
    .expect_err("missing declared package rejects");
    assert!(matches!(
        missing,
        ResolvePackageSourceError::GitWorkspaceSelection(
            crate::resolution::source::git::workspace::GitWorkspaceSelectionError::PackageMissing {
                package_name
            }
        ) if package_name.as_str() == "missing"
    ));

    let duplicate = resolve_selected_git_package_source_with_storage(
        &GitPackageSourceRequest::new(
            acquisition,
            crate::manifest::PackageSelection::Named(
                crate::manifest::PackageName::parse("same-name").unwrap(),
            ),
        ),
        &storage,
        LocalSourceLimits::default(),
    )
    .expect_err("duplicate declared package name rejects");
    assert!(matches!(
        duplicate,
        ResolvePackageSourceError::GitWorkspaceSelection(
            crate::resolution::source::git::workspace::GitWorkspaceSelectionError::PackageDuplicate {
                package_name,
                member_paths,
            }
        ) if package_name.as_str() == "same-name" && member_paths.len() == 2
    ));

    drop(storage);
    let _ = std::fs::remove_dir_all(&repository);
    make_tree_owner_writable(&cache);
    let _ = std::fs::remove_dir_all(&cache);
}

#[cfg(unix)]
#[test]
fn named_git_binding_rejects_symlink_member_navigation() {
    use std::os::unix::fs::symlink;

    let repository = temp_root("git-named-symlink-repository");
    let cache = temp_root("git-named-symlink-cache");
    write_workspace(&repository, &["packages/linked"]);
    write_package(&repository.join("actual"), "linked-package");
    std::fs::create_dir_all(repository.join("packages")).expect("create packages directory");
    symlink("../actual", repository.join("packages/linked")).expect("create member symlink");
    run_test_git(&repository, ["init", "--quiet"]);
    run_test_git(
        &repository,
        ["config", "user.email", "omega@example.invalid"],
    );
    run_test_git(&repository, ["config", "user.name", "Omega Tests"]);
    run_test_git(&repository, ["add", "."]);
    run_test_git(&repository, ["commit", "--quiet", "-m", "workspace"]);
    let acquisition = GitSourceRequest::for_local_test_repository_with_lineage(
        &repository,
        None,
        "https://github.com/CathedralOS/named-symlink.git",
    )
    .expect("local Git request");
    let storage = omega_package_source::SourceResolverStorage::for_hardened_base(&cache)
        .expect("retained storage");

    let error = resolve_selected_git_package_source_with_storage(
        &GitPackageSourceRequest::new(
            acquisition,
            crate::manifest::PackageSelection::Named(
                crate::manifest::PackageName::parse("linked-package").unwrap(),
            ),
        ),
        &storage,
        LocalSourceLimits::default(),
    )
    .expect_err("symlink member navigation rejects");
    assert!(matches!(
        error,
        ResolvePackageSourceError::Source(
            omega_package_source::SourceResolveError::GitTreeInvalid { path, .. }
        ) if path == b"packages/linked/build.omg"
    ));

    drop(storage);
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
    let error =
        crate::resolution::graph::reconcile::resolve_package_source_closure::<std::convert::Infallible, _>(
            crate::resolution::graph::PackageRootSourceRequest::ExternalLocal {
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
