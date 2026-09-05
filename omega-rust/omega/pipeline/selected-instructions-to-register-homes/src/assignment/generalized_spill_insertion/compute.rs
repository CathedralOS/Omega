//! Canonical direct traversal and lowest-offset first-fit proposal.

use std::collections::BTreeMap;

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::{
    GeneralizedSpillActionId, GeneralizedSpillActionSource, GeneralizedSpillEvent,
    GeneralizedSpillInsertionError, GeneralizedSpillInsertionPlan, GeneralizedSpillInsertionPolicy,
    GeneralizedSpillSlot, LogicalSpillStorageClass, SpillRecoveryLogicalAction,
    ValidatedAbstractSpillInsertion, ValidatedSpillRecoveryActions,
};

const SLOT_BYTES: u64 = 8;

#[derive(Clone)]
pub(super) struct PendingAction {
    pub id: GeneralizedSpillActionId,
    pub source: GeneralizedSpillActionSource,
    pub class: LogicalSpillStorageClass,
    pub block: selected_instructions::SelectedBlockId,
    pub live_from: crate::LiveRangePoint,
    pub live_through: crate::LiveRangePoint,
    pub store_instruction: selected_instructions::SelectedInstructionId,
    pub before_reload: Option<GeneralizedSpillActionId>,
    pub store_source: selected_instructions::VirtualRegisterId,
    pub source_view: register_model::RegisterViewId,
    pub reload_instruction: selected_instructions::SelectedInstructionId,
    pub destination_class: register_model::RegisterClassId,
    pub rewrites: Vec<PendingRewrite>,
}

#[derive(Clone, Copy)]
pub(super) struct PendingRewrite {
    pub block: selected_instructions::SelectedBlockId,
    pub point: crate::LiveRangePoint,
    pub instruction: selected_instructions::SelectedInstructionId,
    pub operand: u16,
}

pub(super) fn compute(
    first: &ValidatedAbstractSpillInsertion,
    second: &ValidatedSpillRecoveryActions,
    policy: GeneralizedSpillInsertionPolicy,
    budget: OptimizationWorkBudget,
) -> Result<GeneralizedSpillInsertionPlan, GeneralizedSpillInsertionError> {
    admit_roots(first, second)?;
    admit_policy(policy)?;

    let mut by_function = vec![Vec::new(); first.plan().functions.len()];
    let mut reloads = BTreeMap::new();
    let mut next_ordinal = 0_u32;
    for (function, source) in first.plan().functions.iter().enumerate() {
        if let Some(action) = &source.action {
            let id = GeneralizedSpillActionId {
                epoch: 0,
                ordinal: next_ordinal,
            };
            next_ordinal = next_ordinal
                .checked_add(1)
                .ok_or(GeneralizedSpillInsertionError::NamespaceOverflow)?;
            let pending = first_action(function, id, action)?;
            if reloads
                .insert((function, action.reload.result), id)
                .is_some()
            {
                return Err(GeneralizedSpillInsertionError::InvalidEpochZeroAction { function });
            }
            by_function[function].push(pending);
        }
    }

    for action in &second.plan().actions {
        let function = action.function;
        let Some(first_function) = first.plan().functions.get(function) else {
            return Err(GeneralizedSpillInsertionError::FunctionMismatch { function });
        };
        if first_function.machine != action.machine {
            return Err(GeneralizedSpillInsertionError::FunctionMismatch { function });
        }
        let Some(&before_reload) = reloads.get(&(function, action.source_reload)) else {
            return Err(if first_function.action.is_none() {
                GeneralizedSpillInsertionError::MissingEpochZeroAction { function }
            } else {
                GeneralizedSpillInsertionError::MissingSourceReload {
                    function,
                    reload: action.source_reload,
                }
            });
        };
        by_function[function].push(second_action(function, before_reload, action)?);
    }

    let functions = by_function
        .into_iter()
        .zip(&first.plan().functions)
        .enumerate()
        .map(|(function, (actions, source))| build_function(function, source.machine, actions))
        .collect::<Result<Vec<_>, _>>()?;
    let usage = work_usage(&functions)?;
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

pub(super) fn admit_policy(
    policy: GeneralizedSpillInsertionPolicy,
) -> Result<(), GeneralizedSpillInsertionError> {
    if policy
        != GeneralizedSpillInsertionPolicy::EpochZeroAndOneBlockLocalUnsignedU64ClosedIntervalFirstFitV1
    {
        return Err(GeneralizedSpillInsertionError::UnsupportedPolicy);
    }
    Ok(())
}

pub(super) fn admit_roots(
    first: &ValidatedAbstractSpillInsertion,
    second: &ValidatedSpillRecoveryActions,
) -> Result<(), GeneralizedSpillInsertionError> {
    let first_receipt = first.receipt();
    let second_plan = second.plan();
    if second_plan.abstract_spill_insertion != first_receipt.identity()
        || second_plan.register_environment != first_receipt.register_environment()
        || second_plan.allocator_availability != first_receipt.allocator_availability()
        || second_plan.optimization_unit != first_receipt.optimization_unit()
        || second_plan.fuel_schedule != first_receipt.fuel_schedule()
    {
        return Err(GeneralizedSpillInsertionError::RootMismatch);
    }
    Ok(())
}

fn first_action(
    function: usize,
    id: GeneralizedSpillActionId,
    action: &crate::AbstractSpillInsertionAction,
) -> Result<PendingAction, GeneralizedSpillInsertionError> {
    let Some(first_rewrite) = action.rewrites.first() else {
        return Err(GeneralizedSpillInsertionError::InvalidEpochZeroAction { function });
    };
    if action.slot.class != LogicalSpillStorageClass::NonAddressUnsignedU64V1 {
        return Err(GeneralizedSpillInsertionError::UnsupportedStorageClass {
            function,
            action: id,
        });
    }
    if action.slot.size_bytes != SLOT_BYTES
        || action.slot.alignment_bytes != SLOT_BYTES
        || action.store.slot != action.slot.storage
        || action.reload.slot != action.slot.storage
        || action.reload.before_instruction != first_rewrite.instruction
        || action.rewrites.iter().any(|rewrite| {
            rewrite.block != first_rewrite.block || rewrite.result != action.reload.result
        })
        || action.rewrites.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(GeneralizedSpillInsertionError::InvalidEpochZeroAction { function });
    }
    if action.pressure_point > first_rewrite.point {
        return Err(GeneralizedSpillInsertionError::InvalidLifetime {
            function,
            action: id,
        });
    }
    Ok(PendingAction {
        id,
        source: GeneralizedSpillActionSource::EpochZero {
            storage: action.slot.storage,
            reload: action.reload.result,
        },
        class: action.slot.class,
        block: first_rewrite.block,
        live_from: action.pressure_point,
        live_through: first_rewrite.point,
        store_instruction: action.store.before_instruction,
        before_reload: None,
        store_source: action.store.source,
        source_view: action.store.source_view,
        reload_instruction: action.reload.before_instruction,
        destination_class: action.reload.destination_class,
        rewrites: action
            .rewrites
            .iter()
            .map(|rewrite| PendingRewrite {
                block: rewrite.block,
                point: rewrite.point,
                instruction: rewrite.instruction,
                operand: rewrite.operand,
            })
            .collect(),
    })
}

fn second_action(
    function: usize,
    before_reload: GeneralizedSpillActionId,
    action: &SpillRecoveryLogicalAction,
) -> Result<PendingAction, GeneralizedSpillInsertionError> {
    let id = GeneralizedSpillActionId {
        epoch: action.source_work_item.epoch,
        ordinal: action.source_work_item.ordinal,
    };
    let Some(first_rewrite) = action.rewrites.first() else {
        return Err(GeneralizedSpillInsertionError::InvalidEpochOneAction {
            function,
            action: id,
        });
    };
    if id.epoch != 1
        || action.storage.id.epoch != id.epoch
        || action.storage.id.ordinal != id.ordinal
        || action.reload.result.epoch != id.epoch
        || action.reload.result.ordinal != id.ordinal
        || action.store.storage != action.storage.id
        || action.reload.storage != action.storage.id
        || action.store.before_source_reload != action.source_reload
        || action.store.source != action.victim
        || action.reload.before_instruction != first_rewrite.instruction
        || action
            .rewrites
            .iter()
            .any(|rewrite| rewrite.block != action.block || rewrite.result != action.reload.result)
        || action.rewrites.windows(2).any(|pair| pair[0] >= pair[1])
    {
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
    Ok(PendingAction {
        id,
        source: GeneralizedSpillActionSource::EpochOne {
            work_item: action.source_work_item,
            storage: action.storage.id,
            source_reload: action.source_reload,
            reload: action.reload.result,
        },
        class: action.storage.class,
        block: action.block,
        live_from: action.pressure_point,
        live_through: first_rewrite.point,
        store_instruction: action.store.before_instruction,
        before_reload: Some(before_reload),
        store_source: action.store.source,
        source_view: action.current_view,
        reload_instruction: action.reload.before_instruction,
        destination_class: action.reload.destination_class,
        rewrites: action
            .rewrites
            .iter()
            .map(|rewrite| PendingRewrite {
                block: rewrite.block,
                point: rewrite.point,
                instruction: rewrite.instruction,
                operand: rewrite.operand,
            })
            .collect(),
    })
}

fn build_function(
    function: usize,
    machine: semantic_vocabulary::MachineId,
    mut actions: Vec<PendingAction>,
) -> Result<crate::FunctionGeneralizedSpillInsertion, GeneralizedSpillInsertionError> {
    actions.sort_by_key(|action| {
        (
            action.block.0,
            action.live_from.0,
            action.live_through.0,
            action.id,
        )
    });
    if let Some(pair) = actions.windows(2).find(|pair| pair[0].id == pair[1].id) {
        return Err(GeneralizedSpillInsertionError::DuplicateAction {
            function,
            action: pair[0].id,
        });
    }
    let mut slots = Vec::with_capacity(actions.len());
    for action in &actions {
        let mut offset = 0_u64;
        loop {
            let conflict = slots.iter().any(|assigned: &GeneralizedSpillSlot| {
                assigned.spill_area_offset == offset
                    && assigned.block == action.block
                    && assigned.live_from <= action.live_through
                    && action.live_from <= assigned.live_through
            });
            if !conflict {
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
            live_from: action.live_from,
            live_through: action.live_through,
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
    let mut schedule = Vec::new();
    for action in &actions {
        schedule.push(GeneralizedSpillEvent::Store {
            action: action.id,
            point: action.live_from,
            before_instruction: action.store_instruction,
            before_reload: action.before_reload,
            source: action.store_source,
            source_view: action.source_view,
            slot: action.id,
        });
        schedule.push(GeneralizedSpillEvent::Reload {
            action: action.id,
            point: action.live_through,
            before_instruction: action.reload_instruction,
            slot: action.id,
            result: action.id,
            destination_class: action.destination_class,
        });
        schedule.extend(
            action
                .rewrites
                .iter()
                .map(|rewrite| GeneralizedSpillEvent::Rewrite {
                    action: action.id,
                    block: rewrite.block,
                    point: rewrite.point,
                    instruction: rewrite.instruction,
                    operand: rewrite.operand,
                    result: action.id,
                }),
        );
    }
    schedule.sort_by_key(event_key);
    Ok(crate::FunctionGeneralizedSpillInsertion {
        machine,
        spill_area_bytes,
        slots,
        schedule,
    })
}

fn event_key(event: &GeneralizedSpillEvent) -> (u32, u8, u32, u32, u32, u16) {
    match *event {
        GeneralizedSpillEvent::Store {
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
        GeneralizedSpillEvent::Reload {
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
        GeneralizedSpillEvent::Rewrite {
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
    functions: &[crate::FunctionGeneralizedSpillInsertion],
) -> Result<OptimizationWorkUsage, GeneralizedSpillInsertionError> {
    let function_count = to_u64(functions.len())?;
    let mut actions = 0_u64;
    let mut events = 0_u64;
    let mut probes = 0_u64;
    for function in functions {
        actions = actions
            .checked_add(to_u64(function.slots.len())?)
            .ok_or(GeneralizedSpillInsertionError::WorkOverflow)?;
        events = events
            .checked_add(to_u64(function.schedule.len())?)
            .ok_or(GeneralizedSpillInsertionError::WorkOverflow)?;
        for slot in &function.slots {
            let slot_probes = slot
                .spill_area_offset
                .checked_div(SLOT_BYTES)
                .and_then(|value| value.checked_add(1))
                .ok_or(GeneralizedSpillInsertionError::WorkOverflow)?;
            probes = probes
                .checked_add(slot_probes)
                .ok_or(GeneralizedSpillInsertionError::WorkOverflow)?;
        }
    }
    Ok(OptimizationWorkUsage {
        rule_evaluations: function_count,
        candidates: actions,
        validation_steps: events
            .checked_add(probes)
            .ok_or(GeneralizedSpillInsertionError::WorkOverflow)?,
        commits: actions,
        iterations: probes,
    })
}

fn to_u64(value: usize) -> Result<u64, GeneralizedSpillInsertionError> {
    u64::try_from(value).map_err(|_| GeneralizedSpillInsertionError::WorkOverflow)
}
