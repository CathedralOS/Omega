//! Optimizer module role: executable entrance. Notice, replay, and receipt one proposed closure.

mod candidate;
mod contract;
mod custody;

use abstract_operations::AbstractOperationPlan;
use legalized_operations::LegalizedProjectedStructuralCallReturn;
use optimization_unit::PsiOptimizationUnit;
use target_operations::TargetOperationPlan;

use crate::LegalizationError;
use crate::legalization::model::{
    ProjectedStructuralCallReturnLegalizationError as FamilyError,
    ProjectedStructuralCallReturnLegalizationReceipt,
};

pub(in crate::legalization) fn replay(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed: &[LegalizedProjectedStructuralCallReturn],
) -> Result<Option<ProjectedStructuralCallReturnLegalizationReceipt>, LegalizationError> {
    if !candidate::is_candidate(target, abstract_plan) {
        return proposed
            .is_empty()
            .then_some(None)
            .ok_or(FamilyError::UnexpectedProposedClosure.into());
    }
    let [closure] = proposed else {
        return Err(FamilyError::NonCanonicalProposedClosure.into());
    };
    contract::validate(target, abstract_plan, unit, closure).map(Some)
}
