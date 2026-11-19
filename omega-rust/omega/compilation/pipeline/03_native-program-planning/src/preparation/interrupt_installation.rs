use crate::preparation::installed_interrupts::PreparedInstallationSources;
use target_operations::{TargetOperationPlan, TargetUnitOperation};

pub(crate) fn validate_target(
    target: &TargetOperationPlan,
    source: &PreparedInstallationSources<'_>,
) -> Result<(), crate::preparation::LegalizationError> {
    let fail =
        |operation, reason| crate::preparation::LegalizationError::InstalledInterruptAssociation {
            operation: Some(operation),
            reason,
        };
    for (machine, row) in target.functions.iter().flat_map(|function| {
        function
            .graph
            .operations()
            .map(move |row| (function.machine, row))
    }) {
        let TargetUnitOperation::InstallStaticInterrupt {
            psi_operation,
            boundary,
            arguments,
            ..
        } = row
        else {
            continue;
        };
        if !source
            .boundary_calls((machine, *psi_operation))
            .iter()
            .any(|row| {
                matches!(row,
                abstract_operations::AbstractOperation::BoundaryCall {
                    psi_operation: operation, boundary: declared, arguments: scalars,
                    structural_arguments, ..
                } if operation == psi_operation && declared == boundary && scalars.is_empty()
                    && structural_arguments.as_slice() == arguments)
            })
        {
            return Err(fail(
                *psi_operation,
                "installation changed its authenticated source operation",
            ));
        }
    }
    Ok(())
}
