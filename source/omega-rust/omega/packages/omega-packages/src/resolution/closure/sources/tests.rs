use super::*;
use crate::{
    GitTransportProfile, PackageSourceClosureLimitKind, PackageSourceReviewLimits,
    PackageTriageDisposition, PackageTriageReason, ReviewOnlyCapabilityConflictChange,
    ReviewOnlyCapabilityConflictLimits, assemble_update_source_review,
    compare_review_only_capabilities, compile_resolved_package_reviews, triage_review_update,
};
use omega_package_review::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
    PackageReviewDangerousAuthorityClass, PackageReviewNominalOwner,
    PackageReviewSourceLocationRole,
};
use std::collections::BTreeSet;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../tests/fixtures/packages")
}

fn temp_root(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time follows Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "omega-package-source-adapter-{name}-{}-{stamp}",
        std::process::id()
    ))
}

fn fixture_lineage() -> SourceLineage {
    SourceLineage::git("https://github.com/CathedralOS/package-fixtures.git")
        .expect("fixture lineage")
}

fn write_package(root: &Path, name: &str, dependency: Option<&str>) {
    std::fs::create_dir_all(root).expect("create package");
    let dependency = dependency
        .map(|location| {
            let location = location.replace('\\', "\\\\").replace('"', "\\\"");
            format!("    builder.depend(Source::Path {{ location: \"{location}\" }});\n")
        })
        .unwrap_or_default();
    std::fs::write(
            root.join("build.omg"),
            format!(
                "machine build(builder: &mut Build) {{\n    builder.package(\"{name}\");\n{dependency}}}\n"
            ),
        )
        .expect("write build file");
    std::fs::write(root.join("main.omg"), "machine root() {}\n").expect("write source");
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
        .expect("read test Git HEAD");
    assert!(
        output.status.success(),
        "test Git rev-parse failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("Git object ID is UTF-8")
        .trim()
        .to_owned()
}

#[test]
fn resolves_explicit_workspace_path_closure() {
    let cache_base = temp_root("fixture-cache");
    std::fs::create_dir_all(&cache_base).expect("create private storage base");
    let storage = SourceResolverStorage::create_beneath(&cache_base)
        .expect("create production-shaped private resolver storage");
    let closure = resolve_workspace_package_closure_with_storage(
        &fixture_lineage(),
        WorkspaceMemberPath::parse("graph-workbench").expect("root member"),
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
        WorkspaceMemberPath::parse("packages/root").expect("root member"),
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
fn resolves_external_local_closure_across_directory_boundaries_in_one_context() {
    let sources = temp_root("external-sources");
    let first_cache = temp_root("external-first-cache");
    let second_cache = temp_root("external-second-cache");
    write_package(&sources.join("root"), "root-package", Some("../middle"));
    let leaf = sources.join("leaf");
    let leaf_location = leaf.display().to_string();
    write_package(
        &sources.join("middle"),
        "middle-package",
        Some(&leaf_location),
    );
    write_package(&leaf, "leaf-package", None);
    let first_context = ExternalSourceContext::derive(b"first-consuming-lock");

    let first = resolve_external_local_package_closure(
        sources.join("root"),
        first_context.clone(),
        &first_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve context-bound external closure");

    assert_eq!(first.graph().packages().len(), 3);
    assert!(first.custodies().iter().all(|custody| {
        matches!(
            custody.key().source_lineage(),
            SourceLineage::ExternalLocal(lineage)
                if lineage.source_context() == &first_context
        )
    }));
    let first_root_binding = first.source_requests().root();
    let PackageRootSourceRequest::ExternalLocal {
        requested_root,
        source_context,
    } = first_root_binding.request()
    else {
        panic!("external adapter retains its root request")
    };
    assert_eq!(requested_root, &sources.join("root"));
    assert_eq!(source_context, &first_context);

    let second_context = ExternalSourceContext::derive(b"second-consuming-lock");
    let second = resolve_external_local_package_closure(
        sources.join("root"),
        second_context,
        &second_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve same sources in a different consuming context");
    for first_custody in first.custodies() {
        let second_custody = second
            .custodies()
            .iter()
            .find(|custody| custody.key().name() == first_custody.key().name())
            .expect("same declared package in second closure");
        assert_ne!(first_custody.key(), second_custody.key());
        assert_eq!(first_custody.resolution(), second_custody.resolution());
    }

    let _ = std::fs::remove_dir_all(sources);
    let _ = std::fs::remove_dir_all(first_cache);
    let _ = std::fs::remove_dir_all(second_cache);
}

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

#[test]
fn git_update_escalating_to_process_authority_blocks_and_requests_source_audit() {
    let repository = temp_root("git-process-authority-repository");
    let baseline_cache = temp_root("git-process-authority-baseline-cache");
    let candidate_cache = temp_root("git-process-authority-candidate-cache");
    let compiler_workspace = temp_root("git-process-authority-compiler-workspace");
    let process_fixture = fixture_root().join("process-exit");
    std::fs::create_dir_all(&repository).expect("create process-authority repository");
    std::fs::copy(
        process_fixture.join("build.omg"),
        repository.join("build.omg"),
    )
    .expect("copy stable package declaration");
    std::fs::write(
        repository.join("main.omg"),
        r#"use omega::language::std::console;

pub machine terminate(console: Console, return_code: i32)
{
}
"#,
    )
    .expect("write inert baseline package");
    run_test_git(&repository, ["init", "--quiet"]);
    run_test_git(
        &repository,
        ["config", "user.email", "omega@example.invalid"],
    );
    run_test_git(&repository, ["config", "user.name", "Omega Tests"]);
    run_test_git(&repository, ["add", "."]);
    run_test_git(
        &repository,
        ["commit", "--quiet", "-m", "inert process boundary"],
    );
    let baseline_revision = test_git_head(&repository);

    std::fs::copy(
        process_fixture.join("main.omg"),
        repository.join("main.omg"),
    )
    .expect("copy canonical process-authority candidate");
    run_test_git(&repository, ["add", "main.omg"]);
    run_test_git(
        &repository,
        ["commit", "--quiet", "-m", "exercise process authority"],
    );
    let candidate_revision = test_git_head(&repository);
    assert_ne!(baseline_revision, candidate_revision);

    let canonical_lineage = "https://github.com/CathedralOS/process-exit.git";
    let baseline_request = GitSourceRequest::for_local_test_repository_with_lineage(
        &repository,
        Some(baseline_revision.clone()),
        canonical_lineage,
    )
    .expect("construct exact baseline Git request");
    let candidate_request = GitSourceRequest::for_local_test_repository_with_lineage(
        &repository,
        Some(candidate_revision.clone()),
        canonical_lineage,
    )
    .expect("construct exact candidate Git request");
    let baseline_sources = resolve_git_package_closure(
        &baseline_request,
        &baseline_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve baseline Git custody");
    let candidate_sources = resolve_git_package_closure(
        &candidate_request,
        &candidate_cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("resolve candidate Git custody");

    assert_eq!(
        baseline_sources.graph().root(),
        candidate_sources.graph().root(),
        "declared package identity and canonical Git lineage stay stable"
    );
    let baseline_custody = baseline_sources
        .custody(baseline_sources.graph().root())
        .expect("baseline root custody");
    let candidate_custody = candidate_sources
        .custody(candidate_sources.graph().root())
        .expect("candidate root custody");
    assert_ne!(
        baseline_custody.resolution(),
        candidate_custody.resolution()
    );
    assert_ne!(
        baseline_custody.snapshot_root(),
        candidate_custody.snapshot_root()
    );
    for (closure, expected_revision) in [
        (&baseline_sources, baseline_revision.as_str()),
        (&candidate_sources, candidate_revision.as_str()),
    ] {
        let PackageRootSourceRequest::Git(request) = closure.source_requests().root().request()
        else {
            panic!("authority update root must retain its exact Git request")
        };
        assert_eq!(request.requested_locator(), canonical_lineage);
        assert_eq!(request.requested_revision(), expected_revision);
        assert_eq!(request.transport_profile(), GitTransportProfile::TestFile);
        assert!(closure.source_requests().dependencies().next().is_none());
    }

    let baseline_reviews =
        compile_resolved_package_reviews(&baseline_sources, "windows_x64", &compiler_workspace)
            .expect("compile baseline package evidence");
    let candidate_reviews =
        compile_resolved_package_reviews(&candidate_sources, "windows_x64", &compiler_workspace)
            .expect("compile candidate package evidence");
    let baseline = baseline_reviews
        .review(baseline_sources.graph().root())
        .expect("baseline root review");
    let candidate = candidate_reviews
        .review(candidate_sources.graph().root())
        .expect("candidate root review");
    assert!(baseline.projection().dangerous_authorities().is_empty());
    let [authority] = candidate.projection().dangerous_authorities() else {
        panic!("candidate must derive one effective dangerous authority")
    };
    assert_eq!(
        authority.class(),
        PackageReviewDangerousAuthorityClass::Process
    );
    assert_eq!(authority.service().path(), "Console");
    assert!(matches!(
        authority.service().owner(),
        PackageReviewNominalOwner::ToolchainSource(_)
    ));

    let conflicts = compare_review_only_capabilities(
        &baseline_reviews,
        &candidate_reviews,
        &candidate_sources,
        ReviewOnlyCapabilityConflictLimits::default(),
    )
    .expect("compare compiler-derived authority escalation");
    let [package] = conflicts.packages() else {
        panic!("authority escalation must affect exactly one package")
    };
    assert_eq!(package.conflicts().len(), 2);
    let dangerous = package
        .conflicts()
        .iter()
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::DangerousAuthority)
        .expect("effective authority must produce its own canonical conflict");
    assert_eq!(
        dangerous.change(),
        ReviewOnlyCapabilityConflictChange::Added
    );
    assert_eq!(dangerous.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert!(dangerous.is_blocking());
    assert!(dangerous.baseline_row().is_none());
    assert!(dangerous.baseline_source().is_none());
    assert!(dangerous.candidate_row().is_some());
    let dangerous_locations = dangerous
        .candidate_source()
        .and_then(|source| source.authored_locations())
        .expect("dangerous authority retains compiler-owned source coordinates");
    assert!(dangerous_locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::AuthorityDeclaration
    }));
    assert!(dangerous_locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::AuthorityExposure
            && location.relative_path() == "main.omg"
    }));

    let callable = package
        .conflicts()
        .iter()
        .find(|conflict| conflict.kind() == PackageReviewCanonicalRowKind::Callable)
        .expect("changed declared and realized reach must change the callable row");
    assert_eq!(
        callable.change(),
        ReviewOnlyCapabilityConflictChange::Changed
    );
    assert_eq!(callable.risk(), PackageReviewCanonicalRowRisk::Blocking);
    assert!(callable.is_blocking());
    for source in [
        callable
            .baseline_source()
            .expect("baseline callable source"),
        callable
            .candidate_source()
            .expect("candidate callable source"),
    ] {
        assert!(
            source
                .authored_locations()
                .expect("callable source locations")
                .iter()
                .any(|location| {
                    location.role() == PackageReviewSourceLocationRole::Declaration
                        && location.relative_path() == "main.omg"
                })
        );
    }

    let triage = triage_review_update(&baseline_reviews, &candidate_reviews, &BTreeSet::new());
    let [decision] = triage.decisions() else {
        panic!("authority escalation must produce one package decision")
    };
    assert_eq!(
        decision.disposition(),
        PackageTriageDisposition::BlockedCapabilityChange
    );
    assert_eq!(
        decision.reasons(),
        [
            PackageTriageReason::CapabilityOrApiChanged,
            PackageTriageReason::SourceChanged,
            PackageTriageReason::BuildObservationChanged,
            PackageTriageReason::RetainedDangerousAuthority(
                PackageReviewDangerousAuthorityClass::Process,
            ),
        ]
    );

    let review = assemble_update_source_review(
        &baseline_reviews,
        &candidate_reviews,
        baseline_sources.custodies(),
        &candidate_sources,
        PackageSourceReviewLimits::default(),
    )
    .expect("assemble exact source review for blocked escalation");
    let [patch] = review.source_patches() else {
        panic!("blocked authority escalation must carry one source patch")
    };
    assert_eq!(patch.baseline_key(), Some(baseline.key()));
    assert_eq!(patch.candidate_key(), candidate.key());
    assert_eq!(patch.changed_entries(), 1);
    assert!(!patch.requires_standalone_audit());
    assert!(patch.as_str().contains("mode update\n"));
    assert!(patch.as_str().contains("entry main.omg\n"));
    assert!(patch.as_str().contains("added lf reaches Console"));
    assert!(patch.as_str().contains("added lf invokes console;"));
    assert!(
        patch
            .as_str()
            .contains("added lf     console.exit_process(return_code);")
    );

    let _ = std::fs::remove_dir_all(repository);
    let _ = std::fs::remove_dir_all(baseline_cache);
    let _ = std::fs::remove_dir_all(candidate_cache);
    let _ = std::fs::remove_dir_all(compiler_workspace);
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
        WorkspaceMemberPath::parse("packages/root").expect("root member"),
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
        WorkspaceMemberPath::parse("packages/root").expect("root member"),
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
        WorkspaceMemberPath::parse("packages/root").expect("root member"),
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

#[test]
fn propagates_closure_resource_ceilings() {
    let cache = temp_root("limit-cache");
    let error = resolve_workspace_package_closure(
        &fixture_lineage(),
        WorkspaceMemberPath::parse("graph-workbench").expect("root member"),
        fixture_root(),
        &cache,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits {
            max_packages: 1,
            max_dependency_requests: 8,
            max_depth: 8,
        },
    )
    .expect_err("package ceiling must reject");

    assert!(matches!(
        error,
        ResolveWorkspacePackageClosureError::Closure(
            PackageSourceClosureResolutionError::LimitExceeded {
                kind: PackageSourceClosureLimitKind::Packages,
                ..
            }
        )
    ));

    let _ = std::fs::remove_dir_all(cache);
}
