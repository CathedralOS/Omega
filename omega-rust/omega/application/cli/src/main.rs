//! The `omega` binary: read the command line, run the command on a deep-stack
//! thread, and turn a usage or thread-start error into an exit code. Each
//! command surface lives in `commands::{compile, run, inspect, packages}`; no
//! compilation semantics live here.
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

mod arguments;
mod commands;
mod execution;
mod inspection;

use arguments::Invocation;
use std::process::ExitCode;

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> ExitCode {
    terminal_runtime::run_native_provider_worker_if_requested();
    let invocation = match arguments::parse(std::env::args_os().skip(1)) {
        Ok(invocation) => invocation,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(arguments::USAGE_ERROR_EXIT);
        }
    };
    run_with_large_stack(invocation)
}

fn run_with_large_stack(invocation: Invocation) -> ExitCode {
    std::thread::scope(|scope| {
        let worker = compiler::checked::spawn_compile_worker(scope, move || match invocation {
            Invocation::Compile(request) => commands::compile::run(request),
            Invocation::Run(request) => commands::run::run(request),
            Invocation::InspectTerminal(request) => commands::inspect::run(request),
            Invocation::Package {
                command,
                options,
                accept_all,
            } => commands::packages::run(command, options, accept_all),
            Invocation::AuditSource(request) => commands::packages::run_source_audit(request),
            Invocation::AuditPackages(request) => commands::packages::run_audit(request),
            Invocation::Help(usage) => println!("{usage}"),
            Invocation::Version => println!(
                "omega {} ({}; {})",
                env!("OMEGA_VERSION"),
                env!("OMEGA_COMMIT"),
                env!("OMEGA_HOST")
            ),
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
