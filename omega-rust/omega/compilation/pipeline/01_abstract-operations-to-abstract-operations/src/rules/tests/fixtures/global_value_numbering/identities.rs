//! Typed fixture for obligation-free wrapping identity rows.

use super::super::id;
use abstract_operations::AbstractOperation as O;
use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperationPlan,
    AbstractParameter, AbstractResult, IntegerArithmetic, IntegerArithmeticOperator,
    IntegerOverflowPolicy, IntegerShiftDirection, IntegerShiftPolicy,
};
use optimization_unit::{
    PsiOptimizationUnit, recompute_psi_optimization_unit_identity,
    reconstruct_psi_optimization_unit_seed,
};
use optimization_unit_semantics::validate_psi_optimization_unit;
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, ScalarType, ValueId,
};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WrappingNeutralOperation {
    Add,
    Subtract,
    Multiply,
    ShiftLeft,
    ShiftRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SaturatingNeutralOperation {
    Add,
    Subtract,
    Multiply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BitwiseNeutralOperation {
    And,
    Or,
    Xor,
}

pub(crate) fn bitwise_neutral_identity_unit(
    operation: BitwiseNeutralOperation,
    literal_value: IntegerValue,
    literal_left: bool,
    both_operands_literal: bool,
) -> PsiOptimizationUnit {
    bitwise_neutral_identity_unit_with_type_and_liveness(
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        operation,
        literal_value,
        literal_left,
        both_operands_literal,
        true,
    )
}

pub(crate) fn bitwise_neutral_identity_unit_with_type_and_liveness(
    integer: IntegerType,
    operation: BitwiseNeutralOperation,
    literal_value: IntegerValue,
    literal_left: bool,
    both_operands_literal: bool,
    result_is_live: bool,
) -> PsiOptimizationUnit {
    let mut unit = wrapping_neutral_identity_unit_with_type_and_liveness(
        integer,
        WrappingNeutralOperation::Add,
        literal_value,
        literal_left,
        both_operands_literal,
        result_is_live,
    );
    let node = &mut unit.functions[0].blocks[0].nodes[1];
    let O::IntegerArithmetic {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
        arithmetic,
        ..
    } = node.operation
    else {
        unreachable!("fixture starts from wrapping addition")
    };
    if !arithmetic.is(
        IntegerOverflowPolicy::Wrapping,
        IntegerArithmeticOperator::Add,
    ) {
        unreachable!("fixture starts from wrapping addition")
    }
    node.operation = match operation {
        BitwiseNeutralOperation::And => O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        },
        BitwiseNeutralOperation::Or => O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        },
        BitwiseNeutralOperation::Xor => O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        },
    };
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    validate_psi_optimization_unit(&unit).unwrap();
    unit
}

pub(crate) fn saturating_neutral_identity_unit(
    operation: SaturatingNeutralOperation,
    literal_value: IntegerValue,
    literal_left: bool,
    both_operands_literal: bool,
) -> PsiOptimizationUnit {
    saturating_neutral_identity_unit_with_type_and_liveness(
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        operation,
        literal_value,
        literal_left,
        both_operands_literal,
        true,
    )
}

pub(crate) fn saturating_neutral_identity_unit_with_type_and_liveness(
    integer: IntegerType,
    operation: SaturatingNeutralOperation,
    literal_value: IntegerValue,
    literal_left: bool,
    both_operands_literal: bool,
    result_is_live: bool,
) -> PsiOptimizationUnit {
    let wrapping = match operation {
        SaturatingNeutralOperation::Add => WrappingNeutralOperation::Add,
        SaturatingNeutralOperation::Subtract => WrappingNeutralOperation::Subtract,
        SaturatingNeutralOperation::Multiply => WrappingNeutralOperation::Multiply,
    };
    let mut unit = wrapping_neutral_identity_unit_with_type_and_liveness(
        integer,
        wrapping,
        literal_value,
        literal_left,
        both_operands_literal,
        result_is_live,
    );
    let node = &mut unit.functions[0].blocks[0].nodes[1];
    let O::IntegerArithmetic { arithmetic, .. } = &mut node.operation else {
        unreachable!("fixture selected one arithmetic operation")
    };
    assert_eq!(arithmetic.policy(), IntegerOverflowPolicy::Wrapping);
    *arithmetic = IntegerArithmetic::new(
        IntegerOverflowPolicy::Saturating,
        arithmetic.operator(),
        None,
    )
    .expect("admitted integer arithmetic");
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    validate_psi_optimization_unit(&unit).unwrap();
    unit
}

pub(crate) fn wrapping_neutral_identity_unit(
    operation: WrappingNeutralOperation,
    literal_value: IntegerValue,
    literal_left: bool,
    both_operands_literal: bool,
) -> PsiOptimizationUnit {
    wrapping_neutral_identity_unit_with_type_and_liveness(
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        operation,
        literal_value,
        literal_left,
        both_operands_literal,
        true,
    )
}

pub(crate) fn wrapping_neutral_identity_unit_with_type_and_liveness(
    integer: IntegerType,
    operation: WrappingNeutralOperation,
    literal_value: IntegerValue,
    literal_left: bool,
    both_operands_literal: bool,
    result_is_live: bool,
) -> PsiOptimizationUnit {
    wrapping_neutral_identity_unit_with_value_and_identity_types_and_liveness(
        integer,
        integer,
        operation,
        literal_value,
        literal_left,
        both_operands_literal,
        result_is_live,
    )
}

pub(crate) fn wrapping_neutral_identity_unit_with_value_and_identity_types_and_liveness(
    value_type: IntegerType,
    identity_operand_type: IntegerType,
    operation: WrappingNeutralOperation,
    literal_value: IntegerValue,
    literal_left: bool,
    both_operands_literal: bool,
    result_is_live: bool,
) -> PsiOptimizationUnit {
    let machine = id(1_901, MachineId::new);
    let block = id(1_902, BlockId::new);
    let other = id(1_903, ValueId::new);
    let literal = id(1_904, ValueId::new);
    let result = id(1_905, ValueId::new);
    let literal_operation = id(1_906, OperationId::new);
    let identity_operation = id(1_907, OperationId::new);
    let scalar_type = ScalarType::Integer(value_type);
    let (left, right) = if both_operands_literal {
        (literal, literal)
    } else if literal_left {
        (literal, other)
    } else {
        (other, literal)
    };
    let operation = match operation {
        WrappingNeutralOperation::Add => O::IntegerArithmetic {
            psi_operation: identity_operation,
            result,
            scalar_type: value_type,
            arithmetic: IntegerArithmetic::new(
                IntegerOverflowPolicy::Wrapping,
                IntegerArithmeticOperator::Add,
                None,
            )
            .expect("admitted integer arithmetic"),
            left,
            right,
        },
        WrappingNeutralOperation::Subtract => O::IntegerArithmetic {
            psi_operation: identity_operation,
            result,
            scalar_type: value_type,
            arithmetic: IntegerArithmetic::new(
                IntegerOverflowPolicy::Wrapping,
                IntegerArithmeticOperator::Subtract,
                None,
            )
            .expect("admitted integer arithmetic"),
            left,
            right,
        },
        WrappingNeutralOperation::Multiply => O::IntegerArithmetic {
            psi_operation: identity_operation,
            result,
            scalar_type: value_type,
            arithmetic: IntegerArithmetic::new(
                IntegerOverflowPolicy::Wrapping,
                IntegerArithmeticOperator::Multiply,
                None,
            )
            .expect("admitted integer arithmetic"),
            left,
            right,
        },
        WrappingNeutralOperation::ShiftLeft => O::IntegerShift {
            psi_operation: identity_operation,
            result,
            value_type,
            count_type: identity_operand_type,
            policy: IntegerShiftPolicy::Wrapping,
            direction: IntegerShiftDirection::Left,
            value: other,
            count: literal,
        },
        WrappingNeutralOperation::ShiftRight => O::IntegerShift {
            psi_operation: identity_operation,
            result,
            value_type,
            count_type: identity_operand_type,
            policy: IntegerShiftPolicy::Wrapping,
            direction: IntegerShiftDirection::Right,
            value: other,
            count: literal,
        },
    };
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([49; 32]),
            },
            entry: machine,
            structural_types: Vec::new().into(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: vec![AbstractParameter {
                    value: other,
                    scalar_type,
                }],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: if result_is_live { result } else { other },
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    O::IntegerConstant {
                        psi_operation: literal_operation,
                        result: literal,
                        scalar_type: ScalarType::Integer(identity_operand_type),
                        value: literal_value,
                    },
                    operation,
                    O::Return {
                        psi_edge: id(1_908, EdgeId::new),
                        result: if result_is_live { result } else { other },
                        value: if result_is_live { result } else { other },
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

pub(crate) fn wrapping_multiply_literal_pair_unit(
    left_value: IntegerValue,
    right_value: IntegerValue,
) -> PsiOptimizationUnit {
    let machine = id(1_951, MachineId::new);
    let block = id(1_952, BlockId::new);
    let left = id(1_953, ValueId::new);
    let right = id(1_954, ValueId::new);
    let result = id(1_955, ValueId::new);
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([50; 32]),
            },
            entry: machine,
            structural_types: Vec::new().into(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    structural_parameters: Vec::new(),
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    O::IntegerConstant {
                        psi_operation: id(1_956, OperationId::new),
                        result: left,
                        scalar_type,
                        value: left_value,
                    },
                    O::IntegerConstant {
                        psi_operation: id(1_957, OperationId::new),
                        result: right,
                        scalar_type,
                        value: right_value,
                    },
                    O::IntegerArithmetic {
                        psi_operation: id(1_958, OperationId::new),
                        result,
                        scalar_type: integer,
                        arithmetic: IntegerArithmetic::new(
                            IntegerOverflowPolicy::Wrapping,
                            IntegerArithmeticOperator::Multiply,
                            None,
                        )
                        .expect("admitted integer arithmetic"),
                        left,
                        right,
                    },
                    O::Return {
                        psi_edge: id(1_959, EdgeId::new),
                        result,
                        value: result,
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

pub(crate) fn bitwise_literal_pair_unit(
    operation: BitwiseNeutralOperation,
    left_value: IntegerValue,
    right_value: IntegerValue,
) -> PsiOptimizationUnit {
    let mut unit = wrapping_multiply_literal_pair_unit(left_value, right_value);
    let node = &mut unit.functions[0].blocks[0].nodes[2];
    let O::IntegerArithmetic {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
        arithmetic,
        ..
    } = node.operation
    else {
        unreachable!("pair fixture starts from wrapping multiplication")
    };
    if !arithmetic.is(
        IntegerOverflowPolicy::Wrapping,
        IntegerArithmeticOperator::Multiply,
    ) {
        unreachable!("pair fixture starts from wrapping multiplication")
    }
    node.operation = match operation {
        BitwiseNeutralOperation::And => O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        },
        BitwiseNeutralOperation::Or => O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        },
        BitwiseNeutralOperation::Xor => O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        },
    };
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    validate_psi_optimization_unit(&unit).unwrap();
    unit
}
