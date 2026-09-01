//! Producer-local home-domain, eviction, and interference mechanics.

use std::collections::{BTreeMap, BTreeSet};

use omega_register_model::{
    RegisterClassId, RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel,
};
use omega_selected_instructions::VirtualRegisterId;

use crate::{
    GeneralizedReloadCoexistingHome, GeneralizedReloadCoexistingValue,
    GeneralizedReloadValueHomeError, GeneralizedSpillActionId, LiveRangePoint, VirtualInterference,
};

use super::{ActiveHome, ReloadSpec};

pub(super) fn evict(
    function: usize,
    spec: &ReloadSpec,
    active: &mut Vec<ActiveHome>,
) -> Result<(), GeneralizedReloadValueHomeError> {
    let matches = active
        .iter()
        .filter(|home| {
            home.value == GeneralizedReloadCoexistingValue::Original(spec.victim)
                && home.view == spec.victim_view
        })
        .count();
    if matches != 1 {
        return Err(GeneralizedReloadValueHomeError::PrefixMismatch { function });
    }
    active.retain(|home| home.value != GeneralizedReloadCoexistingValue::Original(spec.victim));
    Ok(())
}

pub(super) fn record_coexistence(
    value: GeneralizedReloadCoexistingValue,
    class: RegisterClassId,
    view: RegisterViewId,
    active: &[ActiveHome],
    coexist: &mut BTreeMap<GeneralizedSpillActionId, BTreeSet<GeneralizedReloadCoexistingHome>>,
) {
    for home in active {
        if let GeneralizedReloadCoexistingValue::Reload(action) = value {
            coexist
                .entry(action)
                .or_default()
                .insert(GeneralizedReloadCoexistingHome {
                    value: home.value,
                    class: home.class,
                    view: home.view,
                });
        }
        if let GeneralizedReloadCoexistingValue::Reload(action) = home.value {
            coexist
                .entry(action)
                .or_default()
                .insert(GeneralizedReloadCoexistingHome { value, class, view });
        }
    }
}

pub(super) fn legality_row<'a>(
    function: usize,
    legality: &'a crate::FunctionAllocationLegality,
    register: VirtualRegisterId,
) -> Result<&'a crate::VirtualRegisterAllocationLegality, GeneralizedReloadValueHomeError> {
    legality
        .virtual_registers
        .iter()
        .find(|row| row.virtual_register == register)
        .ok_or(GeneralizedReloadValueHomeError::VirtualRegisterMismatch {
            function,
            register: register.0,
        })
}

pub(super) fn interval(
    function: usize,
    row: &crate::VirtualRegisterAllocationLegality,
) -> Result<(LiveRangePoint, LiveRangePoint), GeneralizedReloadValueHomeError> {
    let first = row
        .points
        .first()
        .ok_or(GeneralizedReloadValueHomeError::NoLivePoints {
            function,
            register: row.virtual_register.0,
        })?;
    let last = row.points.last().expect("nonempty legality points");
    Ok((
        first.point,
        LiveRangePoint(last.point.0.checked_add(1).ok_or(
            GeneralizedReloadValueHomeError::IntervalOverflow {
                function,
                register: row.virtual_register.0,
            },
        )?),
    ))
}

pub(super) fn original_candidates(
    function: usize,
    row: &crate::VirtualRegisterAllocationLegality,
) -> Result<Vec<RegisterViewId>, GeneralizedReloadValueHomeError> {
    let mut points = row.points.iter();
    let first = points
        .next()
        .ok_or(GeneralizedReloadValueHomeError::NoLivePoints {
            function,
            register: row.virtual_register.0,
        })?;
    let mut shared = first.candidates.clone();
    for point in points {
        shared.retain(|candidate| point.candidates.binary_search(candidate).is_ok());
    }
    if shared.is_empty() {
        return Err(GeneralizedReloadValueHomeError::NoCommonCandidate {
            function,
            register: row.virtual_register.0,
        });
    }
    Ok(shared)
}

pub(super) fn reload_candidates(
    function: usize,
    row: &crate::VirtualRegisterAllocationLegality,
    block: omega_selected_instructions::SelectedBlockId,
    start: LiveRangePoint,
    exclusive_end: LiveRangePoint,
) -> Result<Vec<RegisterViewId>, GeneralizedReloadValueHomeError> {
    let points = row
        .points
        .iter()
        .filter(|point| point.block == block && start <= point.point && point.point < exclusive_end)
        .collect::<Vec<_>>();
    if points.len() != usize::try_from(exclusive_end.0 - start.0).unwrap_or(usize::MAX)
        || points.first().map(|point| point.point) != Some(start)
    {
        return Err(GeneralizedReloadValueHomeError::NoCommonCandidate {
            function,
            register: row.virtual_register.0,
        });
    }
    let mut shared = points[0].candidates.clone();
    for point in &points[1..] {
        shared.retain(|candidate| point.candidates.binary_search(candidate).is_ok());
    }
    if shared.is_empty() {
        return Err(GeneralizedReloadValueHomeError::NoCommonCandidate {
            function,
            register: row.virtual_register.0,
        });
    }
    Ok(shared)
}

pub(super) fn all_blocked(
    candidates: &[RegisterViewId],
    active: &[ActiveHome],
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    candidates
        .iter()
        .all(|candidate| blocked_reload(*candidate, active, physical))
}

pub(super) fn reload_blockers(
    candidates: &[RegisterViewId],
    active: &[ActiveHome],
    physical: &ValidatedPhysicalRegisterModel,
) -> Vec<GeneralizedReloadCoexistingHome> {
    active
        .iter()
        .filter(|home| {
            candidates
                .iter()
                .any(|candidate| views_overlap(*candidate, home.view, physical))
        })
        .map(|home| GeneralizedReloadCoexistingHome {
            value: home.value,
            class: home.class,
            view: home.view,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn all_original_blocked(
    register: VirtualRegisterId,
    candidates: &[RegisterViewId],
    active: &[ActiveHome],
    interference: &[VirtualInterference],
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    candidates
        .iter()
        .all(|candidate| blocked_original(register, *candidate, active, interference, physical))
}

pub(super) fn blocked_original(
    register: VirtualRegisterId,
    candidate: RegisterViewId,
    active: &[ActiveHome],
    interference: &[VirtualInterference],
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    active.iter().any(|home| {
        let overlaps = match home.value {
            GeneralizedReloadCoexistingValue::Original(other) => {
                let (lower, higher) = if register < other {
                    (register, other)
                } else {
                    (other, register)
                };
                interference
                    .binary_search(&VirtualInterference { lower, higher })
                    .is_ok()
            }
            GeneralizedReloadCoexistingValue::Reload(_) => true,
        };
        overlaps && views_overlap(candidate, home.view, physical)
    })
}

pub(super) fn blocked_reload(
    candidate: RegisterViewId,
    active: &[ActiveHome],
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    active
        .iter()
        .any(|home| views_overlap(candidate, home.view, physical))
}

fn views_overlap(
    left: RegisterViewId,
    right: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    match (lookup_view(left, physical), lookup_view(right, physical)) {
        (Some(left), Some(right)) => left
            .units
            .iter()
            .chain(&left.write_units)
            .any(|unit| right.units.contains(unit) || right.write_units.contains(unit)),
        _ => true,
    }
}

fn lookup_view(
    id: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> Option<&RegisterView> {
    physical.model().views.iter().find(|view| view.id == id)
}
