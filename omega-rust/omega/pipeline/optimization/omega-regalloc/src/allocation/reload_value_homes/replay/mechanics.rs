//! Low-level interference and work-accounting mechanics for independent replay.

use omega_optimization_core::OptimizationWorkUsage;
use omega_register_model::{RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::VirtualRegisterId;

use crate::{FunctionReloadValueHomes, ReloadValueHomeError, VirtualInterference};

pub(super) fn contains_interference(
    left: VirtualRegisterId,
    right: VirtualRegisterId,
    interference: &[VirtualInterference],
) -> bool {
    let row = if left < right {
        VirtualInterference {
            lower: left,
            higher: right,
        }
    } else {
        VirtualInterference {
            lower: right,
            higher: left,
        }
    };
    interference.binary_search(&row).is_ok()
}

pub(super) fn views_overlap(
    left: RegisterViewId,
    right: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    match (lookup_view(left, physical), lookup_view(right, physical)) {
        (Some(left), Some(right)) => footprints_overlap(left, right),
        _ => true,
    }
}

fn lookup_view(
    id: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> Option<&RegisterView> {
    physical.model().views.iter().find(|view| view.id == id)
}

fn footprints_overlap(left: &RegisterView, right: &RegisterView) -> bool {
    left.units
        .iter()
        .chain(&left.write_units)
        .any(|unit| right.units.contains(unit) || right.write_units.contains(unit))
}

pub(super) fn reconstruct_usage(
    functions: &[FunctionReloadValueHomes],
) -> Result<OptimizationWorkUsage, ReloadValueHomeError> {
    let mut assignments = 0_u64;
    let mut candidates = 0_u64;
    let mut coexisting = 0_u64;
    for assignment in functions
        .iter()
        .filter_map(|function| function.assignment.as_ref())
    {
        assignments = add(assignments, 1)?;
        candidates = add(candidates, to_u64(assignment.candidates.len())?)?;
        coexisting = add(coexisting, to_u64(assignment.coexisting_homes.len())?)?;
    }
    let validation_steps = add(
        add(candidates, coexisting)?,
        assignments
            .checked_mul(4)
            .ok_or(ReloadValueHomeError::WorkOverflow)?,
    )?;
    let functions = to_u64(functions.len())?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: functions,
        candidates,
        validation_steps,
        commits: assignments,
        iterations: functions,
    })
}

fn add(left: u64, right: u64) -> Result<u64, ReloadValueHomeError> {
    left.checked_add(right)
        .ok_or(ReloadValueHomeError::WorkOverflow)
}

fn to_u64(value: usize) -> Result<u64, ReloadValueHomeError> {
    u64::try_from(value).map_err(|_| ReloadValueHomeError::WorkOverflow)
}
