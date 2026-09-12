mod admissions;
mod audit;
mod compilation;
mod compile_arguments;
mod inspect_terminal;
mod output;
mod package;
mod probe;
mod samples;

use artifacts::allocations::CountingAllocator;
use std::path::PathBuf;
use std::process::ExitCode;

#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator::system();

// Recursive compiler walks must reach their explicit depth guards before the
// host stack runs out. Windows gives the process main thread only one MiB;
// native realization also runs on its caller's stack, so cover every command.
const COMPILER_STACK_SIZE: usize = 256 * 1024 * 1024;

fn main() -> ExitCode {
    let worker = std::thread::Builder::new()
        .name("omega-command".to_owned())
        .stack_size(COMPILER_STACK_SIZE)
        .spawn(|| {
            let mut arguments = std::env::args_os().skip(1);
            let first = arguments.next();
            match first.as_deref().and_then(std::ffi::OsStr::to_str) {
                Some("install") => package::run(
                    package_manager::operations::PackageCommandKind::Install,
                    arguments,
                ),
                Some("update") => package::run(
                    package_manager::operations::PackageCommandKind::Update,
                    arguments,
                ),
                Some("audit") => audit::run(arguments),
                Some("run") => probe::run(arguments),
                Some("inspect-terminal") => inspect_terminal::run(arguments),
                Some("refresh-samples") => {
                    let root = arguments
                        .next()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("samples"));
                    samples::refresh(&root);
                }
                _ => {
                    let arguments =
                        compile_arguments::parse_arguments(first.into_iter().chain(arguments))
                            .unwrap_or_else(|error| {
                                if !error.is_empty() {
                                    eprintln!("{error}");
                                }
                                eprintln!("{}", compile_arguments::usage());
                                std::process::exit(2);
                            });
                    compilation::compile_project(arguments);
                }
            }
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
