/*

Standalone source compilation entry points (`compile_sources`, `check_source`, `produce_source`).

Standalone `.omg` compilations (without an enclosing package project) still
execute any local `build.omg` activation through the shared `build_execution::execute_build`
between `begin_*` and `complete_*`, so the compiler crate itself never
steps a `build.omg` interpreter:
- `compile_sources`: compiles a single or multi-target `CompileRequest` batch
  and finishes any native outputs through `native_completion::complete_products`.
- `check_source`: runs type and proof checking on a single `CheckedCompileRequest`
  for diagnostic callers.
- `produce_source`: produces a single requested `CompilationProduct` (including
  `CompilationProduct::Inspection(machine)`) and completes it via `complete_product`.

*/

use compiler::{CheckedCompileRequest, CompilationSession, CompileFailure};
use diagnostics::Diagnostic;

pub fn compile_sources(
    compiler: &mut CompilationSession,
    request: compiler::CompileRequest,
) -> Result<compiler::CompileOutcomes, Vec<Diagnostic>> {
    compiler::checked::run_on_compile_thread(|| {
        let mut prepared = compiler.begin_compilation(request)?;
        let mut queries = prepared.product_queries();
        let returned =
            crate::compile::build_execution::execute_build(prepared.execution(), queries.as_mut());
        let products = compiler.complete_compilation(prepared, returned)?;
        let mut outcomes = Vec::with_capacity(products.len());
        let products = products
            .into_iter()
            .map(|product| (product.target, product.result))
            .collect();
        for (target, product) in crate::compile::native_completion::complete_products(products) {
            let result = product.and_then(compiler::CompilationOutput::into_report);
            outcomes.push(match result {
                Ok(report) => compiler::CompileTargetOutcome::new(target, Ok(report)),
                Err(error) => error.into_target_outcome(target),
            });
        }
        Ok(compiler::CompileOutcomes::new(outcomes))
    })
}

pub fn check_source(
    compiler: &mut CompilationSession,
    request: CheckedCompileRequest,
) -> compilation_products::CheckedCompileOutcome {
    let result = compiler::checked::run_on_compile_thread(|| {
        let mut prepared = compiler.begin_source_compilation(request)?;
        let returned = execute_prepared_source_build(&mut prepared);
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

/// `check_source` for a caller that also needs the typed trees the checker
/// judged: the same Build stepping, completed through the session's
/// `check_keeping_typed_frontier`. A failure before the Build returns is
/// reported as stopping before the typed frontier.
pub fn check_source_keeping_typed_frontier(
    compiler: &mut CompilationSession,
    request: CheckedCompileRequest,
) -> compiler::TypedFrontierCheck {
    compiler::checked::run_on_compile_thread(|| {
        let mut prepared = compiler.begin_source_compilation(request)?;
        let returned = execute_prepared_source_build(&mut prepared);
        Ok::<_, CompileFailure>(compiler.check_keeping_typed_frontier(
            compiler::TypedFrontierRequest::Source {
                source: prepared,
                returned,
            },
        ))
    })
    .unwrap_or_else(compiler::TypedFrontierCheck::StoppedBeforeTypedFrontier)
}

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
        let returned = execute_prepared_source_build(&mut prepared);
        compiler
            .complete_source_product(prepared, returned, product)
            .and_then(crate::complete_product)
    })
}

fn execute_prepared_source_build(
    prepared: &mut compiler::SourceCompilation,
) -> Result<Option<build_settings::ReturnedBuild>, CompileFailure> {
    let mut queries = prepared.product_queries();
    crate::compile::build_execution::execute_build(prepared.execution(), Some(&mut queries))
}
