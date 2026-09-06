//! Optimizer module role: executable entrance. Unit-body assignment and ordered operation custody.
//!
//! This entrance establishes the unit scalar frame, walks source operations in
//! order, and delegates each operation to the exhaustive router. Scalar calls
//! and normalized foreign calls descend into separate custody owners.

mod dynamic;
mod dynamic_argument;
mod foreign_call;
mod installed_provider;
mod operation;
mod scalar_call;
mod scalar_transport;
pub(super) mod structural_scalar;
mod write_only_primitive_store;

use crate::assignment::shared::*;

pub(super) fn assign(
    function: &TargetFunction,
    operation: &TargetOperation,
    target: NativeTarget,
    native_callbacks: &[target_operations::TargetNativeCallbackArgument],
) -> Result<AssignedOperation, AssignmentError> {
    let TargetOperation::UnitBody(body) = operation else {
        unreachable!("Unit assignment receives a Unit body");
    };
    let mut assigned_scalar_homes = BTreeMap::new();
    let mut assigned_structural_homes = BTreeMap::new();
    let mut next_frame_home = scalar_call::unit_scalar_home_start(body, target)?;
    let mut operations = Vec::with_capacity(body.operations.len());
    for (operation_index, operation) in body.operations.iter().enumerate() {
        let native_callback = match operation {
            TargetUnitOperation::NormalizedForeignCall { psi_operation, .. } => native_callbacks
                .iter()
                .find(|callback| callback.terminal_operation == *psi_operation),
            _ => None,
        };
        let assigned = operation::assign(
            function.machine,
            function.attachment,
            body,
            operation,
            &body.operations[..operation_index],
            &operations,
            target,
            native_callback,
            &mut assigned_scalar_homes,
            &mut assigned_structural_homes,
            &mut next_frame_home,
        )?;
        operations.push(assigned);
    }

    Ok(AssignedOperation::UnitBody(AssignedUnitBody {
        structural_types: body.structural_types.clone(),
        call_plan: body.call_plan.clone(),
        scalar_parameters: body.scalar_parameters.clone(),
        parameters: body.parameters.clone(),
        operations,
    }))
}
