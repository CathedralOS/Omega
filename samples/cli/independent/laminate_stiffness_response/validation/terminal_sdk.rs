//! Capture the accepted native product's retained Terminal artifact through the
//! public runtime SDK. Admission and runtime provisions come from ordinary
//! preparation; this sponsor supplies no substitute provider proposal.
use terminal_runtime::{
    TerminalClockSnapshot, TerminalDomainExitDelegation, TerminalRuntimeRequest,
};
use package_manager::operations::{
    LocalProjectPreparationOptions, PreparedLocalProjectNativeRequest,
    compile_prepared_local_project_for_native, prepare_local_project,
};
use std::{fs, io::Cursor, path::PathBuf};
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};

fn main() {
    let arguments: Vec<_> = std::env::args_os().collect();
    assert!(
        matches!(arguments.len(), 4 | 5),
        "PROJECT CASE_INPUT_DIRECTORY OUTPUT [CASE_COUNT]"
    );
    let case_count: usize = arguments.get(4).map_or(16, |count| {
        count
            .to_str()
            .expect("case count is UTF-8")
            .parse()
            .expect("case count is an integer")
    });
    assert!(case_count > 0, "capture at least one case");
    let project = PathBuf::from(&arguments[1]);
    let inputs = PathBuf::from(&arguments[2]);
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
    .expect("application has its accepted package graph");
    let report = compile_prepared_local_project_for_native(PreparedLocalProjectNativeRequest::new(
        prepared,
        output.join("compile"),
        target,
    ))
    .unwrap_or_else(|error| panic!("accepted native product failed: {error:?}"));
    let artifact = report
        .retained_native_artifact()
        .map(|native| native.psi_artifact())
        .or_else(|| {
            report
                .retained_dynamic_elf_artifact()
                .map(|native| native.psi_artifact())
        })
        .expect("native product retains the exact produced Terminal artifact");
    let mut records = String::new();
    for ordinal in 0..case_count {
        let name = format!("{ordinal:02}");
        let input = fs::read(inputs.join(format!("{name}.stdin"))).unwrap();
        let observation = terminal_runtime::execute(TerminalRuntimeRequest {
            x86_flags: terminal_runtime::capture_host_x86_flags("linux_x86_64").expect("host flags capture"),
            artifact,
            target: Some("linux_x86_64"),
            provisions: report.runtime_provisions(),
            provider_proposal: None,
            fuel_allowance: 10_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(Cursor::new(input))),
            clock: Some(TerminalClockSnapshot::capture_host_epoch().unwrap()),
        })
        .unwrap_or_else(|error| panic!("Terminal case {name} failed: {error:?}"));
        let exit = match &observation.status {
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => 0,
            TerminalExecutionStatus::ExternalCompletion(completion) => completion.return_code,
            other => panic!("Terminal case {name} did not complete: {other:?}"),
        };
        fs::write(output.join(format!("{name}.stdout")), &observation.stdout).unwrap();
        fs::write(output.join(format!("{name}.stderr")), &observation.stderr).unwrap();
        records.push_str(&format!("{name}\t{exit}\n"));
        fs::write(output.join("records.tsv"), &records).unwrap();
        println!("Terminal case {name}: exit {exit}");
    }
}
