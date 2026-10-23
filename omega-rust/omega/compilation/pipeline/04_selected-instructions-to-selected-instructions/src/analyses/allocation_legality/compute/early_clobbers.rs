//! Candidate derivation at early-definition points.

use register_model::{RegisterClass, RegisterViewId};

use super::{fixed_views, view_candidates::CandidateViews};
use crate::AllocationLegalityError;
use register_homes::VirtualEarlyClobberPointLegality;
use selected_instructions::{FunctionLiveRanges, VirtualLiveRange};

#[allow(clippy::too_many_arguments)]
pub(super) fn compute(
    function_index: usize,
    function: &FunctionLiveRanges,
    register: &VirtualLiveRange,
    class: &RegisterClass,
    available: &[RegisterViewId],
    views: &mut CandidateViews<'_>,
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
        let candidates = views.candidates_at(
            function_index,
            register,
            class,
            available,
            early.block,
            early.early_point,
            fixed,
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
            candidates: candidates.to_vec(),
        });
    }
    Ok(early_clobber_points)
}
