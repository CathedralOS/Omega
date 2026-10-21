//! Lower a compiler-owned lifecycle without changing its value operands.
//!
//! Settlement chooses executable meaning; the catalog retains complete image
//! geometry. Neither an accessor roster nor an ordinary provider substitute
//! can fulfill this operation. Its result uses ordinary aggregate storage, so
//! rejection and successful extraction preserve the same caller-visible ABI.

use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{
    AbstractBoundaryResult, AbstractFunction, AbstractOperation, PlacementLifecycleOperands,
};
use semantic_vocabulary::BoundaryMachineId;
use std::collections::BTreeMap;
use target_operations::{
    BoundaryExecutionBinding, BoundaryRealization, BoundarySettlementBinding,
    BoundarySettlementRealization, CompilerBuiltinExecution, TargetUnitOperation,
    TerminalPsiProvenance,
};
use terminal_psi::{StructuralAccess, TerminalPlacementLifecycleExecution};

pub(super) fn lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
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
    let invalid = |reason| LoweringError::InvalidPlacementLifecycle {
        machine: function.machine,
        operation: *psi_operation,
        boundary: *boundary,
        reason,
    };
    let plan = types
        .catalog()
        .placement_lifecycles()
        .iter()
        .find(|plan| plan.boundary == *boundary)
        .ok_or_else(|| invalid("native lifecycle lost its retained whole-value plan"))?;
    let TerminalPlacementLifecycleExecution::CompilerPrimitive { profile_type, .. } =
        &plan.execution
    else {
        return Err(invalid(
            "a selected provider lifecycle is not a compiler primitive",
        ));
    };
    let settlement = settlements
        .get(boundary)
        .ok_or_else(|| invalid("native lifecycle has no execution settlement"))?;
    if settlement.execution
        != BoundaryExecutionBinding::CompilerBuiltin(CompilerBuiltinExecution::PlacementLifecycle {
            boundary: *boundary,
            operation: plan.operation,
        })
        || settlement.realization
            != BoundarySettlementRealization::Builtin(BoundaryRealization::PlacementLifecycle {
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
    let operands = PlacementLifecycleOperands::from_arguments(plan.operation, structural_arguments)
        .map_err(invalid)?;
    let expected = match plan.operation {
        terminal_psi::TerminalPlacementLifecycleOperation::Initialize => {
            [Some(plan.storage.carrier), Some(plan.schema), *profile_type]
        }
        terminal_psi::TerminalPlacementLifecycleOperation::MoveOut => {
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
    let result_home = super::aggregate_results::home(*psi_operation, result, types)?;
    if live
        .structural_homes
        .insert(result.place, result_home.clone())
        .is_some()
    {
        return Err(invalid("native lifecycle replaces an existing result home"));
    }
    operations.push(TargetUnitOperation::PlacementLifecycle {
        psi_operation: *psi_operation,
        boundary: *boundary,
        operands,
        result_home,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
