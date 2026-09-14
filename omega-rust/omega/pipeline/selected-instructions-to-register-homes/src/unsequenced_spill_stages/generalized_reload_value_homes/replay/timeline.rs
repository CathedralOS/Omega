//! Point-indexed allocation replay with explicit reload-before-original events.

use std::collections::{BTreeMap, BTreeSet};

use register_model::ValidatedPhysicalRegisterModel;

use crate::{
    GeneralizedReloadCoexistingValue, GeneralizedReloadValueHomeAssignment,
    GeneralizedReloadValueHomeError, GeneralizedReloadValueHomeOutcome,
    GeneralizedReloadValuePressure, GeneralizedSpillActionId, GeneralizedSpillActionSource,
    LiveRangePoint,
};

use super::{Occupant, ReplaySpec, homes};

#[derive(Default)]
struct PointEvents<'a> {
    reloads: Vec<&'a ReplaySpec>,
    originals: Vec<OriginalEvent<'a>>,
}

#[derive(Clone, Copy)]
struct OriginalEvent<'a> {
    row: &'a crate::VirtualRegisterAllocationLegality,
    exclusive_end: LiveRangePoint,
}

pub(super) fn reconstruct(
    function: usize,
    specs: &[ReplaySpec],
    first: &crate::FunctionAbstractSpillInsertion,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<Vec<GeneralizedReloadValueHomeOutcome>, GeneralizedReloadValueHomeError> {
    let mut points = BTreeMap::<LiveRangePoint, PointEvents<'_>>::new();
    for spec in specs {
        points.entry(spec.start).or_default().reloads.push(spec);
    }
    for row in &legality.virtual_registers {
        let (start, exclusive_end) = homes::original_interval(function, row)?;
        points
            .entry(start)
            .or_default()
            .originals
            .push(OriginalEvent { row, exclusive_end });
    }
    for events in points.values_mut() {
        events.reloads.sort_by_key(|spec| spec.action);
        events
            .originals
            .sort_by_key(|event| event.row.virtual_register);
    }
    let mut occupants = Vec::new();
    let mut selected_views = BTreeMap::new();
    let mut rosters = BTreeMap::<GeneralizedSpillActionId, BTreeSet<_>>::new();
    let mut stores = BTreeSet::new();
    let mut pressure = None;
    'timeline: for (point, events) in points {
        occupants.retain(|occupant: &Occupant| occupant.exclusive_end > point);
        for spec in events.reloads {
            for spill in specs
                .iter()
                .filter(|spill| spill.before_reload == Some(spec.action))
            {
                if spill.store_point != point
                    || !homes::every_reload_view_blocked(&spec.candidates, &occupants, physical)
                {
                    return Err(invalid(function, spill.action));
                }
                homes::remove_spilled(function, spill, &mut occupants)?;
                stores.insert(spill.action);
            }
            let Some(view) = spec
                .candidates
                .iter()
                .copied()
                .find(|view| !homes::reload_conflict(*view, &occupants, physical))
            else {
                pressure = Some((
                    spec.action,
                    homes::reload_blocking_roster(&spec.candidates, &occupants, physical),
                ));
                break 'timeline;
            };
            homes::retain_pair(
                GeneralizedReloadCoexistingValue::Reload(spec.action),
                spec.class,
                view,
                &occupants,
                &mut rosters,
            );
            occupants.push(Occupant {
                value: GeneralizedReloadCoexistingValue::Reload(spec.action),
                class: spec.class,
                exclusive_end: spec.exclusive_end,
                view,
            });
            selected_views.insert(spec.action, view);
        }
        for event in events.originals {
            let row = event.row;
            let domain = homes::original_domain(function, row)?;
            let view = if let Some(action) = first.action.as_ref().filter(|action| {
                action.incoming == row.virtual_register && action.pressure_point == point
            }) {
                let Some(spec) = specs.iter().find(|spec| {
                    matches!(spec.source, GeneralizedSpillActionSource::EpochZero { reload, .. }
                        if reload == action.reload.result)
                }) else {
                    return Err(GeneralizedReloadValueHomeError::PrefixMismatch { function });
                };
                if !homes::every_original_view_blocked(
                    row.virtual_register,
                    &domain,
                    &occupants,
                    &ranges.interference,
                    physical,
                ) {
                    return Err(GeneralizedReloadValueHomeError::PrefixMismatch { function });
                }
                homes::remove_spilled(function, spec, &mut occupants)?;
                stores.insert(spec.action);
                let rebuilt = domain.iter().copied().find(|view| {
                    !homes::original_conflict(
                        row.virtual_register,
                        *view,
                        &occupants,
                        &ranges.interference,
                        physical,
                    )
                });
                if rebuilt != Some(action.incoming_view) {
                    return Err(GeneralizedReloadValueHomeError::PrefixMismatch { function });
                }
                action.incoming_view
            } else {
                domain
                    .iter()
                    .copied()
                    .find(|view| {
                        !homes::original_conflict(
                            row.virtual_register,
                            *view,
                            &occupants,
                            &ranges.interference,
                            physical,
                        )
                    })
                    .ok_or_else(|| {
                        if first
                            .action
                            .as_ref()
                            .is_none_or(|action| point <= action.pressure_point)
                        {
                            GeneralizedReloadValueHomeError::PrefixMismatch { function }
                        } else {
                            GeneralizedReloadValueHomeError::SecondaryPressure {
                                function,
                                register: row.virtual_register.0,
                            }
                        }
                    })?
            };
            homes::retain_pair(
                GeneralizedReloadCoexistingValue::Original(row.virtual_register),
                row.class,
                view,
                &occupants,
                &mut rosters,
            );
            occupants.push(Occupant {
                value: GeneralizedReloadCoexistingValue::Original(row.virtual_register),
                class: row.class,
                exclusive_end: event.exclusive_end,
                view,
            });
        }
    }
    if let Some(spec) = specs.iter().find(|spec| !stores.contains(&spec.action)) {
        return Err(invalid(function, spec.action));
    }
    let mut outcomes = Vec::new();
    for spec in specs {
        if let Some(view) = selected_views.get(&spec.action).copied() {
            outcomes.push(GeneralizedReloadValueHomeOutcome::Assigned(
                GeneralizedReloadValueHomeAssignment {
                    result: spec.action,
                    source: spec.source,
                    block: spec.block,
                    start: spec.start,
                    exclusive_end: spec.exclusive_end,
                    class: spec.class,
                    candidates: spec.candidates.clone(),
                    view,
                    coexisting_homes: rosters
                        .remove(&spec.action)
                        .unwrap_or_default()
                        .into_iter()
                        .collect(),
                },
            ));
        } else if let Some((action, blocking_homes)) = pressure.take() {
            if action != spec.action || outcomes.len() + 1 != specs.len() {
                return Err(invalid(function, action));
            }
            outcomes.push(GeneralizedReloadValueHomeOutcome::Pressure(
                GeneralizedReloadValuePressure {
                    result: spec.action,
                    source: spec.source,
                    block: spec.block,
                    start: spec.start,
                    exclusive_end: spec.exclusive_end,
                    class: spec.class,
                    candidates: spec.candidates.clone(),
                    blocking_homes,
                },
            ));
        } else {
            return Err(GeneralizedReloadValueHomeError::MissingAction {
                function,
                action: spec.action,
            });
        }
    }
    Ok(outcomes)
}

fn invalid(function: usize, action: GeneralizedSpillActionId) -> GeneralizedReloadValueHomeError {
    GeneralizedReloadValueHomeError::InvalidAction { function, action }
}
