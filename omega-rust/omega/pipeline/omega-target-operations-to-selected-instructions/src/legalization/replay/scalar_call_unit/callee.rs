//! Exact equality-conditional callee roster join.

use super::super::functions::replay_function;
use super::super::shared::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_callee(
    function: usize,
    callee: psi_core::MachineId,
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
    let Some((callee_index, target_callee)) = targets.next() else {
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
        .filter(|value| value.machine == callee);
    let (Some(abstract_callee), Some(optimized_callee), Some(proposed_callee)) =
        (abstracts.next(), optimized.next(), proposed.next())
    else {
        return Err(Error::NonCanonicalLegalizedPlan);
    };
    if targets.next().is_some()
        || abstracts.next().is_some()
        || optimized.next().is_some()
        || proposed.next().is_some()
        || proposed_callee.recipe
            != omega_legalized_operations::LegalizationRecipe::ReturnU64IntegerEqualParametersConditionalV1
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    replay_function(
        callee_index,
        target.target.architecture,
        target_callee,
        abstract_callee,
        optimized_callee,
        &unit.accepted_obligation_facts,
        proposed_callee,
    )?;
    Ok(())
}
