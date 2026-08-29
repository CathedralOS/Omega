//! Canonical source-to-legal projection construction.

mod functions;
mod leaves;
mod matchers;
mod shared;
mod structural;

use functions::{derive_source_function, derive_source_unit_function};
use shared::*;
use structural::{derive_source_structural_unit_function, is_plain_unit_function};

pub(crate) fn derive_source_functions(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<Vec<SourceFunction>, LegalizationError> {
    if omega_optimization_validation::validate_psi_optimization_unit(unit).is_err()
        || target.psi != abstract_plan.psi
        || target.psi != unit.psi
        || target.entry != abstract_plan.entry
        || target.entry != unit.entry
        || target.functions.len() != abstract_plan.functions.len()
        || target.functions.len() != unit.functions.len()
        || omega_optimization_unit::recompute_psi_optimization_unit_identity(unit) != unit.identity
    {
        return Err(Error::SourceCustodyMismatch);
    }

    let functions = target
        .functions
        .iter()
        .zip(&abstract_plan.functions)
        .zip(&unit.functions)
        .enumerate()
        .filter(|(_, ((target, _), _))| !matches!(target.operation, TargetOperation::UnitBody(_)))
        .map(|(index, ((target, abstracted), optimized))| {
            derive_source_function(
                index,
                target,
                abstracted,
                optimized,
                &unit.accepted_obligation_facts,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    if functions.iter().any(|function| {
        function.condition_register.architecture() != target.target.architecture
            || match (&function.when_true.value, &function.when_false.value) {
                (
                    SourceLeafValue::EntryParameter { register: left, .. },
                    SourceLeafValue::EntryParameter {
                        register: right, ..
                    },
                ) => {
                    left.architecture() != target.target.architecture
                        || right.architecture() != target.target.architecture
                }
                _ => false,
            }
    }) {
        return Err(Error::SourceCustodyMismatch);
    }
    Ok(functions)
}

pub(crate) fn derive_source_unit_functions(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<Vec<SourceUnitFunction>, LegalizationError> {
    if omega_optimization_validation::validate_psi_optimization_unit(unit).is_err()
        || target.psi != abstract_plan.psi
        || target.psi != unit.psi
        || target.entry != abstract_plan.entry
        || target.entry != unit.entry
        || target.functions.len() != abstract_plan.functions.len()
        || target.functions.len() != unit.functions.len()
        || omega_optimization_unit::recompute_psi_optimization_unit_identity(unit) != unit.identity
    {
        return Err(Error::SourceCustodyMismatch);
    }
    target
        .functions
        .iter()
        .zip(&abstract_plan.functions)
        .zip(&unit.functions)
        .enumerate()
        .filter(|(_, ((target, abstracted), optimized))| {
            is_plain_unit_function(target, abstracted, optimized)
        })
        .map(|(index, ((target, abstracted), optimized))| {
            derive_source_unit_function(index, target, abstracted, optimized)
        })
        .collect()
}

pub(crate) fn derive_source_structural_unit_functions(
    target: &TargetOperationPlan,
    abstract_plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<Vec<SourceStructuralUnitFunction>, LegalizationError> {
    if omega_optimization_validation::validate_psi_optimization_unit(unit).is_err()
        || target.psi != abstract_plan.psi
        || target.psi != unit.psi
        || target.entry != abstract_plan.entry
        || target.entry != unit.entry
        || target.functions.len() != abstract_plan.functions.len()
        || target.functions.len() != unit.functions.len()
        || omega_optimization_unit::recompute_psi_optimization_unit_identity(unit) != unit.identity
    {
        return Err(Error::SourceCustodyMismatch);
    }

    target
        .functions
        .iter()
        .enumerate()
        .filter_map(|(index, target_function)| {
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
                return Some(Err(Error::SourceCustodyMismatch));
            };
            matches!(target_function.operation, TargetOperation::UnitBody(_))
                .then_some((index, target_function, *abstracted, *optimized))
                .filter(|(_, target_function, abstracted, optimized)| {
                    !is_plain_unit_function(target_function, abstracted, optimized)
                })
                .map(|(index, target_function, abstracted, optimized)| {
                    derive_source_structural_unit_function(
                        index,
                        target_function,
                        abstracted,
                        optimized,
                        target,
                        abstract_plan,
                        unit,
                    )
                })
        })
        .collect()
}
