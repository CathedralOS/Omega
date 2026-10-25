//! Public runtime sponsor: compile the accepted sample once and capture fresh
//! Terminal streams for each frozen input. Application numerics stay in Omega.
use terminal_runtime::{
    TerminalClockSnapshot, TerminalDomainExitDelegation, TerminalRuntimeRequest,
};
use package_manager::operations::{LocalProjectPreparationOptions, prepare_local_project};
use omega_build::{PreparedLocalProjectNativeRequest, compile_prepared_local_project_for_native};
use std::{fs, io::Cursor, path::PathBuf};
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};

fn main() {
    std::thread::Builder::new()
        .name("molecular-validation".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(validate)
        .unwrap()
        .join()
        .unwrap();
}
fn validate() {
    let arguments: Vec<_> = std::env::args_os().collect();
    assert_eq!(arguments.len(), 4, "PROJECT CASE_NAMES OUTPUT");
    let project = PathBuf::from(&arguments[1]);
    let names = fs::read_to_string(&arguments[2]).unwrap();
    let output = PathBuf::from(&arguments[3]);
    fs::create_dir_all(&output).unwrap();
    let target = target::TargetProfile::LinuxX64;
    let prepared = prepare_local_project(
        &project.join("main.omg"),
        LocalProjectPreparationOptions {
            target,
            offline: true,
        },
    )
    .unwrap_or_else(|error| panic!("accepted preparation failed: {error}"))
    .expect("sample has an accepted package graph");
    println!("Preparing accepted native/Terminal product");
    let compile_started = std::time::Instant::now();
    let report = compile_prepared_local_project_for_native(PreparedLocalProjectNativeRequest::new(
        prepared,
        output.join("compile"),
        target,
    ))
    .unwrap_or_else(|error| panic!("native compilation failed: {error:?}"));
    let artifact = report
        .retained_native_artifact()
        .map(|native| native.psi_artifact())
        .or_else(|| {
            report
                .retained_dynamic_elf_artifact()
                .map(|native| native.psi_artifact())
        })
        .expect("product retains produced Terminal artifact")
        .clone();
    let provisions = report.runtime_provisions().cloned();
    // Clone the exact retained semantic payload/provisions before ordinary
    // product publication consumes the native report. No source is recompiled.
    let published =
        compilation_products::publication::publish_compilation(report, &output.join("native"))
            .unwrap();
    let executable = published
        .checked_native_executable_path()
        .expect("published executable custody");
    fs::write(
        output.join("native.path"),
        executable.as_os_str().as_encoded_bytes(),
    )
    .unwrap();
    println!(
        "Compiled and published in {:?}: {}",
        compile_started.elapsed(),
        executable.display()
    );
    for name in names.lines() {
        assert!(
            !name.is_empty()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        );
        let input = fs::read(project.join("data").join(format!("{name}.stdin"))).unwrap();
        let started = std::time::Instant::now();
        let observation = terminal_runtime::execute(TerminalRuntimeRequest {
            x86_flags: terminal_runtime::capture_host_x86_flags("linux_x86_64").expect("host flags capture"),
            artifact: &artifact,
            target: Some("linux_x86_64"),
            provisions: provisions.as_ref(),
            provider_proposal: None,
            fuel_allowance: 100_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(Cursor::new(input))),
            clock: Some(TerminalClockSnapshot::capture_host_epoch().unwrap()),
        })
        .unwrap_or_else(|error| panic!("{name}: {error}"));
        let exit = match observation.status {
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => 0,
            TerminalExecutionStatus::ExternalCompletion(completion) => completion.return_code,
            other => panic!("{name}: incomplete runtime {other:?}"),
        };
        fs::write(output.join(format!("{name}.stdout")), observation.stdout).unwrap();
        fs::write(output.join(format!("{name}.stderr")), observation.stderr).unwrap();
        fs::write(output.join(format!("{name}.exit")), exit.to_string()).unwrap();
        fs::write(
            output.join(format!("{name}.seconds")),
            started.elapsed().as_secs_f64().to_string(),
        )
        .unwrap();
        println!("{name}: captured exit {exit} in {:?}", started.elapsed());
    }
}
