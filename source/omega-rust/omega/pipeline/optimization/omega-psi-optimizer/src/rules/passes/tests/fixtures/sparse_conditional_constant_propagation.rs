//! SCCP fixture programs.

use super::*;

mod binary;
mod boolean;
mod unary;

pub(crate) use binary::*;
pub(crate) use boolean::*;
pub(crate) use unary::*;

pub(crate) fn policy_add_unit(saturating: bool) -> PsiOptimizationUnit {
    let mut unit = exact_add_unit();
    let function = &mut unit.functions[0];
    let block = &mut function.blocks[0];
    let O::IntegerConstant { value, .. } = &mut block.nodes[0].operation else {
        unreachable!()
    };
    *value = IntegerValue::Unsigned(250);
    let O::IntegerConstant { value, .. } = &mut block.nodes[1].operation else {
        unreachable!()
    };
    *value = IntegerValue::Unsigned(10);
    let O::ExactIntegerAdd {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
        ..
    } = block.nodes[2].operation
    else {
        unreachable!()
    };
    block.nodes[2].operation = if saturating {
        O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
    } else {
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
    };
    let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[0] else {
        unreachable!()
    };
    *constant = IntegerValue::Unsigned(250);
    let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[1] else {
        unreachable!()
    };
    *constant = IntegerValue::Unsigned(10);
    function.facts.truncate(2);
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    unit
}

pub(crate) fn wrapping_add_unit() -> PsiOptimizationUnit {
    policy_add_unit(false)
}

#[derive(Clone, Copy)]
pub(crate) enum BitwiseFixtureKind {
    And,
    Or,
    Xor,
}

pub(crate) fn bitwise_unit(kind: BitwiseFixtureKind) -> PsiOptimizationUnit {
    let mut unit = exact_add_unit();
    let function = &mut unit.functions[0];
    let block = &mut function.blocks[0];
    let O::IntegerConstant { value, .. } = &mut block.nodes[0].operation else {
        unreachable!()
    };
    *value = IntegerValue::Unsigned(0b1010);
    let O::IntegerConstant { value, .. } = &mut block.nodes[1].operation else {
        unreachable!()
    };
    *value = IntegerValue::Unsigned(0b1100);
    let O::ExactIntegerAdd {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
        ..
    } = block.nodes[2].operation
    else {
        unreachable!()
    };
    block.nodes[2].operation = match kind {
        BitwiseFixtureKind::And => O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        },
        BitwiseFixtureKind::Or => O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        },
        BitwiseFixtureKind::Xor => O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        },
    };
    let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[0] else {
        unreachable!()
    };
    *constant = IntegerValue::Unsigned(0b1010);
    let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[1] else {
        unreachable!()
    };
    *constant = IntegerValue::Unsigned(0b1100);
    function.facts.truncate(2);
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    unit
}

#[derive(Clone, Copy)]
pub(crate) enum ShiftFixtureKind {
    ExactLeft,
    ExactRight,
    WrappingLeft,
    WrappingRight,
}

pub(crate) fn shift_unit(kind: ShiftFixtureKind, value: u128, count: u128) -> PsiOptimizationUnit {
    let mut unit = exact_add_unit();
    let function = &mut unit.functions[0];
    let block = &mut function.blocks[0];
    let O::IntegerConstant {
        value: left_value, ..
    } = &mut block.nodes[0].operation
    else {
        unreachable!()
    };
    *left_value = IntegerValue::Unsigned(value);
    let O::IntegerConstant {
        value: right_value, ..
    } = &mut block.nodes[1].operation
    else {
        unreachable!()
    };
    *right_value = IntegerValue::Unsigned(count);
    let O::ExactIntegerAdd {
        psi_operation,
        obligation,
        result,
        scalar_type,
        left,
        right,
    } = block.nodes[2].operation
    else {
        unreachable!()
    };
    block.nodes[2].operation = match kind {
        ShiftFixtureKind::ExactLeft => O::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type: scalar_type,
            count_type: scalar_type,
            value: left,
            count: right,
        },
        ShiftFixtureKind::ExactRight => O::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            result,
            value_type: scalar_type,
            count_type: scalar_type,
            value: left,
            count: right,
        },
        ShiftFixtureKind::WrappingLeft => O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type: scalar_type,
            count_type: scalar_type,
            value: left,
            count: right,
        },
        ShiftFixtureKind::WrappingRight => O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type: scalar_type,
            count_type: scalar_type,
            value: left,
            count: right,
        },
    };
    let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[0] else {
        unreachable!()
    };
    *constant = IntegerValue::Unsigned(value);
    let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[1] else {
        unreachable!()
    };
    *constant = IntegerValue::Unsigned(count);
    if matches!(
        kind,
        ShiftFixtureKind::WrappingLeft | ShiftFixtureKind::WrappingRight
    ) {
        function.facts.truncate(2);
    }
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    unit
}

pub(crate) fn exact_divide_unit(zero_divisor: bool) -> PsiOptimizationUnit {
    let mut unit = exact_add_unit();
    let function = &mut unit.functions[0];
    let block = &mut function.blocks[0];
    let O::ExactIntegerAdd {
        psi_operation,
        obligation,
        result,
        scalar_type,
        left,
        right,
    } = block.nodes[2].operation
    else {
        unreachable!()
    };
    block.nodes[2].operation = O::ExactIntegerDivide {
        psi_operation,
        obligation,
        result,
        scalar_type,
        left,
        right,
    };
    if zero_divisor {
        let O::IntegerConstant { value, .. } = &mut block.nodes[1].operation else {
            unreachable!()
        };
        *value = IntegerValue::Unsigned(0);
        let OptimizationFact::IntegerConstant { constant, .. } = &mut function.facts[1] else {
            unreachable!()
        };
        *constant = IntegerValue::Unsigned(0);
    }
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    unit
}

pub(crate) fn exact_cast_unit(value: u128) -> PsiOptimizationUnit {
    let machine = id(321, MachineId::new);
    let block = id(322, BlockId::new);
    let operand = id(323, ValueId::new);
    let result = id(324, ValueId::new);
    let source_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let target_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([14; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
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
                    scalar_type: ScalarType::Integer(target_type),
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
                        psi_operation: id(325, OperationId::new),
                        result: operand,
                        scalar_type: ScalarType::Integer(source_type),
                        value: IntegerValue::Unsigned(value),
                    },
                    AbstractOperation::IntegerExactCast {
                        psi_operation: id(326, OperationId::new),
                        obligation: id(327, ObligationId::new),
                        result,
                        source_type,
                        target_type,
                        operand,
                    },
                    AbstractOperation::Return {
                        psi_edge: id(328, EdgeId::new),
                        result,
                        value: result,
                        scalar_type: ScalarType::Integer(target_type),
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

pub(crate) fn goal_free_unary_unit(widen: bool) -> PsiOptimizationUnit {
    let machine = id(331, MachineId::new);
    let block = id(332, BlockId::new);
    let operand = id(333, ValueId::new);
    let result = id(334, ValueId::new);
    let source_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let target_type = if widen {
        IntegerType::new(IntegerSign::Unsigned, 16).unwrap()
    } else {
        source_type
    };
    let unary = if widen {
        AbstractOperation::IntegerWiden {
            psi_operation: id(336, OperationId::new),
            result,
            source_type,
            target_type,
            operand,
        }
    } else {
        AbstractOperation::IntegerBitwiseNot {
            psi_operation: id(336, OperationId::new),
            result,
            scalar_type: source_type,
            operand,
        }
    };
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([15; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
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
                    scalar_type: ScalarType::Integer(target_type),
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
                        psi_operation: id(335, OperationId::new),
                        result: operand,
                        scalar_type: ScalarType::Integer(source_type),
                        value: IntegerValue::Unsigned(15),
                    },
                    unary,
                    AbstractOperation::Return {
                        psi_edge: id(337, EdgeId::new),
                        result,
                        value: result,
                        scalar_type: ScalarType::Integer(target_type),
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

pub(crate) fn proof_range_pair_comparison_unit() -> PsiOptimizationUnit {
    let machine = id(361, MachineId::new);
    let block = id(362, BlockId::new);
    let left_value = id(363, ValueId::new);
    let right_value = id(364, ValueId::new);
    let left_count = id(365, ValueId::new);
    let right_count = id(366, ValueId::new);
    let left_shift = id(367, ValueId::new);
    let right_shift = id(368, ValueId::new);
    let result = id(369, ValueId::new);
    let left_operation = id(370, OperationId::new);
    let right_operation = id(371, OperationId::new);
    let left_obligation = id(372, ObligationId::new);
    let right_obligation = id(373, ObligationId::new);
    let one_bit = IntegerType::new(IntegerSign::Unsigned, 1).unwrap();
    let eight_bit = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let unit = reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([18; 32]),
            },
            entry: machine,
            structural_types: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions: vec![AbstractFunction {
                machine,
                attachment: None,
                entry: block,
                parameters: vec![
                    AbstractParameter {
                        value: left_value,
                        scalar_type: ScalarType::Integer(one_bit),
                    },
                    AbstractParameter {
                        value: right_value,
                        scalar_type: ScalarType::Integer(eight_bit),
                    },
                    AbstractParameter {
                        value: left_count,
                        scalar_type: ScalarType::Integer(eight_bit),
                    },
                    AbstractParameter {
                        value: right_count,
                        scalar_type: ScalarType::Integer(eight_bit),
                    },
                ],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: result,
                    scalar_type: ScalarType::Boolean,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::ExactIntegerShiftRight {
                        psi_operation: left_operation,
                        obligation: left_obligation,
                        result: left_shift,
                        value_type: one_bit,
                        count_type: eight_bit,
                        value: left_value,
                        count: left_count,
                    },
                    AbstractOperation::ExactIntegerShiftRight {
                        psi_operation: right_operation,
                        obligation: right_obligation,
                        result: right_shift,
                        value_type: eight_bit,
                        count_type: eight_bit,
                        value: right_value,
                        count: right_count,
                    },
                    AbstractOperation::IntegerLessOrEqual {
                        psi_operation: id(374, OperationId::new),
                        result,
                        left: left_count,
                        right: right_count,
                    },
                    AbstractOperation::Return {
                        psi_edge: id(375, EdgeId::new),
                        result,
                        value: result,
                        scalar_type: ScalarType::Boolean,
                        cleanup_actions: Vec::new(),
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    let proof_bundle_fingerprint = [30; 32];
    let rows = [
        (left_operation, left_obligation, one_bit, left_count),
        (right_operation, right_obligation, eight_bit, right_count),
    ];
    let mut facts = Vec::new();
    let mut questions = Vec::new();
    for (operation, obligation, value_type, count) in rows {
        let proposition = psi_terminal_semantics::CanonicalScalarGoal::ExactShiftCount {
            value_type,
            count_type: eight_bit,
            count: psi_core::ScalarTerm::value(count, ScalarType::Integer(eight_bit)),
        }
        .kernel_proposition()
        .unwrap();
        let proposition =
            psi_terminal_codec::canonical_proposition_order_key(&proposition).unwrap();
        facts.push(AcceptedObligationFact::new(
            unit.psi,
            proof_bundle_fingerprint,
            machine,
            operation,
            obligation,
            proposition.clone(),
        ));
        questions.push(ProofQuestion::new(
            unit.psi,
            proof_bundle_fingerprint,
            ProofQuestionOwner::Operation { machine, operation },
            obligation,
            ProofQuestionClass::Derivable,
            proposition,
            Vec::new(),
            Vec::new(),
            true,
        ));
    }
    let unit = attach_accepted_obligation_facts(unit, facts).unwrap();
    attach_proof_questions(unit, questions).unwrap()
}
