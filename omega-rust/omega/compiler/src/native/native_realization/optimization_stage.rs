//! Complete the abstract-optimization phase for verified native input.

use crate::native::native_realization::realization_diagnostics::realization_error;
use crate::native::native_realization::realization_request::{
    NativeRealizationInput, NativeRealizationRequest,
};
use abstract_operations_to_abstract_operations::{
    ValidatedOptimizedAbstractPlan, optimize_abstract_operations,
};
use diagnostics::Diagnostic;
use optimization_core::OptimizationWorkBudget;

pub(crate) fn lower_realization_optimization_stage(
    input: NativeRealizationInput,
    request: &NativeRealizationRequest<'_>,
) -> Result<ValidatedOptimizedAbstractPlan, Vec<Diagnostic>> {
    if !request.optimization_selections.is_empty()
        && (!request.admission.providers.native_callbacks.is_empty()
            || !request.callback_thunks.is_empty())
    {
        return Err(realization_error(
            "optimized native callback custody",
            "retained callbacks require the ordinary custody-preserving pipeline",
        ));
    }
    let selections = request.optimization_selections.selections();
    let program = optimize_abstract_operations(
        input.into_optimization_input(),
        selections,
        &selections.project_psi(),
        native_optimization_budget(),
    )
    .map_err(|error| realization_error("canonical abstract optimization", error))?;
    if request.optimization_selections.is_empty()
        && program.plan() != program.verified_input().plan()
    {
        return Err(realization_error(
            "abstract optimization identity",
            "empty selection changed the ordinary abstract-operation plan",
        ));
    }
    Ok(program)
}

/// Product-owned per-pass ceilings shared by program and callback realization.
/// These are not a source-visible optimization level or intensity preset.
pub(super) fn native_optimization_budget() -> OptimizationWorkBudget {
    // Physical preservation counts input traversal under both validation_steps
    // and iterations; these are not solely optimizer fixed-point rounds. Keep
    // their capacities aligned so mandatory identity staging can use the same
    // bounded work allowance. Producers and independent replay still meter it.
    OptimizationWorkBudget::new(1_000_000, 100_000, 100_000, 100_000, 100_000)
        .expect("compiler baseline optimizer ceilings are nonzero")
}
