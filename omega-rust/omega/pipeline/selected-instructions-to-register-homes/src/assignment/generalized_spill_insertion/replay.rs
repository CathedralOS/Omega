//! Independent keyed reconstruction and occupied-offset slot replay.

use std::collections::{BTreeMap, BTreeSet};

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::{
    FunctionGeneralizedSpillInsertion, GeneralizedSpillActionId, GeneralizedSpillActionSource,
    GeneralizedSpillEvent, GeneralizedSpillInsertionError, GeneralizedSpillInsertionPlan,
    GeneralizedSpillInsertionPolicy, GeneralizedSpillSlot, LogicalSpillStorageClass,
    ValidatedAbstractSpillInsertion, ValidatedSpillRecoveryActions,
};

const SLOT_BYTES: u64 = 8;

struct ReplayAction {
    id: GeneralizedSpillActionId,
    source: GeneralizedSpillActionSource,
    class: LogicalSpillStorageClass,
    block: selected_instructions::SelectedBlockId,
    from: crate::LiveRangePoint,
    through: crate::LiveRangePoint,
    store_instruction: selected_instructions::SelectedInstructionId,
    before_reload: Option<GeneralizedSpillActionId>,
    store_source: selected_instructions::VirtualRegisterId,
    source_view: register_model::RegisterViewId,
    reload_instruction: selected_instructions::SelectedInstructionId,
    destination_class: register_model::RegisterClassId,
    rewrites: Vec<ReplayRewrite>,
}

#[derive(Clone, Copy)]
struct ReplayRewrite {
    block: selected_instructions::SelectedBlockId,
    point: crate::LiveRangePoint,
    instruction: selected_instructions::SelectedInstructionId,
    operand: u16,
}

pub(super) fn replay(
    first: &ValidatedAbstractSpillInsertion,
    second: &ValidatedSpillRecoveryActions,
    policy: GeneralizedSpillInsertionPolicy,
    budget: OptimizationWorkBudget,
) -> Result<GeneralizedSpillInsertionPlan, GeneralizedSpillInsertionError> {
    replay_roots(first, second)?;
    if !matches!(
        policy,
        GeneralizedSpillInsertionPolicy::EpochZeroAndOneBlockLocalUnsignedU64ClosedIntervalFirstFitV1
    ) {
        return Err(GeneralizedSpillInsertionError::UnsupportedPolicy);
    }

    let mut first_rows = first
        .plan()
        .functions
        .iter()
        .enumerate()
        .filter_map(|(function, row)| {
            row.action
                .as_ref()
                .map(|action| (function, row.machine, action))
        })
        .collect::<Vec<_>>();
    first_rows.sort_by_key(|(function, _, action)| (*function, action.reload.result));
    let mut by_function: Vec<Vec<ReplayAction>> = (0..first.plan().functions.len())
        .map(|_| Vec::new())
        .collect();
    let mut reload_index = BTreeMap::new();
    for (ordinal, (function, machine, action)) in first_rows.into_iter().enumerate() {
        if first.plan().functions[function].machine != machine {
            return Err(GeneralizedSpillInsertionError::FunctionMismatch { function });
        }
        let id = GeneralizedSpillActionId {
            epoch: 0,
            ordinal: u32::try_from(ordinal)
                .map_err(|_| GeneralizedSpillInsertionError::NamespaceOverflow)?,
        };
        let replayed = replay_first(function, id, action)?;
        if reload_index
            .insert((function, action.reload.result), id)
            .is_some()
        {
            return Err(GeneralizedSpillInsertionError::InvalidEpochZeroAction { function });
        }
        by_function[function].push(replayed);
    }

    let mut recovery = second.plan().actions.iter().collect::<Vec<_>>();
    recovery.sort_by_key(|action| (action.function, action.source_work_item));
    for action in recovery {
        let function = action.function;
        let Some(function_source) = first.plan().functions.get(function) else {
            return Err(GeneralizedSpillInsertionError::FunctionMismatch { function });
        };
        if function_source.machine != action.machine {
            return Err(GeneralizedSpillInsertionError::FunctionMismatch { function });
        }
        let Some(&dependency) = reload_index.get(&(function, action.source_reload)) else {
            return Err(if function_source.action.is_none() {
                GeneralizedSpillInsertionError::MissingEpochZeroAction { function }
            } else {
                GeneralizedSpillInsertionError::MissingSourceReload {
                    function,
                    reload: action.source_reload,
                }
            });
        };
        by_function[function].push(replay_second(function, dependency, action)?);
    }

    let mut functions = Vec::with_capacity(by_function.len());
    let mut action_count = 0_u64;
    let mut event_count = 0_u64;
    let mut probe_count = 0_u64;
    for (function, actions) in by_function.into_iter().enumerate() {
        let (replayed, actions_used, events_used, probes_used) =
            replay_function(function, first.plan().functions[function].machine, actions)?;
        action_count = checked_add(action_count, actions_used)?;
        event_count = checked_add(event_count, events_used)?;
        probe_count = checked_add(probe_count, probes_used)?;
        functions.push(replayed);
    }
    let function_count =
        u64::try_from(functions.len()).map_err(|_| GeneralizedSpillInsertionError::WorkOverflow)?;
    let usage = OptimizationWorkUsage {
        rule_evaluations: function_count,
        candidates: action_count,
        validation_steps: checked_add(event_count, probe_count)?,
        commits: action_count,
        iterations: probe_count,
    };
    if !usage.within(budget) {
        return Err(GeneralizedSpillInsertionError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    Ok(GeneralizedSpillInsertionPlan {
        abstract_spill_insertion: first.receipt().identity(),
        spill_recovery_actions: second.receipt().identity(),
        register_environment: second.plan().register_environment,
        allocator_availability: second.plan().allocator_availability,
        optimization_unit: second.plan().optimization_unit,
        fuel_schedule: second.plan().fuel_schedule,
        policy,
        budget,
        usage,
        functions,
    })
}

fn replay_roots(
    first: &ValidatedAbstractSpillInsertion,
    second: &ValidatedSpillRecoveryActions,
) -> Result<(), GeneralizedSpillInsertionError> {
    let first_receipt = first.receipt();
    if second.plan().abstract_spill_insertion != first_receipt.identity()
        || second.plan().register_environment != first_receipt.register_environment()
        || second.plan().allocator_availability != first_receipt.allocator_availability()
        || second.plan().optimization_unit != first_receipt.optimization_unit()
        || second.plan().fuel_schedule != first_receipt.fuel_schedule()
    {
        return Err(GeneralizedSpillInsertionError::RootMismatch);
    }
    Ok(())
}

fn replay_first(
    function: usize,
    id: GeneralizedSpillActionId,
    action: &crate::AbstractSpillInsertionAction,
) -> Result<ReplayAction, GeneralizedSpillInsertionError> {
    let mut rewrites = action.rewrites.iter().collect::<Vec<_>>();
    rewrites.sort();
    let Some(first_rewrite) = rewrites.first().copied() else {
        return Err(GeneralizedSpillInsertionError::InvalidEpochZeroAction { function });
    };
    let valid = action.slot.class == LogicalSpillStorageClass::NonAddressUnsignedU64V1
        && action.slot.size_bytes == SLOT_BYTES
        && action.slot.alignment_bytes == SLOT_BYTES
        && action.store.slot == action.slot.storage
        && action.reload.slot == action.slot.storage
        && action.reload.before_instruction == first_rewrite.instruction
        && rewrites.iter().all(|rewrite| {
            rewrite.block == first_rewrite.block && rewrite.result == action.reload.result
        })
        && rewrites.windows(2).all(|pair| pair[0] < pair[1]);
    if !valid {
        return Err(
            if action.slot.class != LogicalSpillStorageClass::NonAddressUnsignedU64V1 {
                GeneralizedSpillInsertionError::UnsupportedStorageClass {
                    function,
                    action: id,
                }
            } else {
                GeneralizedSpillInsertionError::InvalidEpochZeroAction { function }
            },
        );
    }
    if action.pressure_point > first_rewrite.point {
        return Err(GeneralizedSpillInsertionError::InvalidLifetime {
            function,
            action: id,
        });
    }
    Ok(ReplayAction {
        id,
        source: GeneralizedSpillActionSource::EpochZero {
            storage: action.slot.storage,
            reload: action.reload.result,
        },
        class: action.slot.class,
        block: first_rewrite.block,
        from: action.pressure_point,
        through: first_rewrite.point,
        store_instruction: action.store.before_instruction,
        before_reload: None,
        store_source: action.store.source,
        source_view: action.store.source_view,
        reload_instruction: action.reload.before_instruction,
        destination_class: action.reload.destination_class,
        rewrites: rewrites
            .into_iter()
            .map(|rewrite| ReplayRewrite {
                block: rewrite.block,
                point: rewrite.point,
                instruction: rewrite.instruction,
                operand: rewrite.operand,
            })
            .collect(),
    })
}

fn replay_second(
    function: usize,
    dependency: GeneralizedSpillActionId,
    action: &crate::SpillRecoveryLogicalAction,
) -> Result<ReplayAction, GeneralizedSpillInsertionError> {
    let id = GeneralizedSpillActionId {
        epoch: action.source_work_item.epoch,
        ordinal: action.source_work_item.ordinal,
    };
    let mut rewrites = action.rewrites.iter().collect::<Vec<_>>();
    rewrites.sort();
    let Some(first_rewrite) = rewrites.first().copied() else {
        return Err(GeneralizedSpillInsertionError::InvalidEpochOneAction {
            function,
            action: id,
        });
    };
    let namespace = (id.epoch, id.ordinal);
    let valid = namespace == (1, action.source_work_item.ordinal)
        && namespace == (action.storage.id.epoch, action.storage.id.ordinal)
        && namespace == (action.reload.result.epoch, action.reload.result.ordinal)
        && action.store.storage == action.storage.id
        && action.reload.storage == action.storage.id
        && action.store.before_source_reload == action.source_reload
        && action.store.source == action.victim
        && action.reload.before_instruction == first_rewrite.instruction
        && rewrites
            .iter()
            .all(|rewrite| rewrite.block == action.block && rewrite.result == action.reload.result)
        && rewrites.windows(2).all(|pair| pair[0] < pair[1]);
    if !valid {
        return Err(GeneralizedSpillInsertionError::InvalidEpochOneAction {
            function,
            action: id,
        });
    }
    if action.storage.class != LogicalSpillStorageClass::NonAddressUnsignedU64V1 {
        return Err(GeneralizedSpillInsertionError::UnsupportedStorageClass {
            function,
            action: id,
        });
    }
    if action.pressure_point > first_rewrite.point {
        return Err(GeneralizedSpillInsertionError::InvalidLifetime {
            function,
            action: id,
        });
    }
    Ok(ReplayAction {
        id,
        source: GeneralizedSpillActionSource::EpochOne {
            work_item: action.source_work_item,
            storage: action.storage.id,
            source_reload: action.source_reload,
            reload: action.reload.result,
        },
        class: action.storage.class,
        block: action.block,
        from: action.pressure_point,
        through: first_rewrite.point,
        store_instruction: action.store.before_instruction,
        before_reload: Some(dependency),
        store_source: action.victim,
        source_view: action.current_view,
        reload_instruction: action.reload.before_instruction,
        destination_class: action.reload.destination_class,
        rewrites: rewrites
            .into_iter()
            .map(|rewrite| ReplayRewrite {
                block: rewrite.block,
                point: rewrite.point,
                instruction: rewrite.instruction,
                operand: rewrite.operand,
            })
            .collect(),
    })
}

fn replay_function(
    function: usize,
    machine: semantic_vocabulary::MachineId,
    mut actions: Vec<ReplayAction>,
) -> Result<(FunctionGeneralizedSpillInsertion, u64, u64, u64), GeneralizedSpillInsertionError> {
    actions.sort_by_key(|action| (action.block.0, action.from.0, action.through.0, action.id));
    let mut ids = BTreeSet::new();
    for action in &actions {
        if !ids.insert(action.id) {
            return Err(GeneralizedSpillInsertionError::DuplicateAction {
                function,
                action: action.id,
            });
        }
    }
    let mut slots = Vec::with_capacity(actions.len());
    let mut probes = 0_u64;
    for action in &actions {
        let occupied = slots
            .iter()
            .filter(|slot: &&GeneralizedSpillSlot| {
                slot.block == action.block
                    && slot.live_from <= action.through
                    && action.from <= slot.live_through
            })
            .map(|slot| slot.spill_area_offset)
            .collect::<BTreeSet<_>>();
        let mut offset = 0_u64;
        loop {
            probes = checked_add(probes, 1)?;
            if !occupied.contains(&offset) {
                break;
            }
            offset = offset
                .checked_add(SLOT_BYTES)
                .ok_or(GeneralizedSpillInsertionError::OffsetOverflow { function })?;
        }
        slots.push(GeneralizedSpillSlot {
            action: action.id,
            source: action.source,
            class: action.class,
            block: action.block,
            live_from: action.from,
            live_through: action.through,
            size_bytes: SLOT_BYTES,
            alignment_bytes: SLOT_BYTES,
            spill_area_offset: offset,
        });
    }
    let spill_area_bytes = slots.iter().try_fold(0_u64, |size, slot| {
        slot.spill_area_offset
            .checked_add(SLOT_BYTES)
            .map(|end| size.max(end))
            .ok_or(GeneralizedSpillInsertionError::OffsetOverflow { function })
    })?;

    let mut schedule = BTreeMap::new();
    for action in &actions {
        insert_event(
            &mut schedule,
            event_key(action.from, 0, action.id, action.store_instruction, 0),
            GeneralizedSpillEvent::Store {
                action: action.id,
                point: action.from,
                before_instruction: action.store_instruction,
                before_reload: action.before_reload,
                source: action.store_source,
                source_view: action.source_view,
                slot: action.id,
            },
            function,
        )?;
        insert_event(
            &mut schedule,
            event_key(action.through, 1, action.id, action.reload_instruction, 0),
            GeneralizedSpillEvent::Reload {
                action: action.id,
                point: action.through,
                before_instruction: action.reload_instruction,
                slot: action.id,
                result: action.id,
                destination_class: action.destination_class,
            },
            function,
        )?;
        for rewrite in &action.rewrites {
            insert_event(
                &mut schedule,
                event_key(
                    rewrite.point,
                    2,
                    action.id,
                    rewrite.instruction,
                    rewrite.operand,
                ),
                GeneralizedSpillEvent::Rewrite {
                    action: action.id,
                    block: rewrite.block,
                    point: rewrite.point,
                    instruction: rewrite.instruction,
                    operand: rewrite.operand,
                    result: action.id,
                },
                function,
            )?;
        }
    }
    let schedule = schedule.into_values().collect::<Vec<_>>();
    let action_count =
        u64::try_from(slots.len()).map_err(|_| GeneralizedSpillInsertionError::WorkOverflow)?;
    let event_count =
        u64::try_from(schedule.len()).map_err(|_| GeneralizedSpillInsertionError::WorkOverflow)?;
    Ok((
        FunctionGeneralizedSpillInsertion {
            machine,
            spill_area_bytes,
            slots,
            schedule,
        },
        action_count,
        event_count,
        probes,
    ))
}

type EventKey = (u32, u8, u32, u32, u32, u16);

fn event_key(
    point: crate::LiveRangePoint,
    rank: u8,
    action: GeneralizedSpillActionId,
    instruction: selected_instructions::SelectedInstructionId,
    operand: u16,
) -> EventKey {
    (
        point.0,
        rank,
        action.epoch,
        action.ordinal,
        instruction.0,
        operand,
    )
}

fn insert_event(
    schedule: &mut BTreeMap<EventKey, GeneralizedSpillEvent>,
    key: EventKey,
    event: GeneralizedSpillEvent,
    function: usize,
) -> Result<(), GeneralizedSpillInsertionError> {
    if schedule.insert(key, event).is_some() {
        return Err(GeneralizedSpillInsertionError::NonCanonicalSchedule { function });
    }
    Ok(())
}

fn checked_add(left: u64, right: u64) -> Result<u64, GeneralizedSpillInsertionError> {
    left.checked_add(right)
        .ok_or(GeneralizedSpillInsertionError::WorkOverflow)
}
