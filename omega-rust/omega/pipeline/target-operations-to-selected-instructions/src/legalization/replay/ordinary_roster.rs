//! Existing per-function replay outside an atomic plan family.

use super::*;

pub(super) fn replay_remaining(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed: &LegalizedOperationPlan,
) -> Result<usize, LegalizationError> {
    let mut decomposition_count = 0usize;
    for (index, target_function) in target.functions.iter().enumerate() {
        if proposed
            .projected_structural_call_returns
            .iter()
            .any(|closure| {
                target_function.machine == closure.caller.machine
                    || target_function.machine == closure.callee.machine
            })
        {
            continue;
        }
        let abstract_matches = abstract_plan
            .functions
            .iter()
            .filter(|candidate| candidate.machine == target_function.machine)
            .collect::<Vec<_>>();
        let optimized_matches = unit
            .functions
            .iter()
            .filter(|candidate| candidate.machine == target_function.machine)
            .collect::<Vec<_>>();
        let ([abstracted], [optimized]) =
            (abstract_matches.as_slice(), optimized_matches.as_slice())
        else {
            return Err(Error::SourceCustodyMismatch);
        };
        let graphs = proposed
            .scalar_functions
            .iter()
            .filter(|candidate| candidate.machine == target_function.machine)
            .collect::<Vec<_>>();
        let [graph] = graphs.as_slice() else {
            return Err(Error::NonCanonicalLegalizedPlan);
        };
        let count = if crate::legalization::scalar_graph_input::match_input(
            target_function,
            abstracted,
            optimized,
            target,
            abstract_plan,
            unit,
        )
        .is_ok()
        {
            super::scalar_graph::replay(
                target_function,
                abstracted,
                optimized,
                target,
                abstract_plan,
                unit,
                proposed,
                graph,
            )?;
            0
        } else {
            replay_structural_unit_function(
                index,
                target_function,
                abstracted,
                optimized,
                graph,
                target,
                abstract_plan,
                unit,
            )?
        };
        decomposition_count = decomposition_count
            .checked_add(count)
            .ok_or(Error::NonCanonicalLegalizedPlan)?;
    }
    Ok(decomposition_count)
}
