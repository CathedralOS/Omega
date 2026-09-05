use super::{base_source, candidate_error};
use crate::{
    AbstractToTargetTranslationValidationError,
    StraightLineWrappingIntegerShiftRightParametersTranslationError,
};
use semantic_vocabulary::{
    EdgeId, IntegerSign, ObligationId, OperationId, StructuralTypeId, ValueId,
};
use target_operations::{
    ScalarParameterLocation, TargetIntegerExpression, TargetOperation, TerminalPsiProvenance,
};

#[test]
fn wrapping_shift_right_target_provenance_machine_and_attachment_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.operations.clear()),
        StraightLineWrappingIntegerShiftRightParametersTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.edges.clear()),
        StraightLineWrappingIntegerShiftRightParametersTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default()
        }),
        StraightLineWrappingIntegerShiftRightParametersTranslationError::TargetProvenance
    );

    let source = base_source();
    let target_profile = target::NativeTarget::linux_x64();
    let mut target = crate::lower_to_target_operations(&source, target_profile).unwrap();
    target.functions[0].machine = semantic_vocabulary::MachineId::new(54_600).unwrap();
    assert_eq!(
        crate::validate_abstract_to_target_translation(&source, target_profile, &target)
            .unwrap_err(),
        AbstractToTargetTranslationValidationError::FunctionMachineMismatch { position: 0 }
    );

    let mut target = crate::lower_to_target_operations(&source, target_profile).unwrap();
    target.functions[0].attachment = Some(StructuralTypeId::new(54_601).unwrap());
    assert_eq!(
        crate::validate_abstract_to_target_translation(&source, target_profile, &target)
            .unwrap_err(),
        AbstractToTargetTranslationValidationError::FunctionAttachmentMismatch {
            machine: source.functions[0].machine,
        }
    );
}

#[test]
fn wrapping_shift_right_target_outer_and_independent_type_corruption_fails_closed() {
    assert_target_operation_error(|operation| {
        let TargetOperation::ReturnIntegerExpression { psi_edge, .. } = operation else {
            unreachable!()
        };
        *psi_edge = EdgeId::new(54_602).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetOperation::ReturnIntegerExpression { source_value, .. } = operation else {
            unreachable!()
        };
        *source_value = ValueId::new(54_603).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetOperation::ReturnIntegerExpression { scalar_type, .. } = operation else {
            unreachable!()
        };
        *scalar_type = super::integer_type(IntegerSign::Unsigned, 64);
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::WrappingShiftRight { psi_operation, .. } =
            expression(operation)
        else {
            unreachable!()
        };
        *psi_operation = OperationId::new(54_604).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::WrappingShiftRight { count_type, .. } = expression(operation)
        else {
            unreachable!()
        };
        *count_type = super::integer_type(IntegerSign::Signed, 8);
    });
}

#[test]
fn wrapping_shift_right_target_nested_value_and_count_corruption_fails_closed() {
    for mutate_value in [true, false] {
        assert_target_operation_error(|operation| {
            let operand = operand(expression(operation), mutate_value);
            let TargetIntegerExpression::Parameter {
                parameter_index, ..
            } = operand
            else {
                unreachable!()
            };
            *parameter_index = if mutate_value { 1 } else { 0 };
        });
        assert_target_operation_error(|operation| {
            let operand = operand(expression(operation), mutate_value);
            let TargetIntegerExpression::Parameter { source_value, .. } = operand else {
                unreachable!()
            };
            *source_value = ValueId::new(if mutate_value { 54_605 } else { 54_606 }).unwrap();
        });
        assert_target_operation_error(|operation| {
            let operand = operand(expression(operation), mutate_value);
            let TargetIntegerExpression::Parameter { location, .. } = operand else {
                unreachable!()
            };
            *location = ScalarParameterLocation::IncomingStack { byte_offset: 0 };
        });
        assert_target_operation_error(|operation| {
            let operand = operand(expression(operation), mutate_value);
            let nested = operand.clone();
            *operand = TargetIntegerExpression::BitwiseNot {
                psi_operation: OperationId::new(54_607).unwrap(),
                operand: Box::new(nested),
            };
        });
    }
}

#[test]
fn wrapping_shift_right_rejects_swaps_and_all_other_binary_expression_variants() {
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::WrappingShiftRight { value, count, .. } =
            expression(operation)
        else {
            unreachable!()
        };
        std::mem::swap(value, count);
    });
    for substitute in 1..=21 {
        assert_target_operation_error(|operation| substitute_expression(operation, substitute));
    }
}

fn substitute_expression(operation: &mut TargetOperation, substitute: u8) {
    let expression = expression(operation);
    let TargetIntegerExpression::WrappingShiftRight {
        psi_operation,
        count_type,
        value,
        count,
    } = expression
    else {
        unreachable!()
    };
    let psi_operation = *psi_operation;
    let count_type = *count_type;
    let value = value.clone();
    let count = count.clone();
    let obligation = ObligationId::new(54_608).unwrap();
    *expression = match substitute {
        1 => TargetIntegerExpression::WrappingShiftLeft {
            psi_operation,
            count_type,
            value,
            count,
        },
        2 => TargetIntegerExpression::ExactShiftLeft {
            psi_operation,
            obligation,
            count_type,
            value,
            count,
        },
        3 => TargetIntegerExpression::ExactShiftRight {
            psi_operation,
            obligation,
            count_type,
            value,
            count,
        },
        4 => binary_bitwise(psi_operation, value, count, 0),
        5 => binary_bitwise(psi_operation, value, count, 1),
        6 => binary_bitwise(psi_operation, value, count, 2),
        7 => binary_arithmetic(psi_operation, obligation, value, count, 0),
        8 => binary_arithmetic(psi_operation, obligation, value, count, 1),
        9 => binary_arithmetic(psi_operation, obligation, value, count, 2),
        10 => binary_arithmetic(psi_operation, obligation, value, count, 3),
        11 => binary_arithmetic(psi_operation, obligation, value, count, 4),
        12 => binary_arithmetic(psi_operation, obligation, value, count, 5),
        13 => binary_arithmetic(psi_operation, obligation, value, count, 6),
        14 => binary_arithmetic(psi_operation, obligation, value, count, 7),
        15 => binary_arithmetic(psi_operation, obligation, value, count, 8),
        16 => binary_arithmetic(psi_operation, obligation, value, count, 9),
        17 => binary_arithmetic(psi_operation, obligation, value, count, 10),
        18 => binary_arithmetic(psi_operation, obligation, value, count, 11),
        19 => binary_arithmetic(psi_operation, obligation, value, count, 12),
        20 => binary_arithmetic(psi_operation, obligation, value, count, 13),
        21 => binary_arithmetic(psi_operation, obligation, value, count, 14),
        _ => unreachable!(),
    };
}

fn binary_bitwise(
    psi_operation: OperationId,
    left: Box<TargetIntegerExpression>,
    right: Box<TargetIntegerExpression>,
    kind: u8,
) -> TargetIntegerExpression {
    match kind {
        0 => TargetIntegerExpression::BitwiseAnd {
            psi_operation,
            left,
            right,
        },
        1 => TargetIntegerExpression::BitwiseOr {
            psi_operation,
            left,
            right,
        },
        2 => TargetIntegerExpression::BitwiseXor {
            psi_operation,
            left,
            right,
        },
        _ => unreachable!(),
    }
}

fn binary_arithmetic(
    psi_operation: OperationId,
    obligation: ObligationId,
    left: Box<TargetIntegerExpression>,
    right: Box<TargetIntegerExpression>,
    kind: u8,
) -> TargetIntegerExpression {
    match kind {
        0 => TargetIntegerExpression::WrappingAdd {
            psi_operation,
            left,
            right,
        },
        1 => TargetIntegerExpression::ExactAdd {
            psi_operation,
            obligation,
            left,
            right,
        },
        2 => TargetIntegerExpression::SaturatingAdd {
            psi_operation,
            left,
            right,
        },
        3 => TargetIntegerExpression::WrappingSubtract {
            psi_operation,
            left,
            right,
        },
        4 => TargetIntegerExpression::ExactSubtract {
            psi_operation,
            obligation,
            left,
            right,
        },
        5 => TargetIntegerExpression::SaturatingSubtract {
            psi_operation,
            left,
            right,
        },
        6 => TargetIntegerExpression::WrappingMultiply {
            psi_operation,
            left,
            right,
        },
        7 => TargetIntegerExpression::ExactMultiply {
            psi_operation,
            obligation,
            left,
            right,
        },
        8 => TargetIntegerExpression::SaturatingMultiply {
            psi_operation,
            left,
            right,
        },
        9 => TargetIntegerExpression::ExactDivide {
            psi_operation,
            obligation,
            left,
            right,
        },
        10 => TargetIntegerExpression::WrappingDivide {
            psi_operation,
            obligation,
            left,
            right,
        },
        11 => TargetIntegerExpression::SaturatingDivide {
            psi_operation,
            obligation,
            left,
            right,
        },
        12 => TargetIntegerExpression::ExactRemainder {
            psi_operation,
            obligation,
            left,
            right,
        },
        13 => TargetIntegerExpression::WrappingRemainder {
            psi_operation,
            obligation,
            left,
            right,
        },
        14 => TargetIntegerExpression::SaturatingRemainder {
            psi_operation,
            obligation,
            left,
            right,
        },
        _ => unreachable!(),
    }
}

fn operand(
    expression: &mut TargetIntegerExpression,
    value_operand: bool,
) -> &mut TargetIntegerExpression {
    let TargetIntegerExpression::WrappingShiftRight { value, count, .. } = expression else {
        unreachable!()
    };
    if value_operand {
        value.as_mut()
    } else {
        count.as_mut()
    }
}

fn assert_target_operation_error(mutate: impl FnOnce(&mut TargetOperation)) {
    assert_eq!(
        candidate_error(|target| mutate(&mut target.functions[0].operation)),
        StraightLineWrappingIntegerShiftRightParametersTranslationError::TargetOperation
    );
}

fn expression(operation: &mut TargetOperation) -> &mut TargetIntegerExpression {
    let TargetOperation::ReturnIntegerExpression { expression, .. } = operation else {
        unreachable!()
    };
    expression
}
