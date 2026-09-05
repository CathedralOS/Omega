//! Keyed reconstruction of slots, stores, reloads, rewrites, and source lineage.

use std::collections::BTreeMap;

use crate::{
    GeneralizedReloadValueHomeOutcome, GeneralizedSpillActionId, LiveRangePoint,
    RecursiveReloadValueHomeError, RecursiveSpillEvent, RecursiveSpillStoredValue,
};

use super::{ReplaySpec, homes};

#[derive(Clone, Copy)]
struct IndexedStore {
    point: LiveRangePoint,
    source: RecursiveSpillStoredValue,
}

pub(super) fn index(
    function: usize,
    recursive: &crate::FunctionRecursiveSpillInsertion,
    prior: &crate::FunctionGeneralizedReloadValueHomes,
    legality: &crate::FunctionAllocationLegality,
) -> Result<Vec<ReplaySpec>, RecursiveReloadValueHomeError> {
    let slots = recursive
        .slots
        .iter()
        .map(|slot| (slot.action, slot))
        .collect::<BTreeMap<_, _>>();
    if slots.len() != recursive.slots.len() {
        return Err(RecursiveReloadValueHomeError::InvalidRecursiveAction {
            function,
            action: recursive.slots[0].action,
        });
    }
    let mut stores = BTreeMap::new();
    let mut reloads = BTreeMap::new();
    let mut rewrites = BTreeMap::<GeneralizedSpillActionId, Vec<LiveRangePoint>>::new();
    for event in &recursive.schedule {
        match *event {
            RecursiveSpillEvent::Store {
                action,
                point,
                source,
                ..
            } => {
                if stores
                    .insert(action, IndexedStore { point, source })
                    .is_some()
                {
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
                if result != action || reloads.insert(action, (point, destination_class)).is_some()
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
                if result != action {
                    return Err(invalid(function, action));
                }
                rewrites.entry(action).or_default().push(point);
            }
        }
    }
    let mut output = Vec::with_capacity(slots.len());
    for (action, slot) in slots {
        let store = stores
            .remove(&action)
            .ok_or_else(|| invalid(function, action))?;
        let (start, class) = reloads
            .remove(&action)
            .ok_or_else(|| invalid(function, action))?;
        let mut points = rewrites
            .remove(&action)
            .ok_or_else(|| invalid(function, action))?;
        points.sort();
        let last = points.last().ok_or_else(|| invalid(function, action))?;
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
            return Err(invalid(function, action));
        }
        let source_register = trace(function, action, &stores, recursive)?;
        let old = prior.outcomes.iter().find(|outcome| match outcome {
            GeneralizedReloadValueHomeOutcome::Assigned(row) => row.result == action,
            GeneralizedReloadValueHomeOutcome::Pressure(row) => row.result == action,
        });
        let candidates = match old {
            Some(GeneralizedReloadValueHomeOutcome::Assigned(row)) => {
                if slot.source != crate::RecursiveSpillActionSource::Prior(row.source)
                    || row.block != slot.block
                    || row.start != start
                    || row.exclusive_end != full_exclusive_end
                    || row.class != class
                {
                    return Err(RecursiveReloadValueHomeError::InvalidPriorOutcome {
                        function,
                        action,
                    });
                }
                row.candidates.clone()
            }
            Some(GeneralizedReloadValueHomeOutcome::Pressure(row)) => {
                if slot.source != crate::RecursiveSpillActionSource::Prior(row.source)
                    || row.block != slot.block
                    || row.start != start
                    || row.exclusive_end != full_exclusive_end
                    || row.class != class
                {
                    return Err(RecursiveReloadValueHomeError::InvalidPriorOutcome {
                        function,
                        action,
                    });
                }
                row.candidates.clone()
            }
            None => {
                let legality_row = homes::find_legality(function, legality, source_register)?;
                if legality_row.class != class {
                    return Err(invalid(function, action));
                }
                homes::domain(
                    function,
                    legality_row,
                    slot.block,
                    start,
                    full_exclusive_end,
                )?
            }
        };
        let exclusive_end = recursive
            .schedule
            .iter()
            .filter_map(|event| match *event {
                RecursiveSpillEvent::Store {
                    point,
                    source: RecursiveSpillStoredValue::Reload(source),
                    ..
                } if source == action && start < point && point < full_exclusive_end => Some(point),
                _ => None,
            })
            .min()
            .unwrap_or(full_exclusive_end);
        output.push(ReplaySpec {
            action,
            source: slot.source,
            block: slot.block,
            start,
            full_exclusive_end,
            exclusive_end,
            class,
            candidates,
        });
    }
    if !stores.is_empty() || !reloads.is_empty() || !rewrites.is_empty() {
        let action = stores
            .keys()
            .chain(reloads.keys())
            .chain(rewrites.keys())
            .next()
            .copied()
            .unwrap();
        return Err(invalid(function, action));
    }
    Ok(output)
}

fn trace(
    function: usize,
    action: GeneralizedSpillActionId,
    remaining_stores: &BTreeMap<GeneralizedSpillActionId, IndexedStore>,
    recursive: &crate::FunctionRecursiveSpillInsertion,
) -> Result<omega_selected_instructions::VirtualRegisterId, RecursiveReloadValueHomeError> {
    let mut all = remaining_stores.clone();
    for event in &recursive.schedule {
        if let RecursiveSpillEvent::Store {
            action,
            point,
            source,
            ..
        } = *event
        {
            all.entry(action).or_insert(IndexedStore { point, source });
        }
    }
    let mut current = action;
    for _ in 0..=all.len() {
        match all.get(&current).map(|row| row.source) {
            Some(RecursiveSpillStoredValue::Original(register)) => return Ok(register),
            Some(RecursiveSpillStoredValue::Reload(source)) => current = source,
            None => break,
        }
    }
    Err(RecursiveReloadValueHomeError::MissingSourceRegister { function, action })
}

fn invalid(function: usize, action: GeneralizedSpillActionId) -> RecursiveReloadValueHomeError {
    RecursiveReloadValueHomeError::InvalidRecursiveAction { function, action }
}
