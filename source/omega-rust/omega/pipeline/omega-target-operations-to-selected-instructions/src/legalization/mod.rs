//! Mandatory target legalization: construct the canonical plan, then replay it independently.
//!
//! Start with `catalog` for every admitted form, descend into `source` for
//! producer projection, and into `replay` for independent acceptance.

mod catalog;
#[cfg(test)]
mod catalog_tests;
mod model;
mod replay;
mod source;

pub use model::{
    LegalizationError, LegalizationValidationReceipt, ValidatedLegalizedOperations,
    legalization_validator_identity,
};

use omega_abstract_operations::AbstractOperationPlan;
use omega_legalized_operations::{LegalizedOperationPlan, legalized_operation_plan_identity};
use omega_optimization_unit::PsiOptimizationUnit;
use omega_target_operations::{TargetOperation, TargetOperationPlan};

use replay::replay_terminal_legalized_plan;
use source::derive_source_function_rosters;

pub fn legalize_target_operations(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<ValidatedLegalizedOperations, LegalizationError> {
    reject_ranked_countdown(target)?;
    let rosters = derive_source_function_rosters(target, abstract_plan, unit)?;
    let plan = LegalizedOperationPlan {
        psi: target.psi,
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        target: target.target,
        entry: target.entry,
        functions: rosters.functions,
        unit_functions: rosters.unit_functions,
        structural_unit_functions: rosters.structural_unit_functions,
    };
    validate_legalized_operations(target, abstract_plan, unit, plan)
}

/// Independently replay the exact admitted V9 projection from the raw target,
/// abstract, and verified optimization-unit custody against every proposed
/// field.
pub fn validate_legalized_operations(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    plan: LegalizedOperationPlan,
) -> Result<ValidatedLegalizedOperations, LegalizationError> {
    reject_ranked_countdown(target)?;
    let decomposition_count = replay_terminal_legalized_plan(target, abstract_plan, unit, &plan)?;
    let receipt = LegalizationValidationReceipt {
        identity: legalized_operation_plan_identity(&plan),
        validator: legalization_validator_identity(),
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        target: target.target,
        function_count: plan.functions.len()
            + plan.unit_functions.len()
            + plan.structural_unit_functions.len(),
        decomposition_count,
    };
    Ok(ValidatedLegalizedOperations { plan, receipt })
}

fn reject_ranked_countdown(target: &TargetOperationPlan) -> Result<(), LegalizationError> {
    if let Some(function) = target
        .functions
        .iter()
        .find(|function| matches!(function.operation, TargetOperation::RankedU32Countdown(_)))
    {
        return Err(LegalizationError::RankedCountdownNotYetSelectable {
            machine: function.machine,
        });
    }
    Ok(())
}
