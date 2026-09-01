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
        let count = if matches!(target_function.operation, TargetOperation::UnitBody(_)) {
            let plain = proposed
                .unit_functions
                .iter()
                .filter(|candidate| candidate.machine == target_function.machine)
                .collect::<Vec<_>>();
            let structural = proposed
                .structural_unit_functions
                .iter()
                .filter(|candidate| candidate.machine == target_function.machine)
                .collect::<Vec<_>>();
            match (plain.as_slice(), structural.as_slice()) {
                ([legalized], []) => {
                    replay_unit_function(index, target_function, abstracted, optimized, legalized)?
                }
                ([], [legalized]) => replay_structural_unit_function(
                    index,
                    target_function,
                    abstracted,
                    optimized,
                    legalized,
                    target,
                    abstract_plan,
                    unit,
                )?,
                _ => return Err(Error::NonCanonicalLegalizedPlan),
            }
        } else {
            let mut matches = proposed
                .functions
                .iter()
                .filter(|candidate| candidate.machine == target_function.machine);
            let legalized = matches.next().ok_or(Error::NonCanonicalLegalizedPlan)?;
            if matches.next().is_some() {
                return Err(Error::NonCanonicalLegalizedPlan);
            }
            replay_function(
                index,
                target.target.architecture,
                target_function,
                abstracted,
                optimized,
                &unit.accepted_obligation_facts,
                legalized,
            )?
        };
        decomposition_count = decomposition_count
            .checked_add(count)
            .ok_or(Error::NonCanonicalLegalizedPlan)?;
    }
    Ok(decomposition_count)
}
