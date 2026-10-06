//! Shell output and exit-code policy for the reusable run operation.

use super::arguments::RunArguments;
use compiler::CompilerRequestOutcome;
use omega::execution::{
    ExecutionOutcome, InterpreterComparison, RunError, RunRequest, run_project,
};

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
            print!("{}", String::from_utf8_lossy(&output.stdout));
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
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
                InterpreterComparison::Disagrees {
                    interpreter_exit_code,
                } => {
                    eprintln!("interp exit: {interpreter_exit_code}");
                    eprintln!("DIVERGENCE: native {native_code} vs interp {interpreter_exit_code}");
                    201
                }
                InterpreterComparison::Failed(diagnostics) => {
                    eprintln!("interp frontend compile FAILED:");
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
