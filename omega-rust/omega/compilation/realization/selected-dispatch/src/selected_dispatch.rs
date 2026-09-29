//! Settle selected execution from checked facts without rewriting Psi.
//!
//! A call or operator application keeps naming the public requirement it
//! resolved to; the provider the Build selected is installed by Omega, not
//! spliced into the checked body. This owner therefore changes no source: it
//! settles the boundary-dispatch rows while retaining the checked execution
//! plans and records selected float comparison executions. Missing-plan
//! diagnostics remain the checked producer's original dependency/phase
//! report; provider selection does not reinterpret a Psi construction failure.

use std::sync::Arc;

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use effects::SelectedProviderPlanFacts;

mod float_comparisons;
mod float_intrinsic;
mod operator_adapter;

pub use float_intrinsic::{
    SelectedCompilerIntrinsicExecutionIdentity,
    derive_selected_compiler_intrinsic_execution_identity,
    derive_selected_primitive_float_binary_execution,
};
pub use operator_adapter::{
    CheckedNongenericOperatorApplicationRealization, CheckedOperatorAuthoredUseKind,
    CheckedSpecializedOperatorApplicationRealization,
    derive_checked_nongeneric_operator_application_realizations,
    derive_checked_specialized_operator_application_realizations,
    validate_selected_operator_terminal_custody,
};

/// Settle selected execution for the checked program the caller hands over.
/// Failure publishes nothing.
pub fn settle_selected_execution_dispatch(
    mut checked: Arc<CheckedTrees>,
    selected_provider_plans: &SelectedProviderPlanFacts,
) -> Result<Arc<CheckedTrees>, Vec<Diagnostic>> {
    // Calls retain their public requirement through Terminal Psi. Selecting
    // its provider adds associations, not new source facts or execution plans;
    // rebuilding Psi here would introduce a second stage entrance and require
    // exclusive custody of an otherwise shareable checked program.
    crate::boundary_dispatch::settle_selected_boundary_adapter_dispatch(
        &mut checked,
        selected_provider_plans,
    )?;
    let executions =
        float_comparisons::selected_executions(&checked, selected_provider_plans.plans())?;
    if !checked
        .facts
        .operators
        .selected_float_comparisons
        .iter()
        .map(|(_, execution)| execution)
        .eq(executions.iter())
    {
        float_comparisons::replace_executions(Arc::make_mut(&mut checked), executions);
    }
    Ok(checked)
}
