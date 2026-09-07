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
        if graphs.is_empty()
            && crate::legalization::scalar_graph_input::match_input(
                target_function,
                abstracted,
                optimized,
                target,
                abstract_plan,
                unit,
            )
            .is_ok()
        {
            return Err(Error::NonCanonicalLegalizedPlan);
        }
        let count = if let [graph] = graphs.as_slice() {
            if proposed
                .functions
                .iter()
                .any(|candidate| candidate.machine() == target_function.machine)
                || proposed
                    .structural_unit_functions
                    .iter()
                    .any(|candidate| candidate.machine == target_function.machine)
            {
                return Err(Error::NonCanonicalLegalizedPlan);
            }
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
        } else if !graphs.is_empty() {
            return Err(Error::NonCanonicalLegalizedPlan);
        } else if matches!(target_function.operation, TargetOperation::UnitBody(_)) {
            let matches = proposed
                .structural_unit_functions
                .iter()
                .filter(|candidate| candidate.machine == target_function.machine)
                .collect::<Vec<_>>();
            let [legalized] = matches.as_slice() else {
                return Err(Error::NonCanonicalLegalizedPlan);
            };
            replay_structural_unit_function(
                index,
                target_function,
                abstracted,
                optimized,
                legalized,
                target,
                abstract_plan,
                unit,
            )?
        } else {
            let mut matches = proposed
                .functions
                .iter()
                .filter(|candidate| candidate.machine() == target_function.machine);
            let legalized = matches.next().ok_or(Error::NonCanonicalLegalizedPlan)?;
            if matches.next().is_some() {
                return Err(Error::NonCanonicalLegalizedPlan);
            }
            match legalized {
                legalized_operations::LegalizedFunction::SharedReturnConditional(legalized) => {
                    super::shared_return::validate(
                        index,
                        target.target,
                        target_function,
                        abstracted,
                        optimized,
                        legalized,
                    )?;
                    0
                }
                legalized_operations::LegalizedFunction::Conditional(legalized) => replay_function(
                    index,
                    target.target.architecture,
                    target_function,
                    abstracted,
                    optimized,
                    &unit.accepted_obligation_facts,
                    legalized,
                )?,
            }
        };
        decomposition_count = decomposition_count
            .checked_add(count)
            .ok_or(Error::NonCanonicalLegalizedPlan)?;
    }
    Ok(decomposition_count)
}
