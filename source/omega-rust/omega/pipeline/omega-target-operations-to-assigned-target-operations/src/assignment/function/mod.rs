//! Optimizer module role: executable entrance. Per-function entrance: route one exhaustive operation carrier, then retain
//! the function's identity, attachment, and provenance around that result.

mod boundary;
mod cleanup;
mod operation_routes;
mod ranked_countdown;
mod scalar;
mod structural;
mod structural_parameter;
mod unit;

use super::shared::*;

pub(super) fn assign_function(
    function: &TargetFunction,
    target: NativeTarget,
    native_callbacks: &[omega_target_operations::TargetNativeCallbackArgument],
) -> Result<AssignedFunction, AssignmentError> {
    let operation = operation_routes::assign_operation(function, target, native_callbacks)?;
    Ok(AssignedFunction {
        machine: function.machine,
        attachment: function.attachment,
        fixed_integer_scalar_abi: function.fixed_integer_scalar_abi.clone(),
        provenance: function.provenance.clone(),
        operation,
    })
}
