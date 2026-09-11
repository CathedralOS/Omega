//! Optimizer module role: stage group. Mandatory target legalization: construct the canonical plan, then replay it independently.
//!
//! `source` projects ordinary instruction graphs; `replay` independently checks
//! them, including structural signatures and instruction-keyed ownership.

mod admission;
mod model;
mod replay;
mod scalar_graph_input;
mod source;
mod source_input;
pub use source_input::LegalizationSource;

pub use model::{
    LegalizationError, LegalizationValidationReceipt, ValidatedLegalizedOperations,
    legalization_validator_identity, legalization_validator_identity_v17_legacy,
    legalization_validator_identity_v18_legacy, legalization_validator_identity_v19_legacy,
    legalization_validator_identity_v20_legacy, legalization_validator_identity_v21_legacy,
    legalization_validator_identity_v22_legacy,
};

use abstract_operations::AbstractOperationPlan;
use legalized_operations::{LegalizedOperationPlan, legalized_operation_plan_identity};
use target_operations::TargetOperationPlan;

use admission::reject_attached_unit_structural_scalar;
use replay::replay_terminal_legalized_plan;
use source::derive_source_function_rosters;

pub fn legalize_target_operations<'source>(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    source: impl Into<LegalizationSource<'source>>,
) -> Result<ValidatedLegalizedOperations, LegalizationError> {
    let source = source.into();
    let unit = source.unit;
    reject_attached_unit_structural_scalar(target)?;
    let rosters =
        derive_source_function_rosters(target, abstract_plan, unit, source.verified_input)?;
    let plan = LegalizedOperationPlan {
        psi: target.psi,
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        target: target.target,
        entry: target.entry,
        scalar_functions: rosters.scalar_functions,
    };
    validate_legalized_operations(target, abstract_plan, source, plan)
}

/// Independently replay the admitted projection from the raw target,
/// abstract, and verified optimization-unit custody against every proposed
/// field.
pub fn validate_legalized_operations<'source>(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    source: impl Into<LegalizationSource<'source>>,
    plan: LegalizedOperationPlan,
) -> Result<ValidatedLegalizedOperations, LegalizationError> {
    let source = source.into();
    let unit = source.unit;
    reject_attached_unit_structural_scalar(target)?;
    replay_terminal_legalized_plan(target, abstract_plan, unit, &plan, source.verified_input)?;
    let receipt = LegalizationValidationReceipt {
        identity: legalized_operation_plan_identity(&plan),
        validator: legalization_validator_identity(),
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        target: target.target,
        function_count: plan.scalar_functions.len(),
    };
    Ok(ValidatedLegalizedOperations { plan, receipt })
}
#[cfg(test)]
pub(crate) use source::accepts_fragment_publication_input;
