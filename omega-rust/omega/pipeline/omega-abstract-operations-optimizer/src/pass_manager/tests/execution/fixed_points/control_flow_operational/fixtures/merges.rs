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

pub(crate) fn isolated_shared_terminal_unit() -> PsiOptimizationUnit {
    let machine = id(34_001, MachineId::new);
    let entry = id(34_002, BlockId::new);
    let predecessor = id(34_003, BlockId::new);
    let target = id(34_004, BlockId::new);
    let condition = id(34_005, ValueId::new);
    let scratch = id(34_006, ValueId::new);
    seed(
        [84; 32],
        machine,
        entry,
        condition,
        vec![block(entry, 0), block(predecessor, 1), block(target, 3)],
        vec![
            AbstractOperation::Conditional {
                condition,
                when_true: successor(34_007, predecessor),
                when_false: successor(34_008, target),
            },
            AbstractOperation::BooleanNot {
                psi_operation: id(34_009, OperationId::new),
                result: scratch,
                operand: condition,
            },
            jump(34_010, target),
            return_unit(34_011),
        ],
    )
}

pub(crate) fn terminal_non_adjacent_merge_unit() -> PsiOptimizationUnit {
    let machine = id(35_001, MachineId::new);
    let entry = id(35_002, BlockId::new);
    let target = id(35_003, BlockId::new);
    let sibling = id(35_004, BlockId::new);
    let predecessor = id(35_005, BlockId::new);
    let condition = id(35_006, ValueId::new);
    let scratch = id(35_007, ValueId::new);
    seed(
        [85; 32],
        machine,
        entry,
        condition,
        vec![
            block(entry, 0),
            block(target, 1),
            block(sibling, 2),
            block(predecessor, 3),
        ],
        vec![
            AbstractOperation::Conditional {
                condition,
                when_true: successor(35_008, predecessor),
                when_false: successor(35_009, sibling),
            },
            return_unit(35_010),
            return_unit(35_011),
            AbstractOperation::BooleanNot {
                psi_operation: id(35_012, OperationId::new),
                result: scratch,
                operand: condition,
            },
            jump(35_013, target),
        ],
    )
}

fn seed(
    fingerprint: [u8; 32],
    machine: MachineId,
    entry: BlockId,
    condition: ValueId,
    block_entries: Vec<AbstractBlockEntry>,
    operations: Vec<AbstractOperation>,
) -> PsiOptimizationUnit {
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes(fingerprint),
            },
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
                block_entries,
                operations,
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

fn block(block: BlockId, operation_offset: usize) -> AbstractBlockEntry {
    AbstractBlockEntry {
        block,
        parameters: Vec::new(),
        operation_offset,
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

fn jump(edge: u64, target: BlockId) -> AbstractOperation {
    AbstractOperation::Jump {
        psi_edge: id(edge, EdgeId::new),
        target,
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

fn return_unit(edge: u64) -> AbstractOperation {
    AbstractOperation::ReturnUnit {
        psi_edge: id(edge, EdgeId::new),
        cleanup_actions: Vec::new(),
    }
}
