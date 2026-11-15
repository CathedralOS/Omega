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

#[cfg(test)]
mod tests {
    use super::instructions;
    use abstract_operations::AbstractOperation;
    use optimization_unit::{
        EffectLink, OptimizationBlock, OptimizationNode, PsiProvenance, WorkSettlement,
    };
    use semantic_vocabulary::{MachineId, OperationId, PlaceId, PsiSemanticId, ScalarType};
    use target_operations::TargetUnitOperation;
    use terminal_psi::{StructuralAccess, StructuralArgument};

    fn id<T: PsiSemanticId>(raw: u64) -> T {
        T::new(raw).unwrap()
    }

    fn node(operation: AbstractOperation) -> OptimizationNode {
        let work = operation
            .psi_operation()
            .map(|operation| WorkSettlement {
                site: PsiProvenance::Operation(operation),
                units: 1,
            })
            .into_iter()
            .collect();
        OptimizationNode {
            operation,
            provenance: vec![],
            work,
            effect: EffectLink {
                input: 3,
                output: 3,
            },
            definitions: vec![],
            uses: vec![],
            successors: vec![],
            ownership: vec![],
        }
    }

    #[test]
    fn decomposition_keeps_its_instruction_slot_and_annotations() {
        let subject = StructuralArgument {
            place: id::<PlaceId>(7),
            path: vec![],
            access: StructuralAccess::Owned,
        };
        let operation = id::<OperationId>(2);
        let following = id::<OperationId>(3);
        let value = id(11);
        let block_id = id(1);
        let mut scalar = node(AbstractOperation::BooleanConstant {
            psi_operation: following,
            result: value,
            value: true,
        });
        scalar.definitions.push(optimization_unit::ValueDefinition {
            value,
            scalar_type: ScalarType::Boolean,
            site: optimization_unit::ValueDefinitionSite::Node {
                block: block_id,
                node: 1,
            },
        });
        let block = OptimizationBlock {
            id: block_id,
            structural_parameters: vec![],
            parameters: vec![],
            nodes: vec![
                node(AbstractOperation::DecomposeStructuralValue {
                    psi_operation: operation,
                    subject: subject.clone(),
                }),
                scalar,
                node(AbstractOperation::ReturnUnit {
                    psi_edge: id(4),
                    cleanup_actions: vec![],
                }),
            ],
        };
        let rows = vec![
            TargetUnitOperation::DecomposeStructuralValue {
                psi_operation: operation,
                subject,
            },
            TargetUnitOperation::BooleanConstant {
                psi_operation: following,
                result: value,
                value: true,
            },
        ];
        let retained = instructions(id::<MachineId>(1), &block, rows.clone()).unwrap();
        assert_eq!(retained.len(), 2);
        let partition = retained[0].source.as_ref().unwrap();
        assert_eq!(partition.operation, operation);
        assert!(partition.result.is_none());
        assert_eq!(partition.metadata.work, block.nodes[0].work);
        assert_eq!(partition.metadata.effect, block.nodes[0].effect);
        assert_eq!(partition.metadata.ownership, block.nodes[0].ownership);
        let scalar = retained[1].source.as_ref().unwrap();
        assert_eq!(scalar.operation, following);
        assert_eq!(scalar.result, Some(block.nodes[1].definitions[0]));
        assert_eq!(retained[0].kind, rows[0]);
        assert_eq!(retained[1].kind, rows[1]);

        let mut missing = rows.clone();
        missing.remove(0);
        let mut duplicate = rows.clone();
        duplicate.insert(0, rows[0].clone());
        for malformed in [missing, duplicate] {
            assert!(matches!(
                instructions(id::<MachineId>(1), &block, malformed),
                Err(crate::LoweringError::InstructionMetadata {
                    reason: "target instruction roster differs from its input",
                    ..
                })
            ));
        }
    }
}
