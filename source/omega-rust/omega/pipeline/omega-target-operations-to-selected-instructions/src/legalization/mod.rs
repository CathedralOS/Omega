//! Mandatory target legalization: construct the canonical plan, then replay it independently.

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
use omega_target_operations::TargetOperationPlan;

use replay::replay_terminal_legalized_plan;
use source::{
    derive_source_functions, derive_source_structural_unit_functions, derive_source_unit_functions,
};

pub fn legalize_target_operations(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<ValidatedLegalizedOperations, LegalizationError> {
    let plan = LegalizedOperationPlan {
        psi: target.psi,
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        target: target.target,
        entry: target.entry,
        functions: derive_source_functions(target, abstract_plan, unit)?,
        unit_functions: derive_source_unit_functions(target, abstract_plan, unit)?,
        structural_unit_functions: derive_source_structural_unit_functions(
            target,
            abstract_plan,
            unit,
        )?,
    };
    validate_legalized_operations(target, abstract_plan, unit, plan)
}

/// Independently replay the exact admitted V8 projection from the raw target,
/// abstract, and verified optimization-unit custody against every proposed
/// field.
pub fn validate_legalized_operations(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    plan: LegalizedOperationPlan,
) -> Result<ValidatedLegalizedOperations, LegalizationError> {
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
