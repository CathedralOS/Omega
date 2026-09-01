//! Replay-local keyed reconstruction of stores, reloads, and rewrite suffixes.

use std::collections::BTreeMap;

use crate::{
    GeneralizedReloadValueHomeError, GeneralizedSpillActionId, GeneralizedSpillActionSource,
    GeneralizedSpillEvent, LiveRangePoint, ValidatedSpillRecoveryActions,
};

use super::{ReplaySpec, homes};

pub(super) fn index(
    function: usize,
    generalized: &crate::FunctionGeneralizedSpillInsertion,
    first: &crate::FunctionAbstractSpillInsertion,
    second: &ValidatedSpillRecoveryActions,
    legality: &crate::FunctionAllocationLegality,
) -> Result<Vec<ReplaySpec>, GeneralizedReloadValueHomeError> {
    let mut slots = BTreeMap::new();
    for slot in &generalized.slots {
        if slots.insert(slot.action, slot).is_some() {
            return Err(invalid(function, slot.action));
        }
    }
    let mut stores = BTreeMap::new();
    let mut reloads = BTreeMap::new();
    let mut rewrites = BTreeMap::<GeneralizedSpillActionId, Vec<_>>::new();
    for event in &generalized.schedule {
        match event {
            GeneralizedSpillEvent::Store { action, .. } => {
                if stores.insert(*action, event).is_some() {
                    return Err(invalid(function, *action));
                }
            }
            GeneralizedSpillEvent::Reload { action, .. } => {
                if reloads.insert(*action, event).is_some() {
                    return Err(invalid(function, *action));
                }
            }
            GeneralizedSpillEvent::Rewrite { action, .. } => {
                rewrites.entry(*action).or_default().push(event);
            }
        }
    }
    let mut result = Vec::with_capacity(slots.len());
    for (action, slot) in slots {
        let Some(GeneralizedSpillEvent::Store {
            point: store_point,
            before_reload,
            source: victim,
            source_view: victim_view,
            slot: store_slot,
            ..
        }) = stores.remove(&action)
        else {
            return Err(invalid(function, action));
        };
        let Some(GeneralizedSpillEvent::Reload {
            point: start,
            slot: reload_slot,
            result: reload_result,
            destination_class,
            ..
        }) = reloads.remove(&action)
        else {
            return Err(invalid(function, action));
        };
        let mut action_rewrites = rewrites.remove(&action).unwrap_or_default();
        action_rewrites.sort_by_key(|event| rewrite_key(event));
        let Some(GeneralizedSpillEvent::Rewrite {
            block,
            point: first_point,
            ..
        }) = action_rewrites.first().copied()
        else {
            return Err(invalid(function, action));
        };
        let Some(GeneralizedSpillEvent::Rewrite {
            point: last_point, ..
        }) = action_rewrites.last().copied()
        else {
            return Err(invalid(function, action));
        };
        let exclusive_end = LiveRangePoint(last_point.0.checked_add(1).ok_or(
            GeneralizedReloadValueHomeError::IntervalOverflow {
                function,
                register: victim.0,
            },
        )?);
        let shape_matches = *store_point == slot.live_from
            && *start == slot.live_through
            && *first_point == *start
            && *block == slot.block
            && *store_slot == action
            && *reload_slot == action
            && *reload_result == action
            && action_rewrites
                .windows(2)
                .all(|pair| rewrite_key(pair[0]) < rewrite_key(pair[1]))
            && action_rewrites.iter().all(|event| {
                matches!(event, GeneralizedSpillEvent::Rewrite { block, result, .. }
                    if *block == slot.block && *result == action)
            });
        if !shape_matches
            || !source_matches(
                function,
                slot.source,
                action,
                first,
                second,
                *victim,
                *victim_view,
            )
        {
            return Err(invalid(function, action));
        }
        let victim_row = homes::find_legality(function, legality, *victim)?;
        if victim_row.class != *destination_class {
            return Err(invalid(function, action));
        }
        result.push(ReplaySpec {
            action,
            source: slot.source,
            block: slot.block,
            store_point: *store_point,
            start: *start,
            exclusive_end,
            class: *destination_class,
            candidates: homes::reload_domain(
                function,
                victim_row,
                slot.block,
                *start,
                exclusive_end,
            )?,
            victim: *victim,
            victim_view: *victim_view,
            before_reload: *before_reload,
        });
    }
    if !(stores.is_empty() && reloads.is_empty() && rewrites.is_empty()) {
        let action = stores
            .keys()
            .chain(reloads.keys())
            .chain(rewrites.keys())
            .next()
            .copied()
            .unwrap_or(GeneralizedSpillActionId {
                epoch: 0,
                ordinal: 0,
            });
        return Err(invalid(function, action));
    }
    Ok(result)
}

fn source_matches(
    function: usize,
    source: GeneralizedSpillActionSource,
    action: GeneralizedSpillActionId,
    first: &crate::FunctionAbstractSpillInsertion,
    second: &ValidatedSpillRecoveryActions,
    victim: omega_selected_instructions::VirtualRegisterId,
    victim_view: omega_register_model::RegisterViewId,
) -> bool {
    match source {
        GeneralizedSpillActionSource::EpochZero { storage, reload } => {
            first.action.as_ref().is_some_and(|row| {
                action.epoch == 0
                    && row.slot.storage == storage
                    && row.reload.result == reload
                    && row.victim == victim
                    && row.victim_view == victim_view
            })
        }
        GeneralizedSpillActionSource::EpochOne {
            work_item,
            storage,
            source_reload,
            reload,
        } => second.plan().actions.iter().any(|row| {
            row.function == function
                && row.source_work_item == work_item
                && row.storage.id == storage
                && row.source_reload == source_reload
                && row.reload.result == reload
                && row.victim == victim
                && row.current_view == victim_view
        }),
    }
}

fn rewrite_key(event: &GeneralizedSpillEvent) -> (u32, u32, u16) {
    match event {
        GeneralizedSpillEvent::Rewrite {
            point,
            instruction,
            operand,
            ..
        } => (point.0, instruction.0, *operand),
        _ => (u32::MAX, u32::MAX, u16::MAX),
    }
}

fn invalid(function: usize, action: GeneralizedSpillActionId) -> GeneralizedReloadValueHomeError {
    GeneralizedReloadValueHomeError::InvalidAction { function, action }
}
