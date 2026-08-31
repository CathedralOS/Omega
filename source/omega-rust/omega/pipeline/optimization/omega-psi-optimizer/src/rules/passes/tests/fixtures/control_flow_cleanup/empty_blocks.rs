use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractSuccessor, ValueBinding,
};
use omega_optimization_unit::{PsiOptimizationUnit, reconstruct_psi_optimization_unit_seed};
use psi_core::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, ScalarType, ValueId,
};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::super::id;

pub(crate) fn linear_empty_block_unit() -> PsiOptimizationUnit {
    let machine = id(901, MachineId::new);
    let entry = id(902, BlockId::new);
    let empty = id(903, BlockId::new);
    let target = id(904, BlockId::new);
    let left = id(905, ValueId::new);
    let right = id(906, ValueId::new);
    let first = id(907, ValueId::new);
    let second = id(908, ValueId::new);
    let target_first = id(909, ValueId::new);
    let target_second = id(910, ValueId::new);
    let scalar_type = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 8).expect("valid fixture integer"),
    );
    let parameter = |value| AbstractParameter { value, scalar_type };
    let binding = |parameter, argument| ValueBinding {
        parameter,
        argument,
        scalar_type,
    };
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([31; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry,
                parameters: vec![
                    AbstractParameter {
                        value: left,
                        scalar_type,
                    },
                    AbstractParameter {
                        value: right,
                        scalar_type,
                    },
                ],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: empty,
                        parameters: vec![parameter(first), parameter(second)],
                        operation_offset: 1,
                    },
                    AbstractBlockEntry {
                        block: target,
                        parameters: vec![parameter(target_first), parameter(target_second)],
                        operation_offset: 2,
                    },
                ],
                operations: vec![
                    AbstractOperation::Jump {
                        psi_edge: id(911, EdgeId::new),
                        target: empty,
                        bindings: vec![binding(first, left), binding(second, right)],
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(912, EdgeId::new),
                        target,
                        bindings: vec![
                            binding(target_first, second),
                            binding(target_second, first),
                        ],
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(913, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

pub(crate) fn path_qualified_empty_block_unit() -> PsiOptimizationUnit {
    let machine = id(921, MachineId::new);
    let entry = id(922, BlockId::new);
    let left_block = id(923, BlockId::new);
    let right_block = id(924, BlockId::new);
    let empty = id(925, BlockId::new);
    let target = id(926, BlockId::new);
    let condition = id(927, ValueId::new);
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([32; 32]),
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
                block_entries: vec![
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: left_block,
                        parameters: Vec::new(),
                        operation_offset: 1,
                    },
                    AbstractBlockEntry {
                        block: right_block,
                        parameters: Vec::new(),
                        operation_offset: 2,
                    },
                    AbstractBlockEntry {
                        block: empty,
                        parameters: Vec::new(),
                        operation_offset: 3,
                    },
                    AbstractBlockEntry {
                        block: target,
                        parameters: Vec::new(),
                        operation_offset: 4,
                    },
                ],
                operations: vec![
                    AbstractOperation::Conditional {
                        condition,
                        when_true: AbstractSuccessor {
                            psi_edge: id(931, EdgeId::new),
                            target: left_block,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: AbstractSuccessor {
                            psi_edge: id(932, EdgeId::new),
                            target: right_block,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(933, EdgeId::new),
                        target: empty,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(934, EdgeId::new),
                        target: empty,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(935, EdgeId::new),
                        target,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(936, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}
