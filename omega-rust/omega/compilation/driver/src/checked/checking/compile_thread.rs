use diagnostics::Diagnostic;

const COMPILE_STACK_SIZE: usize = 256 * 1024 * 1024;

/// Run compiler work on a thread with a large explicit stack. Recursive parsing
/// and representation walks must reach their explicit depth guards on hosts
/// whose default thread stacks are small. This is host execution
/// infrastructure, not a compiler stage or request mode.
/// Thread creation failures return diagnostics; worker panics keep their payload.
pub fn run_on_compile_thread<T, Failure>(
    work: impl FnOnce() -> Result<T, Failure> + Send + 'static,
) -> Result<T, Failure>
where
    T: Send + 'static,
    Failure: From<Vec<Diagnostic>> + Send + 'static,
{
    finish_compile_thread(
        std::thread::Builder::new()
            .name("omega-compile".to_owned())
            .stack_size(COMPILE_STACK_SIZE)
            .spawn(work),
    )
}

fn finish_compile_thread<T, Failure>(
    spawned: std::io::Result<std::thread::JoinHandle<Result<T, Failure>>>,
) -> Result<T, Failure>
where
    Failure: From<Vec<Diagnostic>>,
{
    spawned
        .map_err(|error| {
            Failure::from(vec![Diagnostic::error(format!(
                "failed to spawn compiler thread: {error}"
            ))])
        })?
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}
