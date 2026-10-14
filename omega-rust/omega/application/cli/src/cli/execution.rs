//! Shell output and exit-code policy for the reusable run operation.

use super::arguments::RunArguments;
use compiler::CompilerRequestOutcome;
use omega::execution::{
    CaseRunRequest, ExecutionOutcome, InterpreterComparison, RunError, RunRequest, run_project,
    run_project_cases,
};
use std::io::Write;

pub(crate) fn run(arguments: RunArguments) -> ! {
    if let Some(outputs) = arguments.outputs.clone() {
        run_cases(arguments, outputs);
    }
    let outcome = match run_project(RunRequest {
        private_entry_ram: arguments.private_entry_ram,
        root_path: arguments.main_path,
        target_name: arguments.target_name,
        compare_interpreter: arguments.both,
        keep_artifacts: arguments.keep,
    }) {
        Ok(outcome) => outcome,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(compile_failure_status(&error));
        }
    };
    let status = match outcome.execution {
        ExecutionOutcome::TargetOnly { target_name } => {
            eprintln!(
                "compiled for target `{target_name}` OK ({})",
                outcome.build_dir.display()
            );
            0
        }
        ExecutionOutcome::Host {
            output,
            exit,
            comparison,
            private_entry_ram,
        } => {
            if let Err(error) = forward_program_output(&output) {
                // stderr may itself be the failed pipe. Reporting is best
                // effort, but failed forwarding must never look successful.
                let _ = writeln!(
                    std::io::stderr().lock(),
                    "cannot forward program output: {error}"
                );
                std::process::exit(200);
            }
            for (position, contents) in &private_entry_ram {
                eprint!("entry RAM {position}: ");
                for byte in contents {
                    eprint!("{byte:02x}");
                }
                eprintln!();
            }
            let native_code = exit.code().unwrap_or(-1);
            eprintln!("native {}", exit.describe());
            match comparison {
                InterpreterComparison::NotRequested => native_code,
                InterpreterComparison::Declined(reason) => {
                    eprintln!("interp: DECLINED ({reason})");
                    native_code
                }
                InterpreterComparison::Agrees { exit_code } => {
                    eprintln!("interp exit: {exit_code}");
                    native_code
                }
                InterpreterComparison::AgreesCrash { cause } => {
                    eprintln!("interp crash: {cause:?} (host presentation agrees)");
                    native_code
                }
                InterpreterComparison::DisagreesCrash { cause } => {
                    eprintln!("interp crash: {cause:?}");
                    eprintln!(
                        "DIVERGENCE: native {} vs interp crash {cause:?}",
                        exit.describe()
                    );
                    201
                }
                InterpreterComparison::Disagrees {
                    interpreter_exit_code,
                } => {
                    eprintln!("interp exit: {interpreter_exit_code}");
                    eprintln!("DIVERGENCE: native {native_code} vs interp {interpreter_exit_code}");
                    201
                }
                InterpreterComparison::Failed(diagnostics) => {
                    eprintln!("Terminal interpretation FAILED:");
                    for diagnostic in diagnostics {
                        eprintln!("  {diagnostic}");
                    }
                    201
                }
            }
        }
    };
    if arguments.keep {
        eprintln!("build dir kept: {}", outcome.build_dir.display());
    }
    std::process::exit(status);
}

/// One compilation, one native run (and Terminal replay with `--both`) per
/// case: every `*.stdin` file of `--inputs`, or each `--input` file in order.
/// Per-case streams and statuses go to the output directory; the command
/// exits 201 when any case diverges or fails interpretation.
fn run_cases(arguments: RunArguments, outputs: std::path::PathBuf) -> ! {
    let cases = match &arguments.inputs {
        Some(inputs) => {
            let mut cases = match std::fs::read_dir(inputs) {
                Ok(entries) => entries
                    .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                    .filter(|path| {
                        path.extension()
                            .is_some_and(|extension| extension == "stdin")
                    })
                    .collect::<Vec<_>>(),
                Err(error) => {
                    eprintln!("cannot read --inputs {}: {error}", inputs.display());
                    std::process::exit(200);
                }
            };
            cases.sort();
            if cases.is_empty() {
                eprintln!("--inputs {} holds no *.stdin files", inputs.display());
                std::process::exit(200);
            }
            cases
        }
        None => arguments.input_files.clone(),
    };
    let outcome = match run_project_cases(CaseRunRequest {
        private_entry_ram: arguments.private_entry_ram,
        root_path: arguments.main_path,
        compare_interpreter: arguments.both,
        inputs: cases,
        output_directory: outputs.clone(),
        keep_artifacts: arguments.keep,
    }) {
        Ok(outcome) => outcome,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(compile_failure_status(&error));
        }
    };
    let mut status = 0;
    for case in &outcome.cases {
        let interpreter = match &case.comparison {
            InterpreterComparison::NotRequested => String::new(),
            InterpreterComparison::Declined(reason) => format!("; interp DECLINED ({reason})"),
            InterpreterComparison::Agrees { exit_code } => format!("; interp exit {exit_code}"),
            InterpreterComparison::AgreesCrash { cause } => format!("; interp crash {cause:?}"),
            InterpreterComparison::Disagrees {
                interpreter_exit_code,
            } => {
                status = 201;
                format!("; DIVERGENCE interp exit {interpreter_exit_code}")
            }
            InterpreterComparison::DisagreesCrash { cause } => {
                status = 201;
                format!("; DIVERGENCE interp crash {cause:?}")
            }
            InterpreterComparison::Failed(diagnostics) => {
                status = 201;
                let reasons = diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                format!("; Terminal interpretation FAILED: {}", reasons.join("; "))
            }
        };
        eprintln!(
            "case {}: native {}{interpreter}",
            case.name,
            case.exit.describe()
        );
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

/// Program streams are bytes, not diagnostic text. Forward the captured
/// payloads unchanged and finish each write before presenting CLI diagnostics.
/// Explicit flushes expose a failed buffered write before choosing an exit.
fn forward_program_output(output: &std::process::Output) -> std::io::Result<()> {
    let mut standard_output = std::io::stdout().lock();
    standard_output.write_all(&output.stdout)?;
    standard_output.flush()?;
    let mut standard_error = std::io::stderr().lock();
    standard_error.write_all(&output.stderr)?;
    standard_error.flush()
}

/// A compile that stopped at a named private provision without a verdict on
/// the source (the compiler request's `Incomplete`) exits 202; any other
/// compile or launch failure exits 200. Both stay above the program's own
/// statuses.
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
