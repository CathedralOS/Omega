use diagnostics::Diagnostic;

const COMPILE_STACK_SIZE: usize = 256 * 1024 * 1024;

thread_local! {
    static ON_COMPILER_WORKER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Run compiler work on a thread with a large explicit stack. Recursive parsing
/// and representation walks must reach their explicit depth guards on hosts
/// whose default thread stacks are small. This is host execution
/// infrastructure, not a compiler stage or request mode.
/// Thread creation failures return diagnostics; worker panics keep their payload.
/// An application may enclose a dependency lifecycle in one worker. Compiler
/// operations called on that worker execute inline, never spawn nested stacks.
pub fn run_on_compile_thread<T, Failure>(
    work: impl FnOnce() -> Result<T, Failure> + Send,
) -> Result<T, Failure>
where
    T: Send,
    Failure: From<Vec<Diagnostic>> + Send,
{
    if ON_COMPILER_WORKER.get() {
        return work();
    }
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("omega-compile".to_owned())
            .stack_size(COMPILE_STACK_SIZE)
            .spawn_scoped(scope, || {
                // This dedicated thread exits with the invocation, including
                // unwinding; the marker cannot escape into a reused host thread.
                ON_COMPILER_WORKER.set(true);
                work()
            })
            .map_err(|error| {
                Failure::from(vec![Diagnostic::error(format!(
                    "failed to spawn compiler thread: {error}"
                ))])
            })?
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    })
}
