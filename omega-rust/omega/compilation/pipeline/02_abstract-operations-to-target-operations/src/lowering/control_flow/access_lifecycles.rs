//! Lower a compiler-owned lifecycle without changing its value operands.
//!
//! Settlement chooses executable meaning; the catalog retains complete image
//! geometry. Neither an accessor roster nor an ordinary provider substitute
//! can fulfill this operation. Its result uses ordinary aggregate storage, so
//! rejection and successful extraction preserve the same caller-visible ABI.

use super::RowSite;
use super::aggregate_results::publish_home;
use crate::LoweringError;
use abstract_operations::{AbstractBoundaryResult, AbstractOperation, AccessLifecycleOperands};
use semantic_vocabulary::BoundaryMachineId;
use std::collections::BTreeMap;
use target_operations::{
    BoundaryExecutionBinding, BoundaryRealization, BoundarySettlementBinding,
    BoundarySettlementRealization, CompilerBuiltinExecution, TargetUnitOperation,
};
use terminal_psi::{StructuralAccess, TerminalAccessLifecycleExecution};

pub(super) fn lower(
    operation: &AbstractOperation,
    site: &mut RowSite<'_>,
    boundaries: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
) -> Result<(), LoweringError> {
    let (function, types, live, operations, provenance) = site.parts();
    let AbstractOperation::BoundaryCall {
        psi_operation,
        boundary,
        structural_arguments,
        arguments,
        result,
        ..
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let invalid = |reason| LoweringError::InvalidAccessLifecycle {
        machine: function.machine,
        operation: *psi_operation,
        boundary: *boundary,
        reason,
    };
    let plan = types
        .catalog()
        .access_lifecycles()
        .iter()
        .find(|plan| plan.boundary == *boundary)
        .ok_or_else(|| invalid("native lifecycle lost its retained whole-value plan"))?;
    let TerminalAccessLifecycleExecution::CompilerPrimitive { profile_type, .. } = &plan.execution
    else {
        return Err(invalid(
            "a selected provider lifecycle is not a compiler primitive",
        ));
    };
    let settlement = settlements
        .get(boundary)
        .ok_or_else(|| invalid("native lifecycle has no execution settlement"))?;
    if settlement.execution
        != BoundaryExecutionBinding::CompilerBuiltin(CompilerBuiltinExecution::AccessLifecycle {
            boundary: *boundary,
            operation: plan.operation,
        })
        || settlement.realization
            != BoundarySettlementRealization::Builtin(BoundaryRealization::AccessLifecycle {
                boundary: *boundary,
            })
    {
        return Err(invalid(
            "native lifecycle settlement changed its boundary or operation",
        ));
    }
    if !arguments.is_empty() {
        return Err(invalid("native lifecycle has unexpected scalar arguments"));
    }
    let operands = AccessLifecycleOperands::from_arguments(plan.operation, structural_arguments)
        .map_err(invalid)?;
    let expected = match plan.operation {
        terminal_psi::TerminalAccessLifecycleOperation::Initialize => {
            [Some(plan.storage.carrier), Some(plan.schema), *profile_type]
        }
        terminal_psi::TerminalAccessLifecycleOperation::MoveOut
        | terminal_psi::TerminalAccessLifecycleOperation::View => {
            [Some(plan.storage.carrier), *profile_type, None]
        }
    };
    for (argument, expected) in operands.arguments().zip(expected) {
        let actual = if argument.access == StructuralAccess::Owned {
            super::aggregate_results::value_input(function, types, live, argument)?.0
        } else {
            super::references::reference_source_type(function, live, types, argument)
                .ok_or_else(|| invalid("native lifecycle borrowed input has no current referent"))?
        };
        if Some(actual) != expected {
            return Err(invalid(
                "native lifecycle operand carrier differs from its retained role",
            ));
        }
    }
    let AbstractBoundaryResult::Structural(result) = result else {
        return Err(invalid("native lifecycle lost its structural result"));
    };
    if plan.operation == terminal_psi::TerminalAccessLifecycleOperation::View {
        let declaration = boundaries
            .get(boundary)
            .ok_or_else(|| invalid("borrowed View lost its exact boundary declaration"))?;
        let (path, backing) =
            optimization_unit_semantics::access_references::borrowed_view_formation(
                types.catalog(),
                declaration,
                result,
            )
            .ok_or_else(|| invalid("borrowed View lost its Ready interpretation"))?;
        let AccessLifecycleOperands::View { storage, .. } = &operands else {
            return Err(invalid("borrowed View lost its storage operand"));
        };
        super::references::retain_access_result(
            function, types, live, result, storage, path, backing,
        )?;
    }
    let Some(result_home) = publish_home(live, *psi_operation, result, types)? else {
        return Err(invalid("native lifecycle replaces an existing result home"));
    };
    operations.push(TargetUnitOperation::AccessLifecycle {
        psi_operation: *psi_operation,
        boundary: *boundary,
        operands,
        result_home,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
