//! Lower admitted arrival helpers while preserving ordinary token/result custody.
//! The settlement owner must authenticate the installed cohort before supplying
//! these rows. The declaration enum alone does not establish that association.

use super::RowSite;
use crate::LoweringError;
use abstract_operations::{AbstractBoundaryResult, AbstractOperation};
use language_core::static_interrupts::StaticInterruptOperation;
use target_operations::{
    BoundaryExecutionBinding, BoundaryRealization, BoundarySettlementBinding,
    BoundarySettlementRealization, CompilerBuiltinExecution, TargetUnitOperation,
};
use terminal_psi::{StructuralAccess, StructuralTypeShape};

pub(super) fn lower(
    operation: &AbstractOperation,
    site: &mut RowSite<'_>,
    declaration: &terminal_psi::BoundaryMachineDeclaration,
    settlement: &BoundarySettlementBinding,
) -> Result<(), LoweringError> {
    let (function, types, live, operations, provenance) = site.parts();
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let AbstractOperation::BoundaryCall {
        psi_operation,
        boundary,
        structural_arguments,
        arguments,
        result,
        ..
    } = operation
    else {
        return Err(invalid());
    };
    if declaration.id != *boundary || settlement.boundary != *boundary {
        return Err(invalid());
    }
    let operation = declaration
        .declaration
        .as_ref()
        .and_then(|declaration| declaration.execution.static_interrupt_operation())
        .filter(|operation| *operation != StaticInterruptOperation::Install)
        .ok_or_else(invalid)?;
    if settlement.execution
        != BoundaryExecutionBinding::CompilerBuiltin(CompilerBuiltinExecution::StaticInterrupt {
            boundary: *boundary,
            operation,
        })
        || settlement.realization
            != BoundarySettlementRealization::Builtin(BoundaryRealization::StaticInterrupt {
                boundary: *boundary,
                operation,
            })
    {
        return Err(invalid());
    }
    let [token] = structural_arguments.as_slice() else {
        return Err(invalid());
    };
    let [parameter] = declaration.structural_parameters.as_slice() else {
        return Err(invalid());
    };
    if !arguments.is_empty() {
        return Err(invalid());
    }
    let expected_access = if operation != StaticInterruptOperation::FinishCompletion {
        StructuralAccess::SharedBorrow
    } else {
        StructuralAccess::Owned
    };
    if token.access != expected_access || parameter.access != expected_access {
        return Err(invalid());
    }
    let token_type = if token.access == StructuralAccess::Owned {
        super::aggregate_results::value_input(function, types, live, token)?.0
    } else {
        super::references::reference_source_type(function, live, types, token)
            .ok_or_else(invalid)?
    };
    if token_type != parameter.structural_type {
        return Err(invalid());
    }
    let result_home = match (operation, result) {
        (StaticInterruptOperation::FinishCompletion, AbstractBoundaryResult::Unit) => None,
        (
            StaticInterruptOperation::Context | StaticInterruptOperation::BeginCompletion,
            AbstractBoundaryResult::Structural(result),
        ) => {
            if operation == StaticInterruptOperation::Context {
                if !matches!(
                    types.get(result.structural_type).map(|row| &row.shape),
                    Some(StructuralTypeShape::Reference {
                        access: StructuralAccess::SharedBorrow,
                        ..
                    })
                ) {
                    return Err(invalid());
                }
                super::references::boundary_result_references(function, types, result, live)?;
            }
            Some(super::aggregate_results::established_home(
                live,
                *psi_operation,
                result,
                types,
                function.machine,
            )?)
        }
        _ => return Err(invalid()),
    };
    operations.push(TargetUnitOperation::StaticInterrupt {
        psi_operation: *psi_operation,
        boundary: *boundary,
        operation,
        token: token.clone(),
        result_home,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
