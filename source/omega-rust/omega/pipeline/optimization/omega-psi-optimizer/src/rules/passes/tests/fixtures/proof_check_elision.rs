//! Proof-check-elision fixture programs.

use super::*;

pub(crate) fn exact_add_unit() -> PsiOptimizationUnit {
    exact_chain_unit(false)
}

pub(crate) fn live_exact_add_zero_unit() -> PsiOptimizationUnit {
    let mut unit = exact_add_unit();
    let O::IntegerConstant { value, .. } = &mut unit.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    *value = IntegerValue::Unsigned(0);
    for fact in &mut unit.functions[0].facts {
        if let OptimizationFact::IntegerConstant {
            value, constant, ..
        } = fact
            && *value == id(304, ValueId::new)
        {
            *constant = IntegerValue::Unsigned(0);
        }
    }
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    validate_psi_optimization_unit(&unit).unwrap();
    unit
}

pub(crate) fn live_divide_by_one_unit(
    integer: IntegerType,
    make_operation: impl FnOnce(
        OperationId,
        ObligationId,
        ValueId,
        IntegerType,
        ValueId,
        ValueId,
    ) -> AbstractOperation,
) -> PsiOptimizationUnit {
    live_proof_binary_identity_unit(integer, integer_one(integer), false, make_operation)
}

pub(crate) fn live_exact_multiply_by_zero_unit(
    integer: IntegerType,
    zero_left: bool,
) -> PsiOptimizationUnit {
    live_proof_binary_identity_unit(
        integer,
        integer_zero(integer),
        zero_left,
        |psi_operation, obligation, result, scalar_type, left, right| {
            AbstractOperation::ExactIntegerMultiply {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            }
        },
    )
}

pub(crate) fn live_zero_dividend_unit(
    integer: IntegerType,
    make_operation: impl FnOnce(
        OperationId,
        ObligationId,
        ValueId,
        IntegerType,
        ValueId,
        ValueId,
    ) -> AbstractOperation,
) -> PsiOptimizationUnit {
    live_proof_binary_identity_unit(integer, integer_zero(integer), true, make_operation)
}

pub(crate) fn live_remainder_by_one_unit(
    integer: IntegerType,
    policy: SelfRemainderPolicy,
) -> PsiOptimizationUnit {
    live_proof_binary_identity_unit(
        integer,
        integer_one(integer),
        false,
        |psi_operation, obligation, result, scalar_type, left, right| match policy {
            SelfRemainderPolicy::Exact => AbstractOperation::ExactIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            SelfRemainderPolicy::Wrapping => AbstractOperation::WrappingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            SelfRemainderPolicy::Saturating => AbstractOperation::SaturatingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
        },
    )
}

pub(crate) fn live_signed_remainder_by_negative_one_unit(
    integer: IntegerType,
    policy: SelfRemainderPolicy,
) -> PsiOptimizationUnit {
    live_proof_binary_identity_unit(
        integer,
        IntegerValue::Signed(-1),
        false,
        |psi_operation, obligation, result, scalar_type, left, right| match policy {
            SelfRemainderPolicy::Exact => AbstractOperation::ExactIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            SelfRemainderPolicy::Wrapping => AbstractOperation::WrappingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
            SelfRemainderPolicy::Saturating => AbstractOperation::SaturatingIntegerRemainder {
                psi_operation,
                obligation,
                result,
                scalar_type,
                left,
                right,
            },
        },
    )
}

pub(crate) fn live_exact_zero_value_shift_unit(
    integer: IntegerType,
    left_shift: bool,
) -> PsiOptimizationUnit {
    live_proof_binary_identity_unit(
        integer,
        integer_zero(integer),
        true,
        |psi_operation, obligation, result, value_type, value, count| {
            if left_shift {
                AbstractOperation::ExactIntegerShiftLeft {
                    psi_operation,
                    obligation,
                    result,
                    value_type,
                    count_type: integer,
                    value,
                    count,
                }
            } else {
                AbstractOperation::ExactIntegerShiftRight {
                    psi_operation,
                    obligation,
                    result,
                    value_type,
                    count_type: integer,
                    value,
                    count,
                }
            }
        },
    )
}

pub(crate) fn live_exact_signed_negative_one_shift_right_unit(
    integer: IntegerType,
) -> PsiOptimizationUnit {
    live_proof_binary_identity_unit(
        integer,
        IntegerValue::Signed(-1),
        true,
        |psi_operation, obligation, result, value_type, value, count| {
            AbstractOperation::ExactIntegerShiftRight {
                psi_operation,
                obligation,
                result,
                value_type,
                count_type: integer,
                value,
                count,
            }
        },
    )
}

pub(crate) fn live_exact_self_subtract_unit(integer: IntegerType) -> PsiOptimizationUnit {
    let machine = id(331, MachineId::new);
    let block = id(332, BlockId::new);
    let operand = id(333, ValueId::new);
    let result = id(334, ValueId::new);
    let operation = id(335, OperationId::new);
    let obligation = id(336, ObligationId::new);
    let scalar_type = ScalarType::Integer(integer);
    let unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([33; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            placed_view_inputs: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: vec![AbstractParameter {
                    value: operand,
                    scalar_type,
                }],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::ExactIntegerSubtract {
                        psi_operation: operation,
                        obligation,
                        result,
                        scalar_type: integer,
                        left: operand,
                        right: operand,
                    },
                    AbstractOperation::Return {
                        psi_edge: id(337, EdgeId::new),
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
    .unwrap();
    with_synthetic_accepted_obligations(unit)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelfRemainderPolicy {
    Exact,
    Wrapping,
    Saturating,
}

pub(crate) fn live_self_remainder_unit(
    integer: IntegerType,
    policy: SelfRemainderPolicy,
) -> PsiOptimizationUnit {
    let machine = id(341, MachineId::new);
    let block = id(342, BlockId::new);
    let operand = id(343, ValueId::new);
    let result = id(344, ValueId::new);
    let operation = id(345, OperationId::new);
    let obligation = id(346, ObligationId::new);
    let scalar_type = ScalarType::Integer(integer);
    let operation = match policy {
        SelfRemainderPolicy::Exact => AbstractOperation::ExactIntegerRemainder {
            psi_operation: operation,
            obligation,
            result,
            scalar_type: integer,
            left: operand,
            right: operand,
        },
        SelfRemainderPolicy::Wrapping => AbstractOperation::WrappingIntegerRemainder {
            psi_operation: operation,
            obligation,
            result,
            scalar_type: integer,
            left: operand,
            right: operand,
        },
        SelfRemainderPolicy::Saturating => AbstractOperation::SaturatingIntegerRemainder {
            psi_operation: operation,
            obligation,
            result,
            scalar_type: integer,
            left: operand,
            right: operand,
        },
    };
    let unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([34; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            placed_view_inputs: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: vec![AbstractParameter {
                    value: operand,
                    scalar_type,
                }],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    operation,
                    AbstractOperation::Return {
                        psi_edge: id(347, EdgeId::new),
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
    .unwrap();
    with_synthetic_accepted_obligations(unit)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelfDividePolicy {
    Exact,
    Wrapping,
    Saturating,
}

pub(crate) fn live_self_divide_unit(
    integer: IntegerType,
    policy: SelfDividePolicy,
) -> PsiOptimizationUnit {
    let machine = id(351, MachineId::new);
    let block = id(352, BlockId::new);
    let operand = id(353, ValueId::new);
    let result = id(354, ValueId::new);
    let operation = id(355, OperationId::new);
    let obligation = id(356, ObligationId::new);
    let scalar_type = ScalarType::Integer(integer);
    let operation = match policy {
        SelfDividePolicy::Exact => AbstractOperation::ExactIntegerDivide {
            psi_operation: operation,
            obligation,
            result,
            scalar_type: integer,
            left: operand,
            right: operand,
        },
        SelfDividePolicy::Wrapping => AbstractOperation::WrappingIntegerDivide {
            psi_operation: operation,
            obligation,
            result,
            scalar_type: integer,
            left: operand,
            right: operand,
        },
        SelfDividePolicy::Saturating => AbstractOperation::SaturatingIntegerDivide {
            psi_operation: operation,
            obligation,
            result,
            scalar_type: integer,
            left: operand,
            right: operand,
        },
    };
    let unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([35; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            placed_view_inputs: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: vec![AbstractParameter {
                    value: operand,
                    scalar_type,
                }],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: result,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    operation,
                    AbstractOperation::Return {
                        psi_edge: id(357, EdgeId::new),
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
    .unwrap();
    with_synthetic_accepted_obligations(unit)
}

pub(crate) fn live_proof_binary_identity_unit(
    integer: IntegerType,
    literal_value: IntegerValue,
    literal_left: bool,
    make_operation: impl FnOnce(
        OperationId,
        ObligationId,
        ValueId,
        IntegerType,
        ValueId,
        ValueId,
    ) -> AbstractOperation,
) -> PsiOptimizationUnit {
    let machine = id(321, MachineId::new);
    let block = id(322, BlockId::new);
    let other = id(323, ValueId::new);
    let literal = id(324, ValueId::new);
    let computed = id(325, ValueId::new);
    let literal_operation = id(326, OperationId::new);
    let binary_operation = id(327, OperationId::new);
    let obligation = id(328, ObligationId::new);
    let scalar_type = ScalarType::Integer(integer);
    let (left, right) = if literal_left {
        (literal, other)
    } else {
        (other, literal)
    };
    let unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([31; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            placed_view_inputs: Vec::new(),
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
                    value: computed,
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: literal_operation,
                        result: literal,
                        scalar_type,
                        value: literal_value,
                    },
                    make_operation(binary_operation, obligation, computed, integer, left, right),
                    AbstractOperation::Return {
                        psi_edge: id(329, EdgeId::new),
                        result: computed,
                        value: computed,
                        scalar_type,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    with_synthetic_accepted_obligations(unit)
}

pub(crate) fn dependent_exact_chain_unit() -> PsiOptimizationUnit {
    exact_chain_unit(true)
}

pub(crate) fn exact_chain_unit(include_multiply: bool) -> PsiOptimizationUnit {
    let machine = id(301, MachineId::new);
    let block = id(302, BlockId::new);
    let left = id(303, ValueId::new);
    let right = id(304, ValueId::new);
    let sum = id(305, ValueId::new);
    let product = id(311, ValueId::new);
    let result = if include_multiply { product } else { sum };
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let mut operations = vec![
        AbstractOperation::IntegerConstant {
            psi_operation: id(306, OperationId::new),
            result: left,
            scalar_type: ScalarType::Integer(integer),
            value: IntegerValue::Unsigned(7),
        },
        AbstractOperation::IntegerConstant {
            psi_operation: id(307, OperationId::new),
            result: right,
            scalar_type: ScalarType::Integer(integer),
            value: IntegerValue::Unsigned(8),
        },
        AbstractOperation::ExactIntegerAdd {
            psi_operation: id(308, OperationId::new),
            obligation: id(309, ObligationId::new),
            result: sum,
            scalar_type: integer,
            left,
            right,
        },
    ];
    if include_multiply {
        operations.push(AbstractOperation::ExactIntegerMultiply {
            psi_operation: id(312, OperationId::new),
            obligation: id(313, ObligationId::new),
            result: product,
            scalar_type: integer,
            left: sum,
            right,
        });
    }
    operations.push(AbstractOperation::Return {
        psi_edge: id(310, EdgeId::new),
        result,
        value: result,
        scalar_type: ScalarType::Integer(integer),
        cleanup_actions: Vec::new(),
    });
    let unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([13; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            placed_view_inputs: Vec::new(),
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
                    scalar_type: ScalarType::Integer(integer),
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations,
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    with_synthetic_accepted_obligations(unit)
}
