//! Optimizer module role: executable entrance. Recognize, validate, and seal one source closure.

mod candidate;
mod custody;
mod grammar;

use abstract_operations::AbstractOperationPlan;
use legalized_operations::{
    LegalizedProjectedStructuralCallReturn, ProjectedStructuralCallReturnLegalizationRecipe,
};
use optimization_unit::PsiOptimizationUnit;
use target_operations::TargetOperationPlan;

use crate::LegalizationError;
use crate::legalization::model::ProjectedStructuralCallReturnLegalizationError as FamilyError;

pub(in crate::legalization) fn derive(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<Option<LegalizedProjectedStructuralCallReturn>, LegalizationError> {
    if !candidate::is_candidate(target, abstract_plan) {
        return Ok(None);
    }
    let (
        [target_caller, target_callee],
        [source_caller, source_callee],
        [unit_caller, unit_callee],
    ) = (
        target.functions.as_slice(),
        abstract_plan.functions.as_slice(),
        unit.functions.as_slice(),
    )
    else {
        return Err(FamilyError::UnsupportedSourceShape.into());
    };
    grammar::validate_pair(
        target,
        abstract_plan,
        target_caller,
        target_callee,
        source_caller,
        source_callee,
        unit_caller,
        unit_callee,
    )?;
    Ok(Some(LegalizedProjectedStructuralCallReturn {
        recipe: ProjectedStructuralCallReturnLegalizationRecipe::OwnedLinearDirectV1,
        caller: target_caller.clone(),
        callee: target_callee.clone(),
        caller_entry_block: unit_caller.entry,
        callee_entry_block: unit_callee.entry,
        caller_nodes: custody::node_custody(unit_caller),
        callee_nodes: custody::node_custody(unit_callee),
    }))
}
