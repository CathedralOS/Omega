/*
Diagnostic aggregation and temporary query instrumentation retained while the
legacy source queries move to their declaration owners. Executable body checking
and its publication belong to the lowered-form checker.
*/
use diagnostics::Diagnostic;

pub use checked_trees::operators::ExactIntegerCastFact;
pub use typed_trees::calls::declaration_queries::statement_source_span;

pub(crate) fn is_exact_overflow_diagnostic(diagnostic: &Diagnostic) -> bool {
    diagnostic.is_error()
        && diagnostic.message.starts_with("exact arithmetic in ")
        && diagnostic.message.contains("may overflow")
}

pub(crate) fn finish_diagnostics(diagnostics: Vec<Diagnostic>) -> Result<(), Vec<Diagnostic>> {
    if diagnostics.iter().any(Diagnostic::is_error) {
        return Err(diagnostics);
    }
    for warning in &diagnostics {
        eprintln!("{warning}");
    }
    Ok(())
}

pub(crate) fn finish_strict_diagnostics(
    diagnostics: Vec<Diagnostic>,
) -> Result<(), Vec<Diagnostic>> {
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

const CHECKING_STACK_SIZE: usize = 256 * 1024 * 1024;

pub fn checking_thread_pool() -> &'static rayon::ThreadPool {
    static POOL: std::sync::OnceLock<rayon::ThreadPool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .thread_name(|index| format!("omega-check-{index}"))
            .stack_size(CHECKING_STACK_SIZE)
            .build()
            .expect("machine validation worker pool")
    })
}
