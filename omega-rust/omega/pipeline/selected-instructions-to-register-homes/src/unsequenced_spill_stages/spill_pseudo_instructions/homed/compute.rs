//! Direct V1-pseudo traversal joined to canonical recursive home rows.

mod work;

use optimization_core::OptimizationWorkBudget;

use crate::{
    FunctionHomedSpillPseudoInstructions, HomedSpillPseudoInstruction,
    HomedSpillPseudoInstructionError, HomedSpillPseudoInstructionPlan,
    HomedSpillPseudoInstructionPolicy, SpillPseudoInstruction, ValidatedRecursiveReloadValueHomes,
    ValidatedSpillPseudoInstructions,
};

pub(super) fn compute(
    source: &ValidatedSpillPseudoInstructions,
    homes: &ValidatedRecursiveReloadValueHomes,
    policy: HomedSpillPseudoInstructionPolicy,
    budget: OptimizationWorkBudget,
) -> Result<HomedSpillPseudoInstructionPlan, HomedSpillPseudoInstructionError> {
    admit(source, homes, policy)?;
    let mut functions = Vec::with_capacity(source.plan().functions.len());
    for (function, (source, homes)) in source
        .plan()
        .functions
        .iter()
        .zip(&homes.plan().functions)
        .enumerate()
    {
        functions.push(project(function, source, homes)?);
    }
    let usage = work::usage(&functions)?;
    if !usage.within(budget) {
        return Err(HomedSpillPseudoInstructionError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let receipt = source.receipt();
    Ok(HomedSpillPseudoInstructionPlan {
        spill_pseudo_instructions: receipt.identity(),
        recursive_reload_value_homes: homes.receipt().identity(),
        register_environment: receipt.register_environment(),
        allocator_availability: receipt.allocator_availability(),
        optimization_unit: receipt.optimization_unit(),
        fuel_schedule: receipt.fuel_schedule(),
        policy,
        budget,
        usage,
        functions,
    })
}

fn admit(
    source: &ValidatedSpillPseudoInstructions,
    homes: &ValidatedRecursiveReloadValueHomes,
    policy: HomedSpillPseudoInstructionPolicy,
) -> Result<(), HomedSpillPseudoInstructionError> {
    if policy != HomedSpillPseudoInstructionPolicy::RecursiveLogicalScheduleWithClosedReloadHomesV2
    {
        return Err(HomedSpillPseudoInstructionError::UnsupportedPolicy);
    }
    let first = source.plan();
    let second = homes.plan();
    if first.recursive_spill_insertion != second.recursive_spill_insertion
        || first.register_environment != second.register_environment
        || first.allocator_availability != second.allocator_availability
        || first.optimization_unit != second.optimization_unit
        || first.fuel_schedule != second.fuel_schedule
        || first.functions.len() != second.functions.len()
    {
        return Err(HomedSpillPseudoInstructionError::RootMismatch);
    }
    Ok(())
}

fn project(
    function: usize,
    source: &crate::FunctionSpillPseudoInstructions,
    homes: &crate::FunctionRecursiveReloadValueHomes,
) -> Result<FunctionHomedSpillPseudoInstructions, HomedSpillPseudoInstructionError> {
    if source.machine != homes.machine {
        return Err(HomedSpillPseudoInstructionError::FunctionMismatch { function });
    }
    let mut used = Vec::new();
    let mut instructions = Vec::with_capacity(source.instructions.len());
    for instruction in &source.instructions {
        instructions.push(match *instruction {
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
                let home = homes
                    .assignments
                    .iter()
                    .find(|home| home.result == result)
                    .ok_or(HomedSpillPseudoInstructionError::MissingHome {
                        function,
                        action: result,
                    })?;
                if used.contains(&result) {
                    return Err(HomedSpillPseudoInstructionError::DuplicateHome {
                        function,
                        action: result,
                    });
                }
                if action != result
                    || home.block != block
                    || home.start != point
                    || home.class != destination_class
                    || !home.candidates.contains(&home.view)
                {
                    return Err(HomedSpillPseudoInstructionError::InvalidHome {
                        function,
                        action: result,
                    });
                }
                used.push(result);
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
        });
    }
    if used.len() != homes.assignments.len() {
        let action = homes
            .assignments
            .iter()
            .find(|home| !used.contains(&home.result))
            .map(|home| home.result)
            .unwrap_or(crate::GeneralizedSpillActionId {
                epoch: 0,
                ordinal: 0,
            });
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
