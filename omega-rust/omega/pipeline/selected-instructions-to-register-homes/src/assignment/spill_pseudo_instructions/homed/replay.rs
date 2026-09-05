//! Independent keyed reconstruction of V2 pseudos and destination homes.

mod work;

use std::collections::{BTreeMap, BTreeSet};

use optimization_core::OptimizationWorkBudget;

use crate::{
    FunctionHomedSpillPseudoInstructions, HomedSpillPseudoInstruction,
    HomedSpillPseudoInstructionError, HomedSpillPseudoInstructionPlan,
    HomedSpillPseudoInstructionPolicy, SpillPseudoInstruction, ValidatedRecursiveReloadValueHomes,
    ValidatedSpillPseudoInstructions,
};

pub(super) fn replay(
    source: &ValidatedSpillPseudoInstructions,
    homes: &ValidatedRecursiveReloadValueHomes,
    policy: HomedSpillPseudoInstructionPolicy,
    budget: OptimizationWorkBudget,
) -> Result<HomedSpillPseudoInstructionPlan, HomedSpillPseudoInstructionError> {
    reconstruct_roots(source, homes, policy)?;
    let mut functions = Vec::new();
    for function in 0..source.plan().functions.len() {
        functions.push(reconstruct_function(
            function,
            &source.plan().functions[function],
            &homes.plan().functions[function],
        )?);
    }
    let usage = work::reconstruct(&functions)?;
    if !usage.within(budget) {
        return Err(HomedSpillPseudoInstructionError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    Ok(HomedSpillPseudoInstructionPlan {
        spill_pseudo_instructions: source.receipt().identity(),
        recursive_reload_value_homes: homes.receipt().identity(),
        register_environment: source.plan().register_environment,
        allocator_availability: source.plan().allocator_availability,
        optimization_unit: source.plan().optimization_unit,
        fuel_schedule: source.plan().fuel_schedule,
        policy,
        budget,
        usage,
        functions,
    })
}

fn reconstruct_roots(
    source: &ValidatedSpillPseudoInstructions,
    homes: &ValidatedRecursiveReloadValueHomes,
    policy: HomedSpillPseudoInstructionPolicy,
) -> Result<(), HomedSpillPseudoInstructionError> {
    if !matches!(
        policy,
        HomedSpillPseudoInstructionPolicy::RecursiveLogicalScheduleWithClosedReloadHomesV2
    ) {
        return Err(HomedSpillPseudoInstructionError::UnsupportedPolicy);
    }
    let left = source.plan();
    let right = homes.plan();
    if left.recursive_spill_insertion != right.recursive_spill_insertion
        || [left.register_environment, right.register_environment]
            .into_iter()
            .any(|root| root != left.register_environment)
        || left.allocator_availability != right.allocator_availability
        || left.optimization_unit != right.optimization_unit
        || left.fuel_schedule != right.fuel_schedule
        || left.functions.len() != right.functions.len()
    {
        return Err(HomedSpillPseudoInstructionError::RootMismatch);
    }
    Ok(())
}

fn reconstruct_function(
    function: usize,
    source: &crate::FunctionSpillPseudoInstructions,
    homes: &crate::FunctionRecursiveReloadValueHomes,
) -> Result<FunctionHomedSpillPseudoInstructions, HomedSpillPseudoInstructionError> {
    if source.machine != homes.machine {
        return Err(HomedSpillPseudoInstructionError::FunctionMismatch { function });
    }
    let mut home_by_action = BTreeMap::new();
    for home in &homes.assignments {
        if home_by_action.insert(home.result, home).is_some() {
            return Err(HomedSpillPseudoInstructionError::DuplicateHome {
                function,
                action: home.result,
            });
        }
    }
    let mut ids = BTreeSet::new();
    let mut instructions = Vec::new();
    for instruction in &source.instructions {
        let rebuilt = match *instruction {
            SpillPseudoInstruction::Store {
                id,
                action,
                block,
                point,
                before_instruction,
                before_reload,
                source,
                source_view,
                storage,
            } => HomedSpillPseudoInstruction::Store {
                id,
                action,
                block,
                point,
                before_instruction,
                before_reload,
                source,
                source_view,
                storage,
            },
            SpillPseudoInstruction::Reload {
                id,
                action,
                block,
                point,
                before_instruction,
                storage,
                result,
                destination_class,
            } => {
                let home = home_by_action.remove(&result).ok_or(
                    HomedSpillPseudoInstructionError::MissingHome {
                        function,
                        action: result,
                    },
                )?;
                if action != result
                    || home.block != block
                    || home.start != point
                    || home.class != destination_class
                    || home.candidates.binary_search(&home.view).is_err()
                {
                    return Err(HomedSpillPseudoInstructionError::InvalidHome {
                        function,
                        action: result,
                    });
                }
                HomedSpillPseudoInstruction::Reload {
                    id,
                    action,
                    block,
                    point,
                    before_instruction,
                    storage,
                    result,
                    destination_class,
                    destination_view: home.view,
                }
            }
        };
        if !ids.insert(rebuilt.id()) {
            return Err(HomedSpillPseudoInstructionError::InvalidPseudoOrder { function });
        }
        instructions.push(rebuilt);
    }
    if ids
        .iter()
        .map(|id| id.ordinal)
        .ne(0..u32::try_from(ids.len())
            .map_err(|_| HomedSpillPseudoInstructionError::WorkOverflow)?)
    {
        return Err(HomedSpillPseudoInstructionError::InvalidPseudoOrder { function });
    }
    if let Some((&action, _)) = home_by_action.first_key_value() {
        return Err(HomedSpillPseudoInstructionError::MissingHome { function, action });
    }
    Ok(FunctionHomedSpillPseudoInstructions {
        machine: source.machine,
        spill_area_bytes: source.spill_area_bytes,
        storage: source.storage.clone(),
        instructions,
        rewrites: source.rewrites.clone(),
    })
}
