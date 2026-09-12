mod arguments;
mod compilation;
mod execution;
mod inspection;
mod packages;

use arguments::Invocation;
use artifacts::allocations::CountingAllocator;
use std::process::ExitCode;

#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator::system();

// Some recursive compiler paths still overflow the Windows main-thread stack.
// This is stack provision, not parallel execution. Keep it until those paths
// have bounded stack use; argument parsing needs no compiler worker.
const COMPILER_STACK_SIZE: usize = 256 * 1024 * 1024;

fn main() -> ExitCode {
    let invocation = match arguments::parse(std::env::args_os().skip(1)) {
        Ok(invocation) => invocation,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };

    let worker = std::thread::Builder::new()
        .name("omega-command".to_owned())
        .stack_size(COMPILER_STACK_SIZE)
        .spawn(move || match invocation {
            Invocation::Compile(request) => compilation::compile_project(request),
            Invocation::Run(request) => execution::run(request),
            Invocation::InspectTerminal(request) => inspection::run(request),
            Invocation::Package { command, options } => packages::run(command, options),
            Invocation::AuditSource(request) => packages::source::run(request),
            Invocation::AuditPackages(request) => packages::audit::run(request),
            Invocation::RefreshSamples(root) => compilation::samples::refresh(&root),
            Invocation::Help(usage) => println!("{usage}"),
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
