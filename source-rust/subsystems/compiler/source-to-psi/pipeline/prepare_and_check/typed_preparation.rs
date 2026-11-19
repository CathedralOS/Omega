//! Supply typed specialization with the same exact-application inhabitation proof as lowering.
use typed_trees::source_state::{FrozenTypedTrees, TypedTrees};
pub use typed_trees_to_typed_trees::preparation::*;

pub fn prepare_typed_trees(
    program: TypedTrees,
    request: &PreparationRequest<'_>,
) -> Result<(FrozenTypedTrees, PreparationAdmission), Vec<diagnostics::Diagnostic>> {
    typed_trees_to_typed_trees::preparation::prepare_typed_trees(
        program,
        request,
        typed_trees_to_lowered_program::lowering::proved_empty_sum_cases,
    )
}
