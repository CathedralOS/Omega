//! One-shot source consumers drive the same Build executor as package work.
//!
//! Compiler preparation retains source and target policy in opaque invocations.
//! This application owns execution between preparation and completion; the
//! compiler never steps a Build or repeats its effects to satisfy another stop.

use compiler::{CheckedCompileRequest, CompilationSession, CompileFailure};
use diagnostics::Diagnostic;

/// Compile a target batch with the caller's invocation-local compiler storage.
pub fn compile_sources(
    compiler: &mut CompilationSession,
    request: compiler::CompileRequest,
) -> Result<compiler::CompileOutcomes, Vec<Diagnostic>> {
    compiler::checked::run_on_compile_thread(|| {
        let mut prepared = compiler.begin_compilation(request)?;
        let mut queries = prepared.product_queries();
        let returned =
            crate::build_execution::execute_build(prepared.execution(), queries.as_mut());
        compiler.complete_compilation(prepared, returned)
    })
}

/// Check source for diagnostic consumers, retaining discovery on later refusal.
pub fn check_source(
    compiler: &mut CompilationSession,
    request: CheckedCompileRequest,
) -> compilation_products::CheckedCompileOutcome {
    let result = compiler::checked::run_on_compile_thread(|| {
        let mut prepared = compiler.begin_source_compilation(request)?;
        let mut queries = prepared.product_queries();
        let returned =
            crate::build_execution::execute_build(prepared.execution(), Some(&mut queries));
        Ok::<_, CompileFailure>(compiler.complete_source_check(prepared, returned))
    });
    match result {
        Ok(outcome) => outcome,
        Err(failure) => compilation_products::CheckedCompileOutcome {
            result: Err(failure),
            independent_components: Default::default(),
        },
    }
}

/// Produce one requested artifact or named inspection without exposing stages.
pub fn produce_source(
    compiler: &mut CompilationSession,
    mut request: CheckedCompileRequest,
    product: compiler::CompilationProduct,
) -> Result<compiler::CompilationOutput, compiler::CompilationProductError> {
    compiler::checked::run_on_compile_thread(|| {
        request.optimization_rollback = product.optimization_rollback();
        let mut prepared = compiler
            .begin_source_compilation(request)
            .map_err(|failure| {
                compiler::CompilationProductError::Production(failure.into_diagnostics())
            })?;
        let mut queries = prepared.product_queries();
        let returned =
            crate::build_execution::execute_build(prepared.execution(), Some(&mut queries));
        compiler.complete_source_product(prepared, returned, product)
    })
}
