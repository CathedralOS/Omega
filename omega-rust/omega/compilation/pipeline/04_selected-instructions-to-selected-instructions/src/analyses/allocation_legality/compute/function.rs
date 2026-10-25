//! Per-function and per-virtual-register legality assembly.

use register_model::{ValidatedPhysicalRegisterModel, ValidatedRegisterReservationProfile};

use super::{early_clobbers, fixed_views, live_points, view_candidates::CandidateViews};
use crate::{AllocationLegalityError, ValidatedAllocatorAvailability};
use register_homes::{FunctionAllocationLegality, VirtualRegisterAllocationLegality};
use selected_instructions::FunctionLiveRanges;

pub(crate) fn compute(
    function_index: usize,
    function: &FunctionLiveRanges,
    availability: &ValidatedAllocatorAvailability,
    physical: &ValidatedPhysicalRegisterModel,
    reservations: &ValidatedRegisterReservationProfile,
) -> Result<FunctionAllocationLegality, AllocationLegalityError> {
    let mut candidates = CandidateViews::new(function, physical, reservations);
    let max_early_reg = function
        .early_clobbers
        .iter()
        .map(|early| early.def_virtual_register.0 as usize)
        .max()
        .unwrap_or(0);
    let mut early_by_register: Vec<Vec<&selected_instructions::EarlyClobberConstraint>> =
        if function.early_clobbers.is_empty() {
            Vec::new()
        } else {
            let mut grouped = vec![Vec::new(); max_early_reg + 1];
            for early in &function.early_clobbers {
                grouped[early.def_virtual_register.0 as usize].push(early);
            }
            grouped
        };
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
            let spans = live_points::compute(
                function_index,
                register,
                class,
                available,
                entry_point,
                &mut candidates,
            )?;
            let early_slice = early_by_register
                .get(register.virtual_register.0 as usize)
                .map_or(&[][..], Vec::as_slice);
            let early_clobber_points = early_clobbers::compute(
                function_index,
                early_slice,
                register,
                class,
                available,
                &mut candidates,
            )?;
            let entry_transitions = fixed_views::entry_transitions(register);
            Ok(VirtualRegisterAllocationLegality {
                virtual_register: register.virtual_register,
                class: register.class,
                spans,
                early_clobber_points,
                entry_transitions,
            })
        })
        .collect::<Result<Vec<_>, AllocationLegalityError>>()?;
    let _ = &mut early_by_register;
    Ok(FunctionAllocationLegality {
        machine: function.machine,
        virtual_registers,
    })
}
