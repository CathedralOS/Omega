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
    // The realization is the boundary settlement's own row; an operation that
    // emitted no settlement row has no hosted realization at all.
    let Some(TargetUnitOperation::BoundarySettlement { realization, .. }) =
        super::unique_row(function, |row| {
            matches!(
                row,
                TargetUnitOperation::BoundarySettlement { psi_operation, .. }
                    if *psi_operation == operation
            )
        })?
    else {
        return Err(LegalizationError::custody());
    };
    Ok(*realization)
}
