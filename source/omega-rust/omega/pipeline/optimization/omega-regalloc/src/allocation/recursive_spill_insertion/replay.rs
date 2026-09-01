//! Independent keyed replay of the complete recursive logical schedule.

use std::collections::{BTreeMap, BTreeSet};

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
struct ReplayRow {
    slot: RecursiveSpillSlot,
    store_instruction: omega_selected_instructions::SelectedInstructionId,
    before_reload: Option<GeneralizedSpillActionId>,
    stored_value: RecursiveSpillStoredValue,
    source_view: omega_register_model::RegisterViewId,
    reload_instruction: omega_selected_instructions::SelectedInstructionId,
    destination_class: omega_register_model::RegisterClassId,
    rewrites: Vec<ReplayRewrite>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ReplayRewrite {
    block: omega_selected_instructions::SelectedBlockId,
    point: crate::LiveRangePoint,
    instruction: omega_selected_instructions::SelectedInstructionId,
    operand: u16,
}

pub(super) fn replay(
    base: &ValidatedGeneralizedSpillInsertion,
    recovery: &ValidatedGeneralizedSpillRecoveryActions,
    policy: RecursiveSpillInsertionPolicy,
    budget: OptimizationWorkBudget,
) -> Result<RecursiveSpillInsertionPlan, RecursiveSpillInsertionError> {
    admit_roots(base, recovery)?;
    admit_policy(policy)?;

    let mut rows = BTreeMap::<(usize, GeneralizedSpillActionId), ReplayRow>::new();
    for (function, source) in base.plan().functions.iter().enumerate() {
        let mut stores = BTreeMap::new();
        let mut reloads = BTreeMap::new();
        let mut rewrites = BTreeMap::<GeneralizedSpillActionId, BTreeSet<ReplayRewrite>>::new();
        for event in &source.schedule {
            match *event {
                GeneralizedSpillEvent::Store {
                    action,
                    point,
                    before_instruction,
                    before_reload,
                    source,
                    source_view,
                    slot,
                } => {
                    if stores
                        .insert(
                            action,
                            (
                                point,
                                before_instruction,
                                before_reload,
                                source,
                                source_view,
                                slot,
                            ),
                        )
                        .is_some()
                    {
                        return Err(missing(function, action));
                    }
                }
                GeneralizedSpillEvent::Reload {
                    action,
                    point,
                    before_instruction,
                    slot,
                    result,
                    destination_class,
                } => {
                    if reloads
                        .insert(
                            action,
                            (point, before_instruction, slot, result, destination_class),
                        )
                        .is_some()
                    {
                        return Err(missing(function, action));
                    }
                }
                GeneralizedSpillEvent::Rewrite {
                    action,
                    block,
                    point,
                    instruction,
                    operand,
                    result,
                } => {
                    if result != action
                        || !rewrites.entry(action).or_default().insert(ReplayRewrite {
                            block,
                            point,
                            instruction,
                            operand,
                        })
                    {
                        return Err(missing(function, action));
                    }
                }
            }
        }
        for slot in &source.slots {
            let Some((from, store_instruction, before_reload, stored, source_view, store_slot)) =
                stores.remove(&slot.action)
            else {
                return Err(missing(function, slot.action));
            };
            let Some((through, reload_instruction, reload_slot, result, destination_class)) =
                reloads.remove(&slot.action)
            else {
                return Err(missing(function, slot.action));
            };
            let action_rewrites = rewrites
                .remove(&slot.action)
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<_>>();
            if slot.class != LogicalSpillStorageClass::NonAddressUnsignedU64V1
                || slot.size_bytes != SLOT_BYTES
                || slot.alignment_bytes != SLOT_BYTES
                || from != slot.live_from
                || through != slot.live_through
                || store_slot != slot.action
                || reload_slot != slot.action
                || result != slot.action
                || action_rewrites.is_empty()
                || action_rewrites.iter().any(|row| row.block != slot.block)
            {
                return Err(missing(function, slot.action));
            }
            let row = ReplayRow {
                slot: RecursiveSpillSlot {
                    action: slot.action,
                    source: RecursiveSpillActionSource::Prior(slot.source),
                    class: slot.class,
                    block: slot.block,
                    live_from: slot.live_from,
                    live_through: slot.live_through,
                    size_bytes: SLOT_BYTES,
                    alignment_bytes: SLOT_BYTES,
                    spill_area_offset: 0,
                },
                store_instruction,
                before_reload,
                stored_value: RecursiveSpillStoredValue::Original(stored),
                source_view,
                reload_instruction,
                destination_class,
                rewrites: action_rewrites,
            };
            if rows.insert((function, slot.action), row).is_some() {
                return Err(RecursiveSpillInsertionError::DuplicateAction {
                    function,
                    action: slot.action,
                });
            }
        }
        if !stores.is_empty() || !reloads.is_empty() || !rewrites.is_empty() {
            return Err(RecursiveSpillInsertionError::FunctionMismatch { function });
        }
    }

    for action in &recovery.plan().actions {
        let function = action.function;
        let id = GeneralizedSpillActionId {
            epoch: action.source_work_item.epoch,
            ordinal: action.source_work_item.ordinal,
        };
        let Some(first) = action.rewrites.iter().min() else {
            return Err(invalid(function, id));
        };
        let victim = match (action.victim, action.store.source) {
            (
                crate::GeneralizedSpillRecoveryVictim::Reload(victim),
                crate::GeneralizedSpillRecoveryVictim::Reload(source),
            ) if victim == source => victim,
            (victim, _) => {
                return Err(RecursiveSpillInsertionError::UnsupportedRecoveryVictim {
                    function,
                    action: id,
                    victim,
                });
            }
        };
        let mut action_rewrites = action
            .rewrites
            .iter()
            .map(|row| ReplayRewrite {
                block: row.block,
                point: row.point,
                instruction: row.instruction,
                operand: row.operand,
            })
            .collect::<Vec<_>>();
        action_rewrites.sort();
        let machine_matches =
            base.plan().functions.get(function).map(|row| row.machine) == Some(action.machine);
        if !machine_matches
            || id.epoch != 2
            || action.storage.id != id
            || action.store.storage != id
            || action.reload.storage != id
            || action.reload.result != id
            || action.store.before_pressure_reload != action.source_pressure
            || action.reload.before_instruction != first.instruction
            || action.current_view != action.reclaimed_view
            || action_rewrites.iter().any(|row| row.block != action.block)
            || action_rewrites.windows(2).any(|pair| pair[0] >= pair[1])
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
        let row = ReplayRow {
            slot: RecursiveSpillSlot {
                action: id,
                source: RecursiveSpillActionSource::EpochTwo {
                    work_item: action.source_work_item,
                    source_pressure: action.source_pressure,
                    victim,
                },
                class: action.storage.class,
                block: action.block,
                live_from: action.pressure_point,
                live_through: first.point,
                size_bytes: SLOT_BYTES,
                alignment_bytes: SLOT_BYTES,
                spill_area_offset: 0,
            },
            store_instruction: action.store.before_instruction,
            before_reload: Some(action.store.before_pressure_reload),
            stored_value: RecursiveSpillStoredValue::Reload(victim),
            source_view: action.store.source_view,
            reload_instruction: action.reload.before_instruction,
            destination_class: action.reload.destination_class,
            rewrites: action_rewrites,
        };
        if rows.insert((function, id), row).is_some() {
            return Err(RecursiveSpillInsertionError::DuplicateAction {
                function,
                action: id,
            });
        }
    }

    let mut functions = Vec::with_capacity(base.plan().functions.len());
    for (function, source) in base.plan().functions.iter().enumerate() {
        let mut function_rows = rows
            .iter()
            .filter(|((index, _), _)| *index == function)
            .map(|(_, row)| row.clone())
            .collect::<Vec<_>>();
        function_rows.sort_by_key(|row| {
            (
                row.slot.block.0,
                row.slot.live_from.0,
                row.slot.live_through.0,
                row.slot.action,
            )
        });
        let mut assigned = Vec::<RecursiveSpillSlot>::new();
        let mut schedule = Vec::new();
        for mut row in function_rows {
            let mut offset = 0_u64;
            while assigned.iter().any(|prior| {
                prior.spill_area_offset == offset
                    && prior.block == row.slot.block
                    && prior.live_from <= row.slot.live_through
                    && row.slot.live_from <= prior.live_through
            }) {
                offset = offset
                    .checked_add(SLOT_BYTES)
                    .ok_or(RecursiveSpillInsertionError::OffsetOverflow { function })?;
            }
            row.slot.spill_area_offset = offset;
            assigned.push(row.slot);
            schedule.push(RecursiveSpillEvent::Store {
                action: row.slot.action,
                point: row.slot.live_from,
                before_instruction: row.store_instruction,
                before_reload: row.before_reload,
                source: row.stored_value,
                source_view: row.source_view,
                slot: row.slot.action,
            });
            schedule.push(RecursiveSpillEvent::Reload {
                action: row.slot.action,
                point: row.slot.live_through,
                before_instruction: row.reload_instruction,
                slot: row.slot.action,
                result: row.slot.action,
                destination_class: row.destination_class,
            });
            schedule.extend(
                row.rewrites
                    .into_iter()
                    .map(|rewrite| RecursiveSpillEvent::Rewrite {
                        action: row.slot.action,
                        block: rewrite.block,
                        point: rewrite.point,
                        instruction: rewrite.instruction,
                        operand: rewrite.operand,
                        result: row.slot.action,
                    }),
            );
        }
        schedule.sort_by_key(event_key);
        let spill_area_bytes = assigned.iter().try_fold(0_u64, |size, slot| {
            slot.spill_area_offset
                .checked_add(SLOT_BYTES)
                .map(|end| size.max(end))
                .ok_or(RecursiveSpillInsertionError::OffsetOverflow { function })
        })?;
        functions.push(crate::FunctionRecursiveSpillInsertion {
            machine: source.machine,
            spill_area_bytes,
            slots: assigned,
            schedule,
        });
    }
    let usage = work_usage(&functions)?;
    if !usage.within(budget) {
        return Err(RecursiveSpillInsertionError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    Ok(RecursiveSpillInsertionPlan {
        generalized_spill_insertion: base.receipt().identity(),
        recovery_actions: recovery.receipt().identity(),
        register_environment: recovery.plan().register_environment,
        allocator_availability: recovery.plan().allocator_availability,
        optimization_unit: recovery.receipt().optimization_unit(),
        fuel_schedule: recovery.receipt().fuel_schedule(),
        policy,
        budget,
        usage,
        functions,
    })
}

fn admit_roots(
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

fn admit_policy(policy: RecursiveSpillInsertionPolicy) -> Result<(), RecursiveSpillInsertionError> {
    if !matches!(
        policy,
        RecursiveSpillInsertionPolicy::EpochTwoReloadVictimBlockLocalUnsignedU64ClosedIntervalFirstFitV1
    ) {
        return Err(RecursiveSpillInsertionError::UnsupportedPolicy);
    }
    Ok(())
}

fn event_key(event: &RecursiveSpillEvent) -> (u32, u8, u32, u32, u32, u16) {
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

fn work_usage(
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
