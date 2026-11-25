//! Raw stage operations used to construct and inspect test fixtures.
pub use checked_program_to_psi::TerminalSourceCompilation;
pub use lowered_program_to_checked_program as checking;
pub use prepared_typed_trees_to_lowered_program as lowering;
pub use source_lexer as lexing;
pub use source_parser as parsing;
pub use symbol_resolved_trees_to_typed_trees as typing;
pub use syntax_trees_to_symbol_resolved_trees::resolution;
pub use typed_trees_to_prepared_typed_trees as preparation;

pub use crate::compile_product::layout_scopes::layout_target_scopes;
pub use crate::driver::judge_lowered_source::{FrozenSource, judge_terminal_source};
pub use crate::driver::prepare_typed_source::SourcePreparationRequest;
pub use crate::driver::resolve_and_type::ResolveAndTypeRequest;

/// Complete authored source from a fixture's resolve-and-type request.
pub fn type_authored_source_for_test(
    request: ResolveAndTypeRequest,
    timings: &mut crate::CompileTimings,
) -> Result<typed_trees::TypedTrees, Vec<diagnostics::Diagnostic>> {
    Ok(request
        .prepare(timings)?
        .complete_authored(timings)?
        .finish()
        .typed)
}

/// Inspect the frozen program and its preparation diagnostics.
pub fn prepare_source_for_test(
    request: SourcePreparationRequest<'_>,
) -> Result<FrozenSource, crate::driver::prepare_typed_source::SourcePreparationFailure> {
    request.prepare()
}

#[cfg(test)]
use source_files::LoadedSource;
#[cfg(test)]
#[path = "../../../../tests/support/fixture_repository.rs"]
pub(crate) mod fixture_repository;
#[cfg(test)]
#[path = "../../../../tests/support/frontend_operations.rs"]
pub(crate) mod frontend_fixture;
#[cfg(test)]
#[path = "../tests/support/schema_customer_source.rs"]
pub(crate) mod schema_customer_source;
