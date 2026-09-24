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
// `run_on_compile_thread`; their recursive paths never touch this stack. But
// every command that reaches a package project first runs the package arm's
// project discovery, source-closure resolution, and review walks on the
// caller's stack: `--check`, `run`, and `inspect-terminal` on a package root
// recurse through `prepare_local_project` before the compile worker exists.
// Give every dispatched command this stack; only usage output stays shallow.
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

/// Every invocation except usage output may recurse on the caller's stack:
/// package-management arms own resolution and review walks, and the compile,
/// run, and inspection commands route through that same package preparation
/// whenever the entry root belongs to a package project. The nested
/// `run_on_compile_thread` worker inside the compile path stays the boundary
/// for compiler-stage recursion regardless.
fn needs_command_worker(invocation: &Invocation) -> bool {
    !matches!(invocation, Invocation::Help(_))
}
