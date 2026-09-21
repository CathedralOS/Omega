//! The `omega` binary's entry point: typed-invocation dispatch.
//!
//! `cli::arguments` parses the invocation, `cli::{compilation, execution,
//! inspection, packages}` owns each command surface, and `main` maps outcomes
//! onto process exits. No compilation semantics live at this root.

mod cli;

use arguments::Invocation;
use cli::{arguments, compilation, execution, inspection, packages};
use std::process::ExitCode;

// Compile-bearing operations provision their own large-stack worker inside
// `run_on_compile_thread`; their recursive paths never touch this stack. The
// remaining caller-stack recursion lives in the package-management arms'
// resolution and review walks. Keep this provision for exactly those until
// those paths have bounded stack use; argument parsing needs no worker.
const COMPILER_STACK_SIZE: usize = 256 * 1024 * 1024;

fn main() -> ExitCode {
    let invocation = match arguments::parse(std::env::args_os().skip(1)) {
        Ok(invocation) => invocation,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };

    if !needs_command_worker(&invocation) {
        dispatch(invocation);
        return ExitCode::SUCCESS;
    }

    let worker = std::thread::Builder::new()
        .name("omega-command".to_owned())
        .stack_size(COMPILER_STACK_SIZE)
        .spawn(move || dispatch(invocation));
    match worker {
        Ok(worker) => {
            if let Err(panic) = worker.join() {
                std::panic::resume_unwind(panic);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to start compiler thread: {error}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch(invocation: Invocation) {
    match invocation {
        Invocation::Compile(request) => compilation::compile_project_command(request),
        Invocation::Run(request) => execution::run(request),
        Invocation::InspectTerminal(request) => inspection::run(request),
        Invocation::Package { command, options } => packages::run(command, options),
        Invocation::AuditSource(request) => packages::source::run(request),
        Invocation::AuditPackages(request) => packages::audit::run(request),
        Invocation::RefreshSamples(root) => cli::samples::refresh(&root),
        Invocation::Help(usage) => println!("{usage}"),
    }
}

/// Package-management arms still run resolution and review walks on the
/// caller's stack. Every other invocation either provisions its own compile
/// worker inside the operation (`run_on_compile_thread`) or stays shallow.
fn needs_command_worker(invocation: &Invocation) -> bool {
    matches!(
        invocation,
        Invocation::Package { .. } | Invocation::AuditSource(_) | Invocation::AuditPackages(_)
    )
}
