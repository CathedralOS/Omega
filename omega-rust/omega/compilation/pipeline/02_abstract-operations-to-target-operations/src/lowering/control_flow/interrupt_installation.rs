//! Plan the two checked calls made by one guarded installation operation.
//! The operation retains the actual table and context borrows; a second source
//! call or a fabricated Psi operation is unnecessary.

use super::RowSite;
use crate::LoweringError;
use abstract_operations::{AbstractBoundaryResult, AbstractOperation};
use provider_admission::AdmittedInstalledInterruptCohort;
use semantic_vocabulary::{OperationId, ScalarType};
use target_operations::{
    BoundaryExecutionBinding, BoundaryRealization, BoundarySettlementBinding,
    BoundarySettlementRealization, CompilerBuiltinExecution, TargetInstallationCall,
    TargetUnitOperation,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    operation: &AbstractOperation,
    site: &mut RowSite<'_>,
    scope: &provider_admission::AdmittedInstalledInterruptScope,
    calls: [TargetInstallationCall; 2],
    declaration: &terminal_psi::BoundaryMachineDeclaration,
    settlement: &BoundarySettlementBinding,
) -> Result<(), LoweringError> {
    let machine = site.function.machine;
    let invalid = |reason| LoweringError::InvalidInstalledInterrupt {
        machine,
        operation: scope.operation(),
        reason,
    };
    let AbstractOperation::BoundaryCall {
        psi_operation,
        boundary,
        arguments,
        structural_arguments,
        result: AbstractBoundaryResult::Scalar(result),
        ..
    } = operation
    else {
        return Err(invalid(
            "installation must produce one Boolean result from structural arguments",
        ));
    };
    let kind = language_core::static_interrupts::StaticInterruptOperation::Install;
    if declaration.id != *boundary
        || !arguments.is_empty()
        || result.scalar_type != ScalarType::Boolean
        || declaration
            .declaration
            .as_ref()
            .and_then(|row| row.execution.static_interrupt_operation())
            != Some(kind)
        || settlement.execution
            != BoundaryExecutionBinding::CompilerBuiltin(
                CompilerBuiltinExecution::StaticInterrupt {
                    boundary: *boundary,
                    operation: kind,
                },
            )
        || settlement.realization
            != BoundarySettlementRealization::Builtin(BoundaryRealization::StaticInterrupt {
                boundary: *boundary,
                operation: kind,
            })
    {
        return Err(invalid(
            "installation declaration or settled execution differs from its admitted operation",
        ));
    }
    if scope.boundary() != *boundary || scope.arguments().as_slice() != structural_arguments {
        return Err(invalid(
            "installation substituted its admitted boundary or actual references",
        ));
    }
    let result_home = super::primitive_storage::retain_result(*psi_operation, *result, site.live)?;
    site.operations
        .push(TargetUnitOperation::InstallStaticInterrupt {
            psi_operation: *psi_operation,
            boundary: *boundary,
            arguments: scope.arguments().clone(),
            calls,
            result_home,
        });
    site.provenance.operations.push(*psi_operation);
    Ok(())
}

/// Read one already admitted occurrence before its call owner resolves bodies.
pub(super) fn scope(
    operation: OperationId,
    cohort: Option<&AdmittedInstalledInterruptCohort>,
) -> Result<&provider_admission::AdmittedInstalledInterruptScope, &'static str> {
    let mut scopes = cohort
        .ok_or("installation has no admitted scope cohort")?
        .scopes()
        .iter()
        .filter(|scope| scope.operation() == operation);
    match (scopes.next(), scopes.next()) {
        (Some(scope), None) => Ok(scope),
        _ => Err("installation has no unique admitted scope"),
    }
}
