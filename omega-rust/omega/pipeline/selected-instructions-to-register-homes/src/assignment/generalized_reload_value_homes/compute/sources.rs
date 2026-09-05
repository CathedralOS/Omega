//! Direct reconstruction of both logical reload specifications.

use register_model::RegisterViewId;
use selected_instructions::VirtualRegisterId;

use crate::{
    GeneralizedReloadValueHomeError, GeneralizedSpillActionSource, GeneralizedSpillEvent,
    LiveRangePoint, ValidatedSpillRecoveryActions,
};

use super::{ReloadSpec, homes};

pub(super) fn reconstruct(
    function: usize,
    generalized: &crate::FunctionGeneralizedSpillInsertion,
    first: &crate::FunctionAbstractSpillInsertion,
    second: &ValidatedSpillRecoveryActions,
    legality: &crate::FunctionAllocationLegality,
) -> Result<Vec<ReloadSpec>, GeneralizedReloadValueHomeError> {
    let mut specs = Vec::with_capacity(generalized.slots.len());
    for slot in &generalized.slots {
        let mut store = None;
        let mut reload = None;
        let mut rewrites = Vec::new();
        for event in &generalized.schedule {
            match *event {
                GeneralizedSpillEvent::Store { action, .. } if action == slot.action => {
                    if store.replace(event).is_some() {
                        return Err(invalid(function, slot.action));
                    }
                }
                GeneralizedSpillEvent::Reload { action, .. } if action == slot.action => {
                    if reload.replace(event).is_some() {
                        return Err(invalid(function, slot.action));
                    }
                }
                GeneralizedSpillEvent::Rewrite { action, .. } if action == slot.action => {
                    rewrites.push(event);
                }
                _ => {}
            }
        }
        let (
            Some(GeneralizedSpillEvent::Store {
                point: store_point,
                before_reload,
                source: victim,
                source_view: victim_view,
                slot: store_slot,
                ..
            }),
            Some(GeneralizedSpillEvent::Reload {
                point: start,
                slot: reload_slot,
                result,
                destination_class,
                ..
            }),
            Some(GeneralizedSpillEvent::Rewrite {
                block,
                point: first_point,
                ..
            }),
        ) = (store, reload, rewrites.first().copied())
        else {
            return Err(invalid(function, slot.action));
        };
        let last_point = match rewrites.last().copied() {
            Some(GeneralizedSpillEvent::Rewrite { point, .. }) => point,
            _ => return Err(invalid(function, slot.action)),
        };
        let exclusive_end = LiveRangePoint(last_point.0.checked_add(1).ok_or(
            GeneralizedReloadValueHomeError::IntervalOverflow {
                function,
                register: victim.0,
            },
        )?);
        if *store_point != slot.live_from
            || *start != slot.live_through
            || *first_point != *start
            || *block != slot.block
            || *store_slot != slot.action
            || *reload_slot != slot.action
            || *result != slot.action
            || rewrites.windows(2).any(|pair| pair[0] == pair[1])
            || rewrites.iter().any(|event| {
                !matches!(event, GeneralizedSpillEvent::Rewrite { block, result, .. }
                    if *block == slot.block && *result == slot.action)
            })
        {
            return Err(invalid(function, slot.action));
        }
        validate_source(function, slot, first, second, *victim, *victim_view)?;
        let victim_row = homes::legality_row(function, legality, *victim)?;
        if victim_row.class != *destination_class {
            return Err(invalid(function, slot.action));
        }
        specs.push(ReloadSpec {
            action: slot.action,
            source: slot.source,
            block: slot.block,
            store_point: *store_point,
            start: *start,
            exclusive_end,
            class: *destination_class,
            candidates: homes::reload_candidates(
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
    specs.sort_by_key(|spec| spec.action);
    if let Some(pair) = specs
        .windows(2)
        .find(|pair| pair[0].action == pair[1].action)
    {
        return Err(invalid(function, pair[0].action));
    }
    Ok(specs)
}

fn validate_source(
    function: usize,
    slot: &crate::GeneralizedSpillSlot,
    first: &crate::FunctionAbstractSpillInsertion,
    second: &ValidatedSpillRecoveryActions,
    victim: VirtualRegisterId,
    victim_view: RegisterViewId,
) -> Result<(), GeneralizedReloadValueHomeError> {
    let valid = match slot.source {
        GeneralizedSpillActionSource::EpochZero { storage, reload } => {
            first.action.as_ref().is_some_and(|action| {
                slot.action.epoch == 0
                    && action.slot.storage == storage
                    && action.reload.result == reload
                    && action.victim == victim
                    && action.victim_view == victim_view
            })
        }
        GeneralizedSpillActionSource::EpochOne {
            work_item,
            storage,
            source_reload,
            reload,
        } => second.plan().actions.iter().any(|action| {
            action.function == function
                && action.source_work_item == work_item
                && action.storage.id == storage
                && action.source_reload == source_reload
                && action.reload.result == reload
                && action.victim == victim
                && action.current_view == victim_view
        }),
    };
    if valid {
        Ok(())
    } else {
        Err(invalid(function, slot.action))
    }
}

fn invalid(
    function: usize,
    action: crate::GeneralizedSpillActionId,
) -> GeneralizedReloadValueHomeError {
    GeneralizedReloadValueHomeError::InvalidAction { function, action }
}
