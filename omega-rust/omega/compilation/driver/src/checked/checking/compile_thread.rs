use diagnostics::Diagnostic;

/// Stack budget for one compile invocation.
///
/// Checked lowering keeps whole fact tables, prepared frontends and probes by
/// value, and a build-time product query lowers a demanded closure inside the
/// preparation that issued it, so the frames nest. Every compile entry point
/// provisions this budget explicitly rather than inheriting a host default:
/// the CLI, this driver's compile thread, the parser stage, checking and proof
/// admission each name a 256 MiB thread. A host that runs compiler work on a
/// default-sized thread (2 MiB on macOS, 8 MiB for a process main thread) turns
/// an ordinary deep fixture into a stack overflow, which no caller can catch.
pub const COMPILE_STACK_SIZE: usize = 256 * 1024 * 1024;

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
        phase_profile::count("compile.worker.inline", 1);
        return work();
    }
    phase_profile::count("compile.worker.spawn", 1);
    phase_profile::scope("compile.worker.await", || {
        std::thread::scope(|scope| {
            spawn_compile_worker(scope, work)
                .map_err(|error| {
                    Failure::from(vec![Diagnostic::error(format!(
                        "failed to spawn compiler thread: {error}"
                    ))])
                })?
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
        })
    })
}

/// Scoped compiler work uses the same stack and nesting marker as the public entrance.
pub fn spawn_compile_worker<'scope, 'env, T>(
    scope: &'scope std::thread::Scope<'scope, 'env>,
    work: impl FnOnce() -> T + Send + 'scope,
) -> std::io::Result<std::thread::ScopedJoinHandle<'scope, T>>
where
    T: Send + 'scope,
{
    std::thread::Builder::new()
        .name("omega-compile".to_owned())
        .stack_size(COMPILE_STACK_SIZE)
        .spawn_scoped(scope, move || {
            ON_COMPILER_WORKER.set(true);
            phase_profile::scope("compile.worker.body", work)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_worker_keeps_nested_compilation_on_its_large_stack() {
        std::thread::scope(|scope| {
            spawn_compile_worker(scope, || -> Result<(), Vec<Diagnostic>> {
                let worker = std::thread::current().id();
                let nested = run_on_compile_thread(|| {
                    Ok::<_, Vec<Diagnostic>>(std::thread::current().id())
                })?;
                assert_eq!(worker, nested);
                Ok(())
            })
            .unwrap()
            .join()
            .unwrap()
            .unwrap();
        });
    }
}
