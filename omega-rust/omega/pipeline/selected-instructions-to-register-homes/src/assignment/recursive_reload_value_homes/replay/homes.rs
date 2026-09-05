//! Replay-local interval, candidate-domain, and register-unit mechanics.

use register_model::{RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::VirtualRegisterId;

use crate::{LiveRangePoint, RecursiveReloadValueHomeError};

pub(super) fn find_legality(
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

pub(super) fn original_exclusive_end(
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
    let end =
        last.point
            .0
            .checked_add(1)
            .ok_or(RecursiveReloadValueHomeError::IntervalOverflow {
                function,
                register: row.virtual_register.0,
            })?;
    Ok(LiveRangePoint(end))
}

pub(super) fn domain(
    function: usize,
    row: &crate::VirtualRegisterAllocationLegality,
    block: selected_instructions::SelectedBlockId,
    start: LiveRangePoint,
    exclusive_end: LiveRangePoint,
) -> Result<Vec<RegisterViewId>, RecursiveReloadValueHomeError> {
    let expected = usize::try_from(exclusive_end.0.saturating_sub(start.0)).unwrap_or(usize::MAX);
    let mut matching = row.points.iter().filter(|point| {
        point.block == block && point.point >= start && point.point < exclusive_end
    });
    let first = matching
        .next()
        .ok_or(RecursiveReloadValueHomeError::NoCommonCandidate {
            function,
            register: row.virtual_register.0,
        })?;
    let mut candidates = first.candidates.clone();
    let mut count = 1_usize;
    for point in matching {
        count += 1;
        candidates.retain(|candidate| point.candidates.contains(candidate));
    }
    if first.point != start || count != expected || candidates.is_empty() {
        return Err(RecursiveReloadValueHomeError::NoCommonCandidate {
            function,
            register: row.virtual_register.0,
        });
    }
    Ok(candidates)
}

pub(super) fn conflicts(
    candidate: RegisterViewId,
    occupants: &[super::Occupant],
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    occupants
        .iter()
        .any(|occupant| overlap(candidate, occupant.view, physical))
}

fn overlap(
    left: RegisterViewId,
    right: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    let left = lookup(left, physical);
    let right = lookup(right, physical);
    match (left, right) {
        (Some(left), Some(right)) => {
            left.units
                .iter()
                .any(|unit| right.units.contains(unit) || right.write_units.contains(unit))
                || left
                    .write_units
                    .iter()
                    .any(|unit| right.units.contains(unit) || right.write_units.contains(unit))
        }
        _ => true,
    }
}

fn lookup(id: RegisterViewId, physical: &ValidatedPhysicalRegisterModel) -> Option<&RegisterView> {
    physical.model().views.iter().find(|view| view.id == id)
}
