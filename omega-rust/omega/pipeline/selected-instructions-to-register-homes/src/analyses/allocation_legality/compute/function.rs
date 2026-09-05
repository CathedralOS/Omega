//! Per-function and per-virtual-register legality assembly.

use register_model::{ValidatedPhysicalRegisterModel, ValidatedRegisterReservationProfile};

use super::{early_clobbers, fixed_views, live_points};
use crate::{
    AllocationLegalityError, FunctionAllocationLegality, FunctionLiveRanges,
    ValidatedAllocatorAvailability, VirtualRegisterAllocationLegality,
};

pub(super) fn compute(
    function_index: usize,
    function: &FunctionLiveRanges,
    availability: &ValidatedAllocatorAvailability,
    physical: &ValidatedPhysicalRegisterModel,
    reservations: &ValidatedRegisterReservationProfile,
) -> Result<FunctionAllocationLegality, AllocationLegalityError> {
    let virtual_registers = function
        .virtual_registers
        .iter()
        .map(|register| {
            let class = physical
                .model()
                .classes
                .get(usize::from(register.class.0))
                .filter(|class| class.id == register.class)
                .ok_or(AllocationLegalityError::UnknownClass {
                    function: function_index,
                    register: register.virtual_register.0,
                    class: register.class.0,
                })?;
            let available = availability.unconstrained_views(register.class).ok_or(
                AllocationLegalityError::UnknownClass {
                    function: function_index,
                    register: register.virtual_register.0,
                    class: register.class.0,
                },
            )?;
            let entry_point = register
                .fragments
                .first()
                .map(|fragment| (fragment.block, fragment.start));
            let points = live_points::compute(
                function_index,
                function,
                register,
                class,
                available,
                entry_point,
                physical,
                reservations,
            )?;
            let early_clobber_points = early_clobbers::compute(
                function_index,
                function,
                register,
                class,
                available,
                physical,
                reservations,
            )?;
            let entry_transitions = fixed_views::entry_transitions(register);
            Ok(VirtualRegisterAllocationLegality {
                virtual_register: register.virtual_register,
                class: register.class,
                points,
                early_clobber_points,
                entry_transitions,
            })
        })
        .collect::<Result<Vec<_>, AllocationLegalityError>>()?;
    Ok(FunctionAllocationLegality {
        machine: function.machine,
        virtual_registers,
    })
}
