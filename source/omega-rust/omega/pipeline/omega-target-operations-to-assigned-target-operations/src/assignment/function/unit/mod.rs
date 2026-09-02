//! Optimizer module role: executable entrance. Unit-body assignment and ordered operation custody.
//!
//! This entrance establishes the unit scalar frame, walks source operations in
//! order, and delegates each operation to the exhaustive router. Scalar calls
//! and normalized foreign calls descend into separate custody owners.

mod dynamic_argument;
mod dynamic_scalar;
mod foreign_call;
mod installed_provider;
mod operation;
mod scalar_call;
pub(super) mod structural_scalar;

use crate::assignment::shared::*;

pub(super) fn assign(
    function: &TargetFunction,
    operation: &TargetOperation,
    target: NativeTarget,
    native_callbacks: &[omega_target_operations::TargetNativeCallbackArgument],
) -> Result<AssignedOperation, AssignmentError> {
    let TargetOperation::UnitBody(body) = operation else {
        unreachable!("Unit assignment receives a Unit body");
    };
    let mut assigned_scalar_homes = BTreeMap::new();
    let mut next_scalar_home = scalar_call::unit_scalar_home_start(body, target)?;
    let operations = body
        .operations
        .iter()
        .enumerate()
        .map(|(operation_index, operation)| {
            let native_callback = match operation {
                TargetUnitOperation::NormalizedForeignCall { psi_operation, .. } => {
                    native_callbacks
                        .iter()
                        .find(|callback| callback.terminal_operation == *psi_operation)
                }
                _ => None,
            };
            operation::assign(
                function.machine,
                function.attachment,
                body,
                operation,
                &body.operations[..operation_index],
                target,
                native_callback,
                &mut assigned_scalar_homes,
                &mut next_scalar_home,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(AssignedOperation::UnitBody(AssignedUnitBody {
        structural_types: body.structural_types.clone(),
        call_plan: body.call_plan.clone(),
        scalar_parameters: body.scalar_parameters.clone(),
        parameters: body.parameters.clone(),
        operations,
    }))
}
