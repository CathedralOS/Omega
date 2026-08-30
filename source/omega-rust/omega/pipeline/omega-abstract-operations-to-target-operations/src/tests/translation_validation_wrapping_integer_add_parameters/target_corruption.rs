use super::*;
use omega_target_operations::TerminalPsiProvenance;

#[test]
fn wrapping_integer_add_target_provenance_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.operations.clear()),
        StraightLineWrappingIntegerAddParametersTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.edges.clear()),
        StraightLineWrappingIntegerAddParametersTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default()
        }),
        StraightLineWrappingIntegerAddParametersTranslationError::TargetProvenance
    );
}

#[test]
fn wrapping_integer_add_target_outer_expression_corruption_fails_closed() {
    assert_target_operation_error(|operation| {
        let TargetOperation::ReturnIntegerExpression { psi_edge, .. } = operation else {
            unreachable!()
        };
        *psi_edge = EdgeId::new(48_509).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetOperation::ReturnIntegerExpression { source_value, .. } = operation else {
            unreachable!()
        };
        *source_value = ValueId::new(48_509).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetOperation::ReturnIntegerExpression { scalar_type, .. } = operation else {
            unreachable!()
        };
        *scalar_type = integer_type(IntegerSign::Signed, 64);
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::WrappingAdd { psi_operation, .. } = expression(operation)
        else {
            unreachable!()
        };
        *psi_operation = OperationId::new(48_510).unwrap();
    });
}

#[test]
fn wrapping_integer_add_target_nested_operand_corruption_fails_closed() {
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::WrappingAdd { left, .. } = expression(operation) else {
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
        let TargetIntegerExpression::WrappingAdd { left, .. } = expression(operation) else {
            unreachable!()
        };
        let TargetIntegerExpression::Parameter { source_value, .. } = left.as_mut() else {
            unreachable!()
        };
        *source_value = ValueId::new(48_511).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::WrappingAdd { right, .. } = expression(operation) else {
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
        let TargetIntegerExpression::WrappingAdd { right, .. } = expression(operation) else {
            unreachable!()
        };
        let TargetIntegerExpression::Parameter { source_value, .. } = right.as_mut() else {
            unreachable!()
        };
        *source_value = ValueId::new(48_512).unwrap();
    });
    for mutate_left in [true, false] {
        assert_target_operation_error(|operation| {
            let TargetIntegerExpression::WrappingAdd { left, right, .. } = expression(operation)
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
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::WrappingAdd { left, .. } = expression(operation) else {
            unreachable!()
        };
        let operand = left.clone();
        *left = Box::new(TargetIntegerExpression::BitwiseNot {
            psi_operation: OperationId::new(48_513).unwrap(),
            operand,
        });
    });
}

#[test]
fn wrapping_integer_add_rejects_swapped_operands_and_semantic_substitutions() {
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::WrappingAdd { left, right, .. } = expression(operation) else {
            unreachable!()
        };
        std::mem::swap(left, right);
    });
    for substitute in [1, 2, 3, 4] {
        assert_target_operation_error(|operation| {
            let expression = expression(operation);
            let TargetIntegerExpression::WrappingAdd {
                psi_operation,
                left,
                right,
            } = expression
            else {
                unreachable!()
            };
            *expression = match substitute {
                1 => TargetIntegerExpression::ExactAdd {
                    psi_operation: *psi_operation,
                    obligation: ObligationId::new(48_514).unwrap(),
                    left: left.clone(),
                    right: right.clone(),
                },
                2 => TargetIntegerExpression::SaturatingAdd {
                    psi_operation: *psi_operation,
                    left: left.clone(),
                    right: right.clone(),
                },
                3 => TargetIntegerExpression::WrappingSubtract {
                    psi_operation: *psi_operation,
                    left: left.clone(),
                    right: right.clone(),
                },
                4 => TargetIntegerExpression::WrappingMultiply {
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
        StraightLineWrappingIntegerAddParametersTranslationError::TargetOperation
    );
}

fn expression(operation: &mut TargetOperation) -> &mut TargetIntegerExpression {
    let TargetOperation::ReturnIntegerExpression { expression, .. } = operation else {
        unreachable!()
    };
    expression
}
