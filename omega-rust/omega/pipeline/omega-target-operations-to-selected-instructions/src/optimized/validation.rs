use omega_abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations;

use crate::{
    ValidatedLegalizedOperations, ValidatedSelectedInstructions, validate_legalized_operations,
    validate_selected_instructions,
};
use omega_target_to_register_environment::ValidatedTargetRegisterEnvironment;

use super::constraints::selection_constraints;
use super::model::{OptimizedSelectionCustodyError, StagedOptimizedSelectionCustodyReceipt};

pub fn validate_optimized_selection_custody(
    optimized_target: &ValidatedOptimizedTargetOperations,
    register_environment: &ValidatedTargetRegisterEnvironment,
    legalized: &ValidatedLegalizedOperations,
    selected: &ValidatedSelectedInstructions,
) -> Result<StagedOptimizedSelectionCustodyReceipt, OptimizedSelectionCustodyError> {
    let target = optimized_target.target_operations();
    let plan = selected.plan();
    if target.psi != plan.psi
        || target.target != plan.target
        || target.entry != plan.entry
        || optimized_target.target() != target.target
    {
        return Err(OptimizedSelectionCustodyError::RootMismatch);
    }
    if register_environment.target() != target.target {
        return Err(OptimizedSelectionCustodyError::RegisterEnvironmentTargetMismatch);
    }
    let relegalized = validate_legalized_operations(
        target,
        optimized_target.optimized().plan(),
        optimized_target.optimized().unit(),
        legalized.plan().clone(),
    )
    .map_err(|_| OptimizedSelectionCustodyError::LegalizedPlanRevalidationFailed)?;
    if relegalized.receipt() != legalized.receipt() {
        return Err(OptimizedSelectionCustodyError::LegalizedReceiptMismatch);
    }
    let selection_constraints = selection_constraints(legalized, register_environment);
    let revalidated = validate_selected_instructions(
        legalized,
        &selection_constraints,
        register_environment.physical(),
        register_environment.constraints(),
        selected.plan().clone(),
    )
    .map_err(|_| OptimizedSelectionCustodyError::SelectedPlanRevalidationFailed)?;
    if revalidated.receipt() != selected.receipt() {
        return Err(OptimizedSelectionCustodyError::SelectedReceiptMismatch);
    }
    let unit = optimized_target.optimized().unit();
    if selected.receipt().optimization_unit() != unit.identity {
        return Err(OptimizedSelectionCustodyError::UnitIdentityMismatch);
    }
    if selected.receipt().legalized() != legalized.receipt().identity() {
        return Err(OptimizedSelectionCustodyError::LegalizedReceiptMismatch);
    }
    if selected.receipt().legalization_validator() != legalized.receipt().validator() {
        return Err(OptimizedSelectionCustodyError::LegalizedReceiptMismatch);
    }
    if selected.receipt().fuel_schedule() != unit.fuel_schedule
        || plan.fuel_schedule != unit.fuel_schedule
    {
        return Err(OptimizedSelectionCustodyError::FuelScheduleMismatch);
    }
    if target.functions.len()
        != plan.functions.len()
            + plan.structural_unit_functions.len()
            + 2 * plan.projected_structural_call_returns.len()
        || target.functions.iter().any(|target| {
            let ordinary_matches = plan
                .functions
                .iter()
                .filter(|selected| {
                    target.machine == selected.machine
                        && target.attachment == selected.attachment
                        && target.provenance == selected.provenance
                })
                .count();
            let structural_matches = plan
                .structural_unit_functions
                .iter()
                .filter(|selected| {
                    target.machine == selected.machine
                        && target.attachment == selected.attachment
                        && target.provenance == selected.provenance
                })
                .count();
            let projected_matches = legalized
                .plan()
                .projected_structural_call_returns
                .iter()
                .flat_map(|closure| [&closure.caller, &closure.callee])
                .filter(|source| *source == target)
                .count();
            ordinary_matches + structural_matches + projected_matches != 1
        })
    {
        return Err(OptimizedSelectionCustodyError::FunctionRosterMismatch);
    }
    Ok(StagedOptimizedSelectionCustodyReceipt {
        psi: target.psi,
        target: target.target,
        entry: target.entry,
        optimization: optimized_target.optimized().identity_bundle().identity(),
        projection: optimized_target.optimized().validation().identity(),
        manifest: optimized_target
            .optimized()
            .pre_physical_manifest()
            .record()
            .identity,
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        register_environment: register_environment.identity(),
        legalized: legalized.receipt().identity(),
        legalization_validator: legalized.receipt().validator(),
        selected: selected.receipt().identity(),
        function_count: plan.functions.len()
            + plan.structural_unit_functions.len()
            + 2 * plan.projected_structural_call_returns.len(),
    })
}
