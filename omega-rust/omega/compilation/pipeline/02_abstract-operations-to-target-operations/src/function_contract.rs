//! Retain function storage and instruction contracts alongside target operations.
//!
//! Selection needs this contract, not the earlier body's operations. The input
//! has already been admitted; independent correspondence checks remain in stage 03.

use crate::LoweringError;
use abstract_operations::AbstractOperationPlan;
use optimization_unit::PsiOptimizationUnit;
use target_operations::{TargetOperationPlan, TargetStructuralContract};

/// Attach the admitted instruction obligations while both stage inputs are available.
/// Cleanup calls have a cleanup owner, not a second authored instruction identity.
pub(crate) fn instructions(
    machine: semantic_vocabulary::MachineId,
    block: &optimization_unit::OptimizationBlock,
    operations: Vec<target_operations::TargetUnitOperation>,
) -> Result<Vec<target_operations::TargetInstruction>, LoweringError> {
    use target_operations::{
        TargetInstruction, TargetInstructionSource, TargetOperationMetadata, TargetUnitOperation,
    };
    let invalid = |reason| LoweringError::InstructionMetadata {
        machine,
        block: block.id,
        reason,
    };
    let (_, body) = block
        .nodes
        .split_last()
        .ok_or_else(|| invalid("instruction block has no terminator"))?;
    let ordinary_count = operations
        .iter()
        .position(|operation| matches!(operation, TargetUnitOperation::CleanupCall { .. }))
        .unwrap_or(operations.len());
    if ordinary_count != body.len() {
        return Err(invalid("target instruction roster differs from its input"));
    }
    let mut instructions = Vec::with_capacity(operations.len());
    for (operation_position, kind) in operations.into_iter().enumerate() {
        let source = if operation_position < ordinary_count {
            let node = &body[operation_position];
            let result = match node.definitions.as_slice() {
                [] => None,
                [result] => Some(*result),
                _ => {
                    return Err(invalid(
                        "target instruction has more than one scalar definition",
                    ));
                }
            };
            Some(TargetInstructionSource {
                operation: node
                    .operation
                    .psi_operation()
                    .ok_or_else(|| invalid("nonterminal instruction has no operation identity"))?,
                result,
                metadata: TargetOperationMetadata {
                    work: node.work.clone(),
                    effect: node.effect,
                    ownership: node.ownership.clone(),
                },
            })
        } else {
            if !matches!(kind, TargetUnitOperation::CleanupCall { .. }) {
                return Err(invalid("ordinary instruction follows a cleanup call"));
            }
            None
        };
        instructions.push(TargetInstruction { source, kind });
    }
    Ok(instructions)
}

pub(crate) fn retain(
    source: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    target: &mut TargetOperationPlan,
) -> Result<(), LoweringError> {
    let invalid = |machine, reason| LoweringError::FunctionContract { machine, reason };
    if source.functions.len() != unit.functions.len()
        || source.functions.len() != target.functions.len()
    {
        return Err(invalid(target.entry, "function contract roster differs"));
    }
    for ((source, optimized), target) in source
        .functions
        .iter()
        .zip(&unit.functions)
        .zip(&mut target.functions)
    {
        if source.machine != optimized.machine || source.machine != target.machine {
            return Err(invalid(
                target.machine,
                "function contract identity differs",
            ));
        }
        let has_storage = target.mixed_structural_scalar_abi.is_some()
            || !target.graph.parameters.is_empty()
            || !optimized.declared_places.is_empty();
        target.structural_contract = has_storage.then(|| TargetStructuralContract {
            parameters: source.structural_parameters.clone(),
            places: optimized.structural_places.clone(),
            entry_claims: source.entry_claims.clone(),
            published_service_ceiling: source.published_service_ceiling.clone(),
            result: source.result.structural().cloned(),
        });
    }
    Ok(())
}
