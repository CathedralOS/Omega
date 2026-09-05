use super::super::super::AbstractSuccessor;
use super::super::{id, with_synthetic_accepted_obligations};
use abstract_operations::AbstractOperation as O;
use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use optimization_unit::{
    OptimizationFact, PsiOptimizationUnit, recompute_psi_optimization_unit_identity,
    reconstruct_psi_optimization_unit_seed,
};
use optimization_unit_semantics::validate_psi_optimization_unit;
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, ObligationId,
    OperationId, ScalarType, ValueId,
};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

pub(crate) fn dominator_gvn_unit() -> PsiOptimizationUnit {
    scalar_dominator_gvn_unit(false)
}

pub(crate) fn proof_certified_dominator_gvn_unit() -> PsiOptimizationUnit {
    scalar_dominator_gvn_unit(true)
}

pub(crate) fn compatible_policy_dominator_gvn_unit() -> PsiOptimizationUnit {
    let mut unit = proof_certified_dominator_gvn_unit();
    let node = &mut unit.functions[0].blocks[1].nodes[0];
    let O::ExactIntegerAdd {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
        ..
    } = node.operation
    else {
        unreachable!("proof GVN leader is exact add")
    };
    node.operation = O::SaturatingIntegerAdd {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
    };
    unit.functions[0].facts.retain(|fact| {
        !matches!(fact, OptimizationFact::OperationObligationReference { support, .. }
                if *support == psi_operation)
    });
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    validate_psi_optimization_unit(&unit).unwrap();
    unit
}

pub(crate) fn scalar_dominator_gvn_unit(proof_certified: bool) -> PsiOptimizationUnit {
    let machine = id(1_341, MachineId::new);
    let dominated = id(1_342, BlockId::new);
    let entry = id(1_343, BlockId::new);
    let left = id(1_344, ValueId::new);
    let right = id(1_345, ValueId::new);
    let leader = id(1_346, ValueId::new);
    let redundant = id(1_347, ValueId::new);
    let equal = id(1_348, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let binary = |psi_operation, obligation, result, left, right| {
        if proof_certified {
            AbstractOperation::ExactIntegerAdd {
                psi_operation,
                obligation,
                result,
                scalar_type: integer,
                left,
                right,
            }
        } else {
            AbstractOperation::WrappingIntegerAdd {
                psi_operation,
                result,
                scalar_type: integer,
                left,
                right,
            }
        }
    };
    let unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([42; 32]),
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
                        scalar_type: ScalarType::Integer(integer),
                    },
                    AbstractParameter {
                        value: right,
                        scalar_type: ScalarType::Integer(integer),
                    },
                ],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: equal,
                    scalar_type: ScalarType::Boolean,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: dominated,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 3,
                    },
                ],
                operations: vec![
                    binary(
                        id(1_351, OperationId::new),
                        id(1_354, ObligationId::new),
                        redundant,
                        right,
                        left,
                    ),
                    AbstractOperation::IntegerEqual {
                        psi_operation: id(1_352, OperationId::new),
                        result: equal,
                        left: leader,
                        right: redundant,
                    },
                    AbstractOperation::Return {
                        psi_edge: id(1_353, EdgeId::new),
                        result: equal,
                        value: equal,
                        scalar_type: ScalarType::Boolean,
                        cleanup_actions: Vec::new(),
                    },
                    binary(
                        id(1_349, OperationId::new),
                        id(1_355, ObligationId::new),
                        leader,
                        left,
                        right,
                    ),
                    AbstractOperation::Jump {
                        psi_edge: id(1_350, EdgeId::new),
                        target: dominated,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    if proof_certified {
        with_synthetic_accepted_obligations(unit)
    } else {
        unit
    }
}

pub(crate) fn diamond_dominator_gvn_unit() -> PsiOptimizationUnit {
    let machine = id(1_401, MachineId::new);
    let join = id(1_402, BlockId::new);
    let left_block = id(1_403, BlockId::new);
    let entry = id(1_404, BlockId::new);
    let right_block = id(1_405, BlockId::new);
    let condition = id(1_406, ValueId::new);
    let operand = id(1_407, ValueId::new);
    let outer_first = id(1_408, ValueId::new);
    let outer_second = id(1_409, ValueId::new);
    let inner_first = id(1_410, ValueId::new);
    let inner_second = id(1_411, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([43; 32]),
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
                        value: operand,
                        scalar_type: ScalarType::Integer(integer),
                    },
                ],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: inner_second,
                    scalar_type: ScalarType::Integer(integer),
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: join,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: left_block,
                        parameters: Vec::new(),
                        operation_offset: 3,
                    },
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 4,
                    },
                    AbstractBlockEntry {
                        block: right_block,
                        parameters: Vec::new(),
                        operation_offset: 7,
                    },
                ],
                operations: vec![
                    AbstractOperation::IntegerBitwiseNot {
                        psi_operation: id(1_412, OperationId::new),
                        result: inner_first,
                        scalar_type: integer,
                        operand,
                    },
                    AbstractOperation::IntegerBitwiseNot {
                        psi_operation: id(1_413, OperationId::new),
                        result: inner_second,
                        scalar_type: integer,
                        operand: inner_first,
                    },
                    AbstractOperation::Return {
                        psi_edge: id(1_414, EdgeId::new),
                        result: inner_second,
                        value: inner_second,
                        scalar_type: ScalarType::Integer(integer),
                        cleanup_actions: Vec::new(),
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(1_415, EdgeId::new),
                        target: join,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::IntegerBitwiseNot {
                        psi_operation: id(1_416, OperationId::new),
                        result: outer_first,
                        scalar_type: integer,
                        operand,
                    },
                    AbstractOperation::IntegerBitwiseNot {
                        psi_operation: id(1_417, OperationId::new),
                        result: outer_second,
                        scalar_type: integer,
                        operand: outer_first,
                    },
                    AbstractOperation::Conditional {
                        condition,
                        when_true: AbstractSuccessor {
                            psi_edge: id(1_418, EdgeId::new),
                            target: left_block,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: AbstractSuccessor {
                            psi_edge: id(1_419, EdgeId::new),
                            target: right_block,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(1_420, EdgeId::new),
                        target: join,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

pub(crate) fn sibling_only_gvn_unit() -> PsiOptimizationUnit {
    let machine = id(1_441, MachineId::new);
    let join = id(1_442, BlockId::new);
    let left_block = id(1_443, BlockId::new);
    let entry = id(1_444, BlockId::new);
    let right_block = id(1_445, BlockId::new);
    let condition = id(1_446, ValueId::new);
    let operand = id(1_447, ValueId::new);
    let sibling = id(1_448, ValueId::new);
    let redundant = id(1_449, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
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
                        value: operand,
                        scalar_type: ScalarType::Integer(integer),
                    },
                ],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: redundant,
                    scalar_type: ScalarType::Integer(integer),
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![
                    AbstractBlockEntry {
                        block: join,
                        parameters: Vec::new(),
                        operation_offset: 0,
                    },
                    AbstractBlockEntry {
                        block: left_block,
                        parameters: Vec::new(),
                        operation_offset: 2,
                    },
                    AbstractBlockEntry {
                        block: entry,
                        parameters: Vec::new(),
                        operation_offset: 4,
                    },
                    AbstractBlockEntry {
                        block: right_block,
                        parameters: Vec::new(),
                        operation_offset: 5,
                    },
                ],
                operations: vec![
                    AbstractOperation::IntegerBitwiseNot {
                        psi_operation: id(1_450, OperationId::new),
                        result: redundant,
                        scalar_type: integer,
                        operand,
                    },
                    AbstractOperation::Return {
                        psi_edge: id(1_451, EdgeId::new),
                        result: redundant,
                        value: redundant,
                        scalar_type: ScalarType::Integer(integer),
                        cleanup_actions: Vec::new(),
                    },
                    AbstractOperation::IntegerBitwiseNot {
                        psi_operation: id(1_452, OperationId::new),
                        result: sibling,
                        scalar_type: integer,
                        operand,
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(1_453, EdgeId::new),
                        target: join,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                    AbstractOperation::Conditional {
                        condition,
                        when_true: AbstractSuccessor {
                            psi_edge: id(1_454, EdgeId::new),
                            target: left_block,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: AbstractSuccessor {
                            psi_edge: id(1_455, EdgeId::new),
                            target: right_block,
                            bindings: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    AbstractOperation::Jump {
                        psi_edge: id(1_456, EdgeId::new),
                        target: join,
                        bindings: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}
