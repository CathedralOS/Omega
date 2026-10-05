//! Optimizer module role: stage group. Mandatory target legalization: construct the canonical plan.
//!
//! `source` projects ordinary instruction graphs; `replay` is the retained-plan
//! verifier behind `validate_legalized_operations`.

mod model;
mod replay;
mod runtime_indices;
mod scalar_graph_input;
mod source;
mod source_input;
pub use source_input::LegalizationSource;

pub(crate) use model::legalization_validator_identity;
pub use model::{LegalizationError, LegalizationValidationReceipt, ValidatedLegalizedOperations};

use abstract_operations::AbstractOperationPlan;
use legalized_operations::{LegalizedOperationPlan, legalized_operation_plan_identity};
use target_operations::TargetOperationPlan;

use replay::replay_terminal_legalized_plan;
use source::derive_source_function_rosters;

pub fn legalize_target_operations<'source>(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    source: impl Into<LegalizationSource<'source>>,
) -> Result<ValidatedLegalizedOperations, LegalizationError> {
    let source = source.into();
    let unit = source.unit;
    let rosters = derive_source_function_rosters(target, abstract_plan, unit)?;
    let plan = LegalizedOperationPlan {
        psi: target.psi,
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        target: target.target,
        entry: target.entry,
        scalar_functions: rosters.scalar_functions,
    };
    Ok(seal(target, unit, plan))
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
    replay_terminal_legalized_plan(target, abstract_plan, unit, &plan, source.verified_input)?;
    Ok(seal(target, unit, plan))
}

fn seal(
    target: &TargetOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    plan: LegalizedOperationPlan,
) -> ValidatedLegalizedOperations {
    let receipt = LegalizationValidationReceipt {
        identity: legalized_operation_plan_identity(&plan),
        validator: legalization_validator_identity(),
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        target: target.target,
        function_count: plan.scalar_functions.len(),
    };
    ValidatedLegalizedOperations { plan, receipt }
}
pub(crate) use scalar_graph_input::exact_cast_has_native_carriers;
