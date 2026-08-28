const COMPILE_STACK_SIZE: usize = 256 * 1024 * 1024;

/// Run compiler work on a thread with a large explicit stack. Recursive parsing
/// and representation walks must reach their explicit depth guards on hosts
/// whose default thread stacks are small. This is host execution
/// infrastructure, not a compiler stage or request mode.
pub(crate) fn run_on_compile_thread<T>(work: impl FnOnce() -> T + Send + 'static) -> T
where
    T: Send + 'static,
{
    std::thread::Builder::new()
        .name("omega-compile".to_owned())
        .stack_size(COMPILE_STACK_SIZE)
        .spawn(work)
        .expect("failed to spawn compiler thread")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}
