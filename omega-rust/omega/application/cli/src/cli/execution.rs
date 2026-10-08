//! Shell output and exit-code policy for the reusable run operation.

use super::arguments::RunArguments;
use compiler::CompilerRequestOutcome;
use omega::execution::{
    ExecutionOutcome, InterpreterComparison, RunError, RunRequest, run_project,
};
use std::io::Write;

pub(crate) fn run(arguments: RunArguments) -> ! {
    let outcome = match run_project(RunRequest {
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
