use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractSuccessor,
};
use omega_optimization_unit::{PsiOptimizationUnit, reconstruct_psi_optimization_unit_seed};
use psi_core::{
    BlockId, EdgeId, FuelScheduleIdentity, MachineId, OperationId, ScalarType, ValueId,
};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::rules::tests::id;

pub(crate) fn linear_shared_target_unit() -> PsiOptimizationUnit {
    let machine = id(32_001, MachineId::new);
    let entry = id(32_002, BlockId::new);
    let predecessor = id(32_003, BlockId::new);
    let empty = id(32_004, BlockId::new);
    let target = id(32_005, BlockId::new);
    let condition = id(32_006, ValueId::new);
    let predecessor_scratch = id(32_007, ValueId::new);
    let target_scratch = id(32_008, ValueId::new);
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: identity([82; 32]),
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry,
                parameters: vec![AbstractParameter {
                    value: condition,
                    scalar_type: ScalarType::Boolean,
                }],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    block(entry, 0),
                    block(predecessor, 1),
                    block(empty, 3),
                    block(target, 4),
                ],
                operations: vec![
                    AbstractOperation::Conditional {
                        condition,
                        when_true: successor(32_009, predecessor),
                        when_false: successor(32_010, target),
                    },
                    AbstractOperation::BooleanNot {
                        psi_operation: id(32_011, OperationId::new),
                        result: predecessor_scratch,
                        operand: condition,
                    },
                    jump(32_012, empty),
                    jump(32_013, target),
                    AbstractOperation::BooleanNot {
                        psi_operation: id(32_014, OperationId::new),
                        result: target_scratch,
                        operand: condition,
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(32_015, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

pub(crate) fn path_qualified_direct_edges_unit() -> PsiOptimizationUnit {
    let machine = id(33_001, MachineId::new);
    let entry = id(33_002, BlockId::new);
    let empty = id(33_003, BlockId::new);
    let target = id(33_004, BlockId::new);
    let condition = id(33_005, ValueId::new);
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: identity([83; 32]),
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry,
                parameters: vec![AbstractParameter {
                    value: condition,
                    scalar_type: ScalarType::Boolean,
                }],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![block(entry, 0), block(empty, 1), block(target, 2)],
                operations: vec![
                    AbstractOperation::Conditional {
                        condition,
                        when_true: successor(33_006, empty),
                        when_false: successor(33_007, empty),
                    },
                    jump(33_008, target),
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(33_009, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

fn identity(fingerprint: [u8; 32]) -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes(fingerprint),
    }
}

fn block(block: BlockId, operation_offset: usize) -> AbstractBlockEntry {
    AbstractBlockEntry {
        block,
        parameters: Vec::new(),
        operation_offset,
    }
}

fn jump(edge: u64, target: BlockId) -> AbstractOperation {
    AbstractOperation::Jump {
        psi_edge: id(edge, EdgeId::new),
        target,
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

fn successor(edge: u64, target: BlockId) -> AbstractSuccessor {
    AbstractSuccessor {
        psi_edge: id(edge, EdgeId::new),
        target,
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}
