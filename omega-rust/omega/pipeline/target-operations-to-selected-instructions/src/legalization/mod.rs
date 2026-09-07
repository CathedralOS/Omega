//! Optimizer module role: stage group. Mandatory target legalization: construct the canonical plan, then replay it independently.
//!
//! `source` projects ordinary instruction graphs; `replay` independently checks
//! them, including structural signatures and instruction-keyed ownership.

mod admission;
mod model;
mod projected_structural_call_return;
mod replay;
mod scalar_graph_input;
mod source;

pub use model::{
    LegalizationError, LegalizationValidationReceipt,
    ProjectedStructuralCallReturnLegalizationError,
    ProjectedStructuralCallReturnLegalizationReceipt, ValidatedLegalizedOperations,
    legalization_validator_identity, legalization_validator_identity_v17_legacy,
    legalization_validator_identity_v18_legacy, legalization_validator_identity_v19_legacy,
    legalization_validator_identity_v20_legacy, legalization_validator_identity_v21_legacy,
    legalization_validator_identity_v22_legacy,
};

use abstract_operations::AbstractOperationPlan;
use legalized_operations::{LegalizedOperationPlan, legalized_operation_plan_identity};
use optimization_unit::PsiOptimizationUnit;
use target_operations::TargetOperationPlan;

use admission::reject_attached_unit_structural_scalar;
use replay::replay_terminal_legalized_plan;
use source::derive_source_function_rosters;

pub fn legalize_target_operations(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<ValidatedLegalizedOperations, LegalizationError> {
    reject_attached_unit_structural_scalar(target)?;
    let rosters = derive_source_function_rosters(target, abstract_plan, unit)?;
    let plan = LegalizedOperationPlan {
        psi: target.psi,
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        target: target.target,
        entry: target.entry,
        scalar_functions: rosters.scalar_functions,
        projected_structural_call_returns: rosters.projected_structural_call_returns,
    };
    validate_legalized_operations(target, abstract_plan, unit, plan)
}

/// Independently replay the admitted projection from the raw target,
/// abstract, and verified optimization-unit custody against every proposed
/// field.
pub fn validate_legalized_operations(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    plan: LegalizedOperationPlan,
) -> Result<ValidatedLegalizedOperations, LegalizationError> {
    reject_attached_unit_structural_scalar(target)?;
    let (decomposition_count, projected_structural_call_return) =
        replay_terminal_legalized_plan(target, abstract_plan, unit, &plan)?;
    let receipt = LegalizationValidationReceipt {
        identity: legalized_operation_plan_identity(&plan),
        validator: legalization_validator_identity(),
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        target: target.target,
        function_count: plan.scalar_functions.len()
            + plan.projected_structural_call_returns.len() * 2,
        decomposition_count,
        projected_structural_call_return,
    };
    Ok(ValidatedLegalizedOperations { plan, receipt })
}
#[cfg(test)]
pub(crate) use source::accepts_fragment_publication_input;
pub(crate) use source::is_fragment_publication_program;
