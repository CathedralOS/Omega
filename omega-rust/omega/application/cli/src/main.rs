//! The `omega` binary: read the command line, run the command on a deep-stack
//! thread, and turn a usage or thread-start error into an exit code. Each
//! command surface lives in `cli::{compilation, execution, inspection,
//! packages}`; no compilation semantics live here.
//!
//! The same executable is also its own native-provider helper. When a program
//! calls a foreign library, the runtime relaunches this binary as a child
//! process to load it, so `main` hands that child over before parsing any
//! arguments; it exits and never returns here.
//!
//! Compiler and package commands run on `compiler::checked::spawn_compile_worker`
//! so `ON_COMPILER_WORKER` is active from the start of the command. OS main-thread
//! stacks are small on Windows (1 MB vs. 8 MB on macOS and Linux), and nested
//! `run_on_compile_thread` calls inside package readiness and compilation reuse
//! that single worker inline instead of spawning a second 256 MB thread.
//! Lightweight metadata flags (`--help`, `--version`, `--build-profile`) run
//! directly on `main` without spawning a worker.

mod allocator;
mod cli;

use cli::arguments::{self, Invocation};
use cli::{compilation, execution, inspection, packages};
use std::process::ExitCode;

fn main() -> ExitCode {
    let _allocation_report = allocator::ReportOnExit;
    terminal_runtime::run_native_provider_worker_if_requested();
    phase_profile::start_timeline();
    let invocation = match arguments::parse(std::env::args_os().skip(1)) {
        Ok(invocation) => invocation,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(arguments::USAGE_ERROR_EXIT);
        }
    };
    run_invocation(invocation)
}

fn run_invocation(invocation: Invocation) -> ExitCode {
    match invocation {
        Invocation::Help(usage) => {
            println!("{usage}");
            return ExitCode::SUCCESS;
        }
        Invocation::BuildProfile => {
            println!("{}", compiler::compiler_build_profile());
            return ExitCode::SUCCESS;
        }
        Invocation::Version => {
            println!(
                "omega {}{} ({}; {})",
                env!("OMEGA_VERSION"),
                allocator::VERSION_SUFFIX,
                env!("OMEGA_COMMIT"),
                env!("OMEGA_HOST")
            );
            return ExitCode::SUCCESS;
        }
        _ => {}
    }
    std::thread::scope(|scope| {
        let worker = compiler::checked::spawn_compile_worker(scope, move || match invocation {
            Invocation::Compile(request) => compilation::compile_project_command(request),
            Invocation::Run(request) => execution::run(request),
            Invocation::InspectTerminal(request) => inspection::run(request),
            Invocation::Package {
                command,
                options,
                accept_all,
            } => packages::run(command, options, accept_all),
            Invocation::AuditSource(request) => packages::source::run(request),
            Invocation::AuditPackages(request) => packages::audit::run(request),
            Invocation::Help(_) | Invocation::BuildProfile | Invocation::Version => unreachable!(),
        });
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
    })
}
