//! Real command transactions use executed declarations and retain each Build
//! across review passes and targets. Kept in one process-isolated test because
//! the execution counter measures the complete invocation.

use artifacts::compile_timings::counters;
use omega_build::{
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
        .join("../../../../tests/omega/pass/build")
        .join(name);
    let destination = root.join(name);
    std::fs::create_dir(&destination).unwrap();
    for file in files {
        let output = destination.join(file);
        std::fs::create_dir_all(output.parent().unwrap()).unwrap();
        std::fs::copy(source.join(file), output).unwrap();
    }
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
    let mut options = PackageCommandOptions {
        project_root: project.to_path_buf(),
        targets: ["linux_x86_64", "macos_arm64"]
            .map(|name| TargetProfile::from_canonical_target_name(name).unwrap())
            .to_vec(),
        offline: true,
        build_inputs: None,
    };
    let mut session = BuildSession::create(&project.join("build/package-manager")).unwrap();
    let before = counters::build_execution_count();
    let kind = match command {
        PackageCommand::Install { .. } => PackageCommandKind::Install,
        _ => PackageCommandKind::Update,
    };
    let mut outcome = execute_package_command(command, options.clone(), &mut session).unwrap();
    assert_eq!(
        counters::build_execution_count() - before,
        executions,
        "review passes and two targets must not replay Build"
    );
    for _ in 0..3 {
        if outcome.status == PackageCommandStatus::Published {
            break;
        }
        // Audit the live project without resuming or accepting its pending
        // candidate. In particular, its decision documents stay byte-identical.
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
        let before = counters::build_execution_count();
        outcome = execute_package_command(
            PackageCommand::Resume { kind },
            options.clone(),
            &mut session,
        )
        .unwrap();
        assert_eq!(
            counters::build_execution_count() - before,
            executions,
            "one new resume attempt executes each source once"
        );
    }
    assert_eq!(outcome.status, PackageCommandStatus::Published);
    assert!(
        !outcome.report.contains("Source output unavailable"),
        "{}",
        outcome.report
    );
    assert!(project.join("omega.lock").is_file());
    session.close().unwrap();
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
            destination: NativeDestination::Retained,
            private_entry_ram: Vec::new(),
        },
    ] {
        let check_only = matches!(product, ProjectProduct::Check);
        let before = counters::build_execution_count();
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
        assert_eq!(counters::build_execution_count() - before, executions);
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
    let builds = counters::build_execution_count();
    let outcome = inspect_packages(options).unwrap();
    if let Some(executions) = executions {
        assert_eq!(
            counters::build_execution_count() - builds,
            executions,
            "two-target inspection must not replay Build: {}",
            outcome.report
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

    // An edited root is auditable, but its unchanged authority is not silent
    // acceptance of new source. No automatic update or publication occurs.
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

    // Dependency content is locked, unlike the editable root. The same
    // source mismatch must be visible for both targets, never an empty review.
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

    // Narrowing root inputs never narrows the selected dependency's inventory.
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

    // Changed requests are refused before acquisition even online. This URI
    // must never reach Git; graph admission supplies the failure instead.
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

    // Installation edits a canonical root declaration; the installed source's
    // own dynamic Build still runs through exactly the same discovery/review.
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

    // Readiness suspends before final source admission. Its dependency and
    // exact public backing must survive acceptance, lock reload and Product
    // compilation without executing either Build again for the second target.
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

    // Previously accepting the dependency does not authorize its private
    // implementation. A changed root still has to name a public backing.
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
