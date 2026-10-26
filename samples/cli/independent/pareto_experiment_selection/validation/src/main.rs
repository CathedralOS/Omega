//! IND11's private validation consumer. Compile once through the ordinary reviewed
//! package facade, then replay each finite input against the exact produced Terminal
//! artifact and its admitted provisions. No source evaluator, invented provider,
//! recompilation per row, compiler modification, or expected-value guest code.
use terminal_runtime::{TerminalDomainExitDelegation, TerminalRuntimeRequest};
use package_manager::operations::{LocalProjectPreparationOptions, prepare_local_project};
use omega_build::{PreparedLocalProjectNativeRequest, compile_prepared_local_project_for_native};
use std::path::{Path, PathBuf};
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};

fn main() {
    let namespace = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .to_path_buf()
        });
    // Match the public compiler CLI's documented stack provision.
    let result = std::thread::Builder::new()
        .name("ind11-validation".into())
        .stack_size(256 * 1024 * 1024)
        .spawn(move || validate(&namespace))
        .expect("create compiler worker")
        .join()
        .expect("compiler worker returned");
    if let Err(message) = result {
        eprintln!("IND11 Terminal validation failed: {message}");
        std::process::exit(1);
    }
}

fn validate(namespace: &Path) -> Result<(), String> {
    let profile = target::TargetProfile::from_omega_target_name(Some("linux_x86_64"))
        .map_err(|error| error.to_string())?;
    let prepared = prepare_local_project(
        &namespace.join("main.omg"),
        LocalProjectPreparationOptions {
            target: profile,
            offline: true,
        },
    )
    .map_err(|error| error.to_string())?
    .ok_or_else(|| "IND11 must remain a complete package project".to_string())?;
    let build_directory = namespace.join("build/terminal-validation");
    let report = compile_prepared_local_project_for_native(PreparedLocalProjectNativeRequest::new(
        prepared,
        &build_directory,
        profile,
    ))
    .map_err(|error| error.to_string())?;
    let artifact = report
        .retained_native_artifact()
        .map(|artifact| artifact.psi_artifact())
        .or_else(|| {
            report
                .retained_dynamic_elf_artifact()
                .map(|artifact| artifact.psi_artifact())
        })
        .ok_or_else(|| "native report lost its produced Terminal artifact".to_string())?;
    let output_directory = namespace.join("build/validation-terminal");
    std::fs::create_dir_all(&output_directory).map_err(|error| error.to_string())?;
    let mut fixtures: Vec<_> = std::fs::read_dir(namespace.join("data"))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "stdin")
        })
        .collect();
    fixtures.sort();
    let mut records = String::from("case\texit\texact_stdout\texact_stderr\n");
    for fixture in fixtures {
        let label = fixture.file_stem().unwrap().to_str().unwrap();
        let input = std::fs::read(&fixture).map_err(|error| error.to_string())?;
        let expected = std::fs::read(namespace.join("expected").join(format!("{label}.stdout")))
            .map_err(|error| error.to_string())?;
        let expected_exit = if label.starts_with("invalid_") { 2 } else { 0 };
        let observation = terminal_runtime::execute(TerminalRuntimeRequest {
            native_provider: None,
            x86_flags: terminal_runtime::capture_host_x86_flags("linux_x86_64").expect("host flags capture"),
            artifact,
            target: Some("linux_x86_64"),
            provisions: report.runtime_provisions(),
            provider_proposal: None,
            fuel_allowance: 10_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(std::io::Cursor::new(input))),
            clock: None,
        })
        .map_err(|error| format!("{label}: {error}"))?;
        std::fs::write(
            output_directory.join(format!("{label}.stdout")),
            &observation.stdout,
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(
            output_directory.join(format!("{label}.stderr")),
            &observation.stderr,
        )
        .map_err(|error| error.to_string())?;
        let exit = match observation.status {
            TerminalExecutionStatus::ExternalCompletion(completion) => completion.return_code,
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => 0,
            other => {
                return Err(format!(
                    "{label}: unsupported Terminal completion {other:?}"
                ));
            }
        };
        if exit != expected_exit || observation.stdout != expected || !observation.stderr.is_empty()
        {
            return Err(format!(
                "{label}: exit={exit}, expected={expected_exit}, stdout_match={}, stderr_empty={}",
                observation.stdout == expected,
                observation.stderr.is_empty()
            ));
        }
        println!("{label}: PASS Terminal exit={exit}, exact stdout and stderr");
        records.push_str(&format!("{label}\t{exit}\tmatch\tmatch\n"));
        std::fs::write(output_directory.join("results.tsv"), &records)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
