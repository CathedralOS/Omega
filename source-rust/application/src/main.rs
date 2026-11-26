/*

Entry point for the `omega` binary.

We read the command line, run the selected command on a dedicated compiler
worker thread, and map any usage or thread-start failure to its process exit
code. The source subsystem handles authored-file operations. The other command
adapters live in `commands::{compile, run, inspect, packages}`.

Before we touch `std::env::args_os()`, we check whether this process was
spawned as a native-provider helper (`run_native_provider_worker_if_requested`).
When an Omega program loads a foreign library at runtime, `terminal_runtime`
relaunches this same `omega` executable as a child process with an internal
environment protocol. If that protocol is present, the helper runs the foreign
call and exits the process directly; it never returns here.

*/

/*

Why the whole command runs on the invocation's execution budget
(`run_with_large_stack`):

Deep recursive AST, type, and proof traversals in the compiler can blow the
operating system's default main-thread stack (1 MB on Windows, 8 MB on Linux
and macOS), and parsing and native completion both want parallel workers.
We create one `worker_budget::ExecutionBudget` here: a fixed set of
256 MB-stack workers shared by every parallel stage of this invocation. The
command runs on one of those workers, so `run_on_compile_thread` checks inside
package readiness and compilation run inline, and source parsing and native
completion fan out onto the same workers instead of starting pools of their
own.

*/

mod arguments;
mod commands;
mod diagnostic_output;
mod execution;
mod inspection;
mod saved_program;

use arguments::Invocation;
use std::process::ExitCode;

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> ExitCode {
    build_program_launch::run_worker_if_requested();
    terminal_runtime::run_native_provider_worker_if_requested();
    let (invocation, output) = match arguments::parse(std::env::args_os().skip(1)) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("{}", error.message);
            return ExitCode::from(error.exit);
        }
    };
    run_with_large_stack(invocation, output)
}

fn run_with_large_stack(
    invocation: Invocation,
    output: diagnostic_output::DiagnosticOutput,
) -> ExitCode {
    let workers = match invocation {
        Invocation::ParseSource(_) => worker_budget::ExecutionBudget::with_workers(1),
        _ => worker_budget::ExecutionBudget::for_host(),
    };
    let budget = match workers {
        Ok(budget) => budget,
        Err(error) => {
            eprintln!("failed to start compiler workers: {error}");
            return ExitCode::FAILURE;
        }
    };
    budget.run(move || match invocation {
        Invocation::Compile(request) => commands::compile::run(request, output),
        Invocation::ParseSource(path) => {
            if let Err(error) = omega_source::parse_file(&path) {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        Invocation::Run(request) => commands::run::run(request),
        Invocation::InspectTerminal(request) => commands::inspect::run(request, output),
        Invocation::Package {
            command,
            options,
            accept_all,
        } => commands::packages::run(command, options, accept_all, output),
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
    ExitCode::SUCCESS
}
