//! Point-indexed pressure closure and later reload reconstruction.

use std::collections::{BTreeMap, BTreeSet};

use omega_register_model::ValidatedPhysicalRegisterModel;

use crate::{
    GeneralizedReloadCoexistingValue, GeneralizedReloadValueHomeOutcome, GeneralizedSpillActionId,
    GeneralizedSpillRecoveryVictim, RecursiveReloadCoexistingHome, RecursiveReloadCoexistingValue,
    RecursiveReloadValueHomeAssignment, RecursiveReloadValueHomeError, RecursiveSpillEvent,
    RecursiveSpillStoredValue,
};

use super::{Occupant, ReplaySpec, homes};

#[allow(clippy::too_many_arguments)]
pub(super) fn reconstruct(
    function: usize,
    specs: &[ReplaySpec],
    recursive: &crate::FunctionRecursiveSpillInsertion,
    recovery: &crate::ValidatedGeneralizedSpillRecoveryActions,
    prior: &crate::FunctionGeneralizedReloadValueHomes,
    legality: &crate::FunctionAllocationLegality,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<Vec<RecursiveReloadValueHomeAssignment>, RecursiveReloadValueHomeError> {
    let mut pressure = None;
    let mut assigned_before = BTreeSet::new();
    let mut selected = BTreeMap::new();
    let mut rosters =
        BTreeMap::<GeneralizedSpillActionId, BTreeSet<RecursiveReloadCoexistingHome>>::new();
    for outcome in &prior.outcomes {
        match outcome {
            GeneralizedReloadValueHomeOutcome::Assigned(row) => {
                assigned_before.insert(row.result);
                selected.insert(row.result, row.view);
                rosters.insert(
                    row.result,
                    row.coexisting_homes.iter().map(convert_home).collect(),
                );
            }
            GeneralizedReloadValueHomeOutcome::Pressure(row) => {
                if pressure.is_some() {
                    return Err(RecursiveReloadValueHomeError::MultiplePressures { function });
                }
                pressure = Some(row);
            }
        }
    }
    let pressure = pressure.ok_or(RecursiveReloadValueHomeError::MissingPressure { function })?;
    let pressure_spec = lookup(function, specs, pressure.result)?;
    let mut occupants = Vec::with_capacity(pressure.blocking_homes.len());
    for home in &pressure.blocking_homes {
        let value = convert_value(home.value);
        let exclusive_end = match value {
            RecursiveReloadCoexistingValue::Original(register) => homes::original_exclusive_end(
                function,
                homes::find_legality(function, legality, register)?,
            )?,
            RecursiveReloadCoexistingValue::Reload(action) => {
                lookup(function, specs, action)?.full_exclusive_end
            }
        };
        occupants.push(Occupant {
            value,
            class: home.class,
            exclusive_end,
            view: home.view,
        });
    }

    let action_rows = recovery
        .plan()
        .actions
        .iter()
        .filter(|action| action.function == function && action.source_pressure == pressure.result)
        .collect::<Vec<_>>();
    if action_rows.len() != 1 {
        return Err(RecursiveReloadValueHomeError::VictimMismatch {
            function,
            action: pressure.result,
        });
    }
    let action_row = action_rows[0];
    let recursive_action = GeneralizedSpillActionId {
        epoch: action_row.source_work_item.epoch,
        ordinal: action_row.source_work_item.ordinal,
    };
    let store = recursive
        .schedule
        .iter()
        .filter_map(|event| match *event {
            RecursiveSpillEvent::Store {
                action,
                point,
                before_reload,
                source,
                source_view,
                ..
            } if action == recursive_action => Some((point, before_reload, source, source_view)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if store.len() != 1
        || store[0].0 != pressure.start
        || store[0].1 != Some(pressure.result)
        || store[0].3 != action_row.current_view
    {
        return Err(RecursiveReloadValueHomeError::VictimMismatch {
            function,
            action: recursive_action,
        });
    }
    let victim = match action_row.victim {
        GeneralizedSpillRecoveryVictim::Original(register)
            if store[0].2 == RecursiveSpillStoredValue::Original(register) =>
        {
            RecursiveReloadCoexistingValue::Original(register)
        }
        GeneralizedSpillRecoveryVictim::Reload(action)
            if store[0].2 == RecursiveSpillStoredValue::Reload(action) =>
        {
            RecursiveReloadCoexistingValue::Reload(action)
        }
        _ => {
            return Err(RecursiveReloadValueHomeError::VictimMismatch {
                function,
                action: recursive_action,
            });
        }
    };
    let positions = occupants
        .iter()
        .enumerate()
        .filter(|(_, occupant)| {
            occupant.value == victim && occupant.view == action_row.current_view
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if positions.len() != 1 {
        return Err(RecursiveReloadValueHomeError::VictimMismatch {
            function,
            action: recursive_action,
        });
    }
    occupants.remove(positions[0]);
    let pressure_view = pick(function, pressure_spec, &occupants, physical)?;
    if pressure_view != action_row.reclaimed_view {
        return Err(RecursiveReloadValueHomeError::VictimMismatch {
            function,
            action: recursive_action,
        });
    }
    retain_pairs(pressure_spec, pressure_view, &occupants, &mut rosters);
    occupants.push(Occupant {
        value: RecursiveReloadCoexistingValue::Reload(pressure.result),
        class: pressure_spec.class,
        exclusive_end: pressure_spec.exclusive_end,
        view: pressure_view,
    });
    selected.insert(pressure.result, pressure_view);

    let mut points = BTreeMap::<crate::LiveRangePoint, Vec<&ReplaySpec>>::new();
    for row in specs {
        if !assigned_before.contains(&row.action) && row.action != pressure.result {
            points.entry(row.start).or_default().push(row);
        }
    }
    for rows in points.values_mut() {
        rows.sort_by_key(|row| row.action);
    }
    for (point, rows) in points {
        occupants.retain(|occupant| occupant.exclusive_end > point);
        for row in rows {
            let view = pick(function, row, &occupants, physical)?;
            retain_pairs(row, view, &occupants, &mut rosters);
            occupants.push(Occupant {
                value: RecursiveReloadCoexistingValue::Reload(row.action),
                class: row.class,
                exclusive_end: row.exclusive_end,
                view,
            });
            selected.insert(row.action, view);
        }
    }

    let mut output = Vec::with_capacity(specs.len());
    for row in specs {
        let view = selected.remove(&row.action).ok_or(
            RecursiveReloadValueHomeError::InvalidPriorOutcome {
                function,
                action: row.action,
            },
        )?;
        output.push(RecursiveReloadValueHomeAssignment {
            result: row.action,
            source: row.source,
            block: row.block,
            start: row.start,
            exclusive_end: row.exclusive_end,
            class: row.class,
            candidates: row.candidates.clone(),
            view,
            coexisting_homes: rosters
                .remove(&row.action)
                .unwrap_or_default()
                .into_iter()
                .collect(),
        });
    }
    output.sort_by_key(|row| row.result);
    Ok(output)
}

fn pick(
    function: usize,
    row: &ReplaySpec,
    occupants: &[Occupant],
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<omega_register_model::RegisterViewId, RecursiveReloadValueHomeError> {
    row.candidates
        .iter()
        .copied()
        .find(|candidate| !homes::conflicts(*candidate, occupants, physical))
        .ok_or(RecursiveReloadValueHomeError::ReloadPressure {
            function,
            action: row.action,
        })
}

fn retain_pairs(
    row: &ReplaySpec,
    view: omega_register_model::RegisterViewId,
    occupants: &[Occupant],
    rosters: &mut BTreeMap<GeneralizedSpillActionId, BTreeSet<RecursiveReloadCoexistingHome>>,
) {
    for occupant in occupants {
        rosters
            .entry(row.action)
            .or_default()
            .insert(RecursiveReloadCoexistingHome {
                value: occupant.value,
                class: occupant.class,
                view: occupant.view,
            });
        if let RecursiveReloadCoexistingValue::Reload(prior) = occupant.value {
            rosters
                .entry(prior)
                .or_default()
                .insert(RecursiveReloadCoexistingHome {
                    value: RecursiveReloadCoexistingValue::Reload(row.action),
                    class: row.class,
                    view,
                });
        }
    }
}

fn lookup<'a>(
    function: usize,
    specs: &'a [ReplaySpec],
    action: GeneralizedSpillActionId,
) -> Result<&'a ReplaySpec, RecursiveReloadValueHomeError> {
    specs
        .iter()
        .find(|row| row.action == action)
        .ok_or(RecursiveReloadValueHomeError::InvalidRecursiveAction { function, action })
}

fn convert_home(home: &crate::GeneralizedReloadCoexistingHome) -> RecursiveReloadCoexistingHome {
    RecursiveReloadCoexistingHome {
        value: convert_value(home.value),
        class: home.class,
        view: home.view,
    }
}

const fn convert_value(value: GeneralizedReloadCoexistingValue) -> RecursiveReloadCoexistingValue {
    match value {
        GeneralizedReloadCoexistingValue::Original(register) => {
            RecursiveReloadCoexistingValue::Original(register)
        }
        GeneralizedReloadCoexistingValue::Reload(action) => {
            RecursiveReloadCoexistingValue::Reload(action)
        }
    }
}
