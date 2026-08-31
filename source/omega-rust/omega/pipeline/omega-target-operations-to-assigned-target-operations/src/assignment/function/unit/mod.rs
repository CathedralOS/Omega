//! Optimizer module role: executable entrance. Unit-body assignment and ordered operation custody.
//!
//! This entrance establishes the unit scalar frame, walks source operations in
//! order, and delegates each operation to the exhaustive router. Scalar calls
//! and normalized foreign calls descend into separate custody owners.

mod foreign_call;
mod operation;
mod scalar_call;

use crate::assignment::shared::*;

pub(super) fn assign(
    function: &TargetFunction,
    operation: &TargetOperation,
    target: NativeTarget,
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
            operation::assign(
                function.machine,
                operation,
                &body.operations[..operation_index],
                target,
                &mut assigned_scalar_homes,
                &mut next_scalar_home,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(AssignedOperation::UnitBody(AssignedUnitBody {
        structural_types: body.structural_types.clone(),
        call_plan: body.call_plan.clone(),
        parameters: body.parameters.clone(),
        operations,
    }))
}
