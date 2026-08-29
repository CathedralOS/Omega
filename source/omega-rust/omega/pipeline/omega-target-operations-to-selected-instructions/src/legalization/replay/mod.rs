//! Independent replay of a proposed legal-operation projection.

mod functions;
mod leaf;
mod shared;
mod structural;
mod validators;

use functions::{replay_function, replay_unit_function};
use shared::*;
use structural::replay_structural_unit_function;

/// Independently replay a proposed V8 legal projection against all three raw
/// custody inputs. This module deliberately compares fields in place instead
/// of constructing a second plan with the producer's derivation strategy.
pub(crate) fn replay_terminal_legalized_plan(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    proposed: &LegalizedOperationPlan,
) -> Result<usize, LegalizationError> {
    if omega_optimization_validation::validate_psi_optimization_unit(unit).is_err()
        || omega_optimization_unit::recompute_psi_optimization_unit_identity(unit) != unit.identity
        || target.psi != abstract_plan.psi
        || target.psi != unit.psi
        || target.entry != abstract_plan.entry
        || target.entry != unit.entry
        || target.functions.len() != abstract_plan.functions.len()
        || target.functions.len() != unit.functions.len()
    {
        return Err(Error::SourceCustodyMismatch);
    }
    if proposed.psi != target.psi
        || proposed.optimization_unit != unit.identity
        || proposed.fuel_schedule != unit.fuel_schedule
        || proposed.target != target.target
        || proposed.entry != target.entry
        || proposed.functions.len()
            + proposed.unit_functions.len()
            + proposed.structural_unit_functions.len()
            != target.functions.len()
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }

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
