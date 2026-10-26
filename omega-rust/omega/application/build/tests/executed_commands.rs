//! Real command transactions use executed declarations and retain each Build
//! across review passes and targets. Kept in one process-isolated test because
//! the execution counter measures the complete invocation.

use artifacts::compile_timings::counters;
use omega_build::{BuildSession, execute_package_command};
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
        std::fs::copy(source.join(file), destination.join(file)).unwrap();
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
    command_and_accept(
        &project,
        PackageCommand::Update {
            packages: Vec::new(),
            revision: None,
        },
        1,
    );
    assert_eq!(std::fs::read(project.join("build.omg")).unwrap(), source);

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
    let source = std::fs::read_to_string(project.join("build.omg")).unwrap();
    std::fs::write(
        project.join("build.omg"),
        source.replace("transition true", "transition false"),
    )
    .unwrap();
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
}
