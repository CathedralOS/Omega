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
//! Every command runs on a 256 MB stack because OS main-thread stacks are
//! small and host-dependent (1 MB on Windows, 8 MB on macOS and Linux).
//! Before `compile_packages` enters `run_on_compile_thread`, package source
//! discovery (`ReadinessScheduler::drive_work`) runs `PackageBuildInvocation::advance`
//! and `ProductQueries::describe` (`lower_typed_frontier` name resolution and
//! type-checking) on the command's own thread, which overflows 1 MB on ordinary
//! package projects. The 256 MB is virtual address space reserved up front and
//! committed by the OS only a page at a time as touched, so `omega --version`
//! and `--help` pay only a single thread spawn (~50 µs).

mod allocator;
mod cli;

use cli::arguments::{self, Invocation};
use cli::{compilation, execution, inspection, packages};
use std::process::ExitCode;

const COMMAND_STACK_SIZE: usize = 256 * 1024 * 1024;

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
    run_with_large_stack(invocation)
}

fn run_with_large_stack(invocation: Invocation) -> ExitCode {
    let worker = std::thread::Builder::new()
        .name("omega-command".to_owned())
        .stack_size(COMMAND_STACK_SIZE)
        .spawn(move || match invocation {
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
            Invocation::Help(usage) => println!("{usage}"),
            Invocation::BuildProfile => println!("{}", compiler::compiler_build_profile()),
            Invocation::Version => println!(
                "omega {}{} ({}; {})",
                env!("OMEGA_VERSION"),
                allocator::VERSION_SUFFIX,
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
}
