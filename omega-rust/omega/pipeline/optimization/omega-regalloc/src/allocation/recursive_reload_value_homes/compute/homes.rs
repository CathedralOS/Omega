//! Producer-local candidate, interval, and physical-overlap mechanics.

use omega_register_model::{RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::VirtualRegisterId;

use crate::{LiveRangePoint, RecursiveReloadValueHomeError};

pub(super) fn legality_row(
    function: usize,
    legality: &crate::FunctionAllocationLegality,
    register: VirtualRegisterId,
) -> Result<&crate::VirtualRegisterAllocationLegality, RecursiveReloadValueHomeError> {
    legality
        .virtual_registers
        .iter()
        .find(|row| row.virtual_register == register)
        .ok_or(RecursiveReloadValueHomeError::VirtualRegisterMismatch {
            function,
            register: register.0,
        })
}

pub(super) fn original_end(
    function: usize,
    row: &crate::VirtualRegisterAllocationLegality,
) -> Result<LiveRangePoint, RecursiveReloadValueHomeError> {
    let last = row
        .points
        .last()
        .ok_or(RecursiveReloadValueHomeError::NoLivePoints {
            function,
            register: row.virtual_register.0,
        })?;
    Ok(LiveRangePoint(last.point.0.checked_add(1).ok_or(
        RecursiveReloadValueHomeError::IntervalOverflow {
            function,
            register: row.virtual_register.0,
        },
    )?))
}

pub(super) fn reload_candidates(
    function: usize,
    row: &crate::VirtualRegisterAllocationLegality,
    block: omega_selected_instructions::SelectedBlockId,
    start: LiveRangePoint,
    exclusive_end: LiveRangePoint,
) -> Result<Vec<RegisterViewId>, RecursiveReloadValueHomeError> {
    let points = row
        .points
        .iter()
        .filter(|point| point.block == block && start <= point.point && point.point < exclusive_end)
        .collect::<Vec<_>>();
    if points.is_empty()
        || points.len()
            != usize::try_from(exclusive_end.0.saturating_sub(start.0)).unwrap_or(usize::MAX)
        || points.first().map(|point| point.point) != Some(start)
    {
        return Err(RecursiveReloadValueHomeError::NoCommonCandidate {
            function,
            register: row.virtual_register.0,
        });
    }
    let mut shared = points[0].candidates.clone();
    for point in &points[1..] {
        shared.retain(|candidate| point.candidates.binary_search(candidate).is_ok());
    }
    if shared.is_empty() {
        return Err(RecursiveReloadValueHomeError::NoCommonCandidate {
            function,
            register: row.virtual_register.0,
        });
    }
    Ok(shared)
}

pub(super) fn blocked(
    candidate: RegisterViewId,
    active: &[super::ActiveHome],
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    active
        .iter()
        .any(|home| overlaps(candidate, home.view, physical))
}

fn overlaps(
    left: RegisterViewId,
    right: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    match (lookup(left, physical), lookup(right, physical)) {
        (Some(left), Some(right)) => left
            .units
            .iter()
            .chain(&left.write_units)
            .any(|unit| right.units.contains(unit) || right.write_units.contains(unit)),
        _ => true,
    }
}

fn lookup(id: RegisterViewId, physical: &ValidatedPhysicalRegisterModel) -> Option<&RegisterView> {
    physical.model().views.iter().find(|view| view.id == id)
}
