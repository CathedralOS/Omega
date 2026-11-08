//! Keep the two installation calls attached to their admitted source occurrence.
//! Ordinary ABI and borrowed-operand replay remains owned by scalar graph input.

use crate::installed_interrupts::PreparedInstallationSources;
use target_operations::{InstallationCallRole, TargetOperationPlan, TargetUnitOperation};

pub(crate) fn validate_target(
    target: &TargetOperationPlan,
    module: &terminal_psi::TerminalModule,
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
            calls,
            ..
        } = row
        else {
            continue;
        };
        let Some(scope) = source.scope(*psi_operation) else {
            return Err(fail(
                *psi_operation,
                "installation has no unique admitted Scope",
            ));
        };
        if source.psi() != target.psi
            || scope.boundary() != *boundary
            || scope.arguments() != arguments
            || calls[0].role != InstallationCallRole::Validate
            || calls[0].callee != scope.validator()
            || calls[1].role != InstallationCallRole::Work
            || calls[1].callee != scope.work()
        {
            return Err(fail(
                *psi_operation,
                "installation changed its admitted operands or selected helper calls",
            ));
        }
        if module
            .boundary_machine(*boundary)
            .and_then(|row| row.declaration.as_ref())
            .and_then(|row| row.execution.static_interrupt_operation())
            != Some(language_core::static_interrupts::StaticInterruptOperation::Install)
            || !source.boundary_calls(*psi_operation).iter().any(|row| {
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
