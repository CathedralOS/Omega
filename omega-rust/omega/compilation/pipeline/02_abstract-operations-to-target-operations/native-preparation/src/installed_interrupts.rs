//! Join the target program to its separately admitted interrupt installations.

use provider_admission::AdmittedInstalledInterruptCohort;
use provider_admission::installed_interrupt_helpers::{PreparedInstalledInterruptHelpers, Rows};
use semantic_vocabulary::OperationId;
use terminal_psi::TerminalModule;

pub(crate) struct PreparedInstallationSources<'a> {
    operations: Rows<&'a abstract_operations::AbstractOperation>,
}
impl<'a> PreparedInstallationSources<'a> {
    fn new(source: &'a abstract_operations::AbstractOperationPlan) -> Self {
        Self {
            operations: Rows::new(source.operations().filter_map(|row| {
                if let abstract_operations::AbstractOperation::BoundaryCall {
                    psi_operation, ..
                } = row
                {
                    Some((*psi_operation, row))
                } else {
                    None
                }
            })),
        }
    }
    pub(crate) fn boundary_calls(
        &self,
        operation: OperationId,
    ) -> &[&'a abstract_operations::AbstractOperation] {
        self.operations.get(operation)
    }
}

/// Join retained execution to separately admitted installation authority.
/// Source-to-target operand preservation belongs to the independent audit.
pub(crate) fn authorize_target(
    target: &target_operations::TargetOperationPlan,
    module: &TerminalModule,
    cohort: &AdmittedInstalledInterruptCohort,
) -> Result<(), crate::LegalizationError> {
    use target_operations::{InstallationCallRole, TargetUnitOperation};
    let fail = |operation, reason| crate::LegalizationError::InstalledInterruptAssociation {
        operation,
        reason,
    };
    if cohort.psi() != target.psi {
        return Err(fail(
            None,
            "interrupt helper cohort belongs to another Terminal program",
        ));
    }
    let helpers = PreparedInstalledInterruptHelpers::new(target.psi, module, cohort);
    let scopes = Rows::new(
        cohort
            .scopes()
            .iter()
            .map(|scope| (scope.operation(), scope)),
    );
    for row in target
        .functions
        .iter()
        .flat_map(|function| function.graph.operations())
    {
        match row {
            TargetUnitOperation::StaticInterrupt {
                psi_operation,
                operation,
                ..
            } => helpers
                .verify(*psi_operation, *operation)
                .map_err(|reason| fail(Some(*psi_operation), reason))?,
            TargetUnitOperation::InstallStaticInterrupt {
                psi_operation,
                boundary,
                arguments,
                calls,
                ..
            } => {
                let [scope] = scopes.get(*psi_operation) else {
                    return Err(fail(
                        Some(*psi_operation),
                        "installation has no unique admitted Scope",
                    ));
                };
                if scope.boundary() != *boundary
                    || scope.arguments() != arguments
                    || calls[0].role != InstallationCallRole::Validate
                    || calls[0].callee != scope.validator()
                    || calls[1].role != InstallationCallRole::Work
                    || calls[1].callee != scope.work()
                {
                    return Err(fail(
                        Some(*psi_operation),
                        "installation changed its admitted operands or selected helper calls",
                    ));
                }
                if module
                    .boundary_machine(*boundary)
                    .and_then(|row| row.declaration.as_ref())
                    .and_then(|row| row.execution.static_interrupt_operation())
                    != Some(language_core::static_interrupts::StaticInterruptOperation::Install)
                {
                    return Err(fail(
                        Some(*psi_operation),
                        "installation changed its authenticated declaration",
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn validate_target(
    target: &target_operations::TargetOperationPlan,
    module: &TerminalModule,
    cohort: &AdmittedInstalledInterruptCohort,
    abstract_plan: &abstract_operations::AbstractOperationPlan,
) -> Result<(), crate::LegalizationError> {
    authorize_target(target, module, cohort)?;
    let fail = |operation, reason| crate::LegalizationError::InstalledInterruptAssociation {
        operation,
        reason,
    };
    let sources = PreparedInstallationSources::new(abstract_plan);
    for row in target
        .functions
        .iter()
        .flat_map(|function| function.graph.operations())
    {
        if let target_operations::TargetUnitOperation::StaticInterrupt {
            psi_operation,
            boundary,
            token,
            ..
        } = row
            && !sources.boundary_calls(*psi_operation).iter().any(|source|
                matches!(source, abstract_operations::AbstractOperation::BoundaryCall { psi_operation: actual_operation, boundary: actual, arguments, structural_arguments, .. }
                    if actual_operation == psi_operation && actual == boundary && arguments.is_empty() && structural_arguments.as_slice() == std::slice::from_ref(token))) {
                return Err(fail(Some(*psi_operation), "interrupt helper changed its exact boundary or token operand"));
        }
    }
    crate::interrupt_installation::validate_target(target, &sources)
}
