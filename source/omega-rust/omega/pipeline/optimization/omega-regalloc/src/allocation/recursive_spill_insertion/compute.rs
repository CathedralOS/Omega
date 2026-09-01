//! Direct projection, closed-lifetime recoloring, and canonical scheduling.

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::{
    GeneralizedSpillActionId, GeneralizedSpillEvent, LogicalSpillStorageClass,
    RecursiveSpillActionSource, RecursiveSpillEvent, RecursiveSpillInsertionError,
    RecursiveSpillInsertionPlan, RecursiveSpillInsertionPolicy, RecursiveSpillSlot,
    RecursiveSpillStoredValue, ValidatedGeneralizedSpillInsertion,
    ValidatedGeneralizedSpillRecoveryActions,
};

const SLOT_BYTES: u64 = 8;

#[derive(Clone)]
struct PendingAction {
    id: GeneralizedSpillActionId,
    source: RecursiveSpillActionSource,
    class: LogicalSpillStorageClass,
    block: omega_selected_instructions::SelectedBlockId,
    from: crate::LiveRangePoint,
    through: crate::LiveRangePoint,
    store_instruction: omega_selected_instructions::SelectedInstructionId,
    before_reload: Option<GeneralizedSpillActionId>,
    stored_value: RecursiveSpillStoredValue,
    source_view: omega_register_model::RegisterViewId,
    reload_instruction: omega_selected_instructions::SelectedInstructionId,
    destination_class: omega_register_model::RegisterClassId,
    rewrites: Vec<PendingRewrite>,
}

#[derive(Clone, Copy)]
struct PendingRewrite {
    block: omega_selected_instructions::SelectedBlockId,
    point: crate::LiveRangePoint,
    instruction: omega_selected_instructions::SelectedInstructionId,
    operand: u16,
}

pub(super) fn compute(
    base: &ValidatedGeneralizedSpillInsertion,
    recovery: &ValidatedGeneralizedSpillRecoveryActions,
    policy: RecursiveSpillInsertionPolicy,
    budget: OptimizationWorkBudget,
) -> Result<RecursiveSpillInsertionPlan, RecursiveSpillInsertionError> {
    admit_roots(base, recovery)?;
    admit_policy(policy)?;
    let mut pending = vec![Vec::new(); base.plan().functions.len()];
    for (function, source) in base.plan().functions.iter().enumerate() {
        for slot in &source.slots {
            pending[function].push(project_base_action(function, source, slot)?);
        }
    }
    for action in &recovery.plan().actions {
        let source = base.plan().functions.get(action.function).ok_or(
            RecursiveSpillInsertionError::FunctionMismatch {
                function: action.function,
            },
        )?;
        if source.machine != action.machine {
            return Err(RecursiveSpillInsertionError::FunctionMismatch {
                function: action.function,
            });
        }
        pending[action.function].push(project_recovery_action(action, policy)?);
    }
    let functions = pending
        .into_iter()
        .zip(&base.plan().functions)
        .enumerate()
        .map(|(index, (actions, source))| build_function(index, source.machine, actions))
        .collect::<Result<Vec<_>, _>>()?;
    let usage = work_usage(&functions)?;
    if !usage.within(budget) {
        return Err(RecursiveSpillInsertionError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let receipt = recovery.receipt();
    Ok(RecursiveSpillInsertionPlan {
        generalized_spill_insertion: base.receipt().identity(),
        recovery_actions: receipt.identity(),
        register_environment: recovery.plan().register_environment,
        allocator_availability: recovery.plan().allocator_availability,
        optimization_unit: receipt.optimization_unit(),
        fuel_schedule: receipt.fuel_schedule(),
        policy,
        budget,
        usage,
        functions,
    })
}

pub(super) fn admit_policy(
    policy: RecursiveSpillInsertionPolicy,
) -> Result<(), RecursiveSpillInsertionError> {
    if !matches!(
        policy,
        RecursiveSpillInsertionPolicy::EpochTwoReloadVictimBlockLocalUnsignedU64ClosedIntervalFirstFitV1
            | RecursiveSpillInsertionPolicy::EpochTwoOriginalVictimBlockLocalUnsignedU64ClosedIntervalFirstFitV2
    ) {
        return Err(RecursiveSpillInsertionError::UnsupportedPolicy);
    }
    Ok(())
}

pub(super) fn admit_roots(
    base: &ValidatedGeneralizedSpillInsertion,
    recovery: &ValidatedGeneralizedSpillRecoveryActions,
) -> Result<(), RecursiveSpillInsertionError> {
    let first = base.receipt();
    let second = recovery.plan();
    if second.generalized_spill_insertion != first.identity()
        || second.register_environment != first.register_environment()
        || second.allocator_availability != first.allocator_availability()
        || second.optimization_unit != first.optimization_unit()
        || second.fuel_schedule != first.fuel_schedule()
    {
        return Err(RecursiveSpillInsertionError::RootMismatch);
    }
    Ok(())
}

fn project_base_action(
    function: usize,
    source: &crate::FunctionGeneralizedSpillInsertion,
    slot: &crate::GeneralizedSpillSlot,
) -> Result<PendingAction, RecursiveSpillInsertionError> {
    if slot.class != LogicalSpillStorageClass::NonAddressUnsignedU64V1
        || slot.size_bytes != SLOT_BYTES
        || slot.alignment_bytes != SLOT_BYTES
    {
        return Err(RecursiveSpillInsertionError::UnsupportedStorageClass {
            function,
            action: slot.action,
        });
    }
    let mut store = None;
    let mut reload = None;
    let mut rewrites = Vec::new();
    for event in &source.schedule {
        match *event {
            GeneralizedSpillEvent::Store {
                action,
                point,
                before_instruction,
                before_reload,
                source,
                source_view,
                slot: event_slot,
            } if action == slot.action => {
                if store
                    .replace((
                        point,
                        before_instruction,
                        before_reload,
                        source,
                        source_view,
                        event_slot,
                    ))
                    .is_some()
                {
                    return Err(missing(function, slot.action));
                }
            }
            GeneralizedSpillEvent::Reload {
                action,
                point,
                before_instruction,
                slot: event_slot,
                result,
                destination_class,
            } if action == slot.action => {
                if reload
                    .replace((
                        point,
                        before_instruction,
                        event_slot,
                        result,
                        destination_class,
                    ))
                    .is_some()
                {
                    return Err(missing(function, slot.action));
                }
            }
            GeneralizedSpillEvent::Rewrite {
                action,
                block,
                point,
                instruction,
                operand,
                result,
            } if action == slot.action => {
                if result != action {
                    return Err(missing(function, slot.action));
                }
                rewrites.push(PendingRewrite {
                    block,
                    point,
                    instruction,
                    operand,
                });
            }
            _ => {}
        }
    }
    let Some((from, store_instruction, before_reload, stored, source_view, store_slot)) = store
    else {
        return Err(missing(function, slot.action));
    };
    let Some((through, reload_instruction, reload_slot, result, destination_class)) = reload else {
        return Err(missing(function, slot.action));
    };
    if from != slot.live_from
        || through != slot.live_through
        || store_slot != slot.action
        || reload_slot != slot.action
        || result != slot.action
        || rewrites.is_empty()
        || rewrites.iter().any(|row| row.block != slot.block)
    {
        return Err(missing(function, slot.action));
    }
    Ok(PendingAction {
        id: slot.action,
        source: RecursiveSpillActionSource::Prior(slot.source),
        class: slot.class,
        block: slot.block,
        from,
        through,
        store_instruction,
        before_reload,
        stored_value: RecursiveSpillStoredValue::Original(stored),
        source_view,
        reload_instruction,
        destination_class,
        rewrites,
    })
}

fn project_recovery_action(
    action: &crate::GeneralizedSpillRecoveryLogicalAction,
    policy: RecursiveSpillInsertionPolicy,
) -> Result<PendingAction, RecursiveSpillInsertionError> {
    let function = action.function;
    let id = GeneralizedSpillActionId {
        epoch: action.source_work_item.epoch,
        ordinal: action.source_work_item.ordinal,
    };
    let Some(first) = action.rewrites.first() else {
        return Err(invalid(function, id));
    };
    let (source, stored_value) = match (policy, action.victim, action.store.source) {
        (
            RecursiveSpillInsertionPolicy::EpochTwoReloadVictimBlockLocalUnsignedU64ClosedIntervalFirstFitV1,
            crate::GeneralizedSpillRecoveryVictim::Reload(victim),
            crate::GeneralizedSpillRecoveryVictim::Reload(stored),
        ) if victim == stored => (
            RecursiveSpillActionSource::EpochTwo {
                work_item: action.source_work_item,
                source_pressure: action.source_pressure,
                victim,
            },
            RecursiveSpillStoredValue::Reload(victim),
        ),
        (
            RecursiveSpillInsertionPolicy::EpochTwoOriginalVictimBlockLocalUnsignedU64ClosedIntervalFirstFitV2,
            crate::GeneralizedSpillRecoveryVictim::Original(victim),
            crate::GeneralizedSpillRecoveryVictim::Original(stored),
        ) if victim == stored => (
            RecursiveSpillActionSource::EpochTwoOriginal {
                work_item: action.source_work_item,
                source_pressure: action.source_pressure,
                victim,
            },
            RecursiveSpillStoredValue::Original(victim),
        ),
        (_, victim, _) => {
            return Err(RecursiveSpillInsertionError::UnsupportedRecoveryVictim {
                function,
                action: id,
                victim,
            });
        }
    };
    if id.epoch != 2
        || action.storage.id != id
        || action.store.storage != id
        || action.reload.storage != id
        || action.reload.result != id
        || action.store.before_pressure_reload != action.source_pressure
        || action.reload.before_instruction != first.instruction
        || action.current_view != action.reclaimed_view
        || action
            .rewrites
            .iter()
            .any(|row| row.block != action.block || row.result != id)
        || action.rewrites.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid(function, id));
    }
    if action.storage.class != LogicalSpillStorageClass::NonAddressUnsignedU64V1 {
        return Err(RecursiveSpillInsertionError::UnsupportedStorageClass {
            function,
            action: id,
        });
    }
    if action.pressure_point > first.point {
        return Err(RecursiveSpillInsertionError::InvalidLifetime {
            function,
            action: id,
        });
    }
    Ok(PendingAction {
        id,
        source,
        class: action.storage.class,
        block: action.block,
        from: action.pressure_point,
        through: first.point,
        store_instruction: action.store.before_instruction,
        before_reload: Some(action.store.before_pressure_reload),
        stored_value,
        source_view: action.store.source_view,
        reload_instruction: action.reload.before_instruction,
        destination_class: action.reload.destination_class,
        rewrites: action
            .rewrites
            .iter()
            .map(|row| PendingRewrite {
                block: row.block,
                point: row.point,
                instruction: row.instruction,
                operand: row.operand,
            })
            .collect(),
    })
}

fn build_function(
    function: usize,
    machine: psi_core::MachineId,
    mut actions: Vec<PendingAction>,
) -> Result<crate::FunctionRecursiveSpillInsertion, RecursiveSpillInsertionError> {
    actions.sort_by_key(|row| (row.block.0, row.from.0, row.through.0, row.id));
    let mut slots = Vec::with_capacity(actions.len());
    for action in &actions {
        if slots
            .iter()
            .any(|slot: &RecursiveSpillSlot| slot.action == action.id)
        {
            return Err(RecursiveSpillInsertionError::DuplicateAction {
                function,
                action: action.id,
            });
        }
        let mut offset = 0_u64;
        while slots.iter().any(|slot: &RecursiveSpillSlot| {
            slot.spill_area_offset == offset
                && slot.block == action.block
                && slot.live_from <= action.through
                && action.from <= slot.live_through
        }) {
            offset = offset
                .checked_add(SLOT_BYTES)
                .ok_or(RecursiveSpillInsertionError::OffsetOverflow { function })?;
        }
        slots.push(RecursiveSpillSlot {
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
            .ok_or(RecursiveSpillInsertionError::OffsetOverflow { function })
    })?;
    let mut schedule = Vec::new();
    for action in actions {
        schedule.push(RecursiveSpillEvent::Store {
            action: action.id,
            point: action.from,
            before_instruction: action.store_instruction,
            before_reload: action.before_reload,
            source: action.stored_value,
            source_view: action.source_view,
            slot: action.id,
        });
        schedule.push(RecursiveSpillEvent::Reload {
            action: action.id,
            point: action.through,
            before_instruction: action.reload_instruction,
            slot: action.id,
            result: action.id,
            destination_class: action.destination_class,
        });
        schedule.extend(
            action
                .rewrites
                .into_iter()
                .map(|row| RecursiveSpillEvent::Rewrite {
                    action: action.id,
                    block: row.block,
                    point: row.point,
                    instruction: row.instruction,
                    operand: row.operand,
                    result: action.id,
                }),
        );
    }
    schedule.sort_by_key(event_key);
    Ok(crate::FunctionRecursiveSpillInsertion {
        machine,
        spill_area_bytes,
        slots,
        schedule,
    })
}

pub(super) fn event_key(event: &RecursiveSpillEvent) -> (u32, u8, u32, u32, u32, u16) {
    match *event {
        RecursiveSpillEvent::Store {
            action,
            point,
            before_instruction,
            ..
        } => (
            point.0,
            0,
            action.epoch,
            action.ordinal,
            before_instruction.0,
            0,
        ),
        RecursiveSpillEvent::Reload {
            action,
            point,
            before_instruction,
            ..
        } => (
            point.0,
            1,
            action.epoch,
            action.ordinal,
            before_instruction.0,
            0,
        ),
        RecursiveSpillEvent::Rewrite {
            action,
            point,
            instruction,
            operand,
            ..
        } => (
            point.0,
            2,
            action.epoch,
            action.ordinal,
            instruction.0,
            operand,
        ),
    }
}

pub(super) fn work_usage(
    functions: &[crate::FunctionRecursiveSpillInsertion],
) -> Result<OptimizationWorkUsage, RecursiveSpillInsertionError> {
    let mut actions = 0_u64;
    let mut events = 0_u64;
    let mut probes = 0_u64;
    for function in functions {
        actions = actions
            .checked_add(to_u64(function.slots.len())?)
            .ok_or(RecursiveSpillInsertionError::WorkOverflow)?;
        events = events
            .checked_add(to_u64(function.schedule.len())?)
            .ok_or(RecursiveSpillInsertionError::WorkOverflow)?;
        for slot in &function.slots {
            probes = probes
                .checked_add(slot.spill_area_offset / SLOT_BYTES + 1)
                .ok_or(RecursiveSpillInsertionError::WorkOverflow)?;
        }
    }
    Ok(OptimizationWorkUsage {
        rule_evaluations: to_u64(functions.len())?,
        candidates: actions,
        validation_steps: events
            .checked_add(probes)
            .ok_or(RecursiveSpillInsertionError::WorkOverflow)?,
        commits: actions,
        iterations: probes,
    })
}

fn to_u64(value: usize) -> Result<u64, RecursiveSpillInsertionError> {
    u64::try_from(value).map_err(|_| RecursiveSpillInsertionError::WorkOverflow)
}

fn missing(function: usize, action: GeneralizedSpillActionId) -> RecursiveSpillInsertionError {
    RecursiveSpillInsertionError::MissingBaseAction { function, action }
}

fn invalid(function: usize, action: GeneralizedSpillActionId) -> RecursiveSpillInsertionError {
    RecursiveSpillInsertionError::InvalidRecoveryAction { function, action }
}
