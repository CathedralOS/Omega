//! Per-function entrance: route one exhaustive operation carrier, then retain
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
) -> Result<AssignedFunction, AssignmentError> {
    let operation = operation_routes::assign_operation(function, target)?;
    Ok(AssignedFunction {
        machine: function.machine,
        attachment: function.attachment,
        provenance: function.provenance.clone(),
        operation,
    })
}
