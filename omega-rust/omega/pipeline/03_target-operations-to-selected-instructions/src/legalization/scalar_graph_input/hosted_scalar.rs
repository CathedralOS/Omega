//! Read the exact admitted target mechanism without changing execution custody.
//! Encoding follows the closed realization. The receiving target validator keeps
//! admitted provider execution distinct from an exact compiler-builtin identity;
//! projecting the mechanism must not relabel the provider as a compiler builtin.
use super::{
    AbstractOperation, AbstractOperationPlan, MachineId, PsiOptimizationFunction,
    TargetOperationPlan,
};
use crate::LegalizationError;
use semantic_vocabulary::OperationId;
use target_operations::{BoundaryRealization, TargetUnitOperation};

pub(super) fn validate_tails(
    native: &TargetOperationPlan,
    optimized: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
) -> Result<(), LegalizationError> {
    for block in &optimized.blocks {
        for (position, node) in block.nodes.iter().enumerate() {
            let AbstractOperation::BoundaryCall { psi_operation, .. } = node.operation else {
                continue;
            };
            // Installed checked code has an ordinary call/return edge. Only an
            // explicit hosted process-exit settlement has this no-return tail.
            if super::call_origin::installed_operation(node, optimized, native, plan)?.is_some() {
                continue;
            }
            // An evaluated normalized foreign call carries its own admitted
            // provider execution; it is never a hosted process-exit tail.
            if super::normalized_foreign::row(native, optimized.machine, psi_operation)?.is_some() {
                continue;
            }
            if matches!(
                hosted_realization(native, optimized.machine, psi_operation)?,
                BoundaryRealization::HostedExitProcessI32(_)
            ) && (position + 2 != block.nodes.len()
                || !matches!(&block.nodes[position + 1].operation,
                        AbstractOperation::ReturnUnit { cleanup_actions, .. } if cleanup_actions.is_empty()))
            {
                return Err(LegalizationError::custody());
            }
        }
    }
    Ok(())
}

pub(in crate::legalization) fn hosted_realization(
    native: &TargetOperationPlan,
    machine: MachineId,
    operation: OperationId,
) -> Result<BoundaryRealization, LegalizationError> {
    let mut functions = native
        .functions
        .iter()
        .filter(|function| function.machine == machine);
    let function = functions.next().ok_or(LegalizationError::custody())?;
    if functions.next().is_some() {
        return Err(LegalizationError::custody());
    }
    let mut result = None;
    let mut inspect = |row: &TargetUnitOperation| -> Result<(), LegalizationError> {
        if let TargetUnitOperation::BoundarySettlement {
            psi_operation,
            realization,
            ..
        } = row
            && *psi_operation == operation
            && result.replace(*realization).is_some()
        {
            return Err(LegalizationError::custody());
        }
        Ok(())
    };
    for block in &function.graph.blocks {
        for row in &block.operations {
            inspect(row)?;
        }
    }
    result.ok_or(LegalizationError::custody())
}
