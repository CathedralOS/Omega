use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult, AbstractSuccessor, ValueBinding,
};
use omega_optimization_unit::{PsiOptimizationUnit, reconstruct_psi_optimization_unit_seed};
use psi_core::{
    BlockId, EdgeId, FuelScheduleIdentity, MachineId, OperationId, ScalarType, ValueId,
};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use super::super::id;

pub(crate) fn adjacent_conditional_merge_unit() -> PsiOptimizationUnit {
    let machine = id(1_101, MachineId::new);
    let entry = id(1_102, BlockId::new);
    let decision = id(1_103, BlockId::new);
    let left = id(1_104, BlockId::new);
    let right = id(1_105, BlockId::new);
    let condition = id(1_106, ValueId::new);
    let forwarded = id(1_107, ValueId::new);
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([37; 32]),
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
                        block: decision,
                        parameters: vec![AbstractParameter {
                            value: forwarded,
                            scalar_type: ScalarType::Boolean,
                        }],
                        operation_offset: 1,
                    },
                    AbstractBlockEntry {
                        block: left,
                        parameters: Vec::new(),
                        operation_offset: 2,
                    },
                    AbstractBlockEntry {
                        block: right,
                        parameters: Vec::new(),
                        operation_offset: 3,
                    },
                ],
                operations: vec![
                    AbstractOperation::Jump {
                        psi_edge: id(1_110, EdgeId::new),
                        target: decision,
                        bindings: vec![ValueBinding {
                            parameter: forwarded,
                            argument: condition,
                            scalar_type: ScalarType::Boolean,
                        }],
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::Conditional {
                        condition: forwarded,
                        when_true: AbstractSuccessor {
                            psi_edge: id(1_111, EdgeId::new),
                            target: left,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: AbstractSuccessor {
                            psi_edge: id(1_112, EdgeId::new),
                            target: right,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(1_113, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(1_114, EdgeId::new),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

pub(crate) fn non_adjacent_merge_unit(target_before_predecessor: bool) -> PsiOptimizationUnit {
    let machine = id(1_501, MachineId::new);
    let entry = id(1_502, BlockId::new);
    let descendant = id(1_503, BlockId::new);
    let target = id(1_504, BlockId::new);
    let sibling = id(1_505, BlockId::new);
    let predecessor = id(1_506, BlockId::new);
    let condition = id(1_507, ValueId::new);
    let incoming = id(1_508, ValueId::new);
    let target_parameter = id(1_509, ValueId::new);
    let target_result = id(1_510, ValueId::new);
    let descendant_result = id(1_511, ValueId::new);
    let predecessor_value = id(1_520, ValueId::new);

    let entry_operation = AbstractOperation::Conditional {
        condition,
        when_true: AbstractSuccessor {
            psi_edge: id(1_512, EdgeId::new),
            target: predecessor,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: AbstractSuccessor {
            psi_edge: id(1_513, EdgeId::new),
            target: sibling,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    let descendant_operations = vec![
        AbstractOperation::BooleanEqual {
            psi_operation: id(1_514, OperationId::new),
            result: descendant_result,
            left: target_parameter,
            right: target_result,
        },
        AbstractOperation::Return {
            psi_edge: id(1_515, EdgeId::new),
            result: descendant_result,
            value: descendant_result,
            scalar_type: ScalarType::Boolean,
            cleanup_actions: Vec::new(),
        },
    ];
    let target_operations = vec![
        AbstractOperation::BooleanNot {
            psi_operation: id(1_516, OperationId::new),
            result: target_result,
            operand: target_parameter,
        },
        AbstractOperation::Jump {
            psi_edge: id(1_517, EdgeId::new),
            target: descendant,
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    ];
    let sibling_operation = AbstractOperation::Return {
        psi_edge: id(1_518, EdgeId::new),
        result: descendant_result,
        value: incoming,
        scalar_type: ScalarType::Boolean,
        cleanup_actions: Vec::new(),
    };
    let predecessor_operations = vec![
        AbstractOperation::BooleanNot {
            psi_operation: id(1_521, OperationId::new),
            result: predecessor_value,
            operand: incoming,
        },
        AbstractOperation::Jump {
            psi_edge: id(1_519, EdgeId::new),
            target,
            bindings: vec![ValueBinding {
                parameter: target_parameter,
                argument: predecessor_value,
                scalar_type: ScalarType::Boolean,
            }],
            trivial_affine_discards: Vec::new(),
        },
    ];

    let mut block_entries = Vec::new();
    let mut operations = Vec::new();
    let mut push_block = |block, parameters, block_operations: Vec<_>| {
        block_entries.push(AbstractBlockEntry {
            block,
            parameters,
            operation_offset: operations.len(),
        });
        operations.extend(block_operations);
    };
    push_block(entry, Vec::new(), vec![entry_operation]);
    if target_before_predecessor {
        push_block(descendant, Vec::new(), descendant_operations);
        push_block(
            target,
            vec![AbstractParameter {
                value: target_parameter,
                scalar_type: ScalarType::Boolean,
            }],
            target_operations,
        );
        push_block(sibling, Vec::new(), vec![sibling_operation]);
        push_block(predecessor, Vec::new(), predecessor_operations);
    } else {
        push_block(predecessor, Vec::new(), predecessor_operations);
        push_block(sibling, Vec::new(), vec![sibling_operation]);
        push_block(
            target,
            vec![AbstractParameter {
                value: target_parameter,
                scalar_type: ScalarType::Boolean,
            }],
            target_operations,
        );
        push_block(descendant, Vec::new(), descendant_operations);
    }

    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([44; 32]),
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
                        value: condition,
                        scalar_type: ScalarType::Boolean,
                    },
                    AbstractParameter {
                        value: incoming,
                        scalar_type: ScalarType::Boolean,
                    },
                ],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: descendant_result,
                    scalar_type: ScalarType::Boolean,
                }),
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
