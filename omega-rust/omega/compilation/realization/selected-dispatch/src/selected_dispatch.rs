//! Derive exact operator realizations and Build comparison execution.
//!
//! Product calls retain their public requirements through Terminal Psi.
//! The optional checked-source oracle prepares its own execution bindings in
//! checked_interpreter_dispatch; Product installation does not build that table.

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;

pub(crate) mod float_comparisons;
mod float_intrinsic;
mod operator_adapter;

pub use float_intrinsic::SelectedCompilerIntrinsicExecutionIdentity;
pub(crate) use float_intrinsic::{
    derive_selected_compiler_intrinsic_execution_identity,
    derive_selected_primitive_float_binary_execution,
};
pub use operator_adapter::{
    CheckedOperatorAuthoredUseKind, derive_checked_nongeneric_operator_application_realizations,
    derive_checked_specialized_operator_application_realizations,
};

/// Settle comparison execution without importing runtime adapter dispatch or
/// service grants. Build admission uses this same comparison producer before
/// effects; unlike the product's provisional route, every Build demand must
/// have an executable selected child.
pub fn settle_selected_comparison_execution(
    checked: &mut CheckedTrees,
    scope: source::DependencyScope,
    selected_provider_provenance: &[provider_planning::SelectedProviderReviewProvenance],
) -> Result<(), Vec<Diagnostic>> {
    let executions =
        float_comparisons::selected_executions(checked, scope, selected_provider_provenance)?;
    float_comparisons::replace_executions(checked, executions);
    Ok(())
}
