//! Canonical producer traversal for synthetic reload identities.

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::{
    FunctionSyntheticReloadValues, SyntheticReloadValueBinding, SyntheticReloadValueError,
    SyntheticReloadValueId, SyntheticReloadValuePlan, SyntheticReloadValuePolicy,
    ValidatedAbstractSpillInsertion, ValidatedReloadValueHomes,
};

pub(super) fn compute(
    insertion: &ValidatedAbstractSpillInsertion,
    homes: &ValidatedReloadValueHomes,
    policy: SyntheticReloadValuePolicy,
    budget: OptimizationWorkBudget,
) -> Result<SyntheticReloadValuePlan, SyntheticReloadValueError> {
    validate_roots(insertion, homes)?;
    if policy != SyntheticReloadValuePolicy::ValidatedSingleSpillEpochZeroCanonicalOrderV1 {
        return Err(SyntheticReloadValueError::UnsupportedPolicy);
    }
    let mut next_ordinal = 0_u32;
    let mut functions = Vec::with_capacity(insertion.plan().functions.len());
    for (function, (insertion_function, home_function)) in insertion
        .plan()
        .functions
        .iter()
        .zip(&homes.plan().functions)
        .enumerate()
    {
        if insertion_function.machine != home_function.machine {
            return Err(SyntheticReloadValueError::FunctionMismatch { function });
        }
        let binding = match (&insertion_function.action, &home_function.assignment) {
            (None, None) => None,
            (Some(_), None) => {
                return Err(SyntheticReloadValueError::MissingReloadHome { function });
            }
            (None, Some(_)) => {
                return Err(SyntheticReloadValueError::UnexpectedReloadHome { function });
            }
            (Some(action), Some(home)) => {
                let synthetic = SyntheticReloadValueId {
                    epoch: 0,
                    ordinal: next_ordinal,
                };
                next_ordinal = next_ordinal
                    .checked_add(1)
                    .ok_or(SyntheticReloadValueError::SyntheticNamespaceOverflow)?;
                Some(binding(function, action, home, synthetic)?)
            }
        };
        functions.push(FunctionSyntheticReloadValues {
            machine: insertion_function.machine,
            binding,
        });
    }
    let usage = usage(&functions)?;
    if !usage.within(budget) {
        return Err(SyntheticReloadValueError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    Ok(SyntheticReloadValuePlan {
        abstract_spill_insertion: insertion.receipt().identity(),
        reload_value_homes: homes.receipt().identity(),
        policy,
        budget,
        usage,
        functions,
    })
}

fn validate_roots(
    insertion: &ValidatedAbstractSpillInsertion,
    homes: &ValidatedReloadValueHomes,
) -> Result<(), SyntheticReloadValueError> {
    if homes.receipt().abstract_spill_insertion() != insertion.receipt().identity()
        || insertion.plan().functions.len() != homes.plan().functions.len()
    {
        return Err(SyntheticReloadValueError::RootMismatch);
    }
    Ok(())
}

fn binding(
    function: usize,
    action: &crate::AbstractSpillInsertionAction,
    home: &crate::ReloadValueHomeAssignment,
    synthetic: SyntheticReloadValueId,
) -> Result<SyntheticReloadValueBinding, SyntheticReloadValueError> {
    let first = action
        .rewrites
        .first()
        .ok_or(SyntheticReloadValueError::ReloadMismatch { function })?;
    let fields_match = action.reload.result == home.result
        && action.reload.destination_class == home.class
        && first.block == home.block
        && first.point == home.start;
    if !fields_match {
        return Err(SyntheticReloadValueError::ReloadMismatch { function });
    }
    Ok(SyntheticReloadValueBinding {
        logical: home.result,
        synthetic,
        block: home.block,
        start: home.start,
        exclusive_end: home.exclusive_end,
        class: home.class,
        view: home.view,
    })
}

fn usage(
    functions: &[FunctionSyntheticReloadValues],
) -> Result<OptimizationWorkUsage, SyntheticReloadValueError> {
    let function_count =
        u64::try_from(functions.len()).map_err(|_| SyntheticReloadValueError::WorkOverflow)?;
    let binding_count = u64::try_from(
        functions
            .iter()
            .filter(|function| function.binding.is_some())
            .count(),
    )
    .map_err(|_| SyntheticReloadValueError::WorkOverflow)?;
    let validation_steps = binding_count
        .checked_mul(7)
        .ok_or(SyntheticReloadValueError::WorkOverflow)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: function_count,
        candidates: binding_count,
        validation_steps,
        commits: binding_count,
        iterations: function_count,
    })
}
