//! Whole-plan admission fences that precede legalization construction and replay.

use super::model::LegalizationError;
use target_operations::{TargetOperation, TargetOperationPlan, TargetUnitOperation};

pub(super) fn reject_ranked_countdown(
    target: &TargetOperationPlan,
) -> Result<(), LegalizationError> {
    if let Some(function) = target
        .functions
        .iter()
        .find(|function| matches!(function.operation, TargetOperation::RankedU32Countdown(_)))
    {
        return Err(LegalizationError::RankedCountdownNotYetSelectable {
            machine: function.machine,
        });
    }
    Ok(())
}

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
                TargetUnitOperation::StructuralScalarFieldStore { psi_operation, .. }
                | TargetUnitOperation::StructuralScalarCall { psi_operation, .. } => {
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
