//! Physical-view availability, reservation, and fixed-view filtering.

use omega_register_model::{
    RegisterClass, RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel,
    ValidatedRegisterReservationProfile,
};
use omega_selected_instructions::SelectedBlockId;

use crate::{AllocationLegalityError, FunctionLiveRanges, LiveRangePoint, VirtualLiveRange};

#[allow(clippy::too_many_arguments)]
pub(super) fn unconstrained(
    class: &RegisterClass,
    available: &[RegisterViewId],
    block: SelectedBlockId,
    point: LiveRangePoint,
    function: &FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    reservations: &ValidatedRegisterReservationProfile,
) -> Vec<RegisterViewId> {
    class
        .views
        .iter()
        .copied()
        .filter(|view_id| available.binary_search(view_id).is_ok())
        .filter(|view_id| {
            physical
                .model()
                .views
                .get(usize::from(view_id.0))
                .is_some_and(|view| {
                    view.id == *view_id
                        && view.allocatable
                        && !conflicts(view, block, point, function, reservations)
                })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn restrict_to_fixed(
    function_index: usize,
    register: &VirtualLiveRange,
    block: SelectedBlockId,
    point: LiveRangePoint,
    fixed: Option<RegisterViewId>,
    function: &FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    reservations: &ValidatedRegisterReservationProfile,
    candidates: &mut Vec<RegisterViewId>,
) -> Result<(), AllocationLegalityError> {
    let Some(fixed) = fixed else {
        return Ok(());
    };
    let Some(view) = physical.model().views.get(usize::from(fixed.0)) else {
        return Err(AllocationLegalityError::UnknownFixedView {
            function: function_index,
            register: register.virtual_register.0,
            view: fixed.0,
        });
    };
    if view.id != fixed || view.class != register.class {
        return Err(AllocationLegalityError::UnknownFixedView {
            function: function_index,
            register: register.virtual_register.0,
            view: fixed.0,
        });
    }
    if conflicts(view, block, point, function, reservations) {
        return Err(AllocationLegalityError::IllegalFixedView {
            function: function_index,
            register: register.virtual_register.0,
            view: fixed.0,
        });
    }
    candidates.clear();
    candidates.push(fixed);
    Ok(())
}

fn conflicts(
    view: &RegisterView,
    block: SelectedBlockId,
    point: LiveRangePoint,
    function: &FunctionLiveRanges,
    reservations: &ValidatedRegisterReservationProfile,
) -> bool {
    view.units.iter().chain(&view.write_units).any(|unit| {
        reservations.reserved_units().binary_search(unit).is_ok()
            || function
                .architectural_units
                .iter()
                .find(|row| row.unit == *unit)
                .is_some_and(|row| {
                    row.fragments.iter().any(|fragment| {
                        fragment.block == block && fragment.start <= point && point < fragment.end
                    }) || row
                        .actions
                        .iter()
                        .any(|action| action.block == block && action.point == point)
                })
    })
}
