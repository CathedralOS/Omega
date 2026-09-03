//! Sorted pressure closure and later reload-home assignment.

use std::collections::{BTreeMap, BTreeSet};

use omega_register_model::ValidatedPhysicalRegisterModel;

use crate::{
    GeneralizedReloadCoexistingValue, GeneralizedReloadValueHomeOutcome, GeneralizedSpillActionId,
    GeneralizedSpillRecoveryVictim, RecursiveReloadCoexistingHome, RecursiveReloadCoexistingValue,
    RecursiveReloadValueHomeAssignment, RecursiveReloadValueHomeError, RecursiveSpillEvent,
    RecursiveSpillStoredValue,
};

use super::{ActiveHome, ReloadSpec, homes};

#[allow(clippy::too_many_arguments)]
pub(super) fn assign(
    function: usize,
    specs: &[ReloadSpec],
    recursive: &crate::FunctionRecursiveSpillInsertion,
    recovery: &crate::ValidatedGeneralizedSpillRecoveryActions,
    prior: &crate::FunctionGeneralizedReloadValueHomes,
    legality: &crate::FunctionAllocationLegality,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<Vec<RecursiveReloadValueHomeAssignment>, RecursiveReloadValueHomeError> {
    let mut pressure = None;
    let mut prior_assignments = BTreeMap::new();
    let mut rosters =
        BTreeMap::<GeneralizedSpillActionId, BTreeSet<RecursiveReloadCoexistingHome>>::new();
    let mut views = BTreeMap::new();
    for outcome in &prior.outcomes {
        match outcome {
            GeneralizedReloadValueHomeOutcome::Assigned(row) => {
                prior_assignments.insert(row.result, row);
                views.insert(row.result, row.view);
                rosters.insert(
                    row.result,
                    row.coexisting_homes.iter().map(convert_home).collect(),
                );
            }
            GeneralizedReloadValueHomeOutcome::Pressure(row) => {
                if pressure.replace(row).is_some() {
                    return Err(RecursiveReloadValueHomeError::MultiplePressures { function });
                }
            }
        }
    }
    let pressure = pressure.ok_or(RecursiveReloadValueHomeError::MissingPressure { function })?;
    let pressure_spec = spec(function, specs, pressure.result)?;
    let mut active = pressure
        .blocking_homes
        .iter()
        .map(|home| {
            let value = convert_value(home.value);
            let exclusive_end = match value {
                RecursiveReloadCoexistingValue::Original(register) => homes::original_end(
                    function,
                    homes::legality_row(function, legality, register)?,
                )?,
                RecursiveReloadCoexistingValue::Reload(action) => {
                    spec(function, specs, action)?.full_exclusive_end
                }
            };
            Ok(ActiveHome {
                value,
                class: home.class,
                exclusive_end,
                view: home.view,
            })
        })
        .collect::<Result<Vec<_>, RecursiveReloadValueHomeError>>()?;

    let matching = recovery
        .plan()
        .actions
        .iter()
        .filter(|row| row.function == function && row.source_pressure == pressure.result)
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(RecursiveReloadValueHomeError::VictimMismatch {
            function,
            action: pressure.result,
        });
    }
    let recovery_action = matching[0];
    let recursive_action = GeneralizedSpillActionId {
        epoch: recovery_action.source_work_item.epoch,
        ordinal: recovery_action.source_work_item.ordinal,
    };
    let stored = recursive
        .schedule
        .iter()
        .find_map(|event| match *event {
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
        .ok_or(RecursiveReloadValueHomeError::VictimMismatch {
            function,
            action: recursive_action,
        })?;
    if stored.0 != pressure.start
        || stored.1 != Some(pressure.result)
        || stored.3 != recovery_action.current_view
    {
        return Err(RecursiveReloadValueHomeError::VictimMismatch {
            function,
            action: recursive_action,
        });
    }
    let victim = match recovery_action.victim {
        GeneralizedSpillRecoveryVictim::Original(register) => {
            if stored.2 != RecursiveSpillStoredValue::Original(register) {
                return Err(RecursiveReloadValueHomeError::VictimMismatch {
                    function,
                    action: recursive_action,
                });
            }
            RecursiveReloadCoexistingValue::Original(register)
        }
        GeneralizedSpillRecoveryVictim::Reload(action) => {
            if stored.2 != RecursiveSpillStoredValue::Reload(action) {
                return Err(RecursiveReloadValueHomeError::VictimMismatch {
                    function,
                    action: recursive_action,
                });
            }
            RecursiveReloadCoexistingValue::Reload(action)
        }
    };
    let matches = active
        .iter()
        .filter(|home| home.value == victim && home.view == recovery_action.current_view)
        .count();
    if matches != 1 {
        return Err(RecursiveReloadValueHomeError::VictimMismatch {
            function,
            action: recursive_action,
        });
    }
    active.retain(|home| home.value != victim);
    let view = choose(function, pressure_spec, &active, physical)?;
    if view != recovery_action.reclaimed_view {
        return Err(RecursiveReloadValueHomeError::VictimMismatch {
            function,
            action: recursive_action,
        });
    }
    record(pressure_spec, view, &active, &mut rosters);
    active.push(ActiveHome {
        value: RecursiveReloadCoexistingValue::Reload(pressure_spec.action),
        class: pressure_spec.class,
        exclusive_end: pressure_spec.exclusive_end,
        view,
    });
    views.insert(pressure_spec.action, view);

    let mut later = specs
        .iter()
        .filter(|row| !prior_assignments.contains_key(&row.action) && row.action != pressure.result)
        .collect::<Vec<_>>();
    later.sort_by_key(|row| (row.start, row.action));
    for row in later {
        active.retain(|home| home.exclusive_end > row.start);
        let view = choose(function, row, &active, physical)?;
        record(row, view, &active, &mut rosters);
        active.push(ActiveHome {
            value: RecursiveReloadCoexistingValue::Reload(row.action),
            class: row.class,
            exclusive_end: row.exclusive_end,
            view,
        });
        views.insert(row.action, view);
    }

    specs
        .iter()
        .map(|row| {
            let view = views.get(&row.action).copied().ok_or(
                RecursiveReloadValueHomeError::InvalidPriorOutcome {
                    function,
                    action: row.action,
                },
            )?;
            Ok(RecursiveReloadValueHomeAssignment {
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
            })
        })
        .collect()
}

fn choose(
    function: usize,
    spec: &ReloadSpec,
    active: &[ActiveHome],
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<omega_register_model::RegisterViewId, RecursiveReloadValueHomeError> {
    spec.candidates
        .iter()
        .copied()
        .find(|candidate| !homes::blocked(*candidate, active, physical))
        .ok_or(RecursiveReloadValueHomeError::ReloadPressure {
            function,
            action: spec.action,
        })
}

fn record(
    spec: &ReloadSpec,
    view: omega_register_model::RegisterViewId,
    active: &[ActiveHome],
    rosters: &mut BTreeMap<GeneralizedSpillActionId, BTreeSet<RecursiveReloadCoexistingHome>>,
) {
    for home in active {
        rosters
            .entry(spec.action)
            .or_default()
            .insert(RecursiveReloadCoexistingHome {
                value: home.value,
                class: home.class,
                view: home.view,
            });
        if let RecursiveReloadCoexistingValue::Reload(action) = home.value {
            rosters
                .entry(action)
                .or_default()
                .insert(RecursiveReloadCoexistingHome {
                    value: RecursiveReloadCoexistingValue::Reload(spec.action),
                    class: spec.class,
                    view,
                });
        }
    }
}

fn spec(
    function: usize,
    specs: &[ReloadSpec],
    action: GeneralizedSpillActionId,
) -> Result<&ReloadSpec, RecursiveReloadValueHomeError> {
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
