//! Whole-plan admission fences that precede legalization construction and replay.

use super::model::LegalizationError;
use target_operations::{TargetOperationPlan, TargetUnitOperation};

pub(super) fn reject_attached_unit_structural_scalar(
    target: &TargetOperationPlan,
) -> Result<(), LegalizationError> {
    for function in &target.functions {
        let forbidden = |operation: &TargetUnitOperation| match operation {
            // Exact shared receiver signatures are reconstructed by ordinary
            // graph admission. Other attachment forms retain their fence.
            TargetUnitOperation::StructuralScalarCall { psi_operation, .. }
                if function.attachment.is_some_and(|attachment| {
                    !function
                        .graph
                        .parameters
                        .iter()
                        .chain(
                            function
                                .mixed_structural_scalar_abi
                                .iter()
                                .flat_map(|abi| &abi.structural_parameters),
                        )
                        .any(|parameter| {
                            parameter.structural_type == attachment
                                && parameter.access == terminal_psi::StructuralAccess::SharedBorrow
                        })
                }) =>
            {
                Some(*psi_operation)
            }
            _ => None,
        };
        let operation = function
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(forbidden);
        if let Some(operation) = operation {
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
