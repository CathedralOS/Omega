//! Optimizer module role: executable entrance. Independent replay of a proposed legal-operation projection.

mod custody;
mod functions;
mod leaf;
mod shared;
mod structural;
mod validators;

use functions::{replay_function, replay_unit_function};
use shared::*;
use structural::replay_structural_unit_function;

/// Independently replay a proposed V9 legal projection against all three raw
/// custody inputs. This module deliberately compares fields in place instead
/// of constructing a second plan with the producer's derivation strategy.
pub(crate) fn replay_terminal_legalized_plan(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed: &LegalizedOperationPlan,
) -> Result<usize, LegalizationError> {
    validate_replay_custody(target, abstract_plan, unit, proposed)?;

    let mut decomposition_count = 0usize;
    for (index, target_function) in target.functions.iter().enumerate() {
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
use custody::validate_replay_custody;
