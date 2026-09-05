//! Replay-local candidate, interference, and occupant mechanics.

use std::collections::{BTreeMap, BTreeSet};

use register_model::{
    RegisterClassId, RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel,
};
use selected_instructions::VirtualRegisterId;

use crate::{
    GeneralizedReloadCoexistingHome, GeneralizedReloadCoexistingValue,
    GeneralizedReloadValueHomeError, GeneralizedSpillActionId, LiveRangePoint, VirtualInterference,
};

use super::{Occupant, ReplaySpec};

pub(super) fn remove_spilled(
    function: usize,
    spec: &ReplaySpec,
    occupants: &mut Vec<Occupant>,
) -> Result<(), GeneralizedReloadValueHomeError> {
    let found = occupants.iter().filter(|occupant| {
        occupant.value == GeneralizedReloadCoexistingValue::Original(spec.victim)
            && occupant.view == spec.victim_view
    });
    if found.count() != 1 {
        return Err(GeneralizedReloadValueHomeError::PrefixMismatch { function });
    }
    occupants.retain(|occupant| {
        occupant.value != GeneralizedReloadCoexistingValue::Original(spec.victim)
    });
    Ok(())
}

pub(super) fn retain_pair(
    new_value: GeneralizedReloadCoexistingValue,
    new_class: RegisterClassId,
    new_view: RegisterViewId,
    occupants: &[Occupant],
    rosters: &mut BTreeMap<GeneralizedSpillActionId, BTreeSet<GeneralizedReloadCoexistingHome>>,
) {
    for occupant in occupants {
        match new_value {
            GeneralizedReloadCoexistingValue::Reload(action) => {
                rosters
                    .entry(action)
                    .or_default()
                    .insert(GeneralizedReloadCoexistingHome {
                        value: occupant.value,
                        class: occupant.class,
                        view: occupant.view,
                    });
            }
            GeneralizedReloadCoexistingValue::Original(_) => {}
        }
        if let GeneralizedReloadCoexistingValue::Reload(action) = occupant.value {
            rosters
                .entry(action)
                .or_default()
                .insert(GeneralizedReloadCoexistingHome {
                    value: new_value,
                    class: new_class,
                    view: new_view,
                });
        }
    }
}

pub(super) fn find_legality(
    function: usize,
    legality: &crate::FunctionAllocationLegality,
    register: VirtualRegisterId,
) -> Result<&crate::VirtualRegisterAllocationLegality, GeneralizedReloadValueHomeError> {
    legality
        .virtual_registers
        .iter()
        .find(|row| row.virtual_register == register)
        .ok_or(GeneralizedReloadValueHomeError::VirtualRegisterMismatch {
            function,
            register: register.0,
        })
}

pub(super) fn original_interval(
    function: usize,
    row: &crate::VirtualRegisterAllocationLegality,
) -> Result<(LiveRangePoint, LiveRangePoint), GeneralizedReloadValueHomeError> {
    let Some(first) = row.points.first() else {
        return Err(GeneralizedReloadValueHomeError::NoLivePoints {
            function,
            register: row.virtual_register.0,
        });
    };
    let end = row.points.last().expect("first point established").point.0;
    Ok((
        first.point,
        LiveRangePoint(end.checked_add(1).ok_or(
            GeneralizedReloadValueHomeError::IntervalOverflow {
                function,
                register: row.virtual_register.0,
            },
        )?),
    ))
}

pub(super) fn original_domain(
    function: usize,
    row: &crate::VirtualRegisterAllocationLegality,
) -> Result<Vec<RegisterViewId>, GeneralizedReloadValueHomeError> {
    let Some(first) = row.points.first() else {
        return Err(GeneralizedReloadValueHomeError::NoLivePoints {
            function,
            register: row.virtual_register.0,
        });
    };
    let mut domain = first.candidates.clone();
    row.points
        .iter()
        .skip(1)
        .for_each(|point| domain.retain(|view| point.candidates.binary_search(view).is_ok()));
    nonempty(function, row.virtual_register, domain)
}

pub(super) fn reload_domain(
    function: usize,
    row: &crate::VirtualRegisterAllocationLegality,
    block: selected_instructions::SelectedBlockId,
    start: LiveRangePoint,
    exclusive_end: LiveRangePoint,
) -> Result<Vec<RegisterViewId>, GeneralizedReloadValueHomeError> {
    let mut by_point = row
        .points
        .iter()
        .filter(|point| point.block == block && (start..exclusive_end).contains(&point.point))
        .map(|point| (point.point, &point.candidates))
        .collect::<BTreeMap<_, _>>();
    let mut domain = None::<Vec<RegisterViewId>>;
    for raw in start.0..exclusive_end.0 {
        let point = LiveRangePoint(raw);
        let Some(candidates) = by_point.remove(&point) else {
            return Err(GeneralizedReloadValueHomeError::NoCommonCandidate {
                function,
                register: row.virtual_register.0,
            });
        };
        match &mut domain {
            None => domain = Some(candidates.clone()),
            Some(shared) => shared.retain(|view| candidates.binary_search(view).is_ok()),
        }
    }
    nonempty(function, row.virtual_register, domain.unwrap_or_default())
}

fn nonempty(
    function: usize,
    register: VirtualRegisterId,
    domain: Vec<RegisterViewId>,
) -> Result<Vec<RegisterViewId>, GeneralizedReloadValueHomeError> {
    if domain.is_empty() {
        Err(GeneralizedReloadValueHomeError::NoCommonCandidate {
            function,
            register: register.0,
        })
    } else {
        Ok(domain)
    }
}

pub(super) fn every_reload_view_blocked(
    domain: &[RegisterViewId],
    occupants: &[Occupant],
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    domain
        .iter()
        .all(|view| reload_conflict(*view, occupants, physical))
}

pub(super) fn reload_blocking_roster(
    domain: &[RegisterViewId],
    occupants: &[Occupant],
    physical: &ValidatedPhysicalRegisterModel,
) -> Vec<GeneralizedReloadCoexistingHome> {
    occupants
        .iter()
        .filter(|occupant| {
            domain
                .iter()
                .any(|view| footprints_overlap(*view, occupant.view, physical))
        })
        .map(|occupant| GeneralizedReloadCoexistingHome {
            value: occupant.value,
            class: occupant.class,
            view: occupant.view,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn every_original_view_blocked(
    register: VirtualRegisterId,
    domain: &[RegisterViewId],
    occupants: &[Occupant],
    interference: &[VirtualInterference],
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    domain
        .iter()
        .all(|view| original_conflict(register, *view, occupants, interference, physical))
}

pub(super) fn original_conflict(
    register: VirtualRegisterId,
    view: RegisterViewId,
    occupants: &[Occupant],
    interference: &[VirtualInterference],
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    occupants.iter().any(|occupant| {
        let semantically_overlapping = match occupant.value {
            GeneralizedReloadCoexistingValue::Reload(_) => true,
            GeneralizedReloadCoexistingValue::Original(other) => {
                let row = if register < other {
                    VirtualInterference {
                        lower: register,
                        higher: other,
                    }
                } else {
                    VirtualInterference {
                        lower: other,
                        higher: register,
                    }
                };
                interference.binary_search(&row).is_ok()
            }
        };
        semantically_overlapping && footprints_overlap(view, occupant.view, physical)
    })
}

pub(super) fn reload_conflict(
    view: RegisterViewId,
    occupants: &[Occupant],
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    occupants
        .iter()
        .any(|occupant| footprints_overlap(view, occupant.view, physical))
}

fn footprints_overlap(
    left: RegisterViewId,
    right: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    let left = physical.model().views.iter().find(|view| view.id == left);
    let right = physical.model().views.iter().find(|view| view.id == right);
    match (left, right) {
        (Some(left), Some(right)) => overlap(left, right),
        _ => true,
    }
}

fn overlap(left: &RegisterView, right: &RegisterView) -> bool {
    left.units
        .iter()
        .chain(&left.write_units)
        .any(|unit| right.units.contains(unit) || right.write_units.contains(unit))
}
