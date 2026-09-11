//! Optimizer module role: executable entrance. Independent abstract-to-target translation validation entrance.
//!
//! This module owns whole-plan root/roster custody, then descends into exact
//! semantic-family validators. Its receipt explicitly lists covered function
//! families and does not claim coverage for absent rows.

mod catalog;
mod model;
pub(crate) mod straight_line_byte_sequence_literal_unit_return;
pub(crate) mod straight_line_ieee_float_literal_sequence_unit_return;
pub(crate) mod straight_line_ieee_float_literal_unit_return;
pub(crate) mod straight_line_integer_ieee_float_literal_sequence_unit_return;
pub(crate) mod straight_line_integer_literal_sequence_unit_return;
pub(crate) mod straight_line_integer_literal_unit_return;
pub(crate) mod straight_line_nearest_ieee_float_fused_multiply_add_unit_return;
pub(crate) mod straight_line_port_write_unit_return;
pub(crate) mod straight_line_trivial_affine_local_unit_return;
pub(crate) mod straight_line_unit_call_return;
pub(crate) mod straight_line_unit_return;
pub(crate) mod structural_call_return;
mod structural_shapes;
mod structural_signatures;
mod unit_continuation_scalars;
mod unit_continuations;
mod whole_plan;

pub use model::*;
pub use structural_call_return::{
    StructuralCallReturnCallerTranslationReceipt,
    StructuralCallReturnProjectedQualificationReceipt,
    StructuralCallReturnProjectedQualificationValidationError, StructuralCallReturnRosterLocation,
    StructuralParameterReturnCalleeTranslationReceipt,
};
pub use whole_plan::validate_abstract_to_target_translation_with_ieee_float_fma_settlements;

/// Validate the evidence-free translation surface. Source containing an FMA
/// remains fail-closed because this compatibility entrance supplies no
/// occurrence settlement custody.
pub fn validate_abstract_to_target_translation(
    source: &abstract_operations::AbstractOperationPlan,
    expected_target: target::NativeTarget,
    target: &target_operations::TargetOperationPlan,
) -> Result<AbstractToTargetTranslationValidationReceipt, AbstractToTargetTranslationValidationError>
{
    whole_plan::validate_abstract_to_target_translation(source, expected_target, target)
}
