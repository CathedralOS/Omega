//! Capture actual numerical native and Terminal output from the same product.
//! This uses normal package preparation and preserves its admission checks.
//! Review modes use this exact executable identity; run never accepts decisions.
//!
//! Every independent sample runs the identical product: package preparation,
//! the retained Terminal artifact, the published native product and the
//! per-input captures. Only the sample's own identity and whether it has
//! reviewed package decisions to refresh differ, so each sample declares one
//! [`Sample`] roster and shares this harness. The numerical oracle, its frozen
//! expected output and its tolerance stay with the sample that owns them; this
//! harness captures and never compares.
use omega_build::{
    BuildRequest, BuildSession, BuildTargets, NativeDestination, ProjectProduct,
    execute_package_command,
};
use package_manager::{
    PackageCommand, PackageCommandKind, PackageCommandOptions, PackageCommandStatus,
};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
};
use terminal_runtime::{TerminalDomainExitDelegation, TerminalRuntimeRequest};

/// One sample's roster entry for the shared capture harness.
#[derive(Clone, Copy)]
pub struct Sample {
    /// Cargo package name of the sample's validation crate. It names the
    /// producer in harness diagnostics; the published binary keeps that name.
    pub package: &'static str,
    /// Whether the sample has reviewed package decisions to refresh, which is
    /// the only per-sample difference between capture producers.
    pub review: bool,
}

/// Run one sample's capture producer on the current process arguments.
///
/// `runner ROOT.omg OUTPUT_DIRECTORY INPUT_FILE...` builds the product from
/// the sample's source root and captures actual output for every input.
/// `runner --review DIRECTORY` and `runner --resume-review DIRECTORY` refresh
/// normal package review for samples that declare [`Sample::review`].
pub fn main(sample: Sample) {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || run(sample))
        .unwrap()
        .join()
        .unwrap();
}

fn run(sample: Sample) {
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
        assert!(sample.review, "{} has no review mode", sample.package);
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
    let product_directory = directory.join("product");
    let outcomes = omega_build::build(BuildRequest {
        root_path: root,
        build_directory: Some(product_directory.clone()),
        targets: BuildTargets::Single(profile),
        product: ProjectProduct::Native {
            destination: NativeDestination::Retained,
            private_entry_ram: Vec::new(),
        },
        timings: false,
        offline: false,
        verify: false,
        // Preparation previously unwrapped its optional package project.
        require_package_project: true,
        optimization_rollback: Default::default(),
        build_snapshot: None,
    })
    .unwrap();
    let [outcome] = outcomes
        .try_into()
        .map_err(|_| "one named target produces exactly one Build outcome")
        .unwrap();
    let report = outcome.result.unwrap().into_report().unwrap().report;
    let artifact = report
        .retained_native_program()
        .map(|program| program.psi_artifact())
        .unwrap();
    for input_path in arguments.iter().skip(3).map(PathBuf::from) {
        let case_directory = input_path.parent().unwrap();
        let observation = terminal_runtime::execute(TerminalRuntimeRequest {
            entry: None,
            native_provider: None,
            x86_flags: terminal_runtime::capture_host_x86_flags("linux_x86_64")
                .expect("host flags capture"),
            memory_ordering:
                terminal_runtime::TerminalMemoryOrderingEnvironment::private_sequential(
                    "linux_x86_64",
                ),
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
    let (_, executable) =
        omega_build::publication::publish_native_artifact(report, &product_directory).unwrap();
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
    let mut session = BuildSession::create(&root.join("build/package-manager")).unwrap();
    let result = execute_package_command(command, options, &mut session);
    let outcome = session.dispose(result).unwrap();
    print!("{}", outcome.report);
    for path in outcome.review_paths {
        println!("review: {}", path.display());
    }
    if outcome.status == PackageCommandStatus::ReviewRequired {
        std::process::exit(3);
    }
}
