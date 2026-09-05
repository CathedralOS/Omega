//! Candidate derivation at early-definition points.

use register_model::{
    RegisterClass, RegisterViewId, ValidatedPhysicalRegisterModel,
    ValidatedRegisterReservationProfile,
};

use super::{fixed_views, view_candidates};
use crate::{
    AllocationLegalityError, FunctionLiveRanges, VirtualEarlyClobberPointLegality, VirtualLiveRange,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn compute(
    function_index: usize,
    function: &FunctionLiveRanges,
    register: &VirtualLiveRange,
    class: &RegisterClass,
    available: &[RegisterViewId],
    physical: &ValidatedPhysicalRegisterModel,
    reservations: &ValidatedRegisterReservationProfile,
) -> Result<Vec<VirtualEarlyClobberPointLegality>, AllocationLegalityError> {
    let mut early_clobber_points = Vec::new();
    for early in function
        .early_clobbers
        .iter()
        .filter(|early| early.def_virtual_register == register.virtual_register)
    {
        if early.def_class != register.class {
            return Err(AllocationLegalityError::UnknownClass {
                function: function_index,
                register: register.virtual_register.0,
                class: early.def_class.0,
            });
        }
        let fixed = fixed_views::for_early_clobber(function_index, register, early)?;
        let mut candidates = view_candidates::unconstrained(
            class,
            available,
            early.block,
            early.early_point,
            function,
            physical,
            reservations,
        );
        view_candidates::restrict_to_fixed(
            function_index,
            register,
            early.block,
            early.early_point,
            fixed,
            function,
            physical,
            reservations,
            &mut candidates,
        )?;
        if candidates.is_empty() {
            return Err(AllocationLegalityError::NoCandidateViews {
                function: function_index,
                register: register.virtual_register.0,
                block: early.block.0,
                point: early.early_point.0,
            });
        }
        early_clobber_points.push(VirtualEarlyClobberPointLegality {
            block: early.block,
            position: early.position,
            instruction: early.instruction,
            operand: early.def_operand,
            point: early.early_point,
            candidates,
        });
    }
    Ok(early_clobber_points)
}
