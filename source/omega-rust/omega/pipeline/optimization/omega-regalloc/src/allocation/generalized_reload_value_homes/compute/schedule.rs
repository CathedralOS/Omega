//! Sorted allocation schedule and generalized reload home choice.

use std::collections::{BTreeMap, BTreeSet};

use omega_register_model::ValidatedPhysicalRegisterModel;

use crate::{
    GeneralizedReloadCoexistingValue, GeneralizedReloadValueHomeAssignment,
    GeneralizedReloadValueHomeError, GeneralizedReloadValueHomeOutcome,
    GeneralizedReloadValuePressure, GeneralizedSpillActionId, GeneralizedSpillActionSource,
    LiveRangePoint,
};

use super::{ActiveHome, ReloadSpec, homes};

#[derive(Clone, Copy)]
enum Event<'a> {
    Reload(&'a ReloadSpec),
    Original {
        row: &'a crate::VirtualRegisterAllocationLegality,
        start: LiveRangePoint,
        exclusive_end: LiveRangePoint,
    },
}

impl Event<'_> {
    const fn key(self) -> (LiveRangePoint, u8, u32, u32) {
        match self {
            Self::Reload(spec) => (spec.start, 0, spec.action.epoch, spec.action.ordinal),
            Self::Original { row, start, .. } => (start, 1, 0, row.virtual_register.0),
        }
    }
}

pub(super) fn assign(
    function: usize,
    specs: &[ReloadSpec],
    first: &crate::FunctionAbstractSpillInsertion,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<Vec<GeneralizedReloadValueHomeOutcome>, GeneralizedReloadValueHomeError> {
    let mut events = specs.iter().map(Event::Reload).collect::<Vec<_>>();
    for row in &legality.virtual_registers {
        let (start, exclusive_end) = homes::interval(function, row)?;
        events.push(Event::Original {
            row,
            start,
            exclusive_end,
        });
    }
    events.sort_by_key(|event| event.key());
    let mut active = Vec::new();
    let mut views = BTreeMap::new();
    let mut coexist = BTreeMap::<GeneralizedSpillActionId, BTreeSet<_>>::new();
    let mut consumed = BTreeSet::new();
    let mut pressure = None;
    'timeline: for event in events {
        let point = event.key().0;
        active.retain(|home: &ActiveHome| home.exclusive_end > point);
        match event {
            Event::Reload(spec) => {
                for spill in specs
                    .iter()
                    .filter(|spill| spill.before_reload == Some(spec.action))
                {
                    if spill.store_point != point
                        || !homes::all_blocked(&spec.candidates, &active, physical)
                    {
                        return Err(invalid(function, spill.action));
                    }
                    homes::evict(function, spill, &mut active)?;
                    consumed.insert(spill.action);
                }
                let Some(view) = spec
                    .candidates
                    .iter()
                    .copied()
                    .find(|candidate| !homes::blocked_reload(*candidate, &active, physical))
                else {
                    pressure = Some((
                        spec.action,
                        homes::reload_blockers(&spec.candidates, &active, physical),
                    ));
                    break 'timeline;
                };
                homes::record_coexistence(
                    GeneralizedReloadCoexistingValue::Reload(spec.action),
                    spec.class,
                    view,
                    &active,
                    &mut coexist,
                );
                active.push(ActiveHome {
                    value: GeneralizedReloadCoexistingValue::Reload(spec.action),
                    class: spec.class,
                    exclusive_end: spec.exclusive_end,
                    view,
                });
                views.insert(spec.action, view);
            }
            Event::Original {
                row,
                start,
                exclusive_end,
            } => {
                let domain = homes::original_candidates(function, row)?;
                let view = if let Some(action) = first.action.as_ref().filter(|action| {
                    action.incoming == row.virtual_register && action.pressure_point == start
                }) {
                    let Some(spec) = specs.iter().find(|spec| {
                        matches!(spec.source, GeneralizedSpillActionSource::EpochZero { reload, .. }
                            if reload == action.reload.result)
                    }) else {
                        return Err(GeneralizedReloadValueHomeError::PrefixMismatch { function });
                    };
                    if !homes::all_original_blocked(
                        row.virtual_register,
                        &domain,
                        &active,
                        &ranges.interference,
                        physical,
                    ) {
                        return Err(GeneralizedReloadValueHomeError::PrefixMismatch { function });
                    }
                    homes::evict(function, spec, &mut active)?;
                    consumed.insert(spec.action);
                    let chosen = domain.iter().copied().find(|candidate| {
                        !homes::blocked_original(
                            row.virtual_register,
                            *candidate,
                            &active,
                            &ranges.interference,
                            physical,
                        )
                    });
                    if chosen != Some(action.incoming_view) {
                        return Err(GeneralizedReloadValueHomeError::PrefixMismatch { function });
                    }
                    action.incoming_view
                } else {
                    domain
                        .iter()
                        .copied()
                        .find(|candidate| {
                            !homes::blocked_original(
                                row.virtual_register,
                                *candidate,
                                &active,
                                &ranges.interference,
                                physical,
                            )
                        })
                        .ok_or_else(|| {
                            if first
                                .action
                                .as_ref()
                                .is_none_or(|action| start <= action.pressure_point)
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
                homes::record_coexistence(
                    GeneralizedReloadCoexistingValue::Original(row.virtual_register),
                    row.class,
                    view,
                    &active,
                    &mut coexist,
                );
                active.push(ActiveHome {
                    value: GeneralizedReloadCoexistingValue::Original(row.virtual_register),
                    class: row.class,
                    exclusive_end,
                    view,
                });
            }
        }
    }
    if let Some(spec) = specs.iter().find(|spec| !consumed.contains(&spec.action)) {
        return Err(invalid(function, spec.action));
    }
    let mut outcomes = Vec::new();
    for spec in specs {
        if let Some(view) = views.get(&spec.action).copied() {
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
                    coexisting_homes: coexist
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
