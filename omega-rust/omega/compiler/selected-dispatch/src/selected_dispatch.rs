//! Settle selected execution from checked facts without rewriting Psi.
//!
//! A call or operator application keeps naming the public requirement it
//! resolved to; the provider the Build selected is installed by Omega, not
//! spliced into the checked body. This owner therefore changes no source: it
//! settles the boundary-dispatch rows, rebuilds the Terminal plan lanes when a
//! top-level requirement dispatch needs them, records selected float
//! comparison executions, and names the operator applications Omega cannot
//! install yet so their missing plans report `unimplemented:` instead of an
//! anonymous omission.

use std::sync::Arc;

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use effects::SelectedProviderPlanFacts;

mod float_comparisons;
mod float_intrinsic;
mod operator_adapter;
mod uninstalled;

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
    let uninstalled =
        uninstalled::uninstalled_operator_applications(&checked, selected_provider_plans)?;
    // A settled direct-call row for a top-level boundary requirement changes
    // no source, but the Unit plans built at checking still target the
    // bodyless requirement without its row; rebuild them so the plan builder
    // consumes the settled rows.
    crate::boundary_dispatch::settle_selected_boundary_adapter_dispatch(
        &mut checked,
        selected_provider_plans,
    )?;
    if crate::boundary_dispatch::has_top_level_requirement_dispatch(&checked) {
        let program = Arc::try_unwrap(checked).map_err(|_| {
            vec![Diagnostic::error(
                "selected execution settlement needs sole custody of the checked program",
            )]
        })?;
        checked = Arc::new(typed_trees_to_checked_trees::settle_checked_execution(
            program,
        )?);
    }
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
    uninstalled::name_unplanned_machines(&mut checked, &uninstalled);
    Ok(checked)
}
