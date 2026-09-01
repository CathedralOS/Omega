use super::{candidate_error, integer_type};
use crate::StraightLineSaturatingIntegerRemainderParametersTranslationError;
use omega_target_operations::{
    ScalarParameterLocation, TargetIntegerExpression, TargetOperation, TerminalPsiProvenance,
};
use psi_core::{EdgeId, IntegerSign, ObligationId, OperationId, ValueId};

#[test]
fn saturating_integer_remainder_target_provenance_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.operations.clear()),
        StraightLineSaturatingIntegerRemainderParametersTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.edges.clear()),
        StraightLineSaturatingIntegerRemainderParametersTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default()
        }),
        StraightLineSaturatingIntegerRemainderParametersTranslationError::TargetProvenance
    );
}

#[test]
fn saturating_integer_remainder_target_outer_expression_and_obligation_corruption_fails_closed() {
    assert_target_operation_error(|operation| {
        let TargetOperation::ReturnIntegerExpression { psi_edge, .. } = operation else {
            unreachable!()
        };
        *psi_edge = EdgeId::new(52_509).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetOperation::ReturnIntegerExpression { source_value, .. } = operation else {
            unreachable!()
        };
        *source_value = ValueId::new(52_509).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetOperation::ReturnIntegerExpression { scalar_type, .. } = operation else {
            unreachable!()
        };
        *scalar_type = integer_type(IntegerSign::Signed, 64);
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::SaturatingRemainder { psi_operation, .. } =
            expression(operation)
        else {
            unreachable!()
        };
        *psi_operation = OperationId::new(52_510).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::SaturatingRemainder { obligation, .. } = expression(operation)
        else {
            unreachable!()
        };
        *obligation = ObligationId::new(52_511).unwrap();
    });
}

#[test]
fn saturating_integer_remainder_target_nested_operand_corruption_fails_closed() {
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::SaturatingRemainder { left, .. } = expression(operation)
        else {
            unreachable!()
        };
        let TargetIntegerExpression::Parameter {
            parameter_index, ..
        } = left.as_mut()
        else {
            unreachable!()
        };
        *parameter_index = 1;
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::SaturatingRemainder { left, .. } = expression(operation)
        else {
            unreachable!()
        };
        let TargetIntegerExpression::Parameter { source_value, .. } = left.as_mut() else {
            unreachable!()
        };
        *source_value = ValueId::new(52_512).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::SaturatingRemainder { right, .. } = expression(operation)
        else {
            unreachable!()
        };
        let TargetIntegerExpression::Parameter {
            parameter_index, ..
        } = right.as_mut()
        else {
            unreachable!()
        };
        *parameter_index = 0;
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::SaturatingRemainder { right, .. } = expression(operation)
        else {
            unreachable!()
        };
        let TargetIntegerExpression::Parameter { source_value, .. } = right.as_mut() else {
            unreachable!()
        };
        *source_value = ValueId::new(52_513).unwrap();
    });
    for mutate_left in [true, false] {
        assert_target_operation_error(|operation| {
            let TargetIntegerExpression::SaturatingRemainder { left, right, .. } =
                expression(operation)
            else {
                unreachable!()
            };
            let operand = if mutate_left { left } else { right };
            let TargetIntegerExpression::Parameter { location, .. } = operand.as_mut() else {
                unreachable!()
            };
            *location = ScalarParameterLocation::IncomingStack { byte_offset: 0 };
        });
    }
    for mutate_left in [true, false] {
        assert_target_operation_error(|operation| {
            let TargetIntegerExpression::SaturatingRemainder { left, right, .. } =
                expression(operation)
            else {
                unreachable!()
            };
            let operand = if mutate_left { left } else { right };
            let nested = operand.clone();
            *operand = Box::new(TargetIntegerExpression::BitwiseNot {
                psi_operation: OperationId::new(52_514).unwrap(),
                operand: nested,
            });
        });
    }
}

#[test]
fn saturating_integer_remainder_rejects_swapped_operands_and_semantic_substitutions() {
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::SaturatingRemainder { left, right, .. } =
            expression(operation)
        else {
            unreachable!()
        };
        std::mem::swap(left, right);
    });
    for substitute in 1..=14 {
        assert_target_operation_error(|operation| {
            let expression = expression(operation);
            let TargetIntegerExpression::SaturatingRemainder {
                psi_operation,
                obligation,
                left,
                right,
            } = expression
            else {
                unreachable!()
            };
            *expression = match substitute {
                1 => TargetIntegerExpression::ExactDivide {
                    psi_operation: *psi_operation,
                    obligation: *obligation,
                    left: left.clone(),
                    right: right.clone(),
                },
                2 => TargetIntegerExpression::WrappingDivide {
                    psi_operation: *psi_operation,
                    obligation: *obligation,
                    left: left.clone(),
                    right: right.clone(),
                },
                3 => TargetIntegerExpression::ExactRemainder {
                    psi_operation: *psi_operation,
                    obligation: *obligation,
                    left: left.clone(),
                    right: right.clone(),
                },
                4 => TargetIntegerExpression::WrappingRemainder {
                    psi_operation: *psi_operation,
                    obligation: *obligation,
                    left: left.clone(),
                    right: right.clone(),
                },
                5 => TargetIntegerExpression::SaturatingDivide {
                    psi_operation: *psi_operation,
                    obligation: *obligation,
                    left: left.clone(),
                    right: right.clone(),
                },
                6 => TargetIntegerExpression::ExactAdd {
                    psi_operation: *psi_operation,
                    obligation: ObligationId::new(52_515).unwrap(),
                    left: left.clone(),
                    right: right.clone(),
                },
                7 => TargetIntegerExpression::WrappingAdd {
                    psi_operation: *psi_operation,
                    left: left.clone(),
                    right: right.clone(),
                },
                8 => TargetIntegerExpression::SaturatingAdd {
                    psi_operation: *psi_operation,
                    left: left.clone(),
                    right: right.clone(),
                },
                9 => TargetIntegerExpression::ExactSubtract {
                    psi_operation: *psi_operation,
                    obligation: ObligationId::new(52_516).unwrap(),
                    left: left.clone(),
                    right: right.clone(),
                },
                10 => TargetIntegerExpression::WrappingSubtract {
                    psi_operation: *psi_operation,
                    left: left.clone(),
                    right: right.clone(),
                },
                11 => TargetIntegerExpression::SaturatingSubtract {
                    psi_operation: *psi_operation,
                    left: left.clone(),
                    right: right.clone(),
                },
                12 => TargetIntegerExpression::ExactMultiply {
                    psi_operation: *psi_operation,
                    obligation: ObligationId::new(52_517).unwrap(),
                    left: left.clone(),
                    right: right.clone(),
                },
                13 => TargetIntegerExpression::WrappingMultiply {
                    psi_operation: *psi_operation,
                    left: left.clone(),
                    right: right.clone(),
                },
                14 => TargetIntegerExpression::SaturatingMultiply {
                    psi_operation: *psi_operation,
                    left: left.clone(),
                    right: right.clone(),
                },
                _ => unreachable!(),
            };
        });
    }
}

fn assert_target_operation_error(mutate: impl FnOnce(&mut TargetOperation)) {
    assert_eq!(
        candidate_error(|target| mutate(&mut target.functions[0].operation)),
        StraightLineSaturatingIntegerRemainderParametersTranslationError::TargetOperation
    );
}

fn expression(operation: &mut TargetOperation) -> &mut TargetIntegerExpression {
    let TargetOperation::ReturnIntegerExpression { expression, .. } = operation else {
        unreachable!()
    };
    expression
}
