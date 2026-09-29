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
    selected_provider_provenance: &[provider_planning::SelectedProviderReviewProvenance],
) -> Result<Arc<CheckedTrees>, Vec<Diagnostic>> {
    if selected_provider_provenance.iter().any(|provider| {
        selected_provider_plans
            .plan_by_exact_evidence(provider.plan.report_fingerprint(), &provider.plan)
            .is_none()
    }) {
        return Err(vec![Diagnostic::error(
            "comparison provider provenance disagrees with the selected provider plans",
        )]);
    }
    // Calls retain their public requirement through Terminal Psi. Selecting
    // its provider adds associations, not new source facts or execution plans;
    // rebuilding Psi here would introduce a second stage entrance and require
    // exclusive custody of an otherwise shareable checked program.
    crate::boundary_dispatch::settle_selected_boundary_adapter_dispatch(
        &mut checked,
        selected_provider_plans,
    )?;
    let executions = float_comparisons::selected_executions(
        &checked,
        source::DependencyScope::Product,
        selected_provider_provenance,
    )?;
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
