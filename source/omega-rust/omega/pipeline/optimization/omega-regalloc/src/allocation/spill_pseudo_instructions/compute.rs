//! Direct projection of a recursive logical schedule into spill pseudos.

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::{
    FunctionRecursiveSpillInsertion, FunctionSpillPseudoInstructions, RecursiveSpillEvent,
    RecursiveSpillStoredValue, SpillPseudoInstruction, SpillPseudoInstructionError,
    SpillPseudoInstructionId, SpillPseudoInstructionPlan, SpillPseudoInstructionPolicy,
    SpillPseudoOperandRewrite, SpillPseudoStorage, SpillPseudoStoredValue,
    ValidatedRecursiveSpillInsertion,
};

pub(super) fn compute(
    source: &ValidatedRecursiveSpillInsertion,
    policy: SpillPseudoInstructionPolicy,
    budget: OptimizationWorkBudget,
) -> Result<SpillPseudoInstructionPlan, SpillPseudoInstructionError> {
    if policy != SpillPseudoInstructionPolicy::RecursiveLogicalScheduleV1 {
        return Err(SpillPseudoInstructionError::UnsupportedPolicy);
    }
    let mut functions = Vec::with_capacity(source.plan().functions.len());
    for (function, row) in source.plan().functions.iter().enumerate() {
        functions.push(project_function(function, row)?);
    }
    let usage = work_usage(source, &functions)?;
    if !usage.within(budget) {
        return Err(SpillPseudoInstructionError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let receipt = source.receipt();
    Ok(SpillPseudoInstructionPlan {
        recursive_spill_insertion: receipt.identity(),
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

fn project_function(
    function: usize,
    source: &FunctionRecursiveSpillInsertion,
) -> Result<FunctionSpillPseudoInstructions, SpillPseudoInstructionError> {
    let mut storage = Vec::with_capacity(source.slots.len());
    for slot in &source.slots {
        if storage
            .iter()
            .any(|row: &SpillPseudoStorage| row.id == slot.action)
        {
            return Err(SpillPseudoInstructionError::DuplicateStorage {
                function,
                storage: slot.action,
            });
        }
        storage.push(SpillPseudoStorage {
            id: slot.action,
            class: slot.class,
            block: slot.block,
            live_from: slot.live_from,
            live_through: slot.live_through,
            size_bytes: slot.size_bytes,
            alignment_bytes: slot.alignment_bytes,
            spill_area_offset: slot.spill_area_offset,
        });
    }

    let mut ids = Vec::new();
    for event in &source.schedule {
        if let RecursiveSpillEvent::Store { action, .. }
        | RecursiveSpillEvent::Reload { action, .. } = event
        {
            let ordinal = u32::try_from(ids.len())
                .map_err(|_| SpillPseudoInstructionError::IdentityOverflow)?;
            ids.push((*action, SpillPseudoInstructionId { ordinal }, event));
        }
    }
    let reloads = ids
        .iter()
        .filter_map(|(action, id, event)| match event {
            RecursiveSpillEvent::Reload { result, .. } => Some((*action, *result, *id)),
            _ => None,
        })
        .collect::<Vec<_>>();
    for (index, (action, _, _)) in reloads.iter().enumerate() {
        if reloads[..index].iter().any(|(prior, _, _)| prior == action) {
            return Err(SpillPseudoInstructionError::DuplicateReload {
                function,
                action: *action,
            });
        }
    }

    let mut instructions = Vec::with_capacity(ids.len());
    let mut next_id = 0_usize;
    let mut rewrites = Vec::new();
    for event in &source.schedule {
        match *event {
            RecursiveSpillEvent::Store {
                action,
                point,
                before_instruction,
                before_reload,
                source: stored,
                source_view,
                slot,
            } => {
                let id = ids[next_id].1;
                next_id += 1;
                let block = storage_block(function, &storage, slot)?;
                let before_reload = before_reload
                    .map(|action| reload_id(function, &reloads, action))
                    .transpose()?;
                let source = match stored {
                    RecursiveSpillStoredValue::Original(register) => {
                        SpillPseudoStoredValue::Original(register)
                    }
                    RecursiveSpillStoredValue::Reload(action) => SpillPseudoStoredValue::Reload {
                        action,
                        producer: reload_id(function, &reloads, action)?,
                    },
                };
                if before_reload.is_some_and(|reload| reload <= id)
                    || matches!(source, SpillPseudoStoredValue::Reload { producer, .. } if producer >= id)
                {
                    return Err(SpillPseudoInstructionError::InvalidPseudoOrder { function });
                }
                instructions.push(SpillPseudoInstruction::Store {
                    id,
                    action,
                    block,
                    point,
                    before_instruction,
                    before_reload,
                    source,
                    source_view,
                    storage: slot,
                });
            }
            RecursiveSpillEvent::Reload {
                action,
                point,
                before_instruction,
                slot,
                result,
                destination_class,
            } => {
                let id = ids[next_id].1;
                next_id += 1;
                instructions.push(SpillPseudoInstruction::Reload {
                    id,
                    action,
                    block: storage_block(function, &storage, slot)?,
                    point,
                    before_instruction,
                    storage: slot,
                    result,
                    destination_class,
                });
            }
            RecursiveSpillEvent::Rewrite {
                action,
                block,
                point,
                instruction,
                operand,
                result,
            } => rewrites.push(SpillPseudoOperandRewrite {
                action,
                block,
                point,
                instruction,
                operand,
                result,
                producer: result_id(function, &reloads, result)?,
            }),
        }
    }
    Ok(FunctionSpillPseudoInstructions {
        machine: source.machine,
        spill_area_bytes: source.spill_area_bytes,
        storage,
        instructions,
        rewrites,
    })
}

fn storage_block(
    function: usize,
    storage: &[SpillPseudoStorage],
    id: crate::GeneralizedSpillActionId,
) -> Result<omega_selected_instructions::SelectedBlockId, SpillPseudoInstructionError> {
    storage
        .iter()
        .find(|row| row.id == id)
        .map(|row| row.block)
        .ok_or(SpillPseudoInstructionError::MissingStorage {
            function,
            storage: id,
        })
}

fn reload_id(
    function: usize,
    reloads: &[(
        crate::GeneralizedSpillActionId,
        crate::GeneralizedSpillActionId,
        SpillPseudoInstructionId,
    )],
    action: crate::GeneralizedSpillActionId,
) -> Result<SpillPseudoInstructionId, SpillPseudoInstructionError> {
    reloads
        .iter()
        .find(|(candidate, _, _)| *candidate == action)
        .map(|(_, _, id)| *id)
        .ok_or(SpillPseudoInstructionError::MissingReload { function, action })
}

fn result_id(
    function: usize,
    reloads: &[(
        crate::GeneralizedSpillActionId,
        crate::GeneralizedSpillActionId,
        SpillPseudoInstructionId,
    )],
    result: crate::GeneralizedSpillActionId,
) -> Result<SpillPseudoInstructionId, SpillPseudoInstructionError> {
    reloads
        .iter()
        .find(|(_, candidate, _)| *candidate == result)
        .map(|(_, _, id)| *id)
        .ok_or(SpillPseudoInstructionError::InvalidRewrite {
            function,
            action: result,
        })
}

fn work_usage(
    source: &ValidatedRecursiveSpillInsertion,
    functions: &[FunctionSpillPseudoInstructions],
) -> Result<OptimizationWorkUsage, SpillPseudoInstructionError> {
    let function_count = count(functions.len())?;
    let storage_count = sum(functions.iter().map(|row| row.storage.len()))?;
    let instruction_count = sum(functions.iter().map(|row| row.instructions.len()))?;
    let rewrite_count = sum(functions.iter().map(|row| row.rewrites.len()))?;
    let event_count = sum(source.plan().functions.iter().map(|row| row.schedule.len()))?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: function_count,
        candidates: instruction_count,
        validation_steps: storage_count
            .checked_add(event_count)
            .and_then(|value| value.checked_add(instruction_count))
            .and_then(|value| value.checked_add(rewrite_count))
            .ok_or(SpillPseudoInstructionError::WorkOverflow)?,
        commits: instruction_count
            .checked_add(rewrite_count)
            .ok_or(SpillPseudoInstructionError::WorkOverflow)?,
        iterations: function_count
            .checked_add(storage_count)
            .and_then(|value| value.checked_add(event_count))
            .ok_or(SpillPseudoInstructionError::WorkOverflow)?,
    })
}

fn sum(mut values: impl Iterator<Item = usize>) -> Result<u64, SpillPseudoInstructionError> {
    values.try_fold(0_u64, |total, value| {
        total
            .checked_add(count(value)?)
            .ok_or(SpillPseudoInstructionError::WorkOverflow)
    })
}

fn count(value: usize) -> Result<u64, SpillPseudoInstructionError> {
    u64::try_from(value).map_err(|_| SpillPseudoInstructionError::WorkOverflow)
}
