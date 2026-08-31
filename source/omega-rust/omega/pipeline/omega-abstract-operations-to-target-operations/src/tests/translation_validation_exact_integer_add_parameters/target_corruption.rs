use super::{candidate_error, integer_type};
use crate::StraightLineExactIntegerAddParametersTranslationError;
use omega_target_operations::{
    ScalarParameterLocation, TargetIntegerExpression, TargetOperation, TerminalPsiProvenance,
};
use psi_core::{EdgeId, IntegerSign, ObligationId, OperationId, ValueId};

#[test]
fn exact_integer_add_target_provenance_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.operations.clear()),
        StraightLineExactIntegerAddParametersTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.edges.clear()),
        StraightLineExactIntegerAddParametersTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default()
        }),
        StraightLineExactIntegerAddParametersTranslationError::TargetProvenance
    );
}

#[test]
fn exact_integer_add_target_outer_expression_and_obligation_corruption_fails_closed() {
    assert_target_operation_error(|operation| {
        let TargetOperation::ReturnIntegerExpression { psi_edge, .. } = operation else {
            unreachable!()
        };
        *psi_edge = EdgeId::new(51_509).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetOperation::ReturnIntegerExpression { source_value, .. } = operation else {
            unreachable!()
        };
        *source_value = ValueId::new(51_509).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetOperation::ReturnIntegerExpression { scalar_type, .. } = operation else {
            unreachable!()
        };
        *scalar_type = integer_type(IntegerSign::Signed, 64);
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::ExactAdd { psi_operation, .. } = expression(operation) else {
            unreachable!()
        };
        *psi_operation = OperationId::new(51_510).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::ExactAdd { obligation, .. } = expression(operation) else {
            unreachable!()
        };
        *obligation = ObligationId::new(51_511).unwrap();
    });
}

#[test]
fn exact_integer_add_target_nested_operand_corruption_fails_closed() {
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::ExactAdd { left, .. } = expression(operation) else {
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
        let TargetIntegerExpression::ExactAdd { left, .. } = expression(operation) else {
            unreachable!()
        };
        let TargetIntegerExpression::Parameter { source_value, .. } = left.as_mut() else {
            unreachable!()
        };
        *source_value = ValueId::new(51_512).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::ExactAdd { right, .. } = expression(operation) else {
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
        let TargetIntegerExpression::ExactAdd { right, .. } = expression(operation) else {
            unreachable!()
        };
        let TargetIntegerExpression::Parameter { source_value, .. } = right.as_mut() else {
            unreachable!()
        };
        *source_value = ValueId::new(51_513).unwrap();
    });
    for mutate_left in [true, false] {
        assert_target_operation_error(|operation| {
            let TargetIntegerExpression::ExactAdd { left, right, .. } = expression(operation)
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
        let TargetIntegerExpression::ExactAdd { left, .. } = expression(operation) else {
            unreachable!()
        };
        let operand = left.clone();
        *left = Box::new(TargetIntegerExpression::BitwiseNot {
            psi_operation: OperationId::new(51_514).unwrap(),
            operand,
        });
    });
}

#[test]
fn exact_integer_add_rejects_swapped_operands_and_semantic_substitutions() {
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::ExactAdd { left, right, .. } = expression(operation) else {
            unreachable!()
        };
        std::mem::swap(left, right);
    });
    for saturating in [false, true] {
        assert_target_operation_error(|operation| {
            let expression = expression(operation);
            let TargetIntegerExpression::ExactAdd {
                psi_operation,
                left,
                right,
                ..
            } = expression
            else {
                unreachable!()
            };
            *expression = if saturating {
                TargetIntegerExpression::SaturatingAdd {
                    psi_operation: *psi_operation,
                    left: left.clone(),
                    right: right.clone(),
                }
            } else {
                TargetIntegerExpression::WrappingAdd {
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
        StraightLineExactIntegerAddParametersTranslationError::TargetOperation
    );
}

fn expression(operation: &mut TargetOperation) -> &mut TargetIntegerExpression {
    let TargetOperation::ReturnIntegerExpression { expression, .. } = operation else {
        unreachable!()
    };
    expression
}
