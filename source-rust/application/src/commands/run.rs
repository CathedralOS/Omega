//! Present the selected application engine's execution or retained observation.
mod observation;

use crate::arguments::RunArguments;
use crate::execution::{
    CaseOutcome, CaseRunRequest, ExecutionOutcome, ExecutionSelection, RunError, RunRequest,
    TerminalOutcome, run_project, run_project_cases,
};
use compiler::CompilerRequestOutcome;
use std::io::Write;

use std::path::PathBuf;

pub(crate) fn run(arguments: RunArguments) -> ! {
    let artifact = if arguments
        .main_path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("psi"))
    {
        Some(
            crate::saved_program::input::open_trusted_program(
                &arguments.main_path,
                arguments.trusted_artifact.as_deref(),
            )
            .unwrap_or_else(|diagnostics| {
                let error = RunError::Compilation(diagnostics);
                eprintln!("{error}");
                std::process::exit(compile_failure_status(&error))
            }),
        )
    } else {
        None
    };
    if let Some(outputs) = arguments.outputs.clone()
        && (arguments.inputs.is_some() || !arguments.input_files.is_empty())
    {
        run_cases(arguments, outputs, artifact);
    }
    let selection = arguments.selection;
    let outcome = match run_project(
        artifact,
        RunRequest {
            run_directories: arguments.run_directories,
            build_snapshot: crate::arguments::build_snapshot(arguments.build_inputs, arguments.build_directories),
            private_entry_ram: arguments.private_entry_ram,
            root_path: arguments.main_path,
            target_name: arguments.target_name,
            selection: arguments.selection,
            psi_work_budget: arguments.psi_work_budget,
            keep_artifacts: arguments.keep,
        },
        |row| eprintln!("{row}"),
    ) {
        Ok(outcome) => outcome,
        Err(error) => {
            match (&error, selection) {
                (RunError::Compilation(diagnostics), ExecutionSelection::Terminal) => {
                    eprintln!(
                        "Psi compile FAILED:{}",
                        std::fmt::from_fn(|f| diagnostics::write_bounded_diagnostics(
                            f,
                            diagnostics
                        ))
                    );
                }
                _ => eprintln!("{error}"),
            }
            std::process::exit(compile_failure_status(&error));
        }
    };
    if let Some(directory) = arguments.outputs {
        if arguments.keep {
            eprintln!("build dir kept: {}", outcome.build_dir.display());
        }
        match observation::write(&directory, &outcome.execution) {
            Ok(true) => std::process::exit(0),
            Ok(false) => std::process::exit(200),
            Err(error) => {
                eprintln!(
                    "cannot write execution observation in {}: {error}",
                    directory.display()
                );
                std::process::exit(200);
            }
        }
    }
    let status = match outcome.execution {
        ExecutionOutcome::Terminal(terminal) => present_terminal(terminal),
        ExecutionOutcome::TargetOnly { target_name } => {
            eprintln!(
                "compiled for target `{target_name}` OK ({}). The program did not run.",
                outcome.build_dir.display()
            );
            0
        }
        ExecutionOutcome::Host {
            output,
            exit,
            private_entry_ram,
            ..
        } => {
            forward_output_or_exit(&output.stdout, &output.stderr);
            print_private_entry_ram(&private_entry_ram);
            let native_code = exit.code().unwrap_or(-1);
            eprintln!("native {}", exit.describe());
            native_code
        }
    };
    if arguments.keep {
        eprintln!("build dir kept: {}", outcome.build_dir.display());
    }
    std::process::exit(status);
}

fn present_terminal(outcome: TerminalOutcome) -> i32 {
    use terminal_runtime::{TerminalExecutionResult, TerminalExecutionStatus};
    let observation = match outcome {
        TerminalOutcome::Completed(observation) => observation,
        TerminalOutcome::Unavailable(reason) => {
            eprintln!("Psi: UNAVAILABLE ({reason})");
            return 200;
        }
        TerminalOutcome::Failed(reason) => {
            eprintln!("Psi execution FAILED: {reason}");
            return 200;
        }
    };
    forward_output_or_exit(&observation.stdout, &observation.stderr);
    print_private_entry_ram(&observation.private_entry_ram);
    match observation.status {
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => {
            eprintln!("Psi exit 0");
            0
        }
        TerminalExecutionStatus::ExternalCompletion(completion) => {
            eprintln!("Psi exit {}", completion.return_code);
            completion.return_code
        }
        TerminalExecutionStatus::Crashed(crash) => {
            eprintln!("Psi crash: {:?}", crash.cause);
            203
        }
        TerminalExecutionStatus::Complete(result) => {
            eprintln!("Psi: program entry returned a non-process result: {result:?}");
            200
        }
        TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
            eprintln!("Psi: {exhaustion}; raise --psi-work-budget to permit more work");
            200
        }
        TerminalExecutionStatus::AwaitingEffect(site) => {
            eprintln!("Psi: execution awaits host readiness at {site:?}");
            200
        }
        TerminalExecutionStatus::HostComputationPending(site) => {
            eprintln!("Psi: execution has an unfinished host computation at {site:?}");
            200
        }
    }
}

fn run_cases(
    arguments: RunArguments,
    outputs: PathBuf,
    artifact: Option<psi_authorization::AuthorizedTerminalArtifact>,
) -> ! {
    let cases = match &arguments.inputs {
        Some(inputs) => {
            let entries = std::fs::read_dir(inputs).and_then(|entries| {
                entries
                    .map(|entry| entry.map(|entry| entry.path()))
                    .collect::<std::io::Result<Vec<_>>>()
            });
            let mut cases = match entries {
                Ok(paths) => paths,
                Err(error) => {
                    eprintln!("cannot read --inputs {}: {error}", inputs.display());
                    std::process::exit(200);
                }
            };
            cases.retain(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "stdin")
            });
            cases.sort();
            if cases.is_empty() {
                eprintln!("--inputs {} holds no *.stdin files", inputs.display());
                std::process::exit(200);
            }
            cases
        }
        None => arguments.input_files.clone(),
    };
    let outcome = match run_project_cases(
        artifact,
        CaseRunRequest {
            run_directories: arguments.run_directories,
            build_snapshot: crate::arguments::build_snapshot(arguments.build_inputs, arguments.build_directories),
            private_entry_ram: arguments.private_entry_ram,
            root_path: arguments.main_path,
            target_name: arguments.target_name,
            selection: arguments.selection,
            psi_work_budget: arguments.psi_work_budget,
            inputs: cases,
            output_directory: outputs.clone(),
            keep_artifacts: arguments.keep,
        },
        |row| eprintln!("{row}"),
    ) {
        Ok(outcome) => outcome,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(compile_failure_status(&error));
        }
    };
    let mut status = 0;
    for case in &outcome.cases {
        match case {
            CaseOutcome::Native { name, exit } => {
                eprintln!("case {name}: native {}", exit.describe())
            }
            CaseOutcome::Psi { name, termination } => {
                let (kind, detail) = termination.columns();
                eprintln!("case {name}: Psi {kind} {detail}");
                if !termination.complete() {
                    status = 200;
                }
            }
        }
    }
    eprintln!(
        "{} cases; streams and results.tsv in {}",
        outcome.cases.len(),
        outputs.display()
    );
    if arguments.keep {
        eprintln!("build dir kept: {}", outcome.build_dir.display());
    }
    std::process::exit(status);
}

fn forward_output_or_exit(stdout: &[u8], stderr: &[u8]) {
    let result = (|| -> std::io::Result<()> {
        let mut standard_output = std::io::stdout().lock();
        standard_output.write_all(stdout)?;
        standard_output.flush()?;
        let mut standard_error = std::io::stderr().lock();
        standard_error.write_all(stderr)?;
        standard_error.flush()
    })();
    if let Err(error) = result {
        let _ = writeln!(
            std::io::stderr().lock(),
            "cannot forward program output: {error}"
        );
        std::process::exit(200);
    }
}

fn print_private_entry_ram(private_entry_ram: &[(u32, Vec<u8>)]) {
    for (position, contents) in private_entry_ram {
        eprint!("entry RAM {position}: ");
        for byte in contents {
            eprint!("{byte:02x}");
        }
        eprintln!();
    }
}

fn compile_failure_status(error: &RunError) -> i32 {
    match error {
        RunError::Compilation(diagnostics)
            if matches!(
                CompilerRequestOutcome::of_refusal(diagnostics),
                CompilerRequestOutcome::Incomplete(_)
            ) =>
        {
            202
        }
        _ => 200,
    }
}
