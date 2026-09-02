//! Optimizer module role: executable entrance. Scalar-result lowering: ABI setup, exact special forms, conditionals, then straight-line evaluation.

mod conditional_control;
mod conditional_route;
mod conditional_scalar;
mod expressions;
mod setup;
mod special_forms;
mod straight_line;
mod structural_call;

use super::cleanup::validate_scalar_cleanup_frontier;
use super::conditional_cleanup::{
    finite_boolean_cleanup_return_edges, shared_boolean_cleanup_return_edges,
    uniform_conditional_cleanup,
};
use super::shared::*;
use super::structural_layout::{
    direct_boolean_field_offset, direct_integer_field_offset, structural_shape,
};
use conditional_control::{
    lower_boolean_block, lower_boolean_conditional, lower_integer_conditional,
};
use conditional_scalar::{
    IntegerBinaryKind, WrappingShiftKind, lower_conditional_integer_binary,
    lower_conditional_scalar_operation, lower_exact_shift_left, lower_exact_shift_right,
    lower_wrapping_shift,
};
use expressions::*;
pub(in crate::lowering) use expressions::{scalar_parameter_location, scalar_shape};

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_scalar_function(
    function: &AbstractFunction,
    function_result: AbstractResult,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
) -> Result<TargetFunction, LoweringError> {
    let prepared =
        setup::prepare_scalar_lowering(function, function_result, target, structural_types)?;
    if let Some(lowered) = special_forms::lower_special_form(
        function,
        function_result,
        target,
        functions,
        structural_types,
        settlements,
        &prepared,
    )? {
        return Ok(lowered);
    }
    if let Some(lowered) = conditional_route::lower_conditional(
        function,
        function_result,
        target,
        functions,
        structural_types,
        &prepared,
    )? {
        return Ok(lowered);
    }
    straight_line::lower_straight_line(
        function,
        target,
        functions,
        structural_types,
        prepared.values,
        function_result,
        prepared.call_plan,
        prepared.target_structural_parameters,
    )
}
