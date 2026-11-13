/*
Integration tests for executed `build.omg` declarations, same-session `Build`
retention across review passes and target rosters, and package command / audit
invariants.

Each target shares its source's executed Build result. A fresh session also runs
the separate provider-default supplier once; unchanged Pure results survive
same-session resume.

Run these tests under `cargo nextest` process isolation because the build
execution counters in `artifacts::compile_timings::counters` measure the entire
process invocation.

These tests verify three end-to-end properties of `orchestration`:
1. `pure_build_reuse_preserves_review_discovery_and_changed_requests`: changing a
   project role from `Build::package` to `Build::application` requires an
   authored review decision even for an effect-free program, while retaining and
   reusing matching `Pure` `Build` results across same-session resume passes and
   refusing to publish if the underlying source request changes after proposal.
2. `product_entry_queries_reuse_build_across_accepted_targets`: projects that
   link the full standard library (which exceeds the text review renderer's
   preview budget) still complete accepted source custody, multi-target
   compilation, and `inspect_packages` auditing without re-executing `Build` per
   target, and leave pending proposal and decision files byte-identical when
   audited mid-review.
3. `dynamic_commands_publish_actual_sources_once_and_keep_failures_unpublished`:
   editable root changes do not silently publish or update lockfiles, locked
   dependency source drift is surfaced across all targets, narrowing root source
   capture obligations never narrows a selected dependency's inventory, changed
   dependency projections are refused before online Git acquisition, `install`
   runs the installed dependency's dynamic `Build` through the same discovery
   and review pipeline, and explicit provider readiness preserves public backing
   requirements while rejecting private dependency implementations.
*/

use artifacts::compile_timings::counters;
use orchestration::{
    BuildRequest, BuildSession, BuildTargets, NativeDestination, ProjectProduct, build,
    execute_package_command, inspect_packages,
};
use package_manager::operations::{PackageInspectionOptions, PackageInspectionOutcome};
use package_manager::{
    PackageCommand, PackageCommandKind, PackageCommandOptions, PackageCommandStatus,
};
use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::Path;
use target::TargetProfile;

fn copy_project(root: &Path, name: &str, files: &[&str]) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../tests/fixtures/pass/build")
        .join(name);
    let destination = root.join(name);
    std::fs::create_dir(&destination).unwrap();
    for file in files {
        let output = destination.join(file);
        std::fs::create_dir_all(output.parent().unwrap()).unwrap();
        std::fs::copy(source.join(file), output).unwrap();
    }
}

#[test]
fn pure_build_reuse_preserves_review_discovery_and_changed_requests() {
    let temporary = TemporaryDirectory::create("pure-build-review", false).unwrap();
    let root = temporary.path();
    copy_project(
        root,
        "executed_project_identity_exit",
        &["main.omg", "build.omg", "naming.omg", "result.omg"],
    );
    copy_project(
        root,
        "executed_dependency_source_join_exit",
        &["main.omg", "build.omg"],
    );
    copy_project(
        root,
        "executed_dependency_source_library",
        &["main.omg", "build.omg"],
    );
    copy_project(
        root,
        "product_entry_query_product_scope_alias",
        &[
            "main.omg",
            "build.omg",
            "helper/build.omg",
            "helper/setup.omg",
        ],
    );
    for (fixture, executions) in [
        ("executed_project_identity_exit", 1),
        ("product_entry_query_product_scope_alias", 2),
        ("executed_dependency_source_join_exit", 2),
    ] {
        let project = root.join(fixture);
        let build_source = project.join("build.omg");
        let original = std::fs::read_to_string(&build_source).unwrap();
        let package = original.replace("Build::application(", "Build::package(");
        assert_ne!(package, original);
        std::fs::write(&build_source, package).unwrap();
        let mut baseline = BuildSession::create(&project.join("build/package-manager")).unwrap();
        let mut options = PackageCommandOptions {
            project_root: project.clone(),
            targets: vec![TargetProfile::host_if_supported().unwrap()],
            offline: true,
            build_inputs: None,
        };
        let accepted = execute_package_command(
            PackageCommand::Update {
                packages: Vec::new(),
                revision: None,
            },
            options.clone(),
            &mut baseline,
        )
        .unwrap();
        assert_eq!(
            accepted.status,
            PackageCommandStatus::Published,
            "{fixture}: {}",
            accepted.report
        );
        baseline.close().unwrap();
        let baseline_lock = std::fs::read(project.join("omega.lock")).unwrap();
        std::fs::write(&build_source, &original).unwrap();
        let mut session = BuildSession::create(&project.join("build/package-manager")).unwrap();
        let sponsor = session.resources().evaluation_sponsor().clone();
        let before = counters::source_build_execution_count();
        let mut outcome = execute_package_command(
            PackageCommand::Update {
                packages: Vec::new(),
                revision: None,
            },
            options.clone(),
            &mut session,
        )
        .unwrap();
        assert_eq!(
            counters::source_build_execution_count() - before,
            executions,
            "{fixture}: each source runs once; the separate provider-default supplier is counted separately"
        );
        assert_eq!(
            outcome.status,
            PackageCommandStatus::ReviewRequired,
            "{fixture}: {}",
            outcome.report
        );
        assert!(!outcome.review_paths.is_empty());
        assert!(
            outcome.review_paths.iter().any(|path| {
                std::fs::read_to_string(path)
                    .unwrap()
                    .lines()
                    .any(|line| line.starts_with("decision ") && line.ends_with(" pending"))
            }),
            "{fixture}: the role change must require an authored decision"
        );
        assert_eq!(
            std::fs::read(project.join("omega.lock")).unwrap(),
            baseline_lock
        );
        assert!(sponsor.consumed_work_units() > 0);
        options.targets.clear();
        if fixture == "executed_dependency_source_join_exit" {
            accept_decisions(&outcome.review_paths);
            std::fs::write(
                &build_source,
                original.replace(
                    "../executed_dependency_source_library",
                    "../changed_missing_source",
                ),
            )
            .unwrap();
            let error = execute_package_command(
                PackageCommand::Resume {
                    kind: PackageCommandKind::Update,
                },
                options.clone(),
                &mut session,
            )
            .expect_err("a changed request cannot publish the previously reviewed result");
            assert!(
                error.to_string().contains("changed since the proposal"),
                "{error}"
            );
            assert_eq!(
                std::fs::read(project.join("omega.lock")).unwrap(),
                baseline_lock
            );
            std::fs::write(&build_source, &original).unwrap();
        }
        let mut resumes = 0;
        for _ in 0..3 {
            accept_decisions(&outcome.review_paths);
            let before = counters::build_execution_count();
            let work_before = sponsor.consumed_work_units();
            outcome = execute_package_command(
                PackageCommand::Resume {
                    kind: PackageCommandKind::Update,
                },
                options.clone(),
                &mut session,
            )
            .unwrap();
            resumes += 1;
            assert!(
                sponsor.consumed_work_units() > work_before,
                "{fixture}: consuming a retained result must charge the current occurrence"
            );
            assert_eq!(
                counters::build_execution_count(),
                before,
                "matching Pure results must survive same-session source discovery"
            );
            if outcome.status == PackageCommandStatus::Published {
                break;
            }
        }
        assert_eq!(
            outcome.status,
            PackageCommandStatus::Published,
            "{}",
            outcome.report
        );
        assert!(resumes > 0);
        assert_eq!(std::fs::read_to_string(&build_source).unwrap(), original);
        execute_accepted_project(&project);
        println!(
            "{fixture}: same-session review published; matching Pure Build executions skipped; native and Terminal exit 0"
        );
        session.close().unwrap();
    }
}

fn execute_accepted_project(project: &Path) {
    let mut outcomes = build(BuildRequest {
        root_path: project.join("main.omg"),
        build_directory: Some(project.join("build/accepted-native")),
        targets: BuildTargets::Host,
        product: ProjectProduct::Native {
            destination: NativeDestination::Retained,
            private_entry_ram: Vec::new(),
            installation_assumptions: Vec::new(),
        },
        timings: false,
        offline: true,
        verify: false,
        require_package_project: true,
        optimization_rollback: Default::default(),
        build_snapshot: None,
    })
    .unwrap();
    assert_eq!(outcomes.len(), 1);
    let report = outcomes
        .pop()
        .unwrap()
        .result
        .unwrap()
        .into_report()
        .unwrap()
        .report;
    let artifact = report.retained_native_artifact().unwrap().psi_artifact();
    let reloaded =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    let observation = terminal_runtime::execute(terminal_runtime::TerminalRuntimeRequest {
        native_image: report.retained_native_artifact(),
        entry: None,
        filesystem: Default::default(),
        native_services: Default::default(),
        native_provider: None,
        x86_flags: None,
        memory_ordering: None,
        provider_proposal: None,
        artifact: &reloaded,
        target: Some(TargetProfile::host_if_supported().unwrap().target_name()),
        provisions: report.runtime_provisions(),
        work_budget: 100_000,
        domain_exit: terminal_runtime::TerminalDomainExitDelegation::IsolatedContainingDomain,
        standard_input: None,
        clock: None,
    })
    .unwrap();
    assert!(
        matches!(
            observation.status,
            terminal_runtime::TerminalExecutionStatus::Complete(
                terminal_runtime::TerminalExecutionResult::Unit
            )
        ),
        "{:?}",
        observation.status
    );
    let published = compilation_products::publication::publish_compilation(
        report,
        &project.join("build/native-product"),
    )
    .unwrap();
    let observed = std::process::Command::new(published.checked_native_executable_path().unwrap())
        .output()
        .unwrap();
    assert_eq!(observed.status.code(), Some(0), "{observed:?}");
}

#[test]
fn product_entry_queries_reuse_build_across_accepted_targets() {
    let temporary = TemporaryDirectory::create("product-entry-queries", false).unwrap();
    let root = temporary.path();
    let fixture = "described_runner_entry_left_exit";
    copy_project(
        root,
        fixture,
        &["main.omg", "build.omg", "left.omg", "right.omg"],
    );
    let project = root.join(fixture);
    let standard = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../source/library/std")
        .canonicalize()
        .unwrap();
    let source = std::fs::read_to_string(project.join("build.omg")).unwrap();
    std::fs::write(
        project.join("build.omg"),
        source.replace(
            "../../../../../source/library/std",
            &standard.to_string_lossy().replace('\\', "/"),
        ),
    )
    .unwrap();
    publish_candidate(
        &project,
        PackageCommand::Update {
            packages: Vec::new(),
            revision: None,
        },
        2,
    );
    compile_accepted_project(&project, 2);
    let accepted = audit(&project, Some(2));
    assert!(
        accepted.complete && !accepted.requires_decision,
        "{}",
        accepted.report
    );
}

fn accept_decisions(paths: &[std::path::PathBuf]) {
    for path in paths {
        let document = std::fs::read_to_string(path).unwrap();
        let accepted = document
            .lines()
            .map(|line| {
                if line.starts_with("decision ") {
                    line.strip_suffix(" pending")
                        .map(|subject| format!("{subject} accept"))
                        .unwrap_or_else(|| line.to_owned())
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        std::fs::write(path, accepted).unwrap();
    }
}

fn command_and_accept(project: &Path, command: PackageCommand, executions: u64) {
    let report = publish_candidate(project, command, executions);
    assert!(!report.contains("Source output unavailable"), "{report}");
}

fn publish_candidate(project: &Path, command: PackageCommand, executions: u64) -> String {
    let mut options = PackageCommandOptions {
        project_root: project.to_path_buf(),
        targets: ["linux_x86_64", "macos_arm64"]
            .map(|name| TargetProfile::from_canonical_target_name(name).unwrap())
            .to_vec(),
        offline: true,
        build_inputs: None,
    };
    let mut session = BuildSession::create(&project.join("build/package-manager")).unwrap();
    let before = counters::source_build_execution_count();
    let suppliers = counters::provider_defaults_execution_count();
    let kind = match command {
        PackageCommand::Install { .. } => PackageCommandKind::Install,
        _ => PackageCommandKind::Update,
    };
    let mut outcome = execute_package_command(command, options.clone(), &mut session).unwrap();
    assert_eq!(
        counters::source_build_execution_count() - before,
        executions,
        "review passes and two targets must not replay Build"
    );
    assert_eq!(
        counters::provider_defaults_execution_count() - suppliers,
        1,
        "a fresh session executes the separate provider-default supplier once"
    );
    for _ in 0..3 {
        if outcome.status == PackageCommandStatus::Published {
            break;
        }
        let pending = std::fs::read(project.join("build/package-manager/proposal")).unwrap();
        let decisions: Vec<_> = outcome
            .review_paths
            .iter()
            .map(|path| std::fs::read(path).unwrap())
            .collect();
        let _ = audit(project, None);
        assert_eq!(
            std::fs::read(project.join("build/package-manager/proposal")).unwrap(),
            pending
        );
        for (path, before) in outcome.review_paths.iter().zip(decisions) {
            assert_eq!(std::fs::read(path).unwrap(), before);
        }
        accept_decisions(&outcome.review_paths);
        options.targets.clear();
        let before = counters::source_build_execution_count();
        let suppliers = counters::provider_defaults_execution_count();
        outcome = execute_package_command(
            PackageCommand::Resume { kind },
            options.clone(),
            &mut session,
        )
        .unwrap();
        // These fixtures have output-free Pure Builds. Accepting a review
        // changes the decision, while their exact Build inputs stay unchanged.
        assert_eq!(
            counters::source_build_execution_count() - before,
            0,
            "an unchanged Pure source result must survive same-session resume"
        );
        assert_eq!(
            counters::provider_defaults_execution_count() - suppliers,
            0,
            "resuming the same session must reuse its completed provider defaults"
        );
    }
    assert_eq!(outcome.status, PackageCommandStatus::Published);
    assert!(project.join("omega.lock").is_file());
    session.close().unwrap();
    outcome.report
}

fn audit(project: &Path, executions: Option<u64>) -> PackageInspectionOutcome {
    audit_with_options(
        PackageInspectionOptions {
            project_root: project.to_path_buf(),
            targets: ["linux_x86_64", "macos_arm64"]
                .map(|name| TargetProfile::from_canonical_target_name(name).unwrap())
                .to_vec(),
            details: false,
            offline: true,
            build_inputs: None,
        },
        executions,
    )
}

fn compile_accepted_project(project: &Path, executions: u64) {
    let targets =
        target::ExplicitTargetSet::from_caller_names(["linux_x86_64", "macos_arm64"]).unwrap();
    let expected_targets = targets.profiles().to_vec();
    for product in [
        ProjectProduct::Check,
        ProjectProduct::Native {
            installation_assumptions: Vec::new(),
            destination: NativeDestination::Retained,
            private_entry_ram: Vec::new(),
        },
    ] {
        let check_only = matches!(product, ProjectProduct::Check);
        let before = counters::source_build_execution_count();
        let suppliers = counters::provider_defaults_execution_count();
        let outcomes = build(BuildRequest {
            root_path: project.join("main.omg"),
            build_directory: Some(project.join("build/sdk")),
            targets: BuildTargets::Batch(targets.clone()),
            product,
            timings: false,
            offline: true,
            verify: false,
            require_package_project: true,
            optimization_rollback: Default::default(),
            build_snapshot: None,
        })
        .unwrap();
        assert_eq!(
            counters::source_build_execution_count() - before,
            executions
        );
        assert_eq!(
            counters::provider_defaults_execution_count() - suppliers,
            1,
            "a fresh product session executes its provider-default supplier once"
        );
        assert_eq!(
            outcomes
                .iter()
                .map(|outcome| outcome.target)
                .collect::<Vec<_>>(),
            expected_targets
        );
        let reports: Vec<_> = outcomes
            .into_iter()
            .map(|outcome| outcome.result.unwrap().into_report().unwrap().report)
            .collect();
        if !check_only {
            let first = reports[0]
                .retained_native_artifact()
                .unwrap()
                .psi_artifact();
            for report in &reports[1..] {
                assert!(
                    std::ptr::eq(
                        first,
                        report.retained_native_artifact().unwrap().psi_artifact()
                    ),
                    "SDK target products must share their Terminal artifact"
                );
            }
        }
    }
}

fn audit_with_options(
    options: PackageInspectionOptions,
    executions: Option<u64>,
) -> PackageInspectionOutcome {
    let project = &options.project_root;
    let files = [
        project.join("build.omg"),
        project.join("omega.lock"),
        project.join("build/package-manager/proposal"),
    ];
    let before: Vec<_> = files.iter().map(std::fs::read).collect();
    let builds = counters::source_build_execution_count();
    let suppliers = counters::provider_defaults_execution_count();
    let outcome = inspect_packages(options).unwrap();
    if let Some(executions) = executions {
        assert_eq!(
            counters::source_build_execution_count() - builds,
            executions,
            "two-target inspection must not replay Build: {}",
            outcome.report
        );
        assert_eq!(
            counters::provider_defaults_execution_count() - suppliers,
            1,
            "a fresh inspection session executes its provider-default supplier once"
        );
    }
    for (file, before) in files.iter().zip(before) {
        match before {
            Ok(before) => assert_eq!(std::fs::read(file).unwrap(), before),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => assert!(!file.exists()),
            Err(error) => panic!("cannot capture {}: {error}", file.display()),
        }
    }
    assert_eq!(
        outcome
            .report
            .lines()
            .filter(|line| line.starts_with("target "))
            .count(),
        2,
        "every requested target needs a result: {}",
        outcome.report
    );
    outcome
}

#[test]
fn dynamic_commands_publish_actual_sources_once_and_keep_failures_unpublished() {
    let workspace = TemporaryDirectory::create("executed-commands", false).unwrap();
    let root = workspace.path();
    copy_project(
        root,
        "executed_project_identity_exit",
        &["main.omg", "build.omg", "naming.omg", "result.omg"],
    );
    let project = root.join("executed_project_identity_exit");
    let source = std::fs::read(project.join("build.omg")).unwrap();
    let unlocked = audit(&project, Some(1));
    assert!(unlocked.complete, "{}", unlocked.report);
    command_and_accept(
        &project,
        PackageCommand::Update {
            packages: Vec::new(),
            revision: None,
        },
        1,
    );
    assert_eq!(std::fs::read(project.join("build.omg")).unwrap(), source);
    compile_accepted_project(&project, 1);
    let accepted = audit(&project, Some(1));
    assert!(
        accepted.complete && !accepted.requires_decision,
        "{}",
        accepted.report
    );

    std::fs::write(
        project.join("build.omg"),
        [source.as_slice(), b"\n// root edit\n"].concat(),
    )
    .unwrap();
    let edited = audit(&project, Some(1));
    assert!(edited.complete, "{}", edited.report);
    std::fs::write(project.join("build.omg"), &source).unwrap();

    copy_project(
        root,
        "executed_dependency_source_join_exit",
        &["main.omg", "build.omg"],
    );
    copy_project(
        root,
        "executed_dependency_source_library",
        &["main.omg", "build.omg"],
    );
    let project = root.join("executed_dependency_source_join_exit");
    command_and_accept(
        &project,
        PackageCommand::Update {
            packages: Vec::new(),
            revision: None,
        },
        2,
    );
    let lock = std::fs::read(project.join("omega.lock")).unwrap();
    compile_accepted_project(&project, 2);
    let source = std::fs::read_to_string(project.join("build.omg")).unwrap();
    let accepted = audit(&project, Some(2));
    assert!(
        accepted.complete && !accepted.requires_decision,
        "{}",
        accepted.report
    );

    let dependency = root.join("executed_dependency_source_library/main.omg");
    let contents = std::fs::read(&dependency).unwrap();
    std::fs::write(
        &dependency,
        [contents.as_slice(), b"\n// dependency drift\n"].concat(),
    )
    .unwrap();
    let drift = audit(&project, Some(2));
    assert!(
        !drift.complete && drift.report.contains("executed source graph differs"),
        "{}",
        drift.report
    );
    std::fs::write(&dependency, contents).unwrap();

    let selected_inputs = package_compilation::BuildSourceCaptureRequest::new(
        ["main.omg", "build.omg"].map(|name| {
            (
                name.as_bytes().to_vec(),
                package_compilation::BuildSourceCaptureObligation::Required,
            )
        }),
    )
    .unwrap();
    let scoped = audit_with_options(
        PackageInspectionOptions {
            project_root: project.clone(),
            targets: ["linux_x86_64", "macos_arm64"]
                .map(|name| TargetProfile::from_canonical_target_name(name).unwrap())
                .to_vec(),
            details: true,
            offline: true,
            build_inputs: Some(selected_inputs),
        },
        Some(2),
    );
    assert!(scoped.complete, "{}", scoped.report);

    std::fs::write(project.join("build.omg"), source.replace(
        "Source::Path { location: \"../executed_dependency_source_library\" }",
        "Source::Git { repository: \"https://invalid.invalid/never-fetch.git\", revision: \"main\", selection: PackageSelection::Root }",
    )).unwrap();
    let changed_request = audit_with_options(
        PackageInspectionOptions {
            project_root: project.clone(),
            targets: ["linux_x86_64", "macos_arm64"]
                .map(|name| TargetProfile::from_canonical_target_name(name).unwrap())
                .to_vec(),
            details: false,
            offline: false,
            build_inputs: None,
        },
        Some(1),
    );
    assert!(
        !changed_request.complete
            && changed_request
                .report
                .contains("dependency projection differs"),
        "{}",
        changed_request.report
    );
    std::fs::write(
        project.join("build.omg"),
        source.replace("transition true", "transition false"),
    )
    .unwrap();
    let refused = audit(&project, Some(1));
    assert!(
        !refused.complete && refused.report.contains("dependency projection differs"),
        "{}",
        refused.report
    );
    let mut session = BuildSession::create(&project.join("build/package-manager")).unwrap();
    let error = execute_package_command(
        PackageCommand::Update {
            packages: Vec::new(),
            revision: None,
        },
        PackageCommandOptions {
            project_root: project.clone(),
            targets: Vec::new(),
            offline: true,
            build_inputs: None,
        },
        &mut session,
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("must_not_be_acquired"),
        "{error}"
    );
    assert_eq!(std::fs::read(project.join("omega.lock")).unwrap(), lock);
    assert!(!project.join("build/package-manager/proposal").exists());
    session.close().unwrap();

    let installed = root.join("installed");
    std::fs::create_dir(&installed).unwrap();
    std::fs::copy(project.join("main.omg"), installed.join("main.omg")).unwrap();
    std::fs::write(installed.join("build.omg"), "machine build(builder: &mut Build) {\n    Build::application(builder, \"installed\");\n    Build::bind(builder, linux_x86_64::ProgramEntry, main);\n    Build::bind(builder, macos_arm64::ProgramEntry, main);\n}\n").unwrap();
    command_and_accept(
        &installed,
        PackageCommand::Install {
            source: "../executed_dependency_source_library".into(),
            revision: None,
            alias: Some("selected".into()),
            package: None,
        },
        2,
    );

    copy_project(
        root,
        "explicit_provider_readiness_exit",
        &[
            "main.omg",
            "build.omg",
            "backing/main.omg",
            "backing/build.omg",
        ],
    );
    let project = root.join("explicit_provider_readiness_exit");
    command_and_accept(
        &project,
        PackageCommand::Update {
            packages: Vec::new(),
            revision: None,
        },
        2,
    );
    let accepted_lock = std::fs::read(project.join("omega.lock")).unwrap();
    compile_accepted_project(&project, 2);
    let accepted = audit(&project, Some(2));
    assert!(
        accepted.complete && !accepted.requires_decision,
        "{}",
        accepted.report
    );

    let declaration = std::fs::read_to_string(project.join("build.omg")).unwrap();
    std::fs::write(
        project.join("build.omg"),
        declaration.replace("backing::TestImage", "backing::PrivateImage"),
    )
    .unwrap();
    let private = audit(&project, Some(2));
    assert!(
        !private.complete
            && private.report.contains(
                "Build backing names no public implementation in its exact dependency package"
            ),
        "{}",
        private.report
    );
    assert_eq!(
        std::fs::read(project.join("omega.lock")).unwrap(),
        accepted_lock
    );
    assert!(!project.join("build/package-manager/proposal").exists());
}
