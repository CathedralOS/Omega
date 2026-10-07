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

pub(super) fn validate_normal_call_completions(
    _native: &TargetOperationPlan,
    optimized: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
) -> Result<(), LegalizationError> {
    for block in &optimized.blocks {
        for node in &block.nodes {
            let AbstractOperation::BoundaryCall { boundary, .. } = node.operation else {
                continue;
            };
            if !plan.boundary_machines.iter().any(|declaration| {
                declaration.id == boundary
                    && declaration.completion
                        == semantic_vocabulary::BoundaryCompletion::NormalReturn
            }) {
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
