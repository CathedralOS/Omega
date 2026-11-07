/*

Entry point for the `omega` binary.

We read the command line, run the selected command on a dedicated compiler
worker thread, and map any usage or thread-start failure to its process exit
code. Each command surface lives in `commands::{compile, run, inspect, packages}`;
no compilation or package semantics live in this file.

Before we touch `std::env::args_os()`, we check whether this process was
spawned as a native-provider helper (`run_native_provider_worker_if_requested`).
When an Omega program loads a foreign library at runtime, `terminal_runtime`
relaunches this same `omega` executable as a child process with an internal
environment protocol. If that protocol is present, the helper runs the foreign
call and exits the process directly; it never returns here.

*/

/*

Why we spawn a worker thread at the top of the CLI (`run_with_large_stack`):

Deep recursive AST, type, and proof traversals in the compiler can blow the
operating system's default main-thread stack (1 MB on Windows, 8 MB on Linux
and macOS). Rather than scattering ad-hoc thread spawns across individual
compiler passes, we enter `compiler::checked::spawn_compile_worker` once here.
That call allocates a 256 MB stack and sets the thread-local `ON_COMPILER_WORKER`
flag, so every downstream `run_on_compile_thread` check inside package
readiness and compilation sees that we are already on the large-stack worker
and runs inline instead of spawning another 256 MB thread.

*/

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
