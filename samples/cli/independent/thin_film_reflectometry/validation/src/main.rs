//! Capture actual numerical native and Terminal output from the same product.
//! This uses normal package preparation and preserves its admission checks.
//! Review modes use this exact executable identity; run never accepts decisions.
use terminal_runtime::{TerminalDomainExitDelegation, TerminalRuntimeRequest};
use package_manager::operations::{LocalProjectPreparationOptions};
use omega_build::{PreparedLocalProjectNativeRequest};
use package_manager::{
    PackageBuildSession, PackageCommand, PackageCommandKind, PackageCommandOptions,
    PackageCommandStatus, execute_package_command,
};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
};

fn main() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}
fn run() {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments
        .get(1)
        .is_some_and(|argument| argument == "--review" || argument == "--resume-review")
    {
        assert_eq!(
            arguments.len(),
            3,
            "review requires exactly a sample directory"
        );
        review(&arguments[2], arguments[1] == "--resume-review");
        return;
    }
    assert!(
        arguments.len() >= 4,
        "usage: runner ROOT.omg OUTPUT_DIRECTORY INPUT_FILE..."
    );
    let root = PathBuf::from(&arguments[1]);
    let directory = PathBuf::from(&arguments[2]);
    fs::create_dir_all(&directory).unwrap();
    let profile = target::TargetProfile::from_omega_target_name(Some("linux_x86_64")).unwrap();
    let prepared = package_manager::operations::prepare_local_project(
        &root,
        LocalProjectPreparationOptions {
            target: profile,
            offline: false,
        },
    )
    .unwrap()
    .unwrap();
    let report = omega_build::compile_prepared_local_project_for_native(
        PreparedLocalProjectNativeRequest::new(prepared, directory.join("product"), profile),
    )
    .unwrap();
    let artifact = report
        .retained_native_artifact()
        .map(|artifact| artifact.psi_artifact())
        .or_else(|| {
            report
                .retained_dynamic_elf_artifact()
                .map(|artifact| artifact.psi_artifact())
        })
        .unwrap();
    for input_path in arguments.iter().skip(3).map(PathBuf::from) {
        let case_directory = input_path.parent().unwrap();
        let observation = terminal_runtime::execute(TerminalRuntimeRequest {
            x86_flags: terminal_runtime::capture_host_x86_flags("linux_x86_64").expect("host flags capture"),
            artifact,
            target: Some("linux_x86_64"),
            provisions: report.runtime_provisions(),
            provider_proposal: None,
            fuel_allowance: 500_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(fs::File::open(&input_path).unwrap())),
            clock: None,
        })
        .unwrap();
        fs::write(case_directory.join("terminal.stdout"), &observation.stdout).unwrap();
        fs::write(case_directory.join("terminal.stderr"), &observation.stderr).unwrap();
        fs::write(
            case_directory.join("terminal.status"),
            format!("{:?}", observation.status),
        )
        .unwrap();
        println!(
            "{} Terminal={:?}",
            case_directory.display(),
            observation.status
        );
    }
    let (_, executable) = omega_build::publication::publish_native_artifact(
        report,
        &directory.join("product"),
    )
    .unwrap();
    for input_path in arguments.iter().skip(3).map(PathBuf::from) {
        let case_directory = input_path.parent().unwrap();
        let native = Command::new(&executable)
            .stdin(Stdio::from(fs::File::open(&input_path).unwrap()))
            .output()
            .unwrap();
        fs::write(case_directory.join("native.stdout"), native.stdout).unwrap();
        fs::write(case_directory.join("native.stderr"), native.stderr).unwrap();
        fs::write(
            case_directory.join("native.status"),
            native.status.code().unwrap().to_string(),
        )
        .unwrap();
        println!("{} native={}", case_directory.display(), native.status);
    }
}

// Package identities include the actual executable: refresh normal checked
// review with this producer instead of overriding or weakening that identity.
fn review(directory: &str, resume: bool) {
    let root = PathBuf::from(directory).canonicalize().unwrap();
    let profile = target::TargetProfile::from_omega_target_name(Some("linux_x86_64")).unwrap();
    let command = if resume {
        PackageCommand::Resume {
            kind: PackageCommandKind::Update,
        }
    } else {
        PackageCommand::Update {
            packages: Vec::new(),
            revision: None,
        }
    };
    let options = PackageCommandOptions {
        project_root: root.clone(),
        targets: if resume { Vec::new() } else { vec![profile] },
        offline: true,
        build_inputs: None,
    };
    let mut session = PackageBuildSession::create(&root.join("build/package-manager")).unwrap();
    let result = execute_package_command(command, options, None, &mut session);
    let outcome = session.dispose(result).unwrap();
    print!("{}", outcome.report);
    for path in outcome.review_paths {
        println!("review: {}", path.display());
    }
    if outcome.status == PackageCommandStatus::ReviewRequired {
        std::process::exit(3);
    }
}
