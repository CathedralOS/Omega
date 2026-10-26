//! Capture the existing public Terminal runtime's bytes from one prepared
//! native product. This harness evaluates no physics and manufactures no IR.
use terminal_runtime::{TerminalDomainExitDelegation, TerminalRuntimeRequest};
use package_manager::operations::{LocalProjectPreparationOptions, prepare_local_project};
use omega_build::{PreparedLocalProjectNativeRequest, compile_prepared_local_project_for_native};
use std::{io::Cursor, path::PathBuf};
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments.len() != 3 { return Err("usage: terminal_capture ROOT.omg OUTPUT_DIRECTORY".into()); }
    let root = PathBuf::from(&arguments[1]);
    let output = PathBuf::from(&arguments[2]);
    std::fs::create_dir_all(&output)?;
    let target = target::TargetProfile::from_omega_target_name(Some("linux_x86_64"))
        .map_err(|error| error.to_string())?;
    let prepared = prepare_local_project(&root, LocalProjectPreparationOptions { target, offline: true })?
        .ok_or("expected package project")?;
    let report = compile_prepared_local_project_for_native(
        PreparedLocalProjectNativeRequest::new(prepared, output.join("terminal-build"), target))?;
    let artifact = report.retained_native_artifact().ok_or("expected retained native product")?.psi_artifact();
    let inventory = std::fs::read_to_string(root.parent().ok_or("missing root parent")?
        .join("validation/input_cases.tsv"))?;
    let mut cases: Vec<(String, Vec<u8>)> = Vec::new();
    for line in inventory.lines().filter(|line| !line.starts_with('#')) {
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 3 || fields[1].len() % 2 != 0 {
            return Err("malformed frozen input inventory".into());
        }
        let input = fields[1].as_bytes().chunks_exact(2)
            .map(|pair| Ok(u8::from_str_radix(std::str::from_utf8(pair)?, 16)?))
            .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
        cases.push((fields[0].into(), input));
    }
    for (name, input) in cases {
        std::fs::write(output.join(format!("terminal-{name}.stdin")), &input)?;
        let observed = terminal_runtime::execute(TerminalRuntimeRequest {
            native_provider: None,
            x86_flags: terminal_runtime::capture_host_x86_flags("linux_x86_64").expect("host flags capture"),
            provider_proposal: None,
            artifact, target: Some("linux_x86_64"), provisions: report.runtime_provisions(),
            fuel_allowance: 10_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(Cursor::new(input))), clock: None,
        })?;
        let exit = match observed.status {
            TerminalExecutionStatus::ExternalCompletion(completion) => completion.return_code,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => 0,
            other => return Err(format!("case {name}: unsupported Terminal outcome {other:?}").into()),
        };
        std::fs::write(output.join(format!("terminal-{name}.stdout")), observed.stdout)?;
        std::fs::write(output.join(format!("terminal-{name}.stderr")), observed.stderr)?;
        std::fs::write(output.join(format!("terminal-{name}.exit")), format!("{exit}\n"))?;
        println!("Terminal case {name}: exit {exit}");
    }
    Ok(())
}
