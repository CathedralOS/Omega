//! Optimizer module role: executable entrance. Mandatory target legalization: construct the canonical plan, then replay it independently.
//!
//! Start with `catalog` for every admitted form, descend into `source` for
//! producer projection, and into `replay` for independent acceptance.

mod admission;
mod catalog;
#[cfg(test)]
mod catalog_tests;
#[cfg(test)]
mod condition_tests;
mod integer_sequence_input;
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

use admission::{reject_attached_unit_structural_scalar, reject_ranked_countdown};
use replay::replay_terminal_legalized_plan;
use source::derive_source_function_rosters;

pub fn legalize_target_operations(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<ValidatedLegalizedOperations, LegalizationError> {
    reject_ranked_countdown(target)?;
    reject_attached_unit_structural_scalar(target)?;
    let rosters = derive_source_function_rosters(target, abstract_plan, unit)?;
    let plan = LegalizedOperationPlan {
        psi: target.psi,
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        target: target.target,
        entry: target.entry,
        functions: rosters.functions,
        scalar_functions: rosters.scalar_functions,
        structural_unit_functions: rosters.structural_unit_functions,
        projected_structural_call_returns: rosters.projected_structural_call_returns,
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
    reject_attached_unit_structural_scalar(target)?;
    let (decomposition_count, projected_structural_call_return) =
        replay_terminal_legalized_plan(target, abstract_plan, unit, &plan)?;
    let receipt = LegalizationValidationReceipt {
        identity: legalized_operation_plan_identity(&plan),
        validator: legalization_validator_identity(),
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        target: target.target,
        function_count: plan.functions.len()
            + plan.scalar_functions.len()
            + plan.structural_unit_functions.len()
            + plan.projected_structural_call_returns.len() * 2,
        decomposition_count,
        projected_structural_call_return,
    };
    Ok(ValidatedLegalizedOperations { plan, receipt })
}
#[cfg(test)]
pub(crate) use source::accepts_fragment_publication_input;
pub(crate) use source::is_fragment_publication_program;
