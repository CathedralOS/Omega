//! Capture actual numerical native and Terminal output from the same product.
//! This uses normal package preparation and preserves its admission checks.
use bounded_process::{
    BoundedCaptureBudget, BoundedCaptureLimits, BoundedProcessInput, BoundedProcessLimits,
    BoundedProcessPrepared, run_bounded_process,
};
use terminal_runtime::{TerminalDomainExitDelegation, TerminalRuntimeRequest};
use package_manager::operations::{
    LocalProjectPreparationOptions, PreparedLocalProjectNativeRequest,
};
use std::{fs, path::PathBuf, process::Command, time::Duration};

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
    let report = package_manager::operations::compile_prepared_local_project_for_native(
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
        let prepared = BoundedProcessPrepared::new(
            Command::new(&executable),
            BoundedProcessLimits::new(
                10,
                512 * 1024 * 1024,
                1024 * 1024,
                64,
                8,
                512 * 1024 * 1024,
                512 * 1024 * 1024,
            ),
            "phase change numerical validation",
        )
        .unwrap();
        let native = run_bounded_process(
            prepared,
            BoundedProcessInput::File(fs::File::open(&input_path).unwrap()),
            BoundedCaptureLimits::new(
                64 * 1024,
                16 * 1024,
                Duration::from_secs(15),
                Duration::from_secs(2),
                Duration::from_millis(10),
            ),
            BoundedCaptureBudget::new(80 * 1024),
        )
        .unwrap();
        fs::write(case_directory.join("native.stdout"), native.stdout).unwrap();
        fs::write(case_directory.join("native.stderr"), native.stderr).unwrap();
        fs::write(
            case_directory.join("native.status"),
            native.status.code().unwrap().to_string(),
        )
        .unwrap();
        println!(
            "{} native={}",
            case_directory.display(),
            native.status.code().unwrap()
        );
    }
}
