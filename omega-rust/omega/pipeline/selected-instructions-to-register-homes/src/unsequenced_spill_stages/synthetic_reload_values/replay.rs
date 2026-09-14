//! Independent replay from insertion actions keyed by logical reload identity.

use std::collections::BTreeMap;

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::{
    FunctionSyntheticReloadValues, LogicalReloadValueId, SyntheticReloadValueBinding,
    SyntheticReloadValueError, SyntheticReloadValueId, SyntheticReloadValuePlan,
    SyntheticReloadValuePolicy, ValidatedAbstractSpillInsertion, ValidatedReloadValueHomes,
};

struct PendingBinding {
    function: usize,
    binding: SyntheticReloadValueBinding,
}

pub(super) fn replay(
    insertion: &ValidatedAbstractSpillInsertion,
    homes: &ValidatedReloadValueHomes,
    policy: SyntheticReloadValuePolicy,
    budget: OptimizationWorkBudget,
) -> Result<SyntheticReloadValuePlan, SyntheticReloadValueError> {
    let insertion_identity = insertion.receipt().identity();
    if homes.receipt().abstract_spill_insertion() != insertion_identity
        || insertion.plan().functions.len() != homes.plan().functions.len()
    {
        return Err(SyntheticReloadValueError::RootMismatch);
    }
    if !matches!(
        policy,
        SyntheticReloadValuePolicy::ValidatedSingleSpillEpochZeroCanonicalOrderV1
    ) {
        return Err(SyntheticReloadValueError::UnsupportedPolicy);
    }

    let homes_by_function = index_homes(homes)?;
    let mut pending = Vec::new();
    for (function, source) in insertion.plan().functions.iter().enumerate() {
        let home_function = &homes.plan().functions[function];
        if source.machine != home_function.machine {
            return Err(SyntheticReloadValueError::FunctionMismatch { function });
        }
        match &source.action {
            None if home_function.assignment.is_none() => {}
            None => return Err(SyntheticReloadValueError::UnexpectedReloadHome { function }),
            Some(action) => {
                let home = homes_by_function
                    .get(&(function, action.reload.result))
                    .ok_or(SyntheticReloadValueError::MissingReloadHome { function })?;
                pending.push(PendingBinding {
                    function,
                    binding: reconstruct_binding(function, action, home)?,
                });
            }
        }
    }
    pending.sort_by_key(|pending| (pending.function, pending.binding.logical));
    for (ordinal, pending) in pending.iter_mut().enumerate() {
        pending.binding.synthetic = SyntheticReloadValueId {
            epoch: 0,
            ordinal: u32::try_from(ordinal)
                .map_err(|_| SyntheticReloadValueError::SyntheticNamespaceOverflow)?,
        };
    }

    let mut functions = insertion
        .plan()
        .functions
        .iter()
        .map(|function| FunctionSyntheticReloadValues {
            machine: function.machine,
            binding: None,
        })
        .collect::<Vec<_>>();
    for pending in pending {
        functions[pending.function].binding = Some(pending.binding);
    }
    let usage = reconstruct_usage(&functions)?;
    if !usage.within(budget) {
        return Err(SyntheticReloadValueError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    Ok(SyntheticReloadValuePlan {
        abstract_spill_insertion: insertion_identity,
        reload_value_homes: homes.receipt().identity(),
        policy,
        budget,
        usage,
        functions,
    })
}

fn index_homes(
    homes: &ValidatedReloadValueHomes,
) -> Result<
    BTreeMap<(usize, LogicalReloadValueId), &crate::ReloadValueHomeAssignment>,
    SyntheticReloadValueError,
> {
    let mut indexed = BTreeMap::new();
    for (function, home_function) in homes.plan().functions.iter().enumerate() {
        if let Some(home) = &home_function.assignment
            && indexed.insert((function, home.result), home).is_some()
        {
            return Err(SyntheticReloadValueError::UnexpectedReloadHome { function });
        }
    }
    Ok(indexed)
}

fn reconstruct_binding(
    function: usize,
    action: &crate::AbstractSpillInsertionAction,
    home: &crate::ReloadValueHomeAssignment,
) -> Result<SyntheticReloadValueBinding, SyntheticReloadValueError> {
    let first = action
        .rewrites
        .iter()
        .min()
        .ok_or(SyntheticReloadValueError::ReloadMismatch { function })?;
    if (
        action.reload.result,
        action.reload.destination_class,
        first.block,
        first.point,
    ) != (home.result, home.class, home.block, home.start)
    {
        return Err(SyntheticReloadValueError::ReloadMismatch { function });
    }
    Ok(SyntheticReloadValueBinding {
        logical: action.reload.result,
        synthetic: SyntheticReloadValueId {
            epoch: u32::MAX,
            ordinal: u32::MAX,
        },
        block: first.block,
        start: first.point,
        exclusive_end: home.exclusive_end,
        class: action.reload.destination_class,
        view: home.view,
    })
}

fn reconstruct_usage(
    functions: &[FunctionSyntheticReloadValues],
) -> Result<OptimizationWorkUsage, SyntheticReloadValueError> {
    let mut function_count = 0_u64;
    let mut binding_count = 0_u64;
    for function in functions {
        function_count = function_count
            .checked_add(1)
            .ok_or(SyntheticReloadValueError::WorkOverflow)?;
        if function.binding.is_some() {
            binding_count = binding_count
                .checked_add(1)
                .ok_or(SyntheticReloadValueError::WorkOverflow)?;
        }
    }
    Ok(OptimizationWorkUsage {
        rule_evaluations: function_count,
        candidates: binding_count,
        validation_steps: binding_count
            .checked_mul(7)
            .ok_or(SyntheticReloadValueError::WorkOverflow)?,
        commits: binding_count,
        iterations: function_count,
    })
}
