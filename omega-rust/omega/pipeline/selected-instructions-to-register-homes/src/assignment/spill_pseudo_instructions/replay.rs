//! Independently keyed reconstruction of spill pseudos and rewrite links.

use std::collections::{BTreeMap, BTreeSet};

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::{
    FunctionSpillPseudoInstructions, RecursiveSpillEvent, RecursiveSpillStoredValue,
    SpillPseudoInstruction, SpillPseudoInstructionError, SpillPseudoInstructionId,
    SpillPseudoInstructionPlan, SpillPseudoInstructionPolicy, SpillPseudoOperandRewrite,
    SpillPseudoStorage, SpillPseudoStoredValue, ValidatedRecursiveSpillInsertion,
};

pub(super) fn replay(
    source: &ValidatedRecursiveSpillInsertion,
    policy: SpillPseudoInstructionPolicy,
    budget: OptimizationWorkBudget,
) -> Result<SpillPseudoInstructionPlan, SpillPseudoInstructionError> {
    if !matches!(
        policy,
        SpillPseudoInstructionPolicy::RecursiveLogicalScheduleV1
    ) {
        return Err(SpillPseudoInstructionError::UnsupportedPolicy);
    }
    let mut functions = Vec::new();
    let mut total_storage = 0_u64;
    let mut total_instructions = 0_u64;
    let mut total_rewrites = 0_u64;
    let mut total_events = 0_u64;
    for (function, row) in source.plan().functions.iter().enumerate() {
        let projected = replay_function(function, row)?;
        total_storage = add(total_storage, projected.storage.len())?;
        total_instructions = add(total_instructions, projected.instructions.len())?;
        total_rewrites = add(total_rewrites, projected.rewrites.len())?;
        total_events = add(total_events, row.schedule.len())?;
        functions.push(projected);
    }
    let function_count = count(functions.len())?;
    let usage = OptimizationWorkUsage {
        rule_evaluations: function_count,
        candidates: total_instructions,
        validation_steps: total_storage
            .checked_add(total_events)
            .and_then(|value| value.checked_add(total_instructions))
            .and_then(|value| value.checked_add(total_rewrites))
            .ok_or(SpillPseudoInstructionError::WorkOverflow)?,
        commits: total_instructions
            .checked_add(total_rewrites)
            .ok_or(SpillPseudoInstructionError::WorkOverflow)?,
        iterations: function_count
            .checked_add(total_storage)
            .and_then(|value| value.checked_add(total_events))
            .ok_or(SpillPseudoInstructionError::WorkOverflow)?,
    };
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

fn replay_function(
    function: usize,
    source: &crate::FunctionRecursiveSpillInsertion,
) -> Result<FunctionSpillPseudoInstructions, SpillPseudoInstructionError> {
    let mut storage_by_id = BTreeMap::new();
    for slot in &source.slots {
        let row = SpillPseudoStorage {
            id: slot.action,
            class: slot.class,
            block: slot.block,
            live_from: slot.live_from,
            live_through: slot.live_through,
            size_bytes: slot.size_bytes,
            alignment_bytes: slot.alignment_bytes,
            spill_area_offset: slot.spill_area_offset,
        };
        if storage_by_id.insert(slot.action, row).is_some() {
            return Err(SpillPseudoInstructionError::DuplicateStorage {
                function,
                storage: slot.action,
            });
        }
    }
    if storage_by_id
        .keys()
        .copied()
        .ne(source.slots.iter().map(|slot| slot.action))
    {
        return Err(SpillPseudoInstructionError::InvalidPseudoOrder { function });
    }

    let mut reload_by_action = BTreeMap::new();
    let mut reload_by_result = BTreeMap::new();
    let mut pseudo_ordinal = 0_u32;
    for event in &source.schedule {
        match *event {
            RecursiveSpillEvent::Store { .. } => {
                pseudo_ordinal = pseudo_ordinal
                    .checked_add(1)
                    .ok_or(SpillPseudoInstructionError::IdentityOverflow)?;
            }
            RecursiveSpillEvent::Reload { action, result, .. } => {
                let id = SpillPseudoInstructionId {
                    ordinal: pseudo_ordinal,
                };
                pseudo_ordinal = pseudo_ordinal
                    .checked_add(1)
                    .ok_or(SpillPseudoInstructionError::IdentityOverflow)?;
                if reload_by_action.insert(action, id).is_some()
                    || reload_by_result.insert(result, id).is_some()
                {
                    return Err(SpillPseudoInstructionError::DuplicateReload { function, action });
                }
            }
            RecursiveSpillEvent::Rewrite { .. } => {}
        }
    }

    let mut instructions = Vec::new();
    let mut rewrites = Vec::new();
    let mut seen_ids = BTreeSet::new();
    let mut ordinal = 0_u32;
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
                let id = SpillPseudoInstructionId { ordinal };
                ordinal = ordinal
                    .checked_add(1)
                    .ok_or(SpillPseudoInstructionError::IdentityOverflow)?;
                let block = storage_by_id.get(&slot).map(|row| row.block).ok_or(
                    SpillPseudoInstructionError::MissingStorage {
                        function,
                        storage: slot,
                    },
                )?;
                let before_reload = before_reload
                    .map(|target| {
                        reload_by_action.get(&target).copied().ok_or(
                            SpillPseudoInstructionError::MissingReload {
                                function,
                                action: target,
                            },
                        )
                    })
                    .transpose()?;
                let source = match stored {
                    RecursiveSpillStoredValue::Original(register) => {
                        SpillPseudoStoredValue::Original(register)
                    }
                    RecursiveSpillStoredValue::Reload(source_action) => {
                        SpillPseudoStoredValue::Reload {
                            action: source_action,
                            producer: reload_by_action.get(&source_action).copied().ok_or(
                                SpillPseudoInstructionError::MissingReload {
                                    function,
                                    action: source_action,
                                },
                            )?,
                        }
                    }
                };
                if before_reload.is_some_and(|reload| reload <= id)
                    || matches!(source, SpillPseudoStoredValue::Reload { producer, .. } if producer >= id)
                {
                    return Err(SpillPseudoInstructionError::InvalidPseudoOrder { function });
                }
                seen_ids.insert(id);
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
                let id = SpillPseudoInstructionId { ordinal };
                ordinal = ordinal
                    .checked_add(1)
                    .ok_or(SpillPseudoInstructionError::IdentityOverflow)?;
                let block = storage_by_id.get(&slot).map(|row| row.block).ok_or(
                    SpillPseudoInstructionError::MissingStorage {
                        function,
                        storage: slot,
                    },
                )?;
                if reload_by_action.get(&action).copied() != Some(id)
                    || reload_by_result.get(&result).copied() != Some(id)
                {
                    return Err(SpillPseudoInstructionError::InvalidPseudoOrder { function });
                }
                seen_ids.insert(id);
                instructions.push(SpillPseudoInstruction::Reload {
                    id,
                    action,
                    block,
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
                producer: reload_by_result
                    .get(&result)
                    .copied()
                    .ok_or(SpillPseudoInstructionError::InvalidRewrite { function, action })?,
            }),
        }
    }
    if seen_ids.len() != instructions.len() || seen_ids.iter().map(|id| id.ordinal).ne(0..ordinal) {
        return Err(SpillPseudoInstructionError::InvalidPseudoOrder { function });
    }
    Ok(FunctionSpillPseudoInstructions {
        machine: source.machine,
        spill_area_bytes: source.spill_area_bytes,
        storage: storage_by_id.into_values().collect(),
        instructions,
        rewrites,
    })
}

fn add(total: u64, value: usize) -> Result<u64, SpillPseudoInstructionError> {
    total
        .checked_add(count(value)?)
        .ok_or(SpillPseudoInstructionError::WorkOverflow)
}

fn count(value: usize) -> Result<u64, SpillPseudoInstructionError> {
    u64::try_from(value).map_err(|_| SpillPseudoInstructionError::WorkOverflow)
}
