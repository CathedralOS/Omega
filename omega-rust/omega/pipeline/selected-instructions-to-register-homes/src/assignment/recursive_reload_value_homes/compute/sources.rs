//! Direct reconstruction of every recursive reload segment and source register.

use std::collections::BTreeMap;

use crate::{
    GeneralizedReloadValueHomeOutcome, GeneralizedSpillActionId, LiveRangePoint,
    RecursiveReloadValueHomeError, RecursiveSpillEvent, RecursiveSpillStoredValue,
};

use super::{ReloadSpec, homes};

#[derive(Clone, Copy)]
struct StoreRow {
    point: LiveRangePoint,
    source: RecursiveSpillStoredValue,
}

pub(super) fn reconstruct(
    function: usize,
    recursive: &crate::FunctionRecursiveSpillInsertion,
    prior: &crate::FunctionGeneralizedReloadValueHomes,
    legality: &crate::FunctionAllocationLegality,
) -> Result<Vec<ReloadSpec>, RecursiveReloadValueHomeError> {
    let mut stores = BTreeMap::new();
    let mut reloads = BTreeMap::new();
    let mut rewrite_ends = BTreeMap::<GeneralizedSpillActionId, LiveRangePoint>::new();
    for event in &recursive.schedule {
        match *event {
            RecursiveSpillEvent::Store {
                action,
                point,
                source,
                ..
            } => {
                if stores.insert(action, StoreRow { point, source }).is_some() {
                    return Err(invalid(function, action));
                }
            }
            RecursiveSpillEvent::Reload {
                action,
                point,
                result,
                destination_class,
                ..
            } => {
                if action != result || reloads.insert(action, (point, destination_class)).is_some()
                {
                    return Err(invalid(function, action));
                }
            }
            RecursiveSpillEvent::Rewrite {
                action,
                point,
                result,
                ..
            } => {
                if action != result {
                    return Err(invalid(function, action));
                }
                rewrite_ends
                    .entry(action)
                    .and_modify(|end| *end = (*end).max(point))
                    .or_insert(point);
            }
        }
    }
    let mut specs = Vec::with_capacity(recursive.slots.len());
    for slot in &recursive.slots {
        let store = stores
            .get(&slot.action)
            .ok_or_else(|| invalid(function, slot.action))?;
        let &(start, class) = reloads
            .get(&slot.action)
            .ok_or_else(|| invalid(function, slot.action))?;
        let last = rewrite_ends
            .get(&slot.action)
            .ok_or_else(|| invalid(function, slot.action))?;
        let full_exclusive_end = LiveRangePoint(last.0.checked_add(1).ok_or(
            RecursiveReloadValueHomeError::IntervalOverflow {
                function,
                register: u32::MAX,
            },
        )?);
        if store.point != slot.live_from
            || start != slot.live_through
            || start >= full_exclusive_end
        {
            return Err(invalid(function, slot.action));
        }
        let source_register = resolve_source_register(function, slot.action, &stores)?;
        let prior_row = prior.outcomes.iter().find(|outcome| match outcome {
            GeneralizedReloadValueHomeOutcome::Assigned(row) => row.result == slot.action,
            GeneralizedReloadValueHomeOutcome::Pressure(row) => row.result == slot.action,
        });
        let candidates = if let Some(outcome) = prior_row {
            let (source, block, old_start, old_end, old_class, candidates) = match outcome {
                GeneralizedReloadValueHomeOutcome::Assigned(row) => (
                    row.source,
                    row.block,
                    row.start,
                    row.exclusive_end,
                    row.class,
                    &row.candidates,
                ),
                GeneralizedReloadValueHomeOutcome::Pressure(row) => (
                    row.source,
                    row.block,
                    row.start,
                    row.exclusive_end,
                    row.class,
                    &row.candidates,
                ),
            };
            if slot.source != crate::RecursiveSpillActionSource::Prior(source)
                || block != slot.block
                || old_start != start
                || old_end != full_exclusive_end
                || old_class != class
            {
                return Err(RecursiveReloadValueHomeError::InvalidPriorOutcome {
                    function,
                    action: slot.action,
                });
            }
            candidates.clone()
        } else {
            let row = homes::legality_row(function, legality, source_register)?;
            if row.class != class {
                return Err(invalid(function, slot.action));
            }
            homes::reload_candidates(function, row, slot.block, start, full_exclusive_end)?
        };
        let exclusive_end = stores
            .values()
            .filter_map(|row| match row.source {
                RecursiveSpillStoredValue::Reload(action)
                    if action == slot.action
                        && start < row.point
                        && row.point < full_exclusive_end =>
                {
                    Some(row.point)
                }
                _ => None,
            })
            .min()
            .unwrap_or(full_exclusive_end);
        specs.push(ReloadSpec {
            action: slot.action,
            source: slot.source,
            block: slot.block,
            start,
            full_exclusive_end,
            exclusive_end,
            class,
            candidates,
        });
    }
    specs.sort_by_key(|spec| spec.action);
    if specs
        .windows(2)
        .any(|pair| pair[0].action == pair[1].action)
    {
        return Err(invalid(function, specs[0].action));
    }
    Ok(specs)
}

fn resolve_source_register(
    function: usize,
    action: GeneralizedSpillActionId,
    stores: &BTreeMap<GeneralizedSpillActionId, StoreRow>,
) -> Result<selected_instructions::VirtualRegisterId, RecursiveReloadValueHomeError> {
    let mut current = action;
    for _ in 0..=stores.len() {
        let row = stores
            .get(&current)
            .ok_or(RecursiveReloadValueHomeError::MissingSourceRegister { function, action })?;
        match row.source {
            RecursiveSpillStoredValue::Original(register) => return Ok(register),
            RecursiveSpillStoredValue::Reload(source) => current = source,
        }
    }
    Err(RecursiveReloadValueHomeError::MissingSourceRegister { function, action })
}

fn invalid(function: usize, action: GeneralizedSpillActionId) -> RecursiveReloadValueHomeError {
    RecursiveReloadValueHomeError::InvalidRecursiveAction { function, action }
}
