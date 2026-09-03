use super::*;
use omega_target_operations::TerminalPsiProvenance;

#[test]
fn integer_bitwise_xor_target_provenance_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.operations.clear()),
        StraightLineIntegerBitwiseXorParametersTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.edges.clear()),
        StraightLineIntegerBitwiseXorParametersTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance = TerminalPsiProvenance::default()),
        StraightLineIntegerBitwiseXorParametersTranslationError::TargetProvenance
    );
}

#[test]
fn integer_bitwise_xor_target_expression_corruption_fails_closed() {
    assert_target_operation_error(|operation| {
        let TargetOperation::ReturnIntegerExpression { psi_edge, .. } = operation else {
            unreachable!()
        };
        *psi_edge = EdgeId::new(47_509).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetOperation::ReturnIntegerExpression { source_value, .. } = operation else {
            unreachable!()
        };
        *source_value = ValueId::new(47_509).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetOperation::ReturnIntegerExpression { scalar_type, .. } = operation else {
            unreachable!()
        };
        *scalar_type = integer_type(IntegerSign::Signed, 64);
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::BitwiseXor { psi_operation, .. } = expression(operation)
        else {
            unreachable!()
        };
        *psi_operation = OperationId::new(47_510).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::BitwiseXor { left, right, .. } = expression(operation) else {
            unreachable!()
        };
        std::mem::swap(left, right);
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::BitwiseXor { left, .. } = expression(operation) else {
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
        let TargetIntegerExpression::BitwiseXor { left, .. } = expression(operation) else {
            unreachable!()
        };
        let TargetIntegerExpression::Parameter { source_value, .. } = left.as_mut() else {
            unreachable!()
        };
        *source_value = ValueId::new(47_511).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::BitwiseXor { right, .. } = expression(operation) else {
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
        let TargetIntegerExpression::BitwiseXor { right, .. } = expression(operation) else {
            unreachable!()
        };
        let TargetIntegerExpression::Parameter { source_value, .. } = right.as_mut() else {
            unreachable!()
        };
        *source_value = ValueId::new(47_513).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::BitwiseXor { left, .. } = expression(operation) else {
            unreachable!()
        };
        let TargetIntegerExpression::Parameter { location, .. } = left.as_mut() else {
            unreachable!()
        };
        *location = ScalarParameterLocation::IncomingStack { byte_offset: 0 };
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::BitwiseXor { right, .. } = expression(operation) else {
            unreachable!()
        };
        let TargetIntegerExpression::Parameter { location, .. } = right.as_mut() else {
            unreachable!()
        };
        *location = ScalarParameterLocation::IncomingStack { byte_offset: 0 };
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::BitwiseXor { left, .. } = expression(operation) else {
            unreachable!()
        };
        let operand = left.clone();
        **left = TargetIntegerExpression::BitwiseNot {
            psi_operation: OperationId::new(47_512).unwrap(),
            operand,
        };
    });
    for substitute in [false, true] {
        assert_target_operation_error(|operation| {
            let expression = expression(operation);
            let TargetIntegerExpression::BitwiseXor {
                psi_operation,
                left,
                right,
            } = expression
            else {
                unreachable!()
            };
            *expression = if substitute {
                TargetIntegerExpression::BitwiseAnd {
                    psi_operation: *psi_operation,
                    left: left.clone(),
                    right: right.clone(),
                }
            } else {
                TargetIntegerExpression::BitwiseOr {
                    psi_operation: *psi_operation,
                    left: left.clone(),
                    right: right.clone(),
                }
            };
        });
    }
}

fn assert_target_operation_error(mutate: impl FnOnce(&mut TargetOperation)) {
    assert_eq!(
        candidate_error(|target| mutate(&mut target.functions[0].operation)),
        StraightLineIntegerBitwiseXorParametersTranslationError::TargetOperation
    );
}

fn expression(operation: &mut TargetOperation) -> &mut TargetIntegerExpression {
    let TargetOperation::ReturnIntegerExpression { expression, .. } = operation else {
        unreachable!()
    };
    expression
}
