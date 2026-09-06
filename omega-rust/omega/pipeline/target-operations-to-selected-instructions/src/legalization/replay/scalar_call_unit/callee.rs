//! Exact memory-free pair-call callee roster join.

use super::super::shared::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_callee(
    function: usize,
    callee: semantic_vocabulary::MachineId,
    call_plan: &calling_conventions::CallPlan,
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed_plan: &LegalizedOperationPlan,
) -> Result<(), LegalizationError> {
    let mut targets = target
        .functions
        .iter()
        .enumerate()
        .filter(|(_, value)| value.machine == callee);
    let Some((_callee_index, target_callee)) = targets.next() else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let mut abstracts = abstract_plan
        .functions
        .iter()
        .filter(|value| value.machine == callee);
    let mut optimized = unit
        .functions
        .iter()
        .filter(|value| value.machine == callee);
    let mut proposed = proposed_plan
        .functions
        .iter()
        .filter(|value| value.machine() == callee);
    let (Some(abstract_callee), Some(optimized_callee), Some(proposed_callee)) =
        (abstracts.next(), optimized.next(), proposed.next())
    else {
        return Err(Error::NonCanonicalLegalizedPlan);
    };
    if targets.next().is_some()
        || abstracts.next().is_some()
        || optimized.next().is_some()
        || proposed.next().is_some()
        || proposed_callee.machine() != callee
        || !crate::legalization::scalar_call_contract::accepts(
            target.target,
            target_callee,
            abstract_callee,
            optimized_callee,
            call_plan,
        )
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    // Every ordinary proposed function is independently replayed by the outer
    // roster. This check binds this call to that exact non-recursive member.
    Ok(())
}
