use crate::installed_interrupts::PreparedInstallationSources;
use target_operations::{TargetOperationPlan, TargetUnitOperation};

pub(crate) fn validate_target(
    target: &TargetOperationPlan,
    source: &PreparedInstallationSources<'_>,
) -> Result<(), crate::LegalizationError> {
    let fail = |operation, reason| crate::LegalizationError::InstalledInterruptAssociation {
        operation: Some(operation),
        reason,
    };
    for row in target
        .functions
        .iter()
        .flat_map(|function| function.graph.operations())
    {
        let TargetUnitOperation::InstallStaticInterrupt {
            psi_operation,
            boundary,
            arguments,
            ..
        } = row
        else {
            continue;
        };
        if !source.boundary_calls(*psi_operation).iter().any(|row| {
            matches!(row,
                abstract_operations::AbstractOperation::BoundaryCall {
                    psi_operation: operation, boundary: declared, arguments: scalars,
                    structural_arguments, ..
                } if operation == psi_operation && declared == boundary && scalars.is_empty()
                    && structural_arguments.as_slice() == arguments)
        }) {
            return Err(fail(
                *psi_operation,
                "installation changed its authenticated source operation",
            ));
        }
    }
    Ok(())
}
