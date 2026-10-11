//! Capture the existing admitted Terminal runtime's complete output. This is a
//! validation consumer, never an alternative application or source interpreter.
//! Preparation, native production and runtime provision ownership follow the
//! ordinary CLI route. Source is checked once; Build is not replayed for execution.
use std::{fs::File, path::PathBuf};
use compiler::runtime::{TerminalDomainExitDelegation, TerminalRuntimeRequest};
use package_manager::operations::{LocalProjectPreparationOptions,
    PreparedLocalProjectNativeRequest, compile_prepared_local_project_for_native,
    prepare_local_project};
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};

fn main() {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    assert_eq!(arguments.len(), 4, "root.omg input_directory output_directory build_directory");
    let root = PathBuf::from(&arguments[0]);
    let target = target::TargetProfile::host();
    let prepared = prepare_local_project(&root, LocalProjectPreparationOptions {
        target, offline: true,
    }).expect("ordinary project preparation").expect("project has build metadata");
    let request = PreparedLocalProjectNativeRequest::new(prepared, PathBuf::from(&arguments[3]), target);
    let report = compile_prepared_local_project_for_native(request).expect("native product");
    let artifact = report.retained_native_artifact().map(|product| product.psi_artifact())
        .or_else(|| report.retained_dynamic_elf_artifact().map(|product| product.psi_artifact()))
        .expect("native product retains exact Terminal artifact");
    let input_directory = PathBuf::from(&arguments[1]);
    let output_directory = PathBuf::from(&arguments[2]);
    std::fs::create_dir_all(&output_directory).expect("output directory");
    std::fs::write(output_directory.join("canonical_terminal_artifact.psi"), artifact.to_bytes())
        .expect("retain canonical Terminal artifact");
    let mut inputs: Vec<_> = std::fs::read_dir(&input_directory).expect("input fixtures")
        .map(|entry| entry.expect("input entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "stdin"))
        .collect();
    inputs.sort();
    assert_eq!(inputs.len(), 29, "18 variants and 11 invalid requests");
    for input in inputs {
        // Each execution gets a fresh zero-backed instance and finite stdin;
        // every instance consumes this exact already produced artifact.
        let observed = compiler::runtime::execute(TerminalRuntimeRequest {
            artifact, target: Some(target.target_name()), provisions: report.runtime_provisions(),
            provider_proposal: None,
            fuel_allowance: 10_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(File::open(&input).expect("finite replay input"))),
            clock: None,
        }).expect("admitted Terminal execution");
        let name = input.file_stem().expect("fixture name").to_string_lossy();
        std::fs::write(output_directory.join(format!("{name}.stdout")), &observed.stdout)
            .expect("output observation");
        assert!(observed.stderr.is_empty(), "unexpected guest stderr");
        let code = match observed.status {
            TerminalExecutionStatus::ExternalCompletion(completion) => completion.return_code,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => 0,
            other => panic!("incomplete Terminal execution: {other:?}"),
        };
        let expected = if name.starts_with("invalid_") { 64 } else { 0 };
        std::fs::write(output_directory.join(format!("{name}.exit")), format!("{code}\n"))
            .expect("exit observation");
        eprintln!("Terminal complete: {name}, exit={code}, stdout_bytes={}", observed.stdout.len());
        assert_eq!(code, expected, "Terminal fixture {name}");
    }
}
