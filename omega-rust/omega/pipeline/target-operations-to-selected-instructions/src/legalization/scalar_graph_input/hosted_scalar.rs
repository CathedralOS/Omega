//! Read the exact admitted target role without constructing legalized instructions.
use super::*;
use semantic_vocabulary::OperationId;
use target_operations::{BoundaryExecutionBinding, CompilerBuiltinExecution, TargetUnitOperation};

pub(super) fn validate_tails(
    native: &TargetOperationPlan,
    optimized: &PsiOptimizationFunction,
) -> Result<(), LegalizationError> {
    for block in &optimized.blocks {
        for (position, node) in block.nodes.iter().enumerate() {
            let AbstractOperation::BoundaryCall { psi_operation, .. } = node.operation else {
                continue;
            };
            if hosted_execution(native, optimized.machine, psi_operation)?
                == CompilerBuiltinExecution::HostedExitProcessI32
                && (position + 2 != block.nodes.len()
                    || !matches!(&block.nodes[position + 1].operation,
                        AbstractOperation::ReturnUnit { cleanup_actions, .. } if cleanup_actions.is_empty()))
            {
                return Err(LegalizationError::SourceCustodyMismatch);
            }
        }
    }
    Ok(())
}

pub(in crate::legalization) fn hosted_execution(
    native: &TargetOperationPlan,
    machine: MachineId,
    operation: OperationId,
) -> Result<CompilerBuiltinExecution, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let mut functions = native
        .functions
        .iter()
        .filter(|function| function.machine == machine);
    let function = functions.next().ok_or(invalid.clone())?;
    if functions.next().is_some() {
        return Err(invalid);
    }
    let mut result = None;
    let mut inspect = |row: &TargetUnitOperation| -> Result<(), LegalizationError> {
        if let TargetUnitOperation::BoundarySettlement {
            psi_operation,
            execution: BoundaryExecutionBinding::CompilerBuiltin(execution),
            ..
        } = row
            && *psi_operation == operation
            && result.replace(*execution).is_some()
        {
            return Err(invalid.clone());
        }
        Ok(())
    };
    match &function.operation {
        TargetOperation::UnitBody(body) => {
            for row in &body.operations {
                inspect(row)?;
            }
        }
        TargetOperation::ControlGraph(graph) => {
            for block in &graph.blocks {
                for row in &block.operations {
                    inspect(row)?;
                }
            }
        }
        _ => return Err(invalid),
    }
    result.ok_or(invalid)
}
