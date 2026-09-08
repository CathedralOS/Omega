//! Whole-plan admission fences that precede legalization construction and replay.

use super::model::LegalizationError;
use target_operations::{TargetOperation, TargetOperationPlan, TargetUnitOperation};

pub(super) fn reject_attached_unit_structural_scalar(
    target: &TargetOperationPlan,
) -> Result<(), LegalizationError> {
    for function in &target.functions {
        let TargetOperation::UnitBody(body) = &function.operation else {
            continue;
        };
        if let Some(operation) = body
            .operations
            .iter()
            .find_map(|operation| match operation {
                TargetUnitOperation::StructuralScalarFieldStore { psi_operation, .. } => {
                    Some(*psi_operation)
                }
                // Free shared-view calls are reconstructed by ordinary graph
                // admission. Attachment projections remain unsupported here.
                TargetUnitOperation::StructuralScalarCall { psi_operation, .. }
                    if function.attachment.is_some() =>
                {
                    Some(*psi_operation)
                }
                _ => None,
            })
        {
            return Err(
                LegalizationError::AttachedUnitStructuralScalarNotYetSelectable {
                    machine: function.machine,
                    operation,
                },
            );
        }
    }
    Ok(())
}
